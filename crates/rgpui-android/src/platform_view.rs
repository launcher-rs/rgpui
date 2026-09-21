//! 原生视图嵌入（Platform Views）：把系统原生控件（视频、地图、
//! 相机预览、网页）嵌进 GPUI 渲染树。
//!
//! 沿 Flutter 的混合合成路线：
//! 1. [`PlatformView`] —— 存活原生视图实例；
//! 2. [`PlatformViewFactory`] —— 按类型建实例；
//! 3. [`PlatformViewRegistry`] —— 全局工厂注册表 + 位置跟踪（供命中测试）；
//! 4. [`PlatformViewHandle`] —— 生命周期句柄（drop 即销毁）。
//!
//! M3 只落注册表与命中测试（输入分流用）；真机合成（`FrameLayout`
//! 挂载、输入转发、生命周期）随具体包（视频/地图）在 M3 后逐个接。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// 原生视图实例标识（单调分配）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformViewId(pub u64);

impl PlatformViewId {
    /// 分配新标识。
    pub fn next() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for PlatformViewId {
    /// 展示为 `PlatformView(<n>)`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlatformView({})", self.0)
    }
}

/// 原生视图排布（逻辑像素，相对窗口原点）。
#[derive(Debug, Clone, Copy, Default)]
pub struct PlatformViewBounds {
    /// 横坐标。
    pub x: f32,
    /// 纵坐标。
    pub y: f32,
    /// 宽。
    pub width: f32,
    /// 高。
    pub height: f32,
}

impl PlatformViewBounds {
    /// 点是否落在范围内（含边）。
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

/// 建视图参数。
#[derive(Debug, Clone, Default)]
pub struct PlatformViewParams {
    /// 初始排布（逻辑像素）。
    pub bounds: PlatformViewBounds,
    /// 类型相关创建参数（如视频 URL、地图坐标）。
    pub creation_params: HashMap<String, String>,
}

/// 存活原生视图（Android `View` / iOS `UIView` 的包装）。
pub trait PlatformView: Send + Sync {
    /// 实例标识。
    fn id(&self) -> PlatformViewId;
    /// 视图类型（如 `video_player`）。
    fn view_type(&self) -> &str;
    /// 同步位置尺寸（布局变即调，逻辑像素）。
    fn set_bounds(&self, bounds: PlatformViewBounds);
    /// 显隐（进出可视区时调）。
    fn set_visible(&self, visible: bool);
    /// 层级（越大越上，GPUI 内容盖原生视图靠它）。
    fn set_z_index(&self, z_index: i32);
    /// 销毁（调后不得再用，移出视图层级）。
    fn dispose(&self);
    /// 是否已销毁。
    fn is_disposed(&self) -> bool;
}

/// 按类型建视图的工厂（包初始化时向注册表注册）。
pub trait PlatformViewFactory: Send + Sync {
    /// 建实例（失败回错误串）。
    fn create(&self, params: &PlatformViewParams) -> Result<Box<dyn PlatformView>, String>;
    /// 处理的视图类型。
    fn view_type(&self) -> &str;
}

/// 生命周期句柄（包一层 trait 对象；drop 自动销毁）。
pub struct PlatformViewHandle {
    /// 底层视图。
    view: Box<dyn PlatformView>,
}

impl std::fmt::Debug for PlatformViewHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformViewHandle")
            .field("id", &self.view.id())
            .field("view_type", &self.view.view_type())
            .finish()
    }
}

impl PlatformViewHandle {
    /// 包已有视图。
    pub fn new(view: Box<dyn PlatformView>) -> Self {
        Self { view }
    }

    /// 实例标识。
    pub fn id(&self) -> PlatformViewId {
        self.view.id()
    }

    /// 视图类型。
    pub fn view_type(&self) -> &str {
        self.view.view_type()
    }

    /// 同步位置尺寸（并更新注册表命中跟踪）。
    pub fn set_bounds(&self, bounds: PlatformViewBounds) {
        self.view.set_bounds(bounds);
        PlatformViewRegistry::global().update_view_bounds(self.view.id(), bounds);
    }

    /// 显隐。
    pub fn set_visible(&self, visible: bool) {
        self.view.set_visible(visible);
    }

    /// 设层级。
    pub fn set_z_index(&self, z_index: i32) {
        self.view.set_z_index(z_index);
    }

    /// 取底层 trait 对象。
    pub fn inner(&self) -> &dyn PlatformView {
        &*self.view
    }

    /// 显式销毁（并移出注册表跟踪）。
    pub fn dispose(&self) {
        PlatformViewRegistry::global().remove_view(self.view.id());
        self.view.dispose();
    }
}

impl Drop for PlatformViewHandle {
    /// 未销毁即随句柄销毁（并移出跟踪）。
    fn drop(&mut self) {
        if !self.view.is_disposed() {
            PlatformViewRegistry::global().remove_view(self.view.id());
            self.view.dispose();
        }
    }
}

/// 全局工厂注册表 + 存活视图位置跟踪（命中测试用）。
pub struct PlatformViewRegistry {
    /// 类型 → 工厂。
    factories: Mutex<HashMap<String, Box<dyn PlatformViewFactory>>>,
    /// 存活视图 → 当前排布。
    views: Mutex<HashMap<PlatformViewId, PlatformViewBounds>>,
}

impl PlatformViewRegistry {
    /// 取全局单例。
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<PlatformViewRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            factories: Mutex::new(HashMap::new()),
            views: Mutex::new(HashMap::new()),
        })
    }

    /// 注册类型工厂（同名覆盖）。
    pub fn register(&self, view_type: &str, factory: Box<dyn PlatformViewFactory>) {
        log::info!("PlatformView 注册类型：{view_type}");
        self.factories
            .lock()
            .unwrap()
            .insert(view_type.to_string(), factory);
    }

    /// 注销类型工厂。
    pub fn unregister(&self, view_type: &str) {
        self.factories.lock().unwrap().remove(view_type);
    }

    /// 是否有该类型工厂。
    pub fn has_factory(&self, view_type: &str) -> bool {
        self.factories.lock().unwrap().contains_key(view_type)
    }

    /// 列出已注册类型。
    pub fn registered_types(&self) -> Vec<String> {
        self.factories.lock().unwrap().keys().cloned().collect()
    }

    /// 建视图（失败回错误串；`NativeActivity` 下触摸先经 `hit_test` 分流）。
    pub fn create_view(
        &self,
        view_type: &str,
        params: PlatformViewParams,
    ) -> Result<PlatformViewHandle, String> {
        let factories = self.factories.lock().unwrap();
        let factory = factories
            .get(view_type)
            .ok_or_else(|| format!("未注册视图类型：{view_type}"))?;
        let view = factory.create(&params)?;
        let id = view.id();
        self.views.lock().unwrap().insert(id, params.bounds);
        Ok(PlatformViewHandle::new(view))
    }

    /// 更新跟踪排布（位置尺寸变时调）。
    pub fn update_view_bounds(&self, id: PlatformViewId, bounds: PlatformViewBounds) {
        if let Some(entry) = self.views.lock().unwrap().get_mut(&id) {
            *entry = bounds;
        }
    }

    /// 移出跟踪（销毁时调）。
    pub fn remove_view(&self, id: PlatformViewId) {
        self.views.lock().unwrap().remove(&id);
    }

    /// 点是否落在任一存活视图内（逻辑像素，相对窗口原点）。
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        self.views
            .lock()
            .unwrap()
            .values()
            .any(|bounds| bounds.contains(x, y))
    }

    /// 存活视图数（零即跳过命中测试）。
    pub fn active_view_count(&self) -> usize {
        self.views.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// 注册表是全局单例：碰它的单测串行跑，防并行串扰。
    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    /// 测试桩视图（记调用，不碰系统）。
    struct StubView {
        id: PlatformViewId,
        disposed: Arc<AtomicBool>,
    }

    impl PlatformView for StubView {
        fn id(&self) -> PlatformViewId {
            self.id
        }

        fn view_type(&self) -> &str {
            "stub"
        }

        fn set_bounds(&self, _bounds: PlatformViewBounds) {}

        fn set_visible(&self, _visible: bool) {}

        fn set_z_index(&self, _z_index: i32) {}

        fn dispose(&self) {
            self.disposed.store(true, Ordering::Relaxed);
        }

        fn is_disposed(&self) -> bool {
            self.disposed.load(Ordering::Relaxed)
        }
    }

    /// 测试桩工厂。
    struct StubFactory {
        disposed: Arc<AtomicBool>,
    }

    impl PlatformViewFactory for StubFactory {
        fn create(&self, _params: &PlatformViewParams) -> Result<Box<dyn PlatformView>, String> {
            Ok(Box::new(StubView {
                id: PlatformViewId::next(),
                disposed: Arc::clone(&self.disposed),
            }))
        }

        fn view_type(&self) -> &str {
            "stub"
        }
    }

    /// 用唯一类型名测，避免全局单例在并行单测间串扰。
    fn unique_type() -> String {
        format!("stub-{}", PlatformViewId::next().0)
    }

    /// 注册 → 建视图 → 跟踪计数。
    #[test]
    fn register_create_tracks_view() {
        let _serial = TEST_SERIAL.lock().unwrap();
        let registry = PlatformViewRegistry::global();
        let view_type = unique_type();
        let disposed = Arc::new(AtomicBool::new(false));
        registry.register(
            &view_type,
            Box::new(StubFactory {
                disposed: Arc::clone(&disposed),
            }),
        );
        assert!(registry.has_factory(&view_type));
        let before = registry.active_view_count();
        let handle = registry
            .create_view(
                &view_type,
                PlatformViewParams {
                    bounds: PlatformViewBounds {
                        x: 10.0,
                        y: 10.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    creation_params: HashMap::new(),
                },
            )
            .expect("建视图");
        assert_eq!(registry.active_view_count(), before + 1);
        assert_eq!(handle.view_type(), "stub");
        drop(handle);
        assert!(disposed.load(Ordering::Relaxed));
        registry.unregister(&view_type);
        assert!(!registry.has_factory(&view_type));
    }

    /// 未注册类型建视图报错。
    #[test]
    fn create_unregistered_type_fails() {
        let result = PlatformViewRegistry::global()
            .create_view("never-registered-type", PlatformViewParams::default());
        assert!(result.is_err());
    }

    /// 命中测试：范围内真，范围外假；移出后不再命中。
    #[test]
    fn hit_test_matches_bounds() {
        let _serial = TEST_SERIAL.lock().unwrap();
        let registry = PlatformViewRegistry::global();
        let view_type = unique_type();
        registry.register(
            &view_type,
            Box::new(StubFactory {
                disposed: Arc::new(AtomicBool::new(false)),
            }),
        );
        let handle = registry
            .create_view(
                &view_type,
                PlatformViewParams {
                    bounds: PlatformViewBounds {
                        x: 0.0,
                        y: 0.0,
                        width: 50.0,
                        height: 50.0,
                    },
                    creation_params: HashMap::new(),
                },
            )
            .expect("建视图");
        assert!(registry.hit_test(10.0, 10.0));
        assert!(registry.hit_test(50.0, 50.0));
        assert!(!registry.hit_test(51.0, 10.0));
        handle.set_bounds(PlatformViewBounds {
            x: 100.0,
            y: 100.0,
            width: 50.0,
            height: 50.0,
        });
        assert!(!registry.hit_test(10.0, 10.0));
        assert!(registry.hit_test(120.0, 120.0));
        handle.dispose();
        assert!(!registry.hit_test(120.0, 120.0));
        registry.unregister(&view_type);
    }

    /// 标识单调不重。
    #[test]
    fn view_ids_are_unique() {
        assert_ne!(PlatformViewId::next(), PlatformViewId::next());
    }
}
