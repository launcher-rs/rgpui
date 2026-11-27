//! 图片 / 窗口 / 列表 示例集合。
//!
//! 用法：cargo run -p showcase --bin media_gallery -- <demo>
//! 可用 demo：image_loading / image_gallery / gif_viewer / svg / image / window / custom_title_bar / opacity / shadow / transparent / window_movable / window_positioning / window_shadow / list_example / uniform_list / data_table / tree
//! 不带参数时默认运行 image_loading，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, AssetSource, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement,
    Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

/// Gallery 资源来源：从文件系统加载示例所需的 SVG / 图片资源。
struct GalleryAssets;

impl GalleryAssets {
    /// 把示例里的相对路径映射到共享资产目录，兼容从 workspace 根或 crate 目录启动。
    fn resolve(path: &str) -> std::path::PathBuf {
        let direct = std::path::PathBuf::from(path);
        if direct.exists() {
            return direct;
        }
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("image_showcase")
            .join(path)
    }
}

impl AssetSource for GalleryAssets {
    fn load(&self, path: &str) -> rgpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(Self::resolve(path))
            .map(|bytes| Some(std::borrow::Cow::Owned(bytes)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> rgpui::Result<Vec<rgpui::SharedString>> {
        Ok(std::fs::read_dir(Self::resolve(path))?
            .filter_map(|entry| {
                Some(rgpui::SharedString::from(
                    entry.ok()?.path().to_string_lossy().into_owned(),
                ))
            })
            .collect())
    }
}

mod image_loading {

    //! 图片加载示例：展示加载中骨架动画与失败占位。
    //! 用法：左侧 tab 选「image_loading」，点击图片可重新加载观察不同状态。
    use std::{path::Path, sync::Arc, time::Duration};

    use rgpui::{
        Animation, AnimationExt, App, Asset, AssetLogger, AssetSource, Bounds, Context, Hsla,
        ImageAssetLoader, ImageCacheError, ImgResourceLoader, LOADING_DELAY, Length, RenderImage,
        Resource, SharedString, Window, WindowBounds, WindowOptions, black, div, img, prelude::*,
        pulsating_between, px, red, size,
    };
    use rgpui_platform::application;

    struct Assets {}

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
            std::fs::read(path)
                .map(Into::into)
                .map_err(Into::into)
                .map(Some)
        }

        fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
            Ok(std::fs::read_dir(path)?
                .filter_map(|entry| {
                    Some(SharedString::from(
                        entry.ok()?.path().to_string_lossy().into_owned(),
                    ))
                })
                .collect::<Vec<_>>())
        }
    }

    const IMAGE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../image_showcase/assets/app-icon.png"
    );

    #[derive(Copy, Clone, Hash)]
    struct LoadImageParameters {
        timeout: Duration,
        fail: bool,
    }

    struct LoadImageWithParameters {}

    impl Asset for LoadImageWithParameters {
        type Source = LoadImageParameters;

        type Output = Result<Arc<RenderImage>, ImageCacheError>;

        fn load(
            parameters: Self::Source,
            cx: &mut App,
        ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
            let timer = cx.background_executor().timer(parameters.timeout);
            let data = AssetLogger::<ImageAssetLoader>::load(
                Resource::Path(Path::new(IMAGE).to_path_buf().into()),
                cx,
            );
            async move {
                timer.await;
                if parameters.fail {
                    log::error!("Intentionally failed to load image");
                    Err(anyhow::anyhow!("Failed to load image").into())
                } else {
                    data.await
                }
            }
        }
    }

    struct ImageLoadingExample {}

    impl ImageLoadingExample {
        fn loading_element() -> impl IntoElement {
            div().size_full().flex_none().p_0p5().rounded_xs().child(
                div().size_full().with_animation(
                    "loading-bg",
                    Animation::new(Duration::from_secs(3))
                        .repeat()
                        .with_easing(pulsating_between(0.04, 0.24)),
                    move |this, delta| this.bg(black().opacity(delta)),
                ),
            )
        }

        fn fallback_element() -> impl IntoElement {
            let fallback_color: Hsla = black().opacity(0.5);

            div().size_full().flex_none().p_0p5().child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_xs()
                    .text_sm()
                    .text_color(fallback_color)
                    .border_1()
                    .border_color(fallback_color)
                    .child("?"),
            )
        }
    }

    impl Render for ImageLoadingExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().flex().flex_col().size_full().justify_around().child(
                div().flex().flex_row().w_full().justify_around().child(
                    div()
                        .flex()
                        .bg(rgpui::white())
                        .size(Length::Definite(px(300.0).into()))
                        .justify_center()
                        .items_center()
                        .child({
                            let image_source = LoadImageParameters {
                                timeout: LOADING_DELAY.saturating_sub(Duration::from_millis(25)),
                                fail: false,
                            };

                            // Load within the 'loading delay', should not show loading fallback
                            img(move |window: &mut Window, cx: &mut App| {
                                window.use_asset::<LoadImageWithParameters>(&image_source, cx)
                            })
                            .id("image-1")
                            .border_1()
                            .size_12()
                            .with_fallback(|| Self::fallback_element().into_any_element())
                            .border_color(red())
                            .with_loading(|| Self::loading_element().into_any_element())
                            .on_click(move |_, _, cx| {
                                cx.remove_asset::<LoadImageWithParameters>(&image_source);
                            })
                        })
                        .child({
                            // Load after a long delay
                            let image_source = LoadImageParameters {
                                timeout: Duration::from_secs(5),
                                fail: false,
                            };

                            img(move |window: &mut Window, cx: &mut App| {
                                window.use_asset::<LoadImageWithParameters>(&image_source, cx)
                            })
                            .id("image-2")
                            .with_fallback(|| Self::fallback_element().into_any_element())
                            .with_loading(|| Self::loading_element().into_any_element())
                            .size_12()
                            .border_1()
                            .border_color(red())
                            .on_click(move |_, _, cx| {
                                cx.remove_asset::<LoadImageWithParameters>(&image_source);
                            })
                        })
                        .child({
                            // Fail to load image after a long delay
                            let image_source = LoadImageParameters {
                                timeout: Duration::from_secs(5),
                                fail: true,
                            };

                            // Fail to load after a long delay
                            img(move |window: &mut Window, cx: &mut App| {
                                window.use_asset::<LoadImageWithParameters>(&image_source, cx)
                            })
                            .id("image-3")
                            .with_fallback(|| Self::fallback_element().into_any_element())
                            .with_loading(|| Self::loading_element().into_any_element())
                            .size_12()
                            .border_1()
                            .border_color(red())
                            .on_click(move |_, _, cx| {
                                cx.remove_asset::<LoadImageWithParameters>(&image_source);
                            })
                        })
                        .child({
                            // Ensure that the normal image loader doesn't spam logs
                            let image_source = Path::new(
                                "this/file/really/shouldn't/exist/or/won't/be/an/image/I/hope",
                            )
                            .to_path_buf();
                            img(image_source.clone())
                                .id("image-4")
                                .border_1()
                                .size_12()
                                .with_fallback(|| Self::fallback_element().into_any_element())
                                .border_color(red())
                                .with_loading(|| Self::loading_element().into_any_element())
                                .on_click(move |_, _, cx| {
                                    cx.remove_asset::<ImgResourceLoader>(
                                        &image_source.clone().into(),
                                    );
                                })
                        }),
                ),
            )
        }
    }

    pub fn run_example() {
        env_logger::init();
        application().with_assets(Assets {}).run(|cx: &mut App| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(300.), px(300.)),
                    cx,
                ))),
                ..Default::default()
            };
            cx.open_window(options, |_, cx| {
                cx.activate(false);
                cx.new(|_| ImageLoadingExample {})
            })
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| ImageLoadingExample {}).into()
    }
}

mod image_gallery {

    //! 图片画廊示例：对比手动缓存与 LRU 自动缓存的图片列表。
    //! 用法：左侧 tab 选「image_gallery」，滚动列表观察缓存行为。
    use futures::FutureExt;
    #[cfg(not(target_family = "wasm"))]
    use rgpui::http_client::reqwest_client::ReqwestClient;
    use rgpui::{
        App, AppContext, Asset as _, AssetLogger, Bounds, ClickEvent, Context, ElementId, Entity,
        ImageAssetLoader, ImageCache, ImageCacheProvider, KeyBinding, Menu, MenuItem,
        RetainAllImageCache, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
        actions, div, hash, image_cache, img, prelude::*, px, rgb, size,
    };
    use std::{collections::HashMap, sync::Arc};

    const IMAGES_IN_GALLERY: usize = 30;

    struct ImageGallery {
        image_key: String,
        items_count: usize,
        total_count: usize,
        image_cache: Entity<RetainAllImageCache>,
    }

    impl ImageGallery {
        fn on_next_image(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
            self.image_cache
                .update(cx, |image_cache, cx| image_cache.clear(window, cx));

            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            self.image_key = format!("{}", t);
            self.total_count += self.items_count;
            cx.notify();
        }
    }

    impl Render for ImageGallery {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let image_url: SharedString =
                format!("https://picsum.photos/400/200?t={}", self.image_key).into();

            div()
                .flex()
                .flex_col()
                .text_color(rgpui::white())
                .child("Manually managed image cache:")
                .child(
                    div()
                        .image_cache(self.image_cache.clone())
                        .id("main")
                        .text_color(rgpui::black())
                        .bg(rgb(0xE9E9E9))
                        .overflow_y_scroll()
                        .p_4()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .child(format!(
                                    "展示图片并测试内存占用（已渲染 {} 张图片）。",
                                    self.total_count
                                ))
                                .child(
                                    div()
                                        .id("btn")
                                        .py_1()
                                        .px_4()
                                        .bg(rgpui::black())
                                        .hover(|this| this.opacity(0.8))
                                        .text_color(rgpui::white())
                                        .text_center()
                                        .w_40()
                                        .child("Next Photos")
                                        .on_click(cx.listener(Self::on_next_image)),
                                ),
                        )
                        .child(
                            div()
                                .id("image-gallery")
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_x_4()
                                .gap_y_2()
                                .justify_around()
                                .children(
                                    (0..self.items_count)
                                        .map(|ix| img(format!("{}-{}", image_url, ix)).size_20()),
                                ),
                        ),
                )
                .child("Automatically managed image cache:")
                .child(
                    image_cache(simple_lru_cache("lru-cache", IMAGES_IN_GALLERY)).child(
                        div()
                            .id("main")
                            .bg(rgb(0xE9E9E9))
                            .text_color(rgpui::black())
                            .overflow_y_scroll()
                            .p_4()
                            .size_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("image-gallery")
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_x_4()
                                    .gap_y_2()
                                    .justify_around()
                                    .children(
                                        (0..self.items_count).map(|ix| {
                                            img(format!("{}-{}", image_url, ix)).size_20()
                                        }),
                                    ),
                            ),
                    ),
                )
        }
    }

    fn simple_lru_cache(id: impl Into<ElementId>, max_items: usize) -> SimpleLruCacheProvider {
        SimpleLruCacheProvider {
            id: id.into(),
            max_items,
        }
    }

    struct SimpleLruCacheProvider {
        id: ElementId,
        max_items: usize,
    }

    impl ImageCacheProvider for SimpleLruCacheProvider {
        fn provide(&mut self, window: &mut Window, cx: &mut App) -> rgpui::AnyImageCache {
            window
                .with_global_id(self.id.clone(), |global_id, window| {
                    window.with_element_state::<Entity<SimpleLruCache>, _>(
                        global_id,
                        |lru_cache, _window| {
                            let mut lru_cache = lru_cache.unwrap_or_else(|| {
                                cx.new(|cx| SimpleLruCache::new(self.max_items, cx))
                            });
                            if lru_cache.read(cx).max_items != self.max_items {
                                lru_cache = cx.new(|cx| SimpleLruCache::new(self.max_items, cx));
                            }
                            (lru_cache.clone(), lru_cache)
                        },
                    )
                })
                .into()
        }
    }

    struct SimpleLruCache {
        max_items: usize,
        usages: Vec<u64>,
        cache: HashMap<u64, rgpui::ImageCacheItem>,
    }

    impl SimpleLruCache {
        fn new(max_items: usize, cx: &mut Context<Self>) -> Self {
            cx.on_release(|simple_cache, cx| {
                for (_, mut item) in std::mem::take(&mut simple_cache.cache) {
                    if let Some(Ok(image)) = item.get() {
                        cx.drop_image(image, None);
                    }
                }
            })
            .detach();

            Self {
                max_items,
                usages: Vec::with_capacity(max_items),
                cache: HashMap::with_capacity(max_items),
            }
        }
    }

    impl ImageCache for SimpleLruCache {
        fn load(
            &mut self,
            resource: &rgpui::Resource,
            window: &mut Window,
            cx: &mut App,
        ) -> Option<Result<Arc<rgpui::RenderImage>, rgpui::ImageCacheError>> {
            assert_eq!(self.usages.len(), self.cache.len());
            assert!(self.cache.len() <= self.max_items);

            let hash = hash(resource);

            if let Some(item) = self.cache.get_mut(&hash) {
                let current_ix = self
                    .usages
                    .iter()
                    .position(|item| *item == hash)
                    .expect("缓存与用量必须保持一致");
                self.usages.remove(current_ix);
                self.usages.insert(0, hash);

                return item.get();
            }

            let fut = AssetLogger::<ImageAssetLoader>::load(resource.clone(), cx);
            let task = cx.background_executor().spawn(fut).shared();
            if self.usages.len() == self.max_items {
                let oldest = self.usages.pop().unwrap();
                let mut image = self.cache.remove(&oldest).expect("缓存与用量必须同步");
                if let Some(Ok(image)) = image.get() {
                    cx.drop_image(image, Some(window));
                }
            }
            self.cache
                .insert(hash, rgpui::ImageCacheItem::Loading(task.clone()));
            self.usages.insert(0, hash);

            let entity = window.current_view();
            window
                .spawn(cx, {
                    async move |cx| {
                        _ = task.await;
                        cx.on_next_frame(move |_, cx| {
                            cx.notify(entity);
                        });
                    }
                })
                .detach();

            None
        }
    }

    actions!(image_gallery_image, [Quit]);

    pub fn run_example() {
        env_logger::init();
        #[cfg(not(target_family = "wasm"))]
        let app = rgpui_platform::application();
        #[cfg(target_family = "wasm")]
        let app = rgpui_platform::single_threaded_web();

        app.run(move |cx: &mut App| {
            #[cfg(not(target_family = "wasm"))]
            {
                let http_client = ReqwestClient::user_agent("gpui example").unwrap();
                cx.set_http_client(Arc::new(http_client));
            }
            #[cfg(target_family = "wasm")]
            {
                // Safety: the web examples run single-threaded; the client is
                // created and used exclusively on the main thread.
                let http_client = unsafe {
                    rgpui_web::FetchHttpClient::with_user_agent("gpui example")
                        .expect("创建 FetchHttpClient 失败")
                };
                cx.set_http_client(Arc::new(http_client));
            }

            cx.activate(true);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            cx.set_menus([Menu::new("Image Gallery").items([MenuItem::action("Quit", Quit)])]);

            let window_options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Image Gallery")),
                    appears_transparent: false,
                    ..Default::default()
                }),

                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(860.)),
                    cx,
                ))),

                ..Default::default()
            };

            cx.open_window(window_options, |_, cx| {
                cx.new(|ctx| ImageGallery {
                    image_key: "".into(),
                    items_count: IMAGES_IN_GALLERY,
                    total_count: 0,
                    image_cache: RetainAllImageCache::new(ctx),
                })
            })
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|ctx| ImageGallery {
            image_key: "".into(),
            items_count: IMAGES_IN_GALLERY,
            total_count: 0,
            image_cache: RetainAllImageCache::new(ctx),
        })
        .into()
    }
}

mod gif_viewer {

    //! GIF 动图播放示例：展示 GIF 帧动画解码与渲染。
    //! 用法：左侧 tab 选「gif_viewer」，查看窗口内循环播放的动图。
    use rgpui::{
        App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, img, prelude::*,
        px, size,
    };
    use rgpui_platform::application;
    use std::path::PathBuf;

    struct GifViewer {
        gif_path: PathBuf,
    }

    impl GifViewer {
        fn new(gif_path: PathBuf) -> Self {
            Self { gif_path }
        }
    }

    impl Render for GifViewer {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                img(self.gif_path.clone())
                    .size_full()
                    .object_fit(rgpui::ObjectFit::Contain)
                    .id("gif"),
            )
        }
    }

    pub fn run_example() {
        env_logger::init();
        application().run(|cx: &mut App| {
            let gif_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../image_showcase/assets/black-cat-typing.gif");
            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: true,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| GifViewer::new(gif_path)),
            )
            .unwrap();
            cx.activate(true);
        });
    }
}

mod svg {

    //! SVG 矢量图示例：展示 SVG 加载与 `text_color` 着色。
    //! 用法：左侧 tab 选「svg」，查看矢量图缩放与着色效果。
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Result;
    use rgpui::{
        App, AssetSource, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, rgb, size, svg,
    };
    use rgpui_platform::application;

    struct Assets {
        base: PathBuf,
    }

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
            fs::read(self.base.join(path))
                .map(|data| Some(std::borrow::Cow::Owned(data)))
                .map_err(|err| err.into())
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            fs::read_dir(self.base.join(path))
                .map(|entries| {
                    entries
                        .filter_map(|entry| {
                            entry
                                .ok()
                                .and_then(|entry| entry.file_name().into_string().ok())
                                .map(SharedString::from)
                        })
                        .collect()
                })
                .map_err(|err| err.into())
        }
    }

    struct SvgExample;

    impl Render for SvgExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_row()
                .size_full()
                .justify_center()
                .items_center()
                .gap_8()
                .bg(rgb(0xffffff))
                .child(
                    svg()
                        .path("assets/dragon.svg")
                        .size_8()
                        .text_color(rgb(0xff0000)),
                )
                .child(
                    svg()
                        .path("assets/dragon.svg")
                        .size_8()
                        .text_color(rgb(0x00ff00)),
                )
                .child(
                    svg()
                        .path("assets/dragon.svg")
                        .size_8()
                        .text_color(rgb(0x0000ff)),
                )
        }
    }

    pub fn run_example() {
        application()
            .with_assets(Assets {
                base: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            })
            .run(|cx: &mut App| {
                let bounds = Bounds::centered(None, size(px(300.0), px(300.0)), cx);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| SvgExample),
                )
                .unwrap();
                cx.activate(true);
            });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| SvgExample).into()
    }
}

mod image {

    //! 本地图片查看器：展示本地文件、远程 URL、注册资产三种图片来源。
    //! 用法：左侧 tab 选「image」，可输入路径或 URL 加载图片。
    #![cfg_attr(target_family = "wasm", no_main)]

    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Result;
    #[cfg(not(target_family = "wasm"))]
    use rgpui::http_client::reqwest_client::ReqwestClient;
    use rgpui::{
        App, AppContext, AssetSource, Bounds, Context, ImageSource, KeyBinding, Menu, MenuItem,
        Point, SharedString, SharedUri, TitlebarOptions, Window, WindowBounds, WindowOptions,
        actions, div, img, prelude::*, px, rgb, size,
    };

    struct Assets {
        base: PathBuf,
    }

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
            fs::read(self.base.join(path))
                .map(|data| Some(std::borrow::Cow::Owned(data)))
                .map_err(|e| e.into())
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            fs::read_dir(self.base.join(path))
                .map(|entries| {
                    entries
                        .filter_map(|entry| {
                            entry
                                .ok()
                                .and_then(|entry| entry.file_name().into_string().ok())
                                .map(SharedString::from)
                        })
                        .collect()
                })
                .map_err(|e| e.into())
        }
    }

    #[derive(IntoElement)]
    struct ImageContainer {
        text: SharedString,
        src: ImageSource,
    }

    impl ImageContainer {
        pub fn new(text: impl Into<SharedString>, src: impl Into<ImageSource>) -> Self {
            Self {
                text: text.into(),
                src: src.into(),
            }
        }
    }

    impl RenderOnce for ImageContainer {
        fn render(self, _window: &mut Window, _: &mut App) -> impl IntoElement {
            div().child(
                div()
                    .flex_row()
                    .size_full()
                    .gap_4()
                    .child(self.text)
                    .child(img(self.src).size(px(256.0))),
            )
        }
    }

    struct ImageShowcase {
        local_resource: Arc<std::path::Path>,
        remote_resource: SharedUri,
        asset_resource: SharedString,
    }

    impl Render for ImageShowcase {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
            .id("main")
            .bg(rgpui::white())
            .overflow_y_scroll()
            .p_5()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap_8()
                    .child(img(
                        "https://github.com/zed-industries/zed/actions/workflows/ci.yml/badge.svg",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_center()
                            .items_center()
                            .gap_8()
                            .child(ImageContainer::new(
                                "Image loaded from a local file",
                                self.local_resource.clone(),
                            ))
                            .child(ImageContainer::new(
                                "Image loaded from a remote resource",
                                self.remote_resource.clone(),
                            ))
                            .child(ImageContainer::new(
                                "Image loaded from an asset",
                                self.asset_resource.clone(),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_8()
                            .child(
                                div()
                                    .flex_col()
                                    .child("自动宽度")
                                    .child(img("https://picsum.photos/800/400").h(px(180.))),
                            )
                            .child(
                                div()
                                    .flex_col()
                                    .child("自动高度")
                                    .child(img("https://picsum.photos/800/400").w(px(180.))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .items_center()
                            .w_full()
                            .border_1()
                            .border_color(rgb(0xC0C0C0))
                            .child("image with max width 100%")
                            .child(img("https://picsum.photos/800/400").max_w_full()),
                    ),
            )
        }
    }

    actions!(image_image, [Quit]);

    pub fn run_example() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        #[cfg(not(target_family = "wasm"))]
        let app = rgpui_platform::application();
        #[cfg(target_family = "wasm")]
        let app = rgpui_platform::application();
        app.with_assets(Assets {
            base: manifest_dir.clone(),
        })
        .run(move |cx: &mut App| {
            #[cfg(not(target_family = "wasm"))]
            {
                let http_client = ReqwestClient::user_agent("gpui example").unwrap();
                cx.set_http_client(Arc::new(http_client));
            }
            #[cfg(target_family = "wasm")]
            {
                // Safety: the web examples run single-threaded; the client is
                // created and used exclusively on the main thread.
                let http_client = unsafe {
                    rgpui_web::FetchHttpClient::with_user_agent("gpui example")
                        .expect("创建 FetchHttpClient 失败")
                };
                cx.set_http_client(Arc::new(http_client));
            }

            cx.activate(true);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            cx.set_menus(vec![Menu {
                name: "Image".into(),
                items: vec![MenuItem::action("Quit", Quit)],
                disabled: false,
            }]);

            let window_options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Image Example")),
                    appears_transparent: false,
                    ..Default::default()
                }),

                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    size: size(px(1100.), px(600.)),
                    origin: Point::new(px(200.), px(200.)),
                })),

                ..Default::default()
            };

            cx.open_window(window_options, |_, cx| {
                cx.new(|_| ImageShowcase {
                    // Relative path to your root project path
                    local_resource: manifest_dir
                        .join("../image_showcase/assets/app-icon.png")
                        .into(),
                    remote_resource: "https://picsum.photos/800/400".into(),
                    asset_resource: "assets/color.svg".into(),
                })
            })
            .unwrap();
        });
    }
}

mod window {

    //! 多窗口示例：展示从主窗口打开子窗口与窗口间通信。
    //! 用法：左侧 tab 选「window」，点击按钮打开第二个窗口。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, KeyBinding, PromptButton, PromptLevel, Window, WindowBounds,
        WindowKind, WindowOptions, actions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct SubWindow {
        custom_titlebar: bool,
        is_dialog: bool,
    }

    fn button(text: &str, on_click: impl Fn(&mut Window, &mut App) + 'static) -> impl IntoElement {
        div()
            .id(text.to_string())
            .flex_none()
            .px_2()
            .bg(rgb(0xf7f7f7))
            .active(|this| this.opacity(0.85))
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .rounded_sm()
            .cursor_pointer()
            .child(text.to_string())
            .on_click(move |_, window, cx| on_click(window, cx))
    }

    impl Render for SubWindow {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let window_bounds =
                WindowBounds::Windowed(Bounds::centered(None, size(px(250.0), px(200.0)), cx));

            div()
                .flex()
                .flex_col()
                .bg(rgb(0xffffff))
                .size_full()
                .gap_2()
                .when(self.custom_titlebar, |cx| {
                    cx.child(
                        div()
                            .flex()
                            .h(px(32.))
                            .px_4()
                            .bg(rgpui::blue())
                            .text_color(rgpui::white())
                            .w_full()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size_full()
                                    .child("自定义标题栏"),
                            ),
                    )
                })
                .child(
                    div()
                        .p_8()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("SubWindow")
                        .when(self.is_dialog, |div| {
                            div.child(button("Open Nested Dialog", move |_, cx| {
                                cx.open_window(
                                    WindowOptions {
                                        window_bounds: Some(window_bounds),
                                        kind: WindowKind::Dialog,
                                        ..Default::default()
                                    },
                                    |_, cx| {
                                        cx.new(|_| SubWindow {
                                            custom_titlebar: false,
                                            is_dialog: true,
                                        })
                                    },
                                )
                                .unwrap();
                            }))
                        })
                        .child(button("关闭", |window, _| {
                            window.remove_window();
                        })),
                )
        }
    }

    struct WindowDemo {
        /// 原生标题栏当前是否可见，驱动按钮文案与下一次切换的目标值
        titlebar_visible: bool,
    }

    impl Render for WindowDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let window_bounds =
                WindowBounds::Windowed(Bounds::centered(None, size(px(300.0), px(300.0)), cx));

            div()
                .p_4()
                .flex()
                .flex_wrap()
                .bg(rgb(0xffffff))
                .size_full()
                .justify_center()
                .content_center()
                .gap_2()
                .child(button("Normal", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Popup", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            kind: WindowKind::PopUp,
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Floating", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            kind: WindowKind::Floating,
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Dialog", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(window_bounds),
                            kind: WindowKind::Dialog,
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: true,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("自定义标题栏", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            titlebar: None,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: true,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Invisible", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            show: false,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Unmovable", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            is_movable: false,
                            titlebar: None,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Unresizable", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            is_resizable: false,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child(button("Unminimizable", move |_, cx| {
                    cx.open_window(
                        WindowOptions {
                            is_minimizable: false,
                            window_bounds: Some(window_bounds),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| SubWindow {
                                custom_titlebar: false,
                                is_dialog: false,
                            })
                        },
                    )
                    .unwrap();
                }))
                .child({
                    let entity = cx.entity();
                    // set_titlebar_visible 只在服务端装饰下有效：Wayland 无对应协议、
                    // 客户端装饰下服务端本就不画标题栏（由 TitleBar 元素绘制）。
                    let titlebar_toggle_supported =
                        matches!(window.window_decorations(), rgpui::Decorations::Server);
                    let label: &'static str =
                        match (self.titlebar_visible, titlebar_toggle_supported) {
                            (true, true) => "Hide Titlebar",
                            (false, true) => "Show Titlebar",
                            (true, false) => "Hide Titlebar (N/A)",
                            (false, false) => "Show Titlebar (N/A)",
                        };
                    button(label, move |window, app| {
                        if !titlebar_toggle_supported {
                            return;
                        }
                        let visible = entity.update(app, |demo, _| {
                            demo.titlebar_visible = !demo.titlebar_visible;
                            demo.titlebar_visible
                        });
                        window.set_titlebar_visible(visible);
                    })
                })
                .child(button("Hide Application", |window, cx| {
                    cx.hide();

                    // Restore the application after 3 seconds
                    window
                        .spawn(cx, async move |cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            cx.update(|_, cx| {
                                cx.activate(false);
                            })
                        })
                        .detach();
                }))
                .child(button("Resize", |window, _| {
                    let content_size = window.bounds().size;
                    window.resize(size(content_size.height, content_size.width));
                }))
                .child(button("Prompt", |window, cx| {
                    let answer = window.prompt(
                        PromptLevel::Info,
                        "Are you sure?",
                        None,
                        &["OK", "取消"],
                        cx,
                    );

                    cx.spawn(async move |_| {
                        if answer.await.unwrap() == 0 {
                            println!("You have clicked Ok");
                        } else {
                            println!("You have clicked Cancel");
                        }
                    })
                    .detach();
                }))
                .child(button("Prompt (non-English)", |window, cx| {
                    let answer = window.prompt(
                        PromptLevel::Info,
                        "Are you sure?",
                        None,
                        &[PromptButton::ok("确定"), PromptButton::cancel("取消")],
                        cx,
                    );

                    cx.spawn(async move |_| {
                        if answer.await.unwrap() == 0 {
                            println!("You have clicked Ok");
                        } else {
                            println!("You have clicked Cancel");
                        }
                    })
                    .detach();
                }))
        }
    }

    actions!(window_window, [Quit]);

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        cx.observe_window_bounds(window, move |_, window, _| {
                            println!("Window bounds changed: {:?}", window.bounds());
                        })
                        .detach();

                        WindowDemo {
                            titlebar_visible: true,
                        }
                    })
                },
            )
            .unwrap();

            cx.activate(true);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| SubWindow {
            custom_titlebar: false,
            is_dialog: true,
        })
        .into()
    }
}

mod custom_title_bar {
    //! 自定义标题栏示例：`TitleBar` 的完整用法。
    //!
    //! 运行：`cargo run -p window_showcase --bin custom_title_bar`
    //!
    //! 演示内容：
    //! - 用 `TitleBar::window_options()` 构造窗口选项（推荐做法，
    //!   已包含 `app_owns_titlebar_drag = true`，macOS 下双击/拖拽行为正常）；
    //! - 用 `TitleBar::new()` 渲染自定义标题栏，左侧放标题、右侧放操作区；
    //! - 标题栏内可放置可交互元素（按钮点击不会被拖拽吞掉）；
    //! - 右侧窗口控制按钮（最小化/最大化/关闭）由 `TitleBar` 自动绘制，
    //!   Windows/Linux 生效，macOS 显示红绿灯并预留左侧间距，Web 不显示；
    //! - 内容区复用用户原示例：问候语 + 计数器 + 颜色块。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::title_bar::TitleBar;
    use rgpui::{
        App, Bounds, Context, Entity, Hsla, SharedString, Window, WindowBounds, div, h_flex,
        prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct TitleBarDemo {
        /// 问候语中的名字。
        text: SharedString,
        /// 标题栏与内容区共享的计数器。
        count: usize,
    }

    fn title_label(text: impl Into<SharedString>) -> impl IntoElement {
        div()
            .text_sm()
            .font_weight(rgpui::FontWeight::SEMIBOLD)
            .child(text.into())
    }

    fn title_bar_button(
        id: &'static str,
        label: SharedString,
        entity: Entity<TitleBarDemo>,
        on_click: fn(&mut TitleBarDemo),
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_2()
            .py_0p5()
            .text_xs()
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0xe8e8e8)))
            .active(|style| style.bg(rgb(0xd8d8d8)))
            .child(label)
            .on_click(move |_, _, cx| {
                entity.update(cx, |view, cx| {
                    on_click(view);
                    cx.notify();
                });
            })
    }

    fn content_button(
        id: &'static str,
        label: SharedString,
        entity: Entity<TitleBarDemo>,
        on_click: fn(&mut TitleBarDemo),
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_3()
            .py_1()
            .text_sm()
            .bg(rgb(0xf0f0f0))
            .border_1()
            .border_color(rgb(0xd0d0d0))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0xe4e4e4)))
            .active(|style| style.bg(rgb(0xd4d4d4)))
            .child(label)
            .on_click(move |_, _, cx| {
                entity.update(cx, |view, cx| {
                    on_click(view);
                    cx.notify();
                });
            })
    }

    fn color_swatch(color: Hsla, border: Hsla) -> impl IntoElement {
        div()
            .size_8()
            .bg(color)
            .border_1()
            .border_dashed()
            .rounded_md()
            .border_color(border)
    }

    fn increment(view: &mut TitleBarDemo) {
        view.count += 1;
    }

    fn decrement(view: &mut TitleBarDemo) {
        view.count = view.count.saturating_sub(1);
    }

    impl Render for TitleBarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 当前实体句柄，供标题栏/内容区按钮回调更新计数器。
            let entity = cx.entity();
            // 当前计数值，供标题栏与内容区同时展示。
            let count = self.count;

            v_flex()
                .size_full()
                .bg(rgb(0xffffff))
                // 自定义标题栏：拖拽、双击缩放、窗口控制按钮都由 TitleBar 内部处理。
                .child(
                    TitleBar::new().child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .pr_2()
                            .justify_between()
                            // 左侧：应用名 + 问候对象。
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(title_label("gpui 学习"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x888888))
                                            .child(format!("Hello, {}!", self.text)),
                                    ),
                            )
                            // 右侧：计数展示 + 可交互按钮 + 占位文本。
                            // 右侧的最小化/最大化/关闭按钮由 TitleBar 自动追加。
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x888888))
                                            .child(format!("count: {count}")),
                                    )
                                    .child(title_bar_button(
                                        "titlebar-inc",
                                        "标题栏 +1".into(),
                                        entity.clone(),
                                        increment,
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0xaaaaaa))
                                            .child("Right Item"),
                                    ),
                            ),
                    ),
                )
                // 内容区：问候语 + 计数器 + 颜色块（源自用户原示例，清理了重复修饰）。
                .child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_4()
                        .p_6()
                        .child(div().text_xl().child(format!("Hello, {}!", self.text)))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(content_button(
                                    "content-dec",
                                    "-1".into(),
                                    entity.clone(),
                                    decrement,
                                ))
                                .child(
                                    div()
                                        .text_lg()
                                        .w(px(48.0))
                                        .text_center()
                                        .child(format!("{count}")),
                                )
                                .child(content_button(
                                    "content-inc",
                                    "+1".into(),
                                    entity,
                                    increment,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(color_swatch(rgpui::red(), rgpui::white()))
                                .child(color_swatch(rgpui::green(), rgpui::white()))
                                .child(color_swatch(rgpui::blue(), rgpui::white()))
                                .child(color_swatch(rgpui::yellow(), rgpui::white()))
                                .child(color_swatch(rgpui::black(), rgpui::white()))
                                .child(color_swatch(rgpui::white(), rgpui::black())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_center()
                                .text_color(rgb(0x999999))
                                .child(
                                    "拖拽标题栏空白处移动窗口，双击缩放；\
                                 右侧控制按钮 Windows/Linux 可用，macOS 用红绿灯。",
                                ),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(560.0), px(420.0)), cx);
            // 推荐用 TitleBar::window_options() 做底，再补窗口尺寸与最小尺寸。
            let mut options = TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(bounds));
            options.window_min_size = Some(size(px(400.0), px(300.0)));

            cx.open_window(options, |_, cx| {
                cx.new(|_| TitleBarDemo {
                    text: "World".into(),
                    count: 0,
                })
            })
            .unwrap();
            cx.activate(true);
        });
    }
}

mod opacity {

    //! 窗口不透明度示例：展示 `WindowBackgroundAppearance` 与透明度设置。
    //! 用法：左侧 tab 选「opacity」，观察窗口背景的半透明效果。
    use std::{fs, path::PathBuf};

    use anyhow::Result;
    use rgpui::{
        App, AssetSource, Bounds, BoxShadow, ClickEvent, Context, SharedString, Task, Window,
        WindowBounds, WindowOptions, div, hsla, img, point, prelude::*, px, rgb, size, svg,
    };
    use rgpui_platform::application;

    struct Assets {
        base: PathBuf,
    }

    impl AssetSource for Assets {
        fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
            fs::read(self.base.join(path))
                .map(|data| Some(std::borrow::Cow::Owned(data)))
                .map_err(|e| e.into())
        }

        fn list(&self, path: &str) -> Result<Vec<SharedString>> {
            fs::read_dir(self.base.join(path))
                .map(|entries| {
                    entries
                        .filter_map(|entry| {
                            entry
                                .ok()
                                .and_then(|entry| entry.file_name().into_string().ok())
                                .map(SharedString::from)
                        })
                        .collect()
                })
                .map_err(|e| e.into())
        }
    }

    struct HelloWorld {
        _task: Option<Task<()>>,
        opacity: f32,
        animating: bool,
    }

    impl HelloWorld {
        fn new(_window: &mut Window, _: &mut Context<Self>) -> Self {
            Self {
                _task: None,
                opacity: 0.5,
                animating: false,
            }
        }

        fn start_animation(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
            self.opacity = 0.0;
            self.animating = true;
            cx.notify();
        }
    }

    impl Render for HelloWorld {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.animating {
                self.opacity += 0.005;
                if self.opacity >= 1.0 {
                    self.animating = false;
                    self.opacity = 1.0;
                } else {
                    window.request_animation_frame();
                }
            }

            div()
                .flex()
                .flex_row()
                .size_full()
                .bg(rgb(0xe0e0e0))
                .text_xl()
                .child(
                    div()
                        .flex()
                        .size_full()
                        .justify_center()
                        .items_center()
                        .border_1()
                        .text_color(rgpui::blue())
                        .child(div().child("This is background text.")),
                )
                .child(
                    div()
                        .id("panel")
                        .on_click(cx.listener(Self::start_animation))
                        .absolute()
                        .top_8()
                        .left_8()
                        .right_8()
                        .bottom_8()
                        .opacity(self.opacity)
                        .flex()
                        .justify_center()
                        .items_center()
                        .bg(rgpui::white())
                        .border_3()
                        .border_color(rgpui::red())
                        .text_color(rgpui::yellow())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .justify_center()
                                .items_center()
                                .size(px(300.))
                                .bg(rgpui::blue())
                                .border_3()
                                .border_color(rgpui::black())
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.0, 0.0, 0.5),
                                    blur_radius: px(1.0),
                                    spread_radius: px(5.0),
                                    offset: point(px(10.0), px(10.0)),
                                    inset: false,
                                }])
                                .child(img("image/app-icon.png").size_8())
                                .child("Opacity Panel (Click to test)")
                                .child(
                                    div()
                                        .id("deep-level-text")
                                        .flex()
                                        .justify_center()
                                        .items_center()
                                        .p_4()
                                        .bg(rgpui::black())
                                        .text_color(rgpui::white())
                                        .text_decoration_2()
                                        .text_decoration_wavy()
                                        .text_decoration_color(rgpui::red())
                                        .child(format!("opacity: {:.1}", self.opacity)),
                                )
                                .child(
                                    svg()
                                        .path("assets/arrow_circle.svg")
                                        .text_color(rgpui::black())
                                        .text_2xl()
                                        .size_8(),
                                )
                                .child(div().flex().children(
                                    ["🎊", "✈️", "🎉", "🎈", "🎁", "🎂"].map(|emoji| {
                                        div()
                                            .child(emoji.to_string())
                                            .hover(|style| style.opacity(0.5))
                                    }),
                                ))
                                .child(img("image/black-cat-typing.gif").size_12()),
                        ),
                )
        }
    }

    pub fn run_example() {
        application()
            .with_assets(Assets {
                base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples"),
            })
            .run(|cx: &mut App| {
                let bounds = Bounds::centered(None, size(px(500.0), px(500.0)), cx);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |window, cx| cx.new(|cx| HelloWorld::new(window, cx)),
                )
                .unwrap();
                cx.activate(true);
            });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| HelloWorld::new(window, cx)).into()
    }
}

mod shadow {

    //! 窗口阴影示例：展示窗口阴影配置与效果。
    //! 用法：左侧 tab 选「shadow」，观察窗口边缘阴影。
    use rgpui::{
        App, Bounds, BoxShadow, Context, Div, SharedString, Window, WindowBounds, WindowOptions,
        div, hsla, point, prelude::*, px, relative, rgb, size,
    };
    use rgpui_platform::application;

    struct Shadow {}

    impl Shadow {
        fn base() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded_full()
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn square() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn rounded_small() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded(px(4.))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn rounded_medium() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded(px(8.))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }

        fn rounded_large() -> Div {
            div()
                .size_16()
                .bg(rgb(0xffffff))
                .rounded(px(12.))
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.0, 0.1))
        }
    }

    fn example(label: impl Into<SharedString>, example: impl IntoElement) -> impl IntoElement {
        let label = label.into();

        div()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .w(relative(1. / 6.))
            .border_r_1()
            .border_color(hsla(0.0, 0.0, 0.0, 1.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .py_12()
                    .child(example),
            )
            .child(
                div()
                    .w_full()
                    .border_t_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .p_1()
                    .flex()
                    .items_center()
                    .child(label),
            )
    }

    impl Render for Shadow {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("shadow-example")
                .overflow_y_scroll()
                .bg(rgb(0xffffff))
                .size_full()
                .text_xs()
                .child(div().flex().flex_col().w_full().children(vec![
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .flex_row()
                    .children(vec![
                        example(
                            "Square",
                            Shadow::square()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Rounded 4",
                            Shadow::rounded_small()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Rounded 8",
                            Shadow::rounded_medium()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Rounded 16",
                            Shadow::rounded_large()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Circle",
                            Shadow::base()
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.5, 0.5, 0.3),
                                    offset: point(px(0.), px(8.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                    ]),
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .w_full()
                    .children(vec![
                        example("None", Shadow::base()),
                        // 2Xsmall shadow
                        example("2X Small", Shadow::base().shadow_2xs()),
                        // Xsmall shadow
                        example("超小", Shadow::base().shadow_xs()),
                        // Small shadow
                        example("Small", Shadow::base().shadow_sm()),
                        // Medium shadow
                        example("Medium", Shadow::base().shadow_md()),
                        // Large shadow
                        example("Large", Shadow::base().shadow_lg()),
                        example("超大", Shadow::base().shadow_xl()),
                        example("2X Large", Shadow::base().shadow_2xl()),
                    ]),
                // Horizontal list of increasing blur radii
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Blur 0",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(0.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }]),
                        ),
                        example(
                            "Blur 0",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(0.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 2",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(2.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 4",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(4.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 8",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 16",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(16.),
                                spread_radius: px(0.),
                                inset: false,
                            }]),
                        ),
                        example(
                            "Blur 4",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(4.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Blur 8",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Blur 16",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(16.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Horizontal list of increasing spread radii
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Spread 0",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 2",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(2.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 4",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(4.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 8",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(8.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Spread 16",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(16.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Square spread examples
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Square Spread 0",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Spread 8",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(8.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Spread 16",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(16.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Rounded large spread examples
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Rounded Large Spread 0",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Spread 8",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(8.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Spread 16",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.0, 0.0, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(16.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Directional shadows
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Left",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(-8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Right",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Top",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(-8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Bottom",
                            Shadow::base().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Square directional shadows
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Square Left",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(-8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Right",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Top",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(-8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Square Bottom",
                            Shadow::square().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Rounded large directional shadows
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "Rounded Large Left",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(-8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Right",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(8.), px(0.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Top",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(-8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                        example(
                            "Rounded Large Bottom",
                            Shadow::rounded_large().shadow(vec![BoxShadow {
                                color: hsla(0.0, 0.5, 0.5, 0.3),
                                offset: point(px(0.), px(8.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                            inset: false,
                            }]),
                        ),
                    ]),
                // Multiple shadows for different shapes
                div()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 1.0))
                    .flex()
                    .children(vec![
                        example(
                            "多圆",
                            Shadow::base().shadow(vec![
                                BoxShadow {
                                    color: hsla(0.0 / 360., 1.0, 0.5, 0.3), // Red
                                    offset: point(px(0.), px(-12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(60.0 / 360., 1.0, 0.5, 0.3), // Yellow
                                    offset: point(px(12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(120.0 / 360., 1.0, 0.5, 0.3), // Green
                                    offset: point(px(0.), px(12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(240.0 / 360., 1.0, 0.5, 0.3), // Blue
                                    offset: point(px(-12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                            ]),
                        ),
                        example(
                            "Square Multiple",
                            Shadow::square().shadow(vec![
                                BoxShadow {
                                    color: hsla(0.0 / 360., 1.0, 0.5, 0.3), // Red
                                    offset: point(px(0.), px(-12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(60.0 / 360., 1.0, 0.5, 0.3), // Yellow
                                    offset: point(px(12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(120.0 / 360., 1.0, 0.5, 0.3), // Green
                                    offset: point(px(0.), px(12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(240.0 / 360., 1.0, 0.5, 0.3), // Blue
                                    offset: point(px(-12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                            ]),
                        ),
                        example(
                            "Rounded Large Multiple",
                            Shadow::rounded_large().shadow(vec![
                                BoxShadow {
                                    color: hsla(0.0 / 360., 1.0, 0.5, 0.3), // Red
                                    offset: point(px(0.), px(-12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(60.0 / 360., 1.0, 0.5, 0.3), // Yellow
                                    offset: point(px(12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(120.0 / 360., 1.0, 0.5, 0.3), // Green
                                    offset: point(px(0.), px(12.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                                BoxShadow {
                                    color: hsla(240.0 / 360., 1.0, 0.5, 0.3), // Blue
                                    offset: point(px(-12.), px(0.)),
                                    blur_radius: px(8.),
                                    spread_radius: px(2.),
                                inset: false,
                                },
                            ]),
                        ),
                    ]),
            ]))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1000.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Shadow {}),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| Shadow {}).into()
    }
}

mod transparent {

    //! 透明窗口示例：展示透明背景窗口的构造。
    //! 用法：左侧 tab 选「transparent」，透过窗口看到下层内容。
    //！ 背景透明

    use rgpui::{
        App, Bounds, Context, SharedString, Window, WindowBackgroundAppearance, WindowBounds,
        WindowDecorations, WindowOptions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct HelloWorld {
        text: SharedString,
    }

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .size(px(500.0))
                .justify_center()
                .items_center()
                .text_xl()
                .text_color(rgb(0x505050))
                .child(
                    div()
                        .child(format!("Hello, {}!", self.text))
                        .bg(rgb(0xff00ff)),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // 背景透明
                    window_background: WindowBackgroundAppearance::Transparent,
                    // 去掉标题栏和边框
                    window_decorations: Some(WindowDecorations::Client),

                    titlebar: None,
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| HelloWorld {
                        text: "World".into(),
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {
            text: "World".into(),
        })
        .into()
    }
}

mod window_movable {

    //! 窗口拖动示例：展示可移动窗口与拖拽区域设置。
    //! 用法：左侧 tab 选「window_movable」，拖动窗口标题栏移动窗口。
    use rgpui::{
        App, Bounds, Context, FocusHandle, Window, WindowBounds, WindowOptions, div, prelude::*,
        px, rgb, size,
    };
    use rgpui::{SharedString, TitlebarOptions};
    use rgpui_platform::application;

    struct ExampleWindow {
        label: SharedString,
        focus_handle: FocusHandle,
    }

    impl Render for ExampleWindow {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .track_focus(&self.focus_handle)
                .flex()
                .flex_col()
                .gap_3()
                .bg(rgb(0x2e2e2e))
                .size_full()
                .justify_center()
                .items_center()
                .p_8()
                .text_lg()
                .text_color(rgb(0xffffff))
                .child(self.label.clone())
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0xb0b0b0))
                        .child("Try to drag the titlebar, and check the Window menu."),
                )
        }
    }

    fn open_test_window(
        cx: &mut App,
        bounds: Bounds<rgpui::Pixels>,
        label: &str,
        is_movable: bool,
        appears_transparent: bool,
    ) {
        let label = SharedString::from(format!(
            "{label}\nis_movable: {is_movable}\n\
         appears_transparent: {appears_transparent}"
        ));

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                is_movable,
                titlebar: Some(TitlebarOptions {
                    title: Some(label.clone()),
                    appears_transparent,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let focus_handle = cx.focus_handle();
                    focus_handle.focus(window, cx);
                    ExampleWindow {
                        label,
                        focus_handle,
                    }
                })
            },
        )
        .unwrap();
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let window_size = size(px(420.), px(280.0));
            let base = Bounds::centered(None, window_size, cx);

            // (label, is_movable, appears_transparent, col, row)
            let windows = [
                ("Native titlebar, movable", true, false, 0.0, 0.0),
                ("Native titlebar, NOT movable", false, false, 1.0, 0.0),
                ("Custom titlebar, movable", true, true, 0.0, 1.0),
                ("Custom titlebar, NOT movable", false, true, 1.0, 1.0),
            ];

            for (label, is_movable, appears_transparent, col, row) in windows {
                let mut bounds = base;
                bounds.origin.x += window_size.width * col;
                bounds.origin.y += window_size.height * row;
                open_test_window(cx, bounds, label, is_movable, appears_transparent);
            }
        });
    }
}

mod window_positioning {

    //! 窗口定位示例：展示多屏坐标映射与窗口摆放。
    //! 用法：左侧 tab 选「window_positioning」，观察不同屏幕上的窗口位置。
    use rgpui::{
        App, Bounds, Context, DisplayId, Hsla, Pixels, SharedString, Size, Window,
        WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, div, point,
        prelude::*, px, rgb,
    };
    use rgpui_platform::application;

    struct WindowContent {
        text: SharedString,
        bounds: Bounds<Pixels>,
        bg: Hsla,
    }

    impl Render for WindowContent {
        fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let window_bounds = window.bounds();

            div()
                .flex()
                .flex_col()
                .bg(self.bg)
                .size_full()
                .items_center()
                .text_color(rgb(0xffffff))
                .child(self.text.clone())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .text_sm()
                        .items_center()
                        .size_full()
                        .child(format!(
                            "origin: {}, {} size: {}, {}",
                            self.bounds.origin.x,
                            self.bounds.origin.y,
                            self.bounds.size.width,
                            self.bounds.size.height
                        ))
                        .child(format!(
                            "cx.bounds() origin: {}, {} size {}, {}",
                            window_bounds.origin.x,
                            window_bounds.origin.y,
                            window_bounds.size.width,
                            window_bounds.size.height
                        )),
                )
        }
    }

    fn build_window_options(display_id: DisplayId, bounds: Bounds<Pixels>) -> WindowOptions {
        WindowOptions {
            // Set the bounds of the window in screen coordinates
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // Specify the display_id to ensure the window is created on the correct screen
            display_id: Some(display_id),
            titlebar: None,
            window_background: WindowBackgroundAppearance::Transparent,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            app_id: None,
            window_min_size: None,
            window_decorations: None,
            tabbing_identifier: None,
            ..Default::default()
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // Create several new windows, positioned in the top right corner of each screen
            let size = Size {
                width: px(350.),
                height: px(75.),
            };
            let margin_offset = px(150.);

            for screen in cx.displays() {
                let bounds = Bounds {
                    origin: point(margin_offset, margin_offset),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Top Left {:?}", screen.id()).into(),
                        bg: rgpui::red(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: screen.bounds().top_right()
                        - point(size.width + margin_offset, -margin_offset),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Top Right {:?}", screen.id()).into(),
                        bg: rgpui::red(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: screen.bounds().bottom_left()
                        - point(-margin_offset, size.height + margin_offset),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Bottom Left {:?}", screen.id()).into(),
                        bg: rgpui::blue(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: screen.bounds().bottom_right()
                        - point(size.width + margin_offset, size.height + margin_offset),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Bottom Right {:?}", screen.id()).into(),
                        bg: rgpui::blue(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: point(screen.bounds().center().x - size.center().x, margin_offset),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Top Center {:?}", screen.id()).into(),
                        bg: rgpui::black(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: point(margin_offset, screen.bounds().center().y - size.center().y),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Left Center {:?}", screen.id()).into(),
                        bg: rgpui::black(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: point(
                        screen.bounds().center().x - size.center().x,
                        screen.bounds().center().y - size.center().y,
                    ),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Center {:?}", screen.id()).into(),
                        bg: rgpui::black(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: point(
                        screen.bounds().size.width - size.width - margin_offset,
                        screen.bounds().center().y - size.center().y,
                    ),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Right Center {:?}", screen.id()).into(),
                        bg: rgpui::black(),
                        bounds,
                    })
                })
                .unwrap();

                let bounds = Bounds {
                    origin: point(
                        screen.bounds().center().x - size.center().x,
                        screen.bounds().size.height - size.height - margin_offset,
                    ),
                    size,
                };

                cx.open_window(build_window_options(screen.id(), bounds), |_, cx| {
                    cx.new(|_| WindowContent {
                        text: format!("Bottom Center {:?}", screen.id()).into(),
                        bg: rgpui::black(),
                        bounds,
                    })
                })
                .unwrap();
            }
        });
    }
}

mod window_shadow {

    //! 窗口阴影进阶示例：对比不同阴影参数的效果。
    //! 用法：左侧 tab 选「window_shadow」，观察阴影差异。
    use rgpui::{
        App, Bounds, Context, CursorStyle, Decorations, HitboxBehavior, Hsla, MouseButton, Pixels,
        Point, ResizeEdge, Size, Window, WindowBackgroundAppearance, WindowBounds,
        WindowDecorations, WindowOptions, black, canvas, div, green, point, prelude::*, px, rgb,
        size, transparent_black, white,
    };
    use rgpui_platform::application;

    struct WindowShadow {}

    // Things to do:
    // 1. We need a way of calculating which edge or corner the mouse is on,
    //    and then dispatch on that
    // 2. We need to improve the shadow rendering significantly
    // 3. We need to implement the techniques in here in Zed

    impl Render for WindowShadow {
        fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let decorations = window.window_decorations();
            let rounding = px(10.0);
            let shadow_size = px(10.0);
            let border_size = px(1.0);
            let grey = rgb(0x808080);
            window.set_client_inset(shadow_size);

            div()
                .id("window-backdrop")
                .bg(transparent_black())
                .map(|div| match decorations {
                    Decorations::Server => div,
                    Decorations::Client { tiling, .. } => div
                        .bg(rgpui::transparent_black())
                        .child(
                            canvas(
                                |_bounds, window, _cx| {
                                    window.insert_hitbox(
                                        Bounds::new(
                                            point(px(0.0), px(0.0)),
                                            window.window_bounds().get_bounds().size,
                                        ),
                                        HitboxBehavior::Normal,
                                    )
                                },
                                move |_bounds, hitbox, window, _cx| {
                                    let mouse = window.mouse_position();
                                    let size = window.window_bounds().get_bounds().size;
                                    let Some(edge) = resize_edge(mouse, shadow_size, size) else {
                                        return;
                                    };
                                    window.set_cursor_style(
                                        match edge {
                                            ResizeEdge::Top | ResizeEdge::Bottom => {
                                                CursorStyle::ResizeUpDown
                                            }
                                            ResizeEdge::Left | ResizeEdge::Right => {
                                                CursorStyle::ResizeLeftRight
                                            }
                                            ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                                CursorStyle::ResizeUpLeftDownRight
                                            }
                                            ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                                CursorStyle::ResizeUpRightDownLeft
                                            }
                                        },
                                        &hitbox,
                                    );
                                },
                            )
                            .size_full()
                            .absolute(),
                        )
                        .when(!(tiling.top || tiling.right), |div| {
                            div.rounded_tr(rounding)
                        })
                        .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                        .when(!tiling.top, |div| div.pt(shadow_size))
                        .when(!tiling.bottom, |div| div.pb(shadow_size))
                        .when(!tiling.left, |div| div.pl(shadow_size))
                        .when(!tiling.right, |div| div.pr(shadow_size))
                        .on_mouse_move(|_e, window, _cx| window.refresh())
                        .on_mouse_down(MouseButton::Left, move |e, window, _cx| {
                            let size = window.window_bounds().get_bounds().size;
                            let pos = e.position;

                            match resize_edge(pos, shadow_size, size) {
                                Some(edge) => window.start_window_resize(edge),
                                None => window.start_window_move(),
                            };
                        }),
                })
                .size_full()
                .child(
                    div()
                        .cursor(CursorStyle::Arrow)
                        .map(|div| match decorations {
                            Decorations::Server => div,
                            Decorations::Client { tiling } => div
                                .border_color(grey)
                                .when(!(tiling.top || tiling.right), |div| {
                                    div.rounded_tr(rounding)
                                })
                                .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                                .when(!tiling.top, |div| div.border_t(border_size))
                                .when(!tiling.bottom, |div| div.border_b(border_size))
                                .when(!tiling.left, |div| div.border_l(border_size))
                                .when(!tiling.right, |div| div.border_r(border_size))
                                .when(!tiling.is_tiled(), |div| {
                                    div.shadow(vec![rgpui::BoxShadow {
                                        color: Hsla {
                                            h: 0.,
                                            s: 0.,
                                            l: 0.,
                                            a: 0.4,
                                        },
                                        blur_radius: shadow_size / 2.,
                                        spread_radius: px(0.),
                                        inset: false,
                                        offset: point(px(0.0), px(0.0)),
                                    }])
                                }),
                        })
                        .on_mouse_move(|_e, _, cx| {
                            cx.stop_propagation();
                        })
                        .bg(rgpui::rgb(0xCCCCFF))
                        .size_full()
                        .flex()
                        .flex_col()
                        .justify_around()
                        .child(
                            div().w_full().flex().flex_row().justify_around().child(
                                div()
                                    .flex()
                                    .bg(white())
                                    .size(px(300.0))
                                    .justify_center()
                                    .items_center()
                                    .shadow_lg()
                                    .border_1()
                                    .border_color(rgb(0x0000ff))
                                    .text_xl()
                                    .text_color(rgb(0xffffff))
                                    .child(
                                        div()
                                            .id("hello")
                                            .w(px(200.0))
                                            .h(px(100.0))
                                            .bg(green())
                                            .shadow(vec![rgpui::BoxShadow {
                                                color: Hsla {
                                                    h: 0.,
                                                    s: 0.,
                                                    l: 0.,
                                                    a: 1.0,
                                                },
                                                blur_radius: px(20.0),
                                                spread_radius: px(0.0),
                                                inset: false,
                                                offset: point(px(0.0), px(0.0)),
                                            }])
                                            .map(|div| match decorations {
                                                Decorations::Server => div,
                                                Decorations::Client { .. } => div
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        |_e, window, _| {
                                                            window.start_window_move();
                                                        },
                                                    )
                                                    .on_click(|e, window, _| {
                                                        if e.is_right_click() {
                                                            window.show_window_menu(e.position());
                                                        }
                                                    })
                                                    .text_color(black())
                                                    .child("this is the custom titlebar"),
                                            }),
                                    ),
                            ),
                        ),
                )
        }
    }

    fn resize_edge(
        pos: Point<Pixels>,
        shadow_size: Pixels,
        size: Size<Pixels>,
    ) -> Option<ResizeEdge> {
        let edge = if pos.y < shadow_size && pos.x < shadow_size {
            ResizeEdge::TopLeft
        } else if pos.y < shadow_size && pos.x > size.width - shadow_size {
            ResizeEdge::TopRight
        } else if pos.y < shadow_size {
            ResizeEdge::Top
        } else if pos.y > size.height - shadow_size && pos.x < shadow_size {
            ResizeEdge::BottomLeft
        } else if pos.y > size.height - shadow_size && pos.x > size.width - shadow_size {
            ResizeEdge::BottomRight
        } else if pos.y > size.height - shadow_size {
            ResizeEdge::Bottom
        } else if pos.x < shadow_size {
            ResizeEdge::Left
        } else if pos.x > size.width - shadow_size {
            ResizeEdge::Right
        } else {
            return None;
        };
        Some(edge)
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(600.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Opaque,
                    window_decorations: Some(WindowDecorations::Client),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        cx.observe_window_appearance(window, |_, window, _| {
                            window.refresh();
                        })
                        .detach();
                        WindowShadow {}
                    })
                },
            )
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| {
            cx.observe_window_appearance(window, |_, window, _| {
                window.refresh();
            })
            .detach();
            WindowShadow {}
        })
        .into()
    }
}

mod list_example {

    //! 虚拟列表示例：展示 `List` + `ScrollHandle` 的海量数据渲染。
    //! 用法：左侧 tab 选「list_example」，滚动列表观察虚拟化渲染。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, ListAlignment, ListState, Render, Window, WindowBounds,
        WindowOptions, div, list, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    const ITEM_COUNT: usize = 40;
    const SCROLLBAR_WIDTH: f32 = 12.;

    struct BottomListDemo {
        list_state: ListState,
    }

    impl BottomListDemo {
        fn new() -> Self {
            Self {
                list_state: ListState::new(ITEM_COUNT, ListAlignment::Bottom, px(500.))
                    .measure_all(),
            }
        }
    }

    impl Render for BottomListDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let max_offset = self.list_state.max_offset_for_scrollbar().y;
            let current_offset = -self.list_state.scroll_px_offset_for_scrollbar().y;

            let viewport_height = self.list_state.viewport_bounds().size.height;

            let raw_fraction = if max_offset > px(0.) {
                current_offset / max_offset
            } else {
                0.
            };

            let total_height = viewport_height + max_offset;
            let thumb_height = if total_height > px(0.) {
                px(viewport_height.as_f32() * viewport_height.as_f32() / total_height.as_f32())
                    .max(px(30.))
            } else {
                px(30.)
            };

            let track_space = viewport_height - thumb_height;
            let thumb_top = track_space * raw_fraction;

            let bug_detected = raw_fraction > 1.0;

            div()
                .size_full()
                .bg(rgb(0xFFFFFF))
                .flex()
                .flex_col()
                .p_4()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(format!(
                            "offset: {:.0} / max: {:.0} | fraction: {:.3}",
                            current_offset.as_f32(),
                            max_offset.as_f32(),
                            raw_fraction,
                        ))
                        .child(
                            div()
                                .text_color(if bug_detected {
                                    rgb(0xCC0000)
                                } else {
                                    rgb(0x008800)
                                })
                                .child(if bug_detected {
                                    format!(
                                        "BUG: fraction is {:.3} (> 1.0) — thumb is off-track!",
                                        raw_fraction
                                    )
                                } else {
                                    "OK: fraction <= 1.0 — thumb is within track.".to_string()
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(0xCCCCCC))
                        .rounded_sm()
                        .child(
                            list(self.list_state.clone(), |index, _window, _cx| {
                                let height = px(30. + (index % 5) as f32 * 10.);
                                div()
                                    .h(height)
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .px_3()
                                    .border_b_1()
                                    .border_color(rgb(0xEEEEEE))
                                    .bg(if index % 2 == 0 {
                                        rgb(0xFAFAFA)
                                    } else {
                                        rgb(0xFFFFFF)
                                    })
                                    .text_sm()
                                    .child(format!("Item {index}"))
                                    .into_any()
                            })
                            .flex_1(),
                        )
                        // Scrollbar track
                        .child(
                            div()
                                .w(px(SCROLLBAR_WIDTH))
                                .h_full()
                                .flex_shrink_0()
                                .bg(rgb(0xE0E0E0))
                                .relative()
                                .child(
                                    // Thumb — position is unclamped to expose the bug
                                    div()
                                        .absolute()
                                        .top(thumb_top)
                                        .w_full()
                                        .h(thumb_height)
                                        .bg(if bug_detected {
                                            rgb(0xCC0000)
                                        } else {
                                            rgb(0x888888)
                                        })
                                        .rounded_sm(),
                                ),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(400.), px(500.)), cx);
            cx.open_window(
                WindowOptions {
                    focus: true,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| BottomListDemo::new()),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| BottomListDemo::new()).into()
    }
}

mod uniform_list {

    //! 等高虚拟列表示例：展示 `uniform_list` 的懒加载。
    //! 用法：左侧 tab 选「uniform_list」，滚动查看万行级列表。
    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
        uniform_list,
    };
    use rgpui_platform::application;

    struct UniformListExample {}

    impl Render for UniformListExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(rgb(0xffffff)).child(
                uniform_list(
                    "entries",
                    50,
                    cx.processor(|_this, range, _window, _cx| {
                        let mut items = Vec::new();
                        for ix in range {
                            let item = ix + 1;

                            items.push(
                                div()
                                    .id(ix)
                                    .px_2()
                                    .cursor_pointer()
                                    .on_click(move |_event, _window, _cx| {
                                        println!("clicked Item {item:?}");
                                    })
                                    .child(format!("Item {item}")),
                            );
                        }
                        items
                    }),
                )
                .h_full(),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(300.0), px(300.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| UniformListExample {}),
            )
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| UniformListExample {}).into()
    }
}

mod data_table {

    //! 数据表格示例：展示 `DataTable` 的列定义、排序与选中。
    //! 用法：左侧 tab 选「data_table」，点击表头排序、点击行选中。
    #![cfg_attr(target_family = "wasm", no_main)]

    use std::{ops::Range, rc::Rc, time::Duration};

    use rgpui::{
        App, Bounds, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render,
        SharedString, UniformListScrollHandle, Window, WindowBounds, WindowOptions, canvas, div,
        point, prelude::*, px, rgb, size, uniform_list,
    };
    use rgpui_platform::application;

    const TOTAL_ITEMS: usize = 10000;
    const SCROLLBAR_THUMB_WIDTH: Pixels = px(8.);
    const SCROLLBAR_THUMB_HEIGHT: Pixels = px(100.);

    pub struct Quote {
        name: SharedString,
        symbol: SharedString,
        last_done: f64,
        prev_close: f64,
        open: f64,
        high: f64,
        low: f64,
        timestamp: Duration,
        volume: i64,
        turnover: f64,
        ttm: f64,
        market_cap: f64,
        float_cap: f64,
        shares: f64,
        pb: f64,
        pe: f64,
        eps: f64,
        dividend: f64,
        dividend_yield: f64,
        dividend_per_share: f64,
        dividend_date: SharedString,
        dividend_payment: f64,
    }

    impl Quote {
        pub fn random() -> Self {
            use rand::RngExt;
            let mut rng = rand::rng();
            // simulate a base price in a realistic range
            let prev_close = rng.random_range(100.0..200.0);
            let change = rng.random_range(-5.0..5.0);
            let last_done = prev_close + change;
            let open = prev_close + rng.random_range(-3.0..3.0);
            let high = (prev_close + rng.random_range::<f64, _>(0.0..10.0)).max(open);
            let low = (prev_close - rng.random_range::<f64, _>(0.0..10.0)).min(open);
            let timestamp = Duration::from_secs(rng.random_range(0..86400));
            let volume = rng.random_range(1_000_000..100_000_000);
            let turnover = last_done * volume as f64;
            let symbol = {
                let mut ticker = String::new();
                if rng.random_bool(0.5) {
                    ticker.push_str(&format!(
                        "{:03}.{}",
                        rng.random_range(100..1000),
                        rng.random_range(0..10)
                    ));
                } else {
                    ticker.push_str(&format!(
                        "{}{}",
                        rng.random_range('A'..='Z'),
                        rng.random_range('A'..='Z')
                    ));
                }
                ticker.push_str(&format!(".{}", rng.random_range('A'..='Z')));
                ticker
            };
            let name = format!(
                "{} {} - #{}",
                symbol,
                rng.random_range(1..100),
                rng.random_range(10000..100000)
            );
            let ttm = rng.random_range(0.0..10.0);
            let market_cap = rng.random_range(1_000_000.0..10_000_000.0);
            let float_cap = market_cap + rng.random_range(1_000.0..10_000.0);
            let shares = rng.random_range(100.0..1000.0);
            let pb = market_cap / shares;
            let pe = market_cap / shares;
            let eps = market_cap / shares;
            let dividend = rng.random_range(0.0..10.0);
            let dividend_yield = rng.random_range(0.0..10.0);
            let dividend_per_share = rng.random_range(0.0..10.0);
            let dividend_date = SharedString::new(format!(
                "{}-{}-{}",
                rng.random_range(2000..2023),
                rng.random_range(1..12),
                rng.random_range(1..28)
            ));
            let dividend_payment = rng.random_range(0.0..10.0);

            Self {
                name: name.into(),
                symbol: symbol.into(),
                last_done,
                prev_close,
                open,
                high,
                low,
                timestamp,
                volume,
                turnover,
                pb,
                pe,
                eps,
                ttm,
                market_cap,
                float_cap,
                shares,
                dividend,
                dividend_yield,
                dividend_per_share,
                dividend_date,
                dividend_payment,
            }
        }

        fn change(&self) -> f64 {
            (self.last_done - self.prev_close) / self.prev_close * 100.0
        }

        fn change_color(&self) -> rgpui::Hsla {
            if self.change() > 0.0 {
                rgpui::green()
            } else {
                rgpui::red()
            }
        }

        fn turnover_ratio(&self) -> f64 {
            self.volume as f64 / self.turnover * 100.0
        }
    }

    #[derive(IntoElement)]
    struct TableRow {
        ix: usize,
        quote: Rc<Quote>,
    }
    impl TableRow {
        fn new(ix: usize, quote: Rc<Quote>) -> Self {
            Self { ix, quote }
        }

        fn render_cell(&self, key: &str, width: Pixels, color: rgpui::Hsla) -> impl IntoElement {
            div()
                .whitespace_nowrap()
                .truncate()
                .w(width)
                .px_1()
                .child(match key {
                    "id" => div().child(format!("{}", self.ix)),
                    "symbol" => div().child(self.quote.symbol.clone()),
                    "name" => div().child(self.quote.name.clone()),
                    "last_done" => div()
                        .text_color(color)
                        .child(format!("{:.3}", self.quote.last_done)),
                    "prev_close" => div()
                        .text_color(color)
                        .child(format!("{:.3}", self.quote.prev_close)),
                    "change" => div()
                        .text_color(color)
                        .child(format!("{:.2}%", self.quote.change())),
                    "timestamp" => div()
                        .text_color(color)
                        .child(format!("{:?}", self.quote.timestamp.as_secs())),
                    "open" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.open)),
                    "low" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.low)),
                    "high" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.high)),
                    "ttm" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.ttm)),
                    "eps" => div()
                        .text_color(color)
                        .child(format!("{:.2}", self.quote.eps)),
                    "market_cap" => {
                        div().child(format!("{:.2} M", self.quote.market_cap / 1_000_000.0))
                    }
                    "float_cap" => {
                        div().child(format!("{:.2} M", self.quote.float_cap / 1_000_000.0))
                    }
                    "turnover" => {
                        div().child(format!("{:.2} M", self.quote.turnover / 1_000_000.0))
                    }
                    "volume" => {
                        div().child(format!("{:.2} M", self.quote.volume as f64 / 1_000_000.0))
                    }
                    "turnover_ratio" => div().child(format!("{:.2}%", self.quote.turnover_ratio())),
                    "pe" => div().child(format!("{:.2}", self.quote.pe)),
                    "pb" => div().child(format!("{:.2}", self.quote.pb)),
                    "shares" => div().child(format!("{:.2}", self.quote.shares)),
                    "dividend" => div().child(format!("{:.2}", self.quote.dividend)),
                    "yield" => div().child(format!("{:.2}%", self.quote.dividend_yield)),
                    "dividend_per_share" => {
                        div().child(format!("{:.2}", self.quote.dividend_per_share))
                    }
                    "dividend_date" => div().child(format!("{}", self.quote.dividend_date)),
                    "dividend_payment" => {
                        div().child(format!("{:.2}", self.quote.dividend_payment))
                    }
                    _ => div().child("--"),
                })
        }
    }

    const FIELDS: [(&str, f32); 24] = [
        ("id", 64.),
        ("symbol", 64.),
        ("name", 180.),
        ("last_done", 80.),
        ("prev_close", 80.),
        ("open", 80.),
        ("low", 80.),
        ("high", 80.),
        ("ttm", 50.),
        ("market_cap", 96.),
        ("float_cap", 96.),
        ("turnover", 120.),
        ("volume", 100.),
        ("turnover_ratio", 96.),
        ("pe", 64.),
        ("pb", 64.),
        ("eps", 64.),
        ("shares", 96.),
        ("dividend", 64.),
        ("yield", 64.),
        ("dividend_per_share", 64.),
        ("dividend_date", 96.),
        ("dividend_payment", 64.),
        ("timestamp", 120.),
    ];

    impl RenderOnce for TableRow {
        fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
            let color = self.quote.change_color();
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(rgb(0xE0E0E0))
                .bg(if self.ix.is_multiple_of(2) {
                    rgb(0xFFFFFF)
                } else {
                    rgb(0xFAFAFA)
                })
                .py_0p5()
                .px_2()
                .children(FIELDS.map(|(key, width)| self.render_cell(key, px(width), color)))
        }
    }

    struct DataTable {
        /// Use `Rc` to share the same quote data across multiple items, avoid cloning.
        quotes: Vec<Rc<Quote>>,
        visible_range: Range<usize>,
        scroll_handle: UniformListScrollHandle,
        /// The position in thumb bounds when dragging start mouse down.
        drag_position: Option<Point<Pixels>>,
    }

    impl DataTable {
        fn new() -> Self {
            Self {
                quotes: Vec::new(),
                visible_range: 0..0,
                scroll_handle: UniformListScrollHandle::new(),
                drag_position: None,
            }
        }

        fn generate(&mut self) {
            self.quotes = (0..TOTAL_ITEMS).map(|_| Rc::new(Quote::random())).collect();
        }

        fn table_bounds(&self) -> Bounds<Pixels> {
            self.scroll_handle.0.borrow().base_handle.bounds()
        }

        fn scroll_top(&self) -> Pixels {
            self.scroll_handle.0.borrow().base_handle.offset().y
        }

        fn scroll_height(&self) -> Pixels {
            self.scroll_handle
                .0
                .borrow()
                .last_item_size
                .unwrap_or_default()
                .contents
                .height
        }

        fn render_scrollbar(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let scroll_height = self.scroll_height();
            let table_bounds = self.table_bounds();
            let table_height = table_bounds.size.height;
            if table_height == px(0.) {
                return div().id("scrollbar");
            }

            let percentage = -self.scroll_top() / scroll_height;
            let offset_top = (table_height * percentage).clamp(
                px(4.),
                (table_height - SCROLLBAR_THUMB_HEIGHT - px(4.)).max(px(4.)),
            );
            let entity = cx.entity();
            let scroll_handle = self.scroll_handle.0.borrow().base_handle.clone();

            div()
                .id("scrollbar")
                .absolute()
                .top(offset_top)
                .right_1()
                .h(SCROLLBAR_THUMB_HEIGHT)
                .w(SCROLLBAR_THUMB_WIDTH)
                .bg(rgb(0xC0C0C0))
                .hover(|this| this.bg(rgb(0xA0A0A0)))
                .rounded_lg()
                .child(
                    canvas(
                        |_, _, _| (),
                        move |thumb_bounds, _, window, _| {
                            window.on_mouse_event({
                                let entity = entity.clone();
                                move |ev: &MouseDownEvent, _, _, cx| {
                                    if !thumb_bounds.contains(&ev.position) {
                                        return;
                                    }

                                    entity.update(cx, |this, _| {
                                        this.drag_position = Some(
                                            ev.position - thumb_bounds.origin - table_bounds.origin,
                                        );
                                    })
                                }
                            });
                            window.on_mouse_event({
                                let entity = entity.clone();
                                move |_: &MouseUpEvent, _, _, cx| {
                                    entity.update(cx, |this, _| {
                                        this.drag_position = None;
                                    })
                                }
                            });

                            window.on_mouse_event(move |ev: &MouseMoveEvent, _, _, cx| {
                                if !ev.dragging() {
                                    return;
                                }

                                let Some(drag_pos) = entity.read(cx).drag_position else {
                                    return;
                                };

                                let inside_offset = drag_pos.y;
                                let percentage = ((ev.position.y - table_bounds.origin.y
                                    + inside_offset)
                                    / (table_bounds.size.height))
                                    .clamp(0., 1.);

                                let offset_y = ((scroll_height - table_bounds.size.height)
                                    * percentage)
                                    .clamp(px(0.), scroll_height - SCROLLBAR_THUMB_HEIGHT);
                                scroll_handle.set_offset(point(px(0.), -offset_y));
                                cx.notify(entity.entity_id());
                            })
                        },
                    )
                    .size_full(),
                )
        }
    }

    impl Render for DataTable {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .bg(rgpui::white())
                .text_sm()
                .size_full()
                .p_4()
                .gap_2()
                .flex()
                .flex_col()
                .child(format!(
                    "Total {} items, visible range: {:?}",
                    self.quotes.len(),
                    self.visible_range
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(0xE0E0E0))
                        .rounded_sm()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .overflow_hidden()
                                .border_b_1()
                                .border_color(rgb(0xE0E0E0))
                                .text_color(rgb(0x555555))
                                .bg(rgb(0xF0F0F0))
                                .py_1()
                                .px_2()
                                .text_xs()
                                .children(FIELDS.map(|(key, width)| {
                                    div()
                                        .whitespace_nowrap()
                                        .flex_shrink_0()
                                        .truncate()
                                        .px_1()
                                        .w(px(width))
                                        .child(key.replace("_", " ").to_uppercase())
                                })),
                        )
                        .child(
                            div()
                                .relative()
                                .size_full()
                                .child(
                                    uniform_list(
                                        "items",
                                        self.quotes.len(),
                                        cx.processor(move |this, range: Range<usize>, _, _| {
                                            this.visible_range = range.clone();
                                            let mut items =
                                                Vec::with_capacity(range.end - range.start);
                                            for i in range {
                                                if let Some(quote) = this.quotes.get(i) {
                                                    items.push(TableRow::new(i, quote.clone()));
                                                }
                                            }
                                            items
                                        }),
                                    )
                                    .size_full()
                                    .track_scroll(&self.scroll_handle),
                                )
                                .child(self.render_scrollbar(window, cx)),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    focus: true,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1280.0), px(1000.0)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| {
                        let mut table = DataTable::new();
                        table.generate();
                        table
                    })
                },
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| {
            let mut table = DataTable::new();
            table.generate();
            table
        })
        .into()
    }
}

mod tree {

    //! 深层级树示例：用于验证深层嵌套结构的渲染性能。
    //! 用法：左侧 tab 选「tree」，展开/折叠观察层级渲染。
    #![cfg_attr(target_family = "wasm", no_main)]
    //! Renders a div with deep children hierarchy. This example is useful to exemplify that Zed can
    //! handle deep hierarchies (even though it cannot just yet!).
    use std::sync::LazyLock;

    use rgpui::{
        App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct Tree {}

    static DEPTH: LazyLock<u64> = LazyLock::new(|| {
        std::env::var("GPUI_TREE_DEPTH")
            .ok()
            .and_then(|depth| depth.parse().ok())
            .unwrap_or(50)
    });

    impl Render for Tree {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut depth = *DEPTH;
            static COLORS: [rgpui::Hsla; 4] =
                [rgpui::red(), rgpui::blue(), rgpui::green(), rgpui::yellow()];
            let mut colors = COLORS.iter().cycle().copied();
            let mut next_div = || div().p_0p5().bg(colors.next().unwrap());
            let mut innermost_node = next_div();
            while depth > 0 {
                innermost_node = next_div().child(innermost_node);
                depth -= 1;
            }
            innermost_node
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(300.0), px(300.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Tree {}),
            )
            .unwrap();
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| Tree {}).into()
    }
}

/// Gallery 根视图：左侧竖行 tab + 右侧示例内容。
struct GalleryApp {
    titles: Vec<&'static str>,
    /// 当前示例的介绍与用法。
    active: usize,
    views: Vec<AnyView>,
}

impl GalleryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let titles: Vec<&'static str> = vec![
            "image_loading",
            "image_gallery",
            "svg",
            "window",
            "opacity",
            "shadow",
            "transparent",
            "window_shadow",
            "list_example",
            "uniform_list",
            "data_table",
            "tree",
        ];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] = &[
            image_loading::build,
            image_gallery::build,
            svg::build,
            window::build,
            opacity::build,
            shadow::build,
            transparent::build,
            window_shadow::build,
            list_example::build,
            uniform_list::build,
            data_table::build,
            tree::build,
        ];
        let views = builds.iter().map(|build| build(window, cx)).collect();
        Self {
            titles,
            active: 0,
            views,
        }
    }
}

impl Render for GalleryApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let theme = cx.theme();
        div()
            .id("gallery-app")
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .id("gallery-tabs")
                    .h_full()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .flex_col()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(self.titles.iter().enumerate().map(|(ix, title)| {
                        let this = this.clone();
                        Button::new(("gallery-tab", ix))
                            .label(*title)
                            .w_full()
                            .when(ix == self.active, |b| b.primary())
                            .when(ix != self.active, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                this.update(cx, |gallery, cx| {
                                    gallery.active = ix;
                                    cx.notify();
                                });
                            })
                    })),
            )
            .child(
                div()
                    .id("gallery-content")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.views[self.active].clone()),
            )
    }
}

fn run() {
    // 传 -- <demo> 时保留原有独立窗口行为；不带参数时打开 gallery 导航界面。
    if let Some(name) = std::env::args().nth(1) {
        match name.as_str() {
            "image_loading" => image_loading::run_example(),
            "image_gallery" => image_gallery::run_example(),
            "gif_viewer" => gif_viewer::run_example(),
            "svg" => svg::run_example(),
            "image" => image::run_example(),
            "window" => window::run_example(),
            "custom_title_bar" => custom_title_bar::run_example(),
            "opacity" => opacity::run_example(),
            "shadow" => shadow::run_example(),
            "transparent" => transparent::run_example(),
            "window_movable" => window_movable::run_example(),
            "window_positioning" => window_positioning::run_example(),
            "window_shadow" => window_shadow::run_example(),
            "list_example" => list_example::run_example(),
            "uniform_list" => uniform_list::run_example(),
            "data_table" => data_table::run_example(),
            "tree" => tree::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application()
        .with_assets(GalleryAssets)
        .run(|cx: &mut App| {
            rgpui::theme::init(cx);
            rgpui::input_ui::init(cx);
            rgpui::menu::init(cx);
            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| GalleryApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
