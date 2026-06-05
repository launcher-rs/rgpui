//! # rgpui - GPU 加速的跨平台 UI 框架
//!
//! rgpui 是基于 Zed 编辑器的 `gpui` 框架的跨平台移植版本。
//! 它提供了一套完整的 GPU 加速 UI 系统，包括：
//! - **实体系统（Entity System）**：基于 Arena 分配器的实体管理
//! - **响应式事件系统**：观察者模式的订阅和通知机制
//! - **键盘事件分发**：层级化的按键事件处理管线
//! - **布局引擎**：基于 Taffy 的 Flexbox 布局
//! - **文本渲染**：GPU 加速的文本排版和渲染
//! - **平台抽象层**：支持 Windows、macOS、Linux、Web 等平台
//!
//! ## 快速入门
//!
//! ```rust,no_run
//! use rgpui::prelude::*;
//! use rgpui::App;
//!
//! fn main() {
//!     App::new().run(|cx| {
//!         // 在这里创建你的第一个窗口和视图
//!     });
//! }
//! ```

#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)] // 回调在 GPUI 中大量使用，类型复杂度检查不适用
#![allow(clippy::collapsible_else_if)] // 平台特定代码中可能产生误报
#![allow(unused_mut)] // 平台特定代码中可能产生误报

/// 将自身 crate 重导出为 `rgpui`，使得宏展开时能正确引用
extern crate self as rgpui;

/// GPUI crate 的 manifest 目录路径，编译时由 Cargo 注入
#[doc(hidden)]
pub static GPUI_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

/// Action 系统 - 定义用户交互动作的序列化与分发机制
#[macro_use]
mod action;

/// 应用核心模块 - 包含 App 主结构体、Context、EntityMap 等
mod app;

/// Arena 分配器 - 高效的实体内存分配策略
mod arena;

/// 资源缓存 - 管理已加载的资源数据
mod asset_cache;

/// 资源缓存 - 管理已加载的资源数据
mod assets;

/// 资源系统 - 定义 Asset trait 和资源加载管线
mod bounds_tree;

/// 边界树 - 用于空间查询的高性能数据结构
pub mod collections;

/// 集合类型 - 提供 FxHashMap、FxHashSet 等高性能集合
mod color;

/// 颜色模块 - 定义 HSL、RGBA 等颜色类型及转换
/// GPUI 默认使用的颜色常量和主题定义
pub mod colors;

/// 元素系统核心 - 定义 Element trait 和元素树构建机制
mod element;

/// 内置元素 - 提供 Div、Canvas、Img 等预定义 UI 元素
mod elements;

/// 执行器模块 - 定义前台和后台任务执行器 trait
mod executor;

/// 平台调度器 - 跨平台的定时器和任务调度实现
mod platform_scheduler;

/// 精化类型 - 支持部分初始化的类型安全模式
pub mod refineable;

/// 任务调度器 - 异步任务的执行和协调
pub mod scheduler;

/// 和树数据结构 - 一种并发友好的 B-tree 变体，用于文本缓冲区等场景
pub mod sum_tree;

pub(crate) use platform_scheduler::PlatformScheduler;

/// 几何类型 - 定义 Point、Size、Bounds、BoundsRect 等几何图元
mod geometry;

/// 全局状态 - 定义 Global trait，用于跨组件共享全局状态
mod global;

/// HTTP 客户端库 - 提供异步 HTTP 请求能力
pub mod http_client;

/// 输入事件 - 定义键盘、鼠标、触摸等输入事件类型
mod input;

/// 检查器 - 开发调试用的 UI 检查工具
mod inspector;

/// 交互系统 - 定义 Focusable、ClickArea 等交互行为 trait
mod interactive;

/// 键盘事件分发 - 层级化的按键事件处理管线
mod key_dispatch;

/// 快捷键映射 - 定义 Keymap 和 KeyBinding，管理键盘快捷键绑定
mod keymap;

/// 路径构建器 - 链式调用构建路径字符串的工具
mod path_builder;

/// 性能基准测试工具 - 提供帧率和渲染性能的基准测试支持
pub mod perf;

/// 平台抽象层 - 定义 Platform trait，各平台实现此 trait 以提供系统能力
mod platform;

/// 预导入模块 - 常用类型的便捷重导出
pub mod prelude;

/// 性能分析工具 - 任务计时和线程性能追踪
pub mod profiler;

/// 优先级队列 - Windows/Linux/WASM 平台下的异步消息队列
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
pub mod queue;

mod rgpui_util;

/// 场景系统 - 元素树渲染后的场景描述，用于绘制指令生成
mod scene;

/// 共享字符串 - 引用计数的不可变字符串，降低内存拷贝开销
mod shared_string;

/// 共享 URI - 引用计数的 URI 类型
mod shared_uri;

/// 单实例模式 - 确保应用程序只有一个实例运行
pub mod single_instance;

/// 样式系统 - 定义 Style 结构体，包含布局和视觉属性
mod style;

/// 样式化 trait - 为元素添加链式样式配置的能力
mod styled;

/// 订阅系统 - 观察者模式的事件订阅与通知机制
mod subscription;

/// SVG 渲染器 - 将 SVG 路径数据光栅化为 GPU 纹理
mod svg_renderer;

/// Tab 停止位 - 文本编辑器中的 Tab 键对齐位置计算
mod tab_stop;

/// Taffy 布局引擎封装 - Flexbox 和 Grid 布局的 Rust 实现
mod taffy;

/// 测试支持模块 - 提供测试用的 mock 上下文和工具
#[cfg(any(test, feature = "test-support"))]
pub mod test;

/// 文本系统 - 文本排版、字体管理和 GPU 文本渲染管线
mod text_system;

/// 系统托盘 - 跨平台的系统托盘图标和菜单管理
mod tray;

/// 工具函数 - 异步超时、延迟执行等通用辅助工具
#[allow(missing_docs)]
pub mod util;

/// 视图系统 - 定义 View trait 和窗口视图的生命周期管理
mod view;

/// 窗口系统 - 窗口创建、管理、渲染管线和事件处理
mod window;

/// 窗口定位器 - 计算新窗口在屏幕上的初始位置
pub mod window_positioner;

/// 属性测试框架 - 用于基于属性的自动化测试
#[cfg(any(test, feature = "test-support"))]
pub use proptest;

/// 无障碍访问文档模块 - 描述 GPUI 的无障碍访问架构
#[cfg(doc)]
pub mod _accessibility;

/// 所有权与数据流文档模块 - 描述 GPUI 的数据所有权模型
#[cfg(doc)]
pub mod _ownership_and_data_flow;

/// 内部私有模块 - 供 rgpui_macros 等过程宏使用的底层依赖
/// 请勿直接使用，此处内容仅供宏展开时引用
#[doc(hidden)]
pub mod private {
    pub use anyhow;
    pub use inventory;
    pub use schemars;
    pub use serde;
    pub use serde_json;
}

/// 密封 trait 机制 - 限制 trait 只能在 rgpui 内部实现
/// 参见: <https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/>
mod seal {
    /// 密封 trait，阻止外部 crate 实现此 trait
    pub trait Sealed {}
}

/// 无障碍访问库 - 提供屏幕阅读器等辅助功能的接口
pub use accesskit;
/// 无障碍访问动作类型重导出
pub use accesskit::Action as AccessibleAction;
/// 无障碍访问枚举类型重导出（方向、角色、切换状态）
pub use accesskit::{Orientation, Role, Toggled};

// ==================== 模块重导出 ====================

/// 重导出所有集合类型
pub use crate::collections::*;
/// 重导出所有 Action 相关类型
pub use action::*;
/// 重导出 anyhow::Result 作为统一错误类型
pub use anyhow::Result;
/// 重导出 App 核心模块（App、Context、Entity、EntityId 等）
pub use app::*;
/// 重导出 Arena 分配器内部类型（crate 内部使用）
pub(crate) use arena::*;
/// 重导出资源缓存相关类型
pub use asset_cache::*;
/// 重导出资源加载相关类型
pub use assets::*;
/// 重导出颜色类型（Hsla、Rgba、HslOverride 等）
pub use color::*;
/// 重导出 ctor 宏，用于在程序启动前执行初始化函数
pub use ctor::ctor;
/// 重导出元素系统核心类型
pub use element::*;
/// 重导出所有内置元素类型
pub use elements::*;
/// 重导出执行器类型（ForegroundExecutor、BackgroundExecutor）
pub use executor::*;
/// 重导出几何类型（Point、Size、Bounds 等）
pub use geometry::*;
/// 重导出全局状态相关类型
pub use global::*;
/// 重导出 rgpui_macros 中的过程宏（AppContext、IntoElement、Render 等）
pub use rgpui_macros::{
    AppContext, IntoElement, Render, VisualContext, bench, property_test, register_action, test,
};

/// 定义用于标注了 `#[rgpui::bench]` 的基准测试的 Criterion 基准测试组。
///
/// 这镜像了 `criterion::criterion_group!`，使 GPUI 基准测试文件可以保持与普通
/// Criterion 基准测试相同的形式。
///
/// [`rgpui::bench`]: crate::bench
#[macro_export]
macro_rules! bench_group {
    ($($tokens:tt)*) => {
        criterion::criterion_group!($($tokens)*);
    };
}

/// 定义 GPUI Criterion 基准测试组的入口点。
///
/// 这镜像了 `criterion::criterion_main!`，使 GPUI 基准测试文件可以保持与普通
/// Criterion 基准测试相同的形式。
#[macro_export]
macro_rules! bench_main {
    ($($tokens:tt)*) => {
        criterion::criterion_main!($($tokens)*);
    };
}

/// 重导出 ArcCow - 引用计数的写时复制智能指针
pub use rgpui_util::arc_cow::ArcCow;
/// 重导出 SharedString 相关类型
pub use shared_string::*;

// rgpui_util::Deferred 与 elements 中的 Deferred 冲突，因此不直接导出 rgpui_util 中的 Deferred
/// 重导出精化类型（Refined、Incomplete 等部分初始化类型）
pub use crate::refineable::*;
/// 重导出输入事件相关类型
pub use input::*;
/// 重导出检查器相关类型
pub use inspector::*;
/// 重导出交互系统相关类型（Focusable、ClickArea 等）
pub use interactive::*;
use key_dispatch::*;
/// 重导出快捷键映射相关类型
pub use keymap::*;
/// 重导出路径构建器
pub use path_builder::*;
/// 重导出平台抽象层相关类型
pub use platform::*;
/// 重导出性能分析工具
pub use profiler::*;
/// 重导出优先级队列类型（仅 Windows/Linux/WASM）
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
pub use queue::{PriorityQueueReceiver, PriorityQueueSender};
/// 重导出 rgpui_util 中的常用工具（defer、log_err、measure 等）
pub use rgpui_util::{
    Deferred as UtilDeferred, ResultExt, TryFutureExt, TryFutureExtBacktrace, defer, log_err,
    measure, post_inc, some_or_debug_panic,
};
/// 重导出场景系统相关类型
pub use scene::*;
/// 重导出共享 URI 类型
pub use shared_uri::*;
/// 重导出单实例管理相关类型
pub use single_instance::*;
use std::{any::Any, future::Future};
/// 重导出样式类型（Style、Display、Position 等）
pub use style::*;
/// 重导出样式化 trait
pub use styled::*;
/// 重导出订阅系统类型（Subscription、SubscriberSet 等）
pub use subscription::*;
/// 重导出 SVG 渲染器相关类型
pub use svg_renderer::*;
/// 重导出 Tab 停止位计算（crate 内部使用）
pub(crate) use tab_stop::*;
use taffy::TaffyLayoutEngine;
/// 重导出 Taffy 布局引擎类型（AvailableSpace、LayoutId）
pub use taffy::{AvailableSpace, LayoutId};
/// 重导出测试支持工具
#[cfg(any(test, feature = "test-support"))]
pub use test::*;
/// 重导出文本系统相关类型
pub use text_system::*;
/// 重导出系统托盘相关类型
pub use tray::*;
/// 重导出异步工具（FutureExt、Timeout）
pub use util::{FutureExt, Timeout};
/// 重导出视图系统类型
pub use view::*;
/// 重导出窗口系统类型
pub use window::*;

/// 重导出 pollster::block_on 用于在同步上下文中阻塞等待异步任务
pub use pollster::block_on;

/// 应用上下文 trait - GPUI 中最核心的 trait，允许不同的上下文类型在某些操作中互换使用。
///
/// 该 trait 提供了以下核心能力：
/// - **实体管理**：创建（`new`）、更新（`update_entity`）、读取（`read_entity`）实体
/// - **窗口操作**：更新窗口（`update_window`）、获取当前窗口（`with_window`）
/// - **后台任务**：在后台线程上生成异步任务（`background_spawn`）
/// - **全局状态**：读取全局值（`read_global`）
///
/// # 实现说明
///
/// 主要实现者包括：
/// - [`App`] - 应用级别的上下文
/// - [`Window`] - 窗口级别的上下文
/// - [`Context<T>`] - 实体级别的上下文
pub trait AppContext {
    /// 在应用上下文中创建新实体。
    ///
    /// 通过闭包 `build_entity` 构建实体的初始状态，闭包会收到一个 `Context<T>`，
    /// 用于在实体创建过程中进行事件订阅、子实体创建等操作。
    ///
    /// # 参数
    /// * `build_entity` - 构建实体的闭包，接收 `Context<T>`，返回实体实例 `T`
    ///
    /// # 返回值
    /// 返回新创建的实体句柄 `Entity<T>`
    #[expect(
        clippy::wrong_self_convention,
        reason = "`App::new` is an ubiquitous function for creating entities"
    )]
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>;

    /// 为稍后插入的实体预留一个槽位。
    ///
    /// 返回的 [`Reservation`] 允许你在实体实际创建之前就获取其 [`EntityId`]，
    /// 这在需要预先知道实体 ID 的场景下非常有用（如循环创建实体时避免借用冲突）。
    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T>;

    /// 基于之前从 [`reserve_entity`] 获得的 [`Reservation`] 在应用上下文中插入新实体。
    ///
    /// 与 `new` 不同，`insert_entity` 使用之前预留的槽位来存储实体，
    /// 确保预留时获取的 EntityId 与最终创建的实体一致。
    ///
    /// [`reserve_entity`]: Self::reserve_entity
    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T>;

    /// 更新应用上下文中的实体。
    ///
    /// 通过闭包 `update` 获取实体的可变引用和上下文，对实体进行修改。
    /// 闭包的返回值会作为 `update_entity` 的返回值传递出去。
    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R
    where
        T: 'static;

    /// 获取实体的可变借用句柄（`GpuiBorrow`），用于需要长时间持有可变引用的场景。
    ///
    /// 返回的 `GpuiBorrow` 实现了 `Deref` 和 `DerefMut`，
    /// 允许直接操作实体数据。
    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static;

    /// 从应用上下文中读取实体。
    ///
    /// 通过闭包 `read` 获取实体的不可变引用和 App 句柄，进行只读操作。
    /// 与 `update_entity` 不同，此方法不会修改实体状态。
    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static;

    /// 更新给定句柄的窗口。
    ///
    /// 闭包接收三个参数：窗口的 AnyView、可变 Window 引用、可变 App 引用。
    /// 通过此方法可以修改窗口内容、替换根视图等。
    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T;

    /// 对实体的*当前*窗口运行闭包 `f`。
    ///
    /// "当前窗口"指实体最近一次被渲染到的窗口。如果实体没有当前窗口或该窗口不可用，
    /// 则返回 `None`。
    ///
    /// 参见 [`App::with_window`] 了解底层查找逻辑。
    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R>;

    /// 从应用上下文中读取窗口。
    ///
    /// 通过窗口句柄获取实体和 App 的不可变引用，进行只读操作。
    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static;

    /// 在后台线程上生成异步任务。
    ///
    /// 生成的 Future 会在后台线程池中执行，不阻塞前台 UI。
    /// 返回的 [`Task`] 可用于等待任务完成或取消任务。
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static;

    /// 从此应用上下文中读取全局值。
    ///
    /// 全局值通过 [`Global`] trait 标记，通常是应用级别的单例状态
    /// （如主题配置、全局设置等）。
    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global;
}

/// 实体预留槽位 - 由 [`Context::reserve_entity`] 返回，用于稍后传递给 [`Context::insert_entity`]。
///
/// 允许你在实体创建之前获取其 [`EntityId`]，解决了循环引用场景下
/// 实体 ID 不可预知的问题。
pub struct Reservation<T>(pub(crate) Slot<T>);

impl<T: 'static> Reservation<T> {
    /// 返回实体插入后将关联的 [`EntityId`]。
    ///
    /// 此 ID 在预留时就已确定，即使实体尚未实际创建。
    pub fn entity_id(&self) -> EntityId {
        self.0.entity_id()
    }
}

/// 可视化上下文 trait - GPUI 中需要窗口才能运行的上下文类型。
///
/// 该 trait 扩展了 [`AppContext`]，添加了窗口特定的操作：
/// - 创建窗口级别的实体
/// - 替换窗口的根视图
/// - 聚焦特定实体
///
/// # 主要实现者
/// - [`Window`] - 窗口上下文
/// - [`WindowContext`] - 窗口上下文的便捷别名
pub trait VisualContext: AppContext {
    /// 窗口操作的结果类型。在同步上下文中为 `T`，在异步上下文中为 `Task<T>`。
    type Result<T>;

    /// 返回与此上下文关联的窗口句柄。
    fn window_handle(&self) -> AnyWindowHandle;

    /// 使用给定回调更新窗口中的实体。
    ///
    /// 闭包可以同时访问实体数据和窗口，适用于需要同时操作 UI 和窗口的场景。
    fn update_window_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Self::Result<R>;

    /// 创建新实体，并可访问窗口。
    ///
    /// 新创建的实体可以订阅窗口事件、操作窗口内容等。
    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// 用新视图替换窗口的根视图。
    ///
    /// 新视图会成为窗口的主要内容，旧视图将被丢弃。
    /// 视图必须实现 [`Render`] trait。
    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Self::Result<Entity<V>>
    where
        V: 'static + Render;

    /// 聚焦窗口中的实体，前提是该实体实现了 [`Focusable`] trait。
    ///
    /// 聚焦后，该实体将成为键盘事件的主要接收者。
    fn focus<V>(&mut self, entity: &Entity<V>) -> Self::Result<()>
    where
        V: Focusable;
}

/// 事件发射器 trait - 用于绑定实体类型与其可发出的事件类型。
///
/// 实现此 trait 的实体可以通过上下文的 `emit` 方法发出指定类型的事件，
/// 其他组件可以通过 `subscribe` 方法监听这些事件。
///
/// # 示例
///
/// ```rust,ignore
/// struct MyEntity { /* ... */ }
///
/// struct MyEvent { data: String }
///
/// impl EventEmitter<MyEvent> for MyEntity {}
/// ```
pub trait EventEmitter<E: Any>: 'static {}

/// 应用上下文借用 trait - 为可以互换使用的上下文自动实现全局状态管理方法。
///
/// 此 trait 为所有实现了 `BorrowMut<App>` 的类型提供了：
/// - `set_global` - 设置全局值
/// - `update_global` - 更新全局值
/// - `update_default_global` - 更新全局值，不存在则创建默认值
pub trait BorrowAppContext {
    /// 在上下文中设置全局值。
    fn set_global<T: Global>(&mut self, global: T);

    /// 更新给定类型的全局状态。
    ///
    /// 通过闭包 `f` 获取全局值的可变引用和当前上下文的可变引用，
    /// 闭包返回值会作为 `update_global` 的返回值传递出去。
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global;

    /// 更新给定类型的全局状态，如果之前不存在则先创建默认值。
    ///
    /// 适用于需要确保全局状态一定存在的场景。
    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default;
}

impl<C> BorrowAppContext for C
where
    C: std::borrow::BorrowMut<App>,
{
    fn set_global<G: Global>(&mut self, global: G) {
        self.borrow_mut().set_global(global)
    }

    #[track_caller]
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global,
    {
        // 通过租赁机制获取全局值的可变引用，避免长时间持有借用
        let mut global = self.borrow_mut().lease_global::<G>();
        let result = f(&mut global, self);
        // 归还租赁的全局值
        self.borrow_mut().end_global_lease(global);
        result
    }

    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default,
    {
        // 确保全局值已初始化
        self.borrow_mut().default_global::<G>();
        self.update_global(f)
    }
}

/// GPU 规格信息 - 描述 rgpui 运行所在的 GPU 硬件信息。
///
/// 包含 GPU 是否为软件模拟、设备名称、驱动程序名称和详细信息。
/// 可通过 [`App::gpu_specs`] 方法获取。
#[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GpuSpecs {
    /// GPU 是否为软件模拟（如 `llvmpipe`）在 CPU 上运行。
    /// 当此值为 `true` 时，渲染性能可能显著下降。
    pub is_software_emulated: bool,
    /// 设备名称，由 Vulkan/OpenGL 报告（如 "NVIDIA GeForce RTX 3080"）。
    pub device_name: String,
    /// 驱动程序名称，由 Vulkan/OpenGL 报告。
    pub driver_name: String,
    /// 驱动程序详细信息，由 Vulkan/OpenGL 报告（如版本号等）。
    pub driver_info: String,
}
