use std::error::Error;
use std::sync::{LazyLock, OnceLock};
use std::{borrow::Cow, mem, pin::Pin, task::Poll, time::Duration};

use anyhow::anyhow;
use bytes::{BufMut, Bytes, BytesMut};
use futures::{AsyncRead, FutureExt as _, TryStreamExt as _};
use super::{RedirectPolicy, Url, http};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    redirect,
};

/// 延迟执行闭包的守卫类型
pub struct Deferred<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Deferred<F> {
    /// 中止延迟执行，丢弃时不会运行闭包
    pub fn abort(mut self) {
        self.0.take();
    }
}

impl<F: FnOnce()> Drop for Deferred<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f()
        }
    }
}

/// 创建延迟执行闭包，在丢弃时运行
pub fn defer<F: FnOnce()>(f: F) -> Deferred<F> {
    Deferred(Some(f))
}

/// 默认缓冲区容量
const DEFAULT_CAPACITY: usize = 4096;
/// 全局 Tokio 运行时，仅初始化一次
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
/// 用于脱敏 URL 中密钥参数的正则表达式
static REDACT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"key=[^&]+").unwrap());

pub struct ReqwestClient {
    client: reqwest::Client,
    proxy: Option<Url>,
    user_agent: Option<HeaderValue>,
    handle: tokio::runtime::Handle,
}

impl ReqwestClient {
    fn builder() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
    }

    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("Failed to initialize HTTP client")
            .into()
    }

    pub fn user_agent(agent: &str) -> anyhow::Result<Self> {
        let mut map = HeaderMap::new();
        map.insert(http::header::USER_AGENT, HeaderValue::from_str(agent)?);
        let client = Self::builder().default_headers(map).build()?;
        Ok(client.into())
    }

    pub fn proxy_and_user_agent(proxy: Option<Url>, user_agent: &str) -> anyhow::Result<Self> {
        let user_agent = HeaderValue::from_str(user_agent)?;

        let mut map = HeaderMap::new();
        map.insert(http::header::USER_AGENT, user_agent.clone());
        let mut client = Self::builder().default_headers(map);
        let client_has_proxy;

        if let Some(proxy) = proxy.as_ref().and_then(|proxy_url| {
            reqwest::Proxy::all(proxy_url.clone())
                .inspect_err(|e| {
                    log::error!(
                        "Failed to parse proxy URL '{}': {}",
                        proxy_url,
                        e.source().unwrap_or(&e as &_)
                    )
                })
                .ok()
        }) {
            client = client.proxy(proxy.no_proxy(reqwest::NoProxy::from_env()));
            client_has_proxy = true;
        } else {
            client_has_proxy = false;
        };

        let client = client
            .use_preconfigured_tls(crate::tls::tls_config())
            .build()?;
        let mut client: ReqwestClient = client.into();
        client.proxy = client_has_proxy.then_some(proxy).flatten();
        client.user_agent = Some(user_agent);
        Ok(client)
    }
}

/// 获取全局 Tokio 运行时
pub fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to initialize HTTP client")
    })
}

impl From<reqwest::Client> for ReqwestClient {
    fn from(client: reqwest::Client) -> Self {
        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            log::debug!("no tokio runtime found, creating one for Reqwest...");
            runtime().handle().clone()
        });
        Self {
            client,
            handle,
            proxy: None,
            user_agent: None,
        }
    }
}

/// 将 reqwest 响应流转换为异步字节流的读取器
struct StreamReader {
    reader: Option<Pin<Box<dyn futures::AsyncRead + Send + Sync>>>,
    buf: BytesMut,
    capacity: usize,
}

impl StreamReader {
    fn new(reader: Pin<Box<dyn futures::AsyncRead + Send + Sync>>) -> Self {
        Self {
            reader: Some(reader),
            buf: BytesMut::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }
}

impl futures::Stream for StreamReader {
    type Item = std::io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut();

        let mut reader = match this.reader.take() {
            Some(r) => r,
            None => return Poll::Ready(None),
        };

        if this.buf.capacity() == 0 {
            let capacity = this.capacity;
            this.buf.reserve(capacity);
        }

        match poll_read_buf(&mut reader, cx, &mut this.buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(err)) => {
                self.reader = None;
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(Ok(0)) => {
                self.reader = None;
                Poll::Ready(None)
            }
            Poll::Ready(Ok(_)) => {
                let chunk = this.buf.split();
                self.reader = Some(reader);
                Poll::Ready(Some(Ok(chunk.freeze())))
            }
        }
    }
}

pub fn poll_read_buf(
    io: &mut Pin<Box<dyn futures::AsyncRead + Send + Sync>>,
    cx: &mut std::task::Context<'_>,
    buf: &mut BytesMut,
) -> Poll<std::io::Result<usize>> {
    if !buf.has_remaining_mut() {
        return Poll::Ready(Ok(0));
    }

    let n = {
        let dst = buf.chunk_mut();

        let dst = unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>]) };
        let mut buf = tokio::io::ReadBuf::uninit(dst);
        let ptr = buf.filled().as_ptr();
        let unfilled_portion = buf.initialize_unfilled();
        let io_pin = unsafe { Pin::new_unchecked(io) };
        std::task::ready!(io_pin.poll_read(cx, unfilled_portion)?);

        assert_eq!(ptr, buf.filled().as_ptr());
        buf.filled().len()
    };

    unsafe {
        buf.advance_mut(n);
    }

    Poll::Ready(Ok(n))
}

/// 脱敏错误信息中的 URL 密钥参数
fn redact_error(mut error: reqwest::Error) -> reqwest::Error {
    if let Some(url) = error.url_mut()
        && let Some(query) = url.query()
        && let Cow::Owned(redacted) = REDACT_REGEX.replace_all(query, "key=REDACTED")
    {
        url.set_query(Some(redacted.as_str()));
    }
    error
}

impl super::HttpClient for ReqwestClient {
    fn proxy(&self) -> Option<&Url> {
        self.proxy.as_ref()
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        self.user_agent.as_ref()
    }

    fn send(
        &self,
        req: http::Request<super::AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        anyhow::Result<super::Response<super::AsyncBody>>,
    > {
        let (parts, body) = req.into_parts();

        let mut request = self.client.request(parts.method, parts.uri.to_string());
        request = request.headers(parts.headers);
        if let Some(redirect_policy) = parts.extensions.get::<RedirectPolicy>() {
            request = request.redirect_policy(match redirect_policy {
                RedirectPolicy::NoFollow => redirect::Policy::none(),
                RedirectPolicy::FollowLimit(limit) => redirect::Policy::limited(*limit as usize),
                RedirectPolicy::FollowAll => redirect::Policy::limited(100),
            });
        }
        let request = request.body(match body.0 {
            crate::Inner::Empty => reqwest::Body::default(),
            crate::Inner::Bytes(cursor) => cursor.into_inner().into(),
            crate::Inner::AsyncReader(stream) => {
                reqwest::Body::wrap_stream(StreamReader::new(stream))
            }
        });

        let handle = self.handle.clone();
        async move {
            let join_handle = handle.spawn(async { request.send().await });
            let abort_handle = join_handle.abort_handle();
            let _abort_on_drop = defer(move || abort_handle.abort());

            let mut response = join_handle.await?.map_err(redact_error)?;

            let headers = mem::take(response.headers_mut());
            let mut builder = http::Response::builder()
                .status(response.status().as_u16())
                .version(response.version());
            *builder.headers_mut().unwrap() = headers;

            let bytes = response
                .bytes_stream()
                .map_err(futures::io::Error::other)
                .into_async_read();
            let body = super::AsyncBody::from_reader(bytes);

            builder.body(body).map_err(|e| anyhow!(e))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::{HttpClient, Url};

    use super::ReqwestClient;

    #[test]
    fn test_proxy_uri() {
        let client = ReqwestClient::new();
        assert_eq!(client.proxy(), None);

        let proxy = Url::parse("http://localhost:10809").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("https://localhost:10809").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks4://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks4a://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks5://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks5h://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));
    }

    #[test]
    fn test_invalid_proxy_uri() {
        let proxy = Url::parse("socks://127.0.0.1:20170").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy), "test").unwrap();
        assert!(
            client.proxy.is_none(),
            "An invalid proxy URL should add no proxy to the client!"
        )
    }
}
