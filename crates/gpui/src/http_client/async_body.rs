use std::{
    io::{Cursor, Read},
    pin::Pin,
    task::Poll,
};

use bytes::Bytes;
use futures::AsyncRead;
use http_body::{Body, Frame};
use serde::Serialize;

/// 基于 AsyncBody 实现，参考：
/// <https://github.com/sagebind/isahc/blob/5c533f1ef4d6bdf1fd291b5103c22110f41d0bf0/src/body/mod.rs>
pub struct AsyncBody(pub Inner);

/// 请求体的内部表示
pub enum Inner {
    /// 空请求体
    Empty,

    /// 存储在内存中的请求体
    Bytes(std::io::Cursor<Bytes>),

    /// 异步读取器
    AsyncReader(Pin<Box<dyn futures::AsyncRead + Send + Sync>>),
}

impl AsyncBody {
    /// 创建新的空请求体。
    ///
    /// 空请求体表示主体的*缺失*，这与长度为零的请求体在语义上不同。
    pub fn empty() -> Self {
        Self(Inner::Empty)
    }
    /// 创建流式请求体，从给定读取器读取数据
    pub fn from_reader<R>(read: R) -> Self
    where
        R: AsyncRead + Send + Sync + 'static,
    {
        Self(Inner::AsyncReader(Box::pin(read)))
    }

    pub fn from_bytes(bytes: Bytes) -> Self {
        Self(Inner::Bytes(Cursor::new(bytes)))
    }
}

impl Default for AsyncBody {
    fn default() -> Self {
        Self(Inner::Empty)
    }
}

impl From<()> for AsyncBody {
    fn from(_: ()) -> Self {
        Self(Inner::Empty)
    }
}

impl From<Bytes> for AsyncBody {
    fn from(bytes: Bytes) -> Self {
        Self::from_bytes(bytes)
    }
}

impl From<Vec<u8>> for AsyncBody {
    fn from(body: Vec<u8>) -> Self {
        Self::from_bytes(body.into())
    }
}

impl From<String> for AsyncBody {
    fn from(body: String) -> Self {
        Self::from_bytes(body.into())
    }
}

impl From<&'static [u8]> for AsyncBody {
    #[inline]
    fn from(s: &'static [u8]) -> Self {
        Self::from_bytes(Bytes::from_static(s))
    }
}

impl From<&'static str> for AsyncBody {
    #[inline]
    fn from(s: &'static str) -> Self {
        Self::from_bytes(Bytes::from_static(s.as_bytes()))
    }
}

/// 新类型包装器，将值序列化为 JSON 到 `AsyncBody` 中
pub struct Json<T: Serialize>(pub T);

impl<T: Serialize> From<Json<T>> for AsyncBody {
    fn from(json: Json<T>) -> Self {
        Self::from_bytes(
            serde_json::to_vec(&json.0)
                .expect("failed to serialize JSON")
                .into(),
        )
    }
}

impl<T: Into<Self>> From<Option<T>> for AsyncBody {
    fn from(body: Option<T>) -> Self {
        match body {
            Some(body) => body.into(),
            None => Self::empty(),
        }
    }
}

impl futures::AsyncRead for AsyncBody {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        // SAFETY: Standard Enum pin projection
        let inner = unsafe { &mut self.get_unchecked_mut().0 };
        match inner {
            Inner::Empty => Poll::Ready(Ok(0)),
            // Blocking call is over an in-memory buffer
            Inner::Bytes(cursor) => Poll::Ready(cursor.read(buf)),
            Inner::AsyncReader(async_reader) => {
                AsyncRead::poll_read(async_reader.as_mut(), cx, buf)
            }
        }
    }
}

impl Body for AsyncBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let mut buffer = vec![0; 8192];
        match AsyncRead::poll_read(self.as_mut(), cx, &mut buffer) {
            Poll::Ready(Ok(0)) => Poll::Ready(None),
            Poll::Ready(Ok(n)) => {
                let data = Bytes::copy_from_slice(&buffer[..n]);
                Poll::Ready(Some(Ok(Frame::data(data))))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
