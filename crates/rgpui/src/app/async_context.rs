//! 异步应用上下文，提供可在 await 点之间持有的静态生命周期异步友好接口。

use crate::{
    AnyView, AnyWindowHandle, App, AppCell, AppContext, BackgroundExecutor, BorrowAppContext,
    Entity, EntityId, EventEmitter, Focusable, ForegroundExecutor, Global, GpuiBorrow,
    PromptButton, PromptLevel, Render, Reservation, Result, Subscription, Task, VisualContext,
    Window, WindowHandle,
};
use anyhow::{Context as _, bail};
use derive_more::{Deref, DerefMut};
use futures::channel::oneshot;
use futures::future::FutureExt;
use std::{future::Future, rc::Weak};

use super::{Context, WeakEntity};

/// [App] 的异步友好版本，具有静态生命周期，因此可以在异步代码的 `await` 点之间持有。
/// 调用 [App::spawn] 时会提供此实例，你也可以通过 [App::to_async] 创建。
///
/// 内部持有对 `App` 的弱引用。如果 app 已被丢弃，方法将 panic，
/// 但在使用 `cx.spawn()` 生成的前台任务时实际上不会发生这种情况，
/// 因为执行器会在运行每个任务之前检查 app 是否存活。
#[derive(Clone)]
pub struct AsyncApp {
    pub(crate) app: Weak<AppCell>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
}

impl AsyncApp {
    /// 获取内部 app 的强引用
    fn app(&self) -> std::rc::Rc<AppCell> {
        self.app
            .upgrade()
            .expect("app was released before async operation completed")
    }

    /// wasm 上重借安全的实体更新。
    ///
    /// 异步任务（例如 `BlinkCursor` 的定时闪烁）可能在 `App` 已被借出的时刻被泵送执行。
    /// 此时若直接 `borrow_mut` 会触发 `RefCell` 重借 panic。这里在重借时把更新延后到
    /// `App` 释放后的下一个空闲时机（通过 `spawn`）执行，对闪烁这类场景延后一个 tick 无可见影响。
    #[cfg(target_family = "wasm")]
    pub fn update_entity_reentrant<T: 'static, R: Default>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R + 'static,
    ) -> R {
        let app = self.app();
        match app.try_borrow_mut() {
            Ok(mut app) => app.update_entity(handle, update),
            Err(_) => {
                let handle = handle.clone();
                let _task = self.spawn(async move |cx: &mut AsyncApp| {
                    handle.update(cx, update);
                });
                R::default()
            }
        }
    }

    /// 非 wasm 平台直接更新实体。
    #[cfg(not(target_family = "wasm"))]
    pub fn update_entity_reentrant<T: 'static, R: Default>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R + 'static,
    ) -> R {
        self.update_entity(handle, update)
    }
}

impl AppContext for AsyncApp {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.insert_entity(reservation, build_entity)
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("无法在异步上下文中使用 as_mut。请先调用 update()")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, callback: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let app = self.app();
        let lock = app.borrow();
        lock.read_entity(handle, callback)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let app = self.app.upgrade().context("app was released")?;
        let mut lock = app.try_borrow_mut()?;
        if lock.quitting {
            bail!("app is quitting");
        }
        lock.update_window(window, f)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let app = self.app.upgrade()?;
        let mut lock = app.try_borrow_mut().ok()?;
        if lock.quitting {
            return None;
        }
        lock.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let app = self.app.upgrade().context("app was released")?;
        let lock = app.borrow();
        if lock.quitting {
            bail!("app is quitting");
        }
        lock.read_window(window, read)
    }

    #[track_caller]
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.update(|this| this.read_global(callback))
    }
}

impl AsyncApp {
    /// 调度应用程序中所有窗口进行重绘。
    pub fn refresh(&self) {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.refresh_windows();
    }

    /// 获取可用于在后台生成 future 的执行器。
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// 获取可用于在前台生成 future 的执行器。
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    /// 在应用上下文中调用给定函数，然后刷新其执行期间产生的所有效果。
    pub fn update<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.update(f)
    }

    /// 安排给定的回调在指定实体发出给定类型的事件时被调用。
    /// 回调接收发出实体的句柄和发出事件的引用。
    pub fn subscribe<T, Event>(
        &mut self,
        entity: &Entity<T>,
        on_event: impl FnMut(Entity<T>, &Event, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Event>,
        Event: 'static,
    {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.subscribe(entity, on_event)
    }

    /// 根据给定函数返回的根视图，使用给定选项打开窗口。
    pub fn open_window<V>(
        &self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>>
    where
        V: 'static + Render,
    {
        let app = self.app();
        let mut lock = app.borrow_mut();
        if lock.quitting {
            bail!("app is quitting");
        }
        lock.open_window(options, build_root_view)
    }

    /// 调度一个 future 在前台被轮询。
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        let mut cx = self.clone();
        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// 确定是否已分配指定类型的全局状态。
    pub fn has_global<G: Global>(&self) -> bool {
        let app = self.app();
        let app = app.borrow_mut();
        app.has_global::<G>()
    }

    /// 读取指定类型的全局状态，并传递给给定的回调。
    ///
    /// 如果未分配指定类型的全局状态则 panic。
    pub fn read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> R {
        let app = self.app();
        let app = app.borrow_mut();
        read(app.global(), &app)
    }

    /// 读取指定类型的全局状态，并传递给给定的回调。
    ///
    /// 类似于 [`AsyncApp::read_global`]，但返回错误而不是 panic。
    pub fn try_read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> Option<R> {
        let app = self.app();
        let app = app.borrow_mut();
        if app.quitting {
            return None;
        }
        Some(read(app.try_global()?, &app))
    }

    /// 读取指定类型的全局状态，并传递给给定的回调。
    /// 如果尚未分配此类型的全局，则分配一个默认值。
    pub fn read_default_global<G: Global + Default, R>(
        &self,
        read: impl FnOnce(&G, &App) -> R,
    ) -> R {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.update(|cx| {
            cx.default_global::<G>();
        });
        read(app.global(), &app)
    }

    /// [`App::update_global`](BorrowAppContext::update_global) 的便捷方法，
    /// 用于更新指定类型的全局状态。
    pub fn update_global<G: Global, R>(&self, update: impl FnOnce(&mut G, &mut App) -> R) -> R {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.update(|cx| cx.update_global(update))
    }

    /// 在实体和上下文上运行某个操作，当返回的结构体被丢弃时执行
    pub fn on_drop<T: 'static, Callback: FnOnce(&mut T, &mut Context<T>) + 'static>(
        &self,
        entity: &WeakEntity<T>,
        f: Callback,
    ) -> crate::rgpui_util::Deferred<impl FnOnce() + use<T, Callback>> {
        let entity = entity.clone();
        let app = self.app.clone();
        crate::rgpui_util::defer(move || {
            if let Some(app) = app.upgrade() {
                app.borrow_mut().0.update(|cx| {
                    entity.update(cx, |t, cx| f(t, cx)).ok();
                });
            }
        })
    }
}

/// 对应用上下文的克隆 owned 句柄，
/// 与当前任务关联的窗口组合在一起。
#[derive(Clone, Deref, DerefMut)]
pub struct AsyncWindowContext {
    #[deref]
    #[deref_mut]
    app: AsyncApp,
    window: AnyWindowHandle,
}

impl AsyncWindowContext {
    /// 创建新的异步窗口上下文
    pub(crate) fn new_context(app: AsyncApp, window: AnyWindowHandle) -> Self {
        Self { app, window }
    }

    /// 获取此上下文关联的窗口句柄。
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    /// [`App::update_window`] 的便捷方法。
    pub fn update<R>(&mut self, update: impl FnOnce(&mut Window, &mut App) -> R) -> Result<R> {
        self.app
            .update_window(self.window, |_, window, cx| update(window, cx))
    }

    /// [`App::update_window`] 的便捷方法。
    pub fn update_root<R>(
        &mut self,
        update: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R> {
        self.app.update_window(self.window, update)
    }

    /// [`Window::on_next_frame`] 的便捷方法。
    pub fn on_next_frame(&mut self, f: impl FnOnce(&mut Window, &mut App) + 'static) {
        self.app
            .update_window(self.window, |_, window, _| window.on_next_frame(f))
            .ok();
    }

    /// [`App::global`] 的便捷方法。
    pub fn read_global<G: Global, R>(
        &mut self,
        read: impl FnOnce(&G, &Window, &App) -> R,
    ) -> Result<R> {
        self.app
            .update_window(self.window, |_, window, cx| read(cx.global(), window, cx))
    }

    /// [`App::update_global`](BorrowAppContext::update_global) 的便捷方法，
    /// 用于更新指定类型的全局状态。
    pub fn update_global<G, R>(
        &mut self,
        update: impl FnOnce(&mut G, &mut Window, &mut App) -> R,
    ) -> Result<R>
    where
        G: Global,
    {
        self.app.update_window(self.window, |_, window, cx| {
            cx.update_global(|global, cx| update(global, window, cx))
        })
    }

    /// 调度一个未来在主线程上执行。用于收集后台任务的结果并更新 UI。
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
        R: 'static,
    {
        let mut cx = self.clone();
        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// 呈现平台对话框。
    /// 将显示提供的消息，以及每个答案的按钮。
    /// 当点击按钮时，返回的 Receiver 将收到被点击按钮的索引。
    pub fn prompt<T>(
        &mut self,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[T],
    ) -> oneshot::Receiver<usize>
    where
        T: Clone + Into<PromptButton>,
    {
        self.app
            .update_window(self.window, |_, window, cx| {
                window.prompt(level, message, detail, answers, cx)
            })
            .unwrap_or_else(|_| oneshot::channel().1)
    }
}

impl AppContext for AsyncWindowContext {
    fn new<T>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>
    where
        T: 'static,
    {
        let mut build_entity = Some(build_entity);
        match self.app.update_window(self.window, |_, _, cx| {
            cx.new(
                build_entity
                    .take()
                    .expect("build_entity is taken exactly once"),
            )
        }) {
            Ok(entity) => entity,
            Err(_) => self.app.new(
                build_entity
                    .take()
                    .expect("update_window returned Err without invoking the closure"),
            ),
        }
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        self.app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let mut args = Some((reservation, build_entity));
        match self.app.update_window(self.window, |_, _, cx| {
            let (reservation, build_entity) = args.take().expect("args are taken exactly once");
            cx.insert_entity(reservation, build_entity)
        }) {
            Ok(entity) => entity,
            Err(_) => {
                let (reservation, build_entity) = args
                    .take()
                    .expect("update_window returned Err without invoking the closure");
                self.app.insert_entity(reservation, build_entity)
            }
        }
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        self.app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("无法从异步上下文使用 as_mut()，请调用 `update`")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        self.app.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.app.update_window(window, update)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        self.app.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        self.app.read_window(window, read)
    }

    #[track_caller]
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.app.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        self.app.read_global(callback)
    }
}

impl VisualContext for AsyncWindowContext {
    type Result<T> = Result<T>;

    fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Result<Entity<T>> {
        self.app.update_window(self.window, |_, window, cx| {
            cx.new(|cx| build_entity(window, cx))
        })
    }

    fn update_window_entity<T: 'static, R>(
        &mut self,
        view: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Result<R> {
        let view = view.clone();
        self.app
            .with_window(view.entity_id(), |window, app| {
                view.update(app, |entity, cx| update(entity, window, cx))
            })
            .context("entity has no current window")
    }

    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Result<Entity<V>>
    where
        V: 'static + Render,
    {
        self.app.update_window(self.window, |_, window, cx| {
            window.replace_root(cx, build_view)
        })
    }

    fn focus<V>(&mut self, view: &Entity<V>) -> Result<()>
    where
        V: Focusable,
    {
        self.app.update_window(self.window, |_, window, cx| {
            view.read(cx).focus_handle(cx).focus(window, cx);
        })
    }
}
