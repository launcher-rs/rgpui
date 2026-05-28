#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)] // Not useful, GPUI makes heavy use of callbacks
#![allow(clippy::collapsible_else_if)] // False positives in platform specific code
#![allow(unused_mut)] // False positives in platform specific code

extern crate self as rgpui;
#[doc(hidden)]
pub static GPUI_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
#[macro_use]
mod action;
mod app;

mod arena;
mod asset_cache;
mod assets;
mod bounds_tree;
pub mod collections;
mod color;
/// The default colors used by GPUI.
pub mod colors;
mod element;
mod elements;
mod executor;
mod platform_scheduler;
/// Refinement types for partial initialization.
pub mod refineable;
/// Task scheduler for async execution.
pub mod scheduler;
/// A sum tree data structure, a concurrency-friendly B-tree.
pub mod sum_tree;
pub(crate) use platform_scheduler::PlatformScheduler;
mod geometry;
mod global;
/// HTTP client library.
pub mod http_client;
mod input;
mod inspector;
mod interactive;
mod key_dispatch;
mod keymap;
mod path_builder;
/// Performance benchmarking utilities.
pub mod perf;
mod platform;
pub mod prelude;
/// Profiling utilities for task timing and thread performance tracking.
pub mod profiler;
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
#[expect(missing_docs)]
pub mod queue;
mod rgpui_util;
mod scene;
mod shared_string;
mod shared_uri;
pub mod single_instance;
mod style;
mod styled;
mod subscription;
mod svg_renderer;
mod tab_stop;
mod taffy;
#[cfg(any(test, feature = "test-support"))]
pub mod test;
mod text_system;
mod tray;
#[allow(missing_docs)]
pub mod util;
mod view;
mod window;
pub mod window_positioner;

#[cfg(any(test, feature = "test-support"))]
pub use proptest;

#[cfg(doc)]
pub mod _accessibility;
#[cfg(doc)]
pub mod _ownership_and_data_flow;

/// Do not touch, here be dragons for use by rgpui_macros and such.
#[doc(hidden)]
pub mod private {
    pub use anyhow;
    pub use inventory;
    pub use schemars;
    pub use serde;
    pub use serde_json;
}

mod seal {
    /// A mechanism for restricting implementations of a trait to only those in GPUI.
    /// See: <https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/>
    pub trait Sealed {}
}

pub use accesskit;
pub use accesskit::Action as AccessibleAction;
pub use accesskit::{Orientation, Role, Toggled};

pub use crate::collections::*;
pub use action::*;
pub use anyhow::Result;
pub use app::*;
pub(crate) use arena::*;
pub use asset_cache::*;
pub use assets::*;
pub use color::*;
pub use ctor::ctor;
pub use element::*;
pub use elements::*;
pub use executor::*;
pub use geometry::*;
pub use global::*;
pub use rgpui_macros::{
    AppContext, IntoElement, Render, VisualContext, property_test, register_action, test,
};
pub use rgpui_util::arc_cow::ArcCow;
pub use shared_string::*;

// rgpui_util::Deferred 和 elements中的 Deferred冲突，所有 rgpui_util中的不直接导出
pub use crate::refineable::*;
pub use input::*;
pub use inspector::*;
pub use interactive::*;
use key_dispatch::*;
pub use keymap::*;
pub use path_builder::*;
pub use platform::*;
pub use profiler::*;
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
pub use queue::{PriorityQueueReceiver, PriorityQueueSender};
pub use rgpui_util::{
    Deferred as UtilDeferred, ResultExt, TryFutureExt, TryFutureExtBacktrace, defer, log_err,
    measure, post_inc, some_or_debug_panic,
};
pub use scene::*;
pub use shared_uri::*;
pub use single_instance::*;
use std::{any::Any, future::Future};
pub use style::*;
pub use styled::*;
pub use subscription::*;
pub use svg_renderer::*;
pub(crate) use tab_stop::*;
use taffy::TaffyLayoutEngine;
pub use taffy::{AvailableSpace, LayoutId};
#[cfg(any(test, feature = "test-support"))]
pub use test::*;
pub use text_system::*;
pub use tray::*;
pub use util::{FutureExt, Timeout};
pub use view::*;
pub use window::*;

pub use pollster::block_on;

/// GPUI 中的上下文 trait，允许不同的上下文类型
/// 在某些操作中可以互换使用。
///
/// 该 trait 提供了创建、更新和读取实体（Entity）的方法，
/// 以及窗口操作和后台任务调度的功能。
pub trait AppContext {
    /// 在应用上下文中创建新实体。
    #[expect(
        clippy::wrong_self_convention,
        reason = "`App::new` is an ubiquitous function for creating entities"
    )]
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>;

    /// 为稍后插入的实体预留一个槽位。
    /// 返回的 [Reservation] 允许你获取未来实体的 [EntityId]。
    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T>;

    /// 基于之前从 [`reserve_entity`] 获得的 [Reservation] 在应用上下文中插入新实体。
    ///
    /// [`reserve_entity`]: Self::reserve_entity
    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T>;

    /// 更新应用上下文中的实体。
    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R
    where
        T: 'static;

    /// 更新应用上下文中的实体。
    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static;

    /// 从应用上下文中读取实体。
    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static;

    /// 更新给定句柄的窗口。
    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T;

    /// 对实体的*当前*窗口运行 `f` —— 最近引用的
    /// 渲染窗口。如果实体没有当前窗口或该窗口不可用，则返回 `None`。
    /// 参见 [`App::with_window`] 了解底层查找。
    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R>;

    /// 从应用上下文中读取窗口。
    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static;

    /// 在后台线程上生成未来任务
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static;

    /// 从此应用上下文中读取全局值
    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global;
}

/// 由 [Context::reserve_entity] 返回，用于稍后传递给 [Context::insert_entity]。
/// 允许你在实体创建之前获取其 [EntityId]。
pub struct Reservation<T>(pub(crate) Slot<T>);

impl<T: 'static> Reservation<T> {
    /// 返回实体插入后将关联的 [EntityId]。
    pub fn entity_id(&self) -> EntityId {
        self.0.entity_id()
    }
}

/// 用于 GPUI 中不同可视化上下境的 trait，
/// 这些上下文需要窗口才能运行。
///
/// 该 trait 扩展了 `AppContext`，添加了窗口特定的操作，
/// 如创建窗口实体、替换根视图等。
pub trait VisualContext: AppContext {
    /// 窗口操作的结果类型。
    type Result<T>;

    /// 返回与此上下文关联的窗口句柄。
    fn window_handle(&self) -> AnyWindowHandle;

    /// 使用给定回调更新视图
    fn update_window_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Self::Result<R>;

    /// 创建新实体，可访问 `Window`。
    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// 用新视图替换窗口的根视图。
    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Self::Result<Entity<V>>
    where
        V: 'static + Render;

    /// 聚焦窗口中的实体，如果它实现了 [`Focusable`] trait。
    fn focus<V>(&mut self, entity: &Entity<V>) -> Self::Result<()>
    where
        V: Focusable;
}

/// 用于绑定 GPUI 实体类型与其可发出的事件类型的 trait。
///
/// 实现此 trait 的实体可以通过上下文发出指定类型的事件，
/// 其他组件可以订阅这些事件。
pub trait EventEmitter<E: Any>: 'static {}

/// 一个辅助 trait，用于在可以互换使用的上下文上
/// 自动实现某些方法。
pub trait BorrowAppContext {
    /// 在上下文上设置全局值。
    fn set_global<T: Global>(&mut self, global: T);
    /// 更新给定类型的全局状态。
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global;
    /// 更新给定类型的全局状态，如果之前不存在则创建默认值。
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
        let mut global = self.borrow_mut().lease_global::<G>();
        let result = f(&mut global, self);
        self.borrow_mut().end_global_lease(global);
        result
    }

    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default,
    {
        self.borrow_mut().default_global::<G>();
        self.update_global(f)
    }
}

/// 关于 GPUI 运行所在的 GPU 的信息。
///
/// 包含 GPU 是否为软件模拟、设备名称、
/// 驱动程序名称和驱动程序详细信息。
#[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GpuSpecs {
    /// GPU 是否为软件模拟（如 `llvmpipe`）在 CPU 上运行。
    pub is_software_emulated: bool,
    /// 设备名称，由 Vulkan 报告。
    pub device_name: String,
    /// 驱动程序名称，由 Vulkan 报告。
    pub driver_name: String,
    /// 有关驱动程序的更多信息，由 Vulkan 报告。
    pub driver_info: String,
}
