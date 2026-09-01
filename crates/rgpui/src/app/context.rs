use crate::{
    AnyView, AnyWindowHandle, AppContext, AsyncApp, DispatchPhase, Effect, EntityId, EventEmitter,
    FocusHandle, FocusOutEvent, Focusable, Global, KeystrokeObserver, Priority, Reservation,
    SubscriberSet, Subscription, Task, WeakEntity, WeakFocusHandle, Window, WindowHandle,
};
use anyhow::Result;
use futures::FutureExt;
use std::{
    any::{Any, TypeId},
    borrow::{Borrow, BorrowMut},
    future::Future,
    ops,
    sync::Arc,
};

use super::{App, AsyncWindowContext, Entity, KeystrokeEvent};

/// 实体上下文 - 针对给定实体类型 `T` 提供专门操作的上下文。
///
/// `Context<'a, T>` 是 RGPUI 中最常用的上下文类型，它封装了：
/// - 对 `App` 的可变引用（通过 Deref/DerefMut 自动解引用）
/// - 对实体的弱引用（`WeakEntity<T>`）
///
/// 通过此上下文，你可以：
/// - 观察其他实体的变化（`observe`）
/// - 订阅事件（`subscribe`）
/// - 注册键盘事件监听（`on_keystroke`）
/// - 在后台执行异步任务（`spawn`）
/// - 通知实体重新渲染（`notify`）
///
/// # 生命周期
///
/// `Context<'a, T>` 的生命周期与 `App` 的借用绑定。
/// 当 `Context` 被 drop 时，对 `App` 的借用会释放。
pub struct Context<'a, T> {
    /// 对 App 的可变引用
    app: &'a mut App,
    /// 对当前实体的弱引用
    entity_state: WeakEntity<T>,
}

impl<'a, T> ops::Deref for Context<'a, T> {
    type Target = App;

    fn deref(&self) -> &Self::Target {
        self.app
    }
}

impl<'a, T> ops::DerefMut for Context<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.app
    }
}

impl<'a, T: 'static> Context<'a, T> {
    /// 创建新的实体上下文
    pub(crate) fn new_context(app: &'a mut App, entity_state: WeakEntity<T>) -> Self {
        Self { app, entity_state }
    }

    /// 返回此上下文关联的实体 ID
    pub fn entity_id(&self) -> EntityId {
        self.entity_state.entity_id
    }

    /// 返回此上下文所属实体的强引用句柄。
    ///
    /// 实体必须存活，否则会 panic。
    pub fn entity(&self) -> Entity<T> {
        self.weak_entity()
            .upgrade()
            .expect("当我们拥有实体上下文时，实体必须存活")
    }

    /// 返回此上下文所属实体的弱引用句柄。
    ///
    /// 弱引用不会延长实体的生命周期，适合在闭包中捕获以避免循环引用。
    pub fn weak_entity(&self) -> WeakEntity<T> {
        self.entity_state.clone()
    }

    /// 观察给定实体的变化。
    ///
    /// 当被观察的实体调用 `notify` 时，`on_notify` 回调会被调用。
    /// 回调接收当前实体的可变引用、被观察实体的句柄和上下文。
    ///
    /// 返回的 [`Subscription`] 取消后停止观察。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// cx.observe(&other_entity, |this, other_entity, cx| {
    ///     // 当 other_entity 通知时执行
    ///     this.handle_observation(&other_entity, cx);
    /// });
    /// ```
    pub fn observe<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(&mut T, Entity<W>, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: 'static,
        W: 'static,
    {
        let this = self.weak_entity();
        self.app.observe_internal(entity, move |e, cx| {
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| on_notify(this, e, cx));
                true
            } else {
                false
            }
        })
    }

    /// 观察自身的变化
    pub fn observe_self(
        &mut self,
        mut on_event: impl FnMut(&mut T, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let this = self.entity();
        self.app.observe(&this, move |this, cx| {
            this.update(cx, |this, cx| on_event(this, cx))
        })
    }

    /// 从另一个实体订阅事件类型
    pub fn subscribe<T2, Evt>(
        &mut self,
        entity: &Entity<T2>,
        mut on_event: impl FnMut(&mut T, Entity<T2>, &Evt, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: 'static,
        T2: 'static + EventEmitter<Evt>,
        Evt: 'static,
    {
        let this = self.weak_entity();
        self.app.subscribe_internal(entity, move |e, event, cx| {
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| on_event(this, e, event, cx));
                true
            } else {
                false
            }
        })
    }

    /// 从自身订阅事件类型
    pub fn subscribe_self<Evt>(
        &mut self,
        mut on_event: impl FnMut(&mut T, &Evt, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Evt>,
        Evt: 'static,
    {
        let this = self.entity();
        self.app.subscribe(&this, move |this, evt, cx| {
            this.update(cx, |this, cx| on_event(this, evt, cx))
        })
    }

    /// 注册回调，在 RGPUI 释放此实体时被调用。
    pub fn on_release(&self, on_release: impl FnOnce(&mut T, &mut App) + 'static) -> Subscription
    where
        T: 'static,
    {
        let (subscription, activate) = self.app.release_listeners.insert(
            self.entity_state.entity_id,
            Box::new(move |this, cx| {
                let this = this.downcast_mut().expect("invalid entity type");
                on_release(this, cx);
            }),
        );
        activate();
        subscription
    }

    /// 注册回调，在另一个实体释放时运行
    pub fn observe_release<T2>(
        &self,
        entity: &Entity<T2>,
        on_release: impl FnOnce(&mut T, &mut T2, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: Any,
        T2: 'static,
    {
        let entity_id = entity.entity_id();
        let this = self.weak_entity();
        let (subscription, activate) = self.app.release_listeners.insert(
            entity_id,
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                if let Some(this) = this.upgrade() {
                    this.update(cx, |this, cx| on_release(this, entity, cx));
                }
            }),
        );
        activate();
        subscription
    }

    /// 注册回调以更新给定的全局状态
    pub fn observe_global<G: 'static>(
        &mut self,
        mut f: impl FnMut(&mut T, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let handle = self.weak_entity();
        let (subscription, activate) = self.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| handle.update(cx, |view, cx| f(view, cx)).is_ok()),
        );
        self.defer(move |_| activate());
        subscription
    }

    /// 注册回调，在应用程序即将重启时被调用。
    pub fn on_app_restart(
        &self,
        mut on_restart: impl FnMut(&mut T, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let handle = self.weak_entity();
        self.app.on_app_restart(move |cx| {
            handle.update(cx, |entity, cx| on_restart(entity, cx)).ok();
        })
    }

    /// 安排在应用程序退出时调用给定函数。
    /// 此回调返回的 future 将被轮询最多 [crate::SHUTDOWN_TIMEOUT] 直到应用完全退出。
    pub fn on_app_quit<Fut>(
        &self,
        mut on_quit: impl FnMut(&mut T, &mut Context<T>) -> Fut + 'static,
    ) -> Subscription
    where
        Fut: 'static + Future<Output = ()>,
        T: 'static,
    {
        let handle = self.weak_entity();
        self.app.on_app_quit(move |cx| {
            let future = handle.update(cx, |entity, cx| on_quit(entity, cx)).ok();
            async move {
                if let Some(future) = future {
                    future.await;
                }
            }
            .boxed_local()
        })
    }

    /// 告诉 RGPUI 此实体已发生变化，其观察者应被通知。
    pub fn notify(&mut self) {
        self.app.notify(self.entity_state.entity_id);
    }

    /// 生成给定函数返回的未来。
    /// 函数提供实体所属的弱句柄和可跨 await 点持有的上下文。
    /// 返回的任务必须被持有或分离。
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        T: 'static,
        AsyncFn: AsyncFnOnce(WeakEntity<T>, &mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        let this = self.weak_entity();
        self.app.spawn(async move |cx| f(this, cx).await)
    }

    /// 在事件回调中访问视图状态的便捷方法。
    ///
    /// 许多 RGPUI 回调的形式为 `Fn(&E, &mut Window, &mut App)`，
    /// 但在这些回调中能够访问视图状态通常很有用。此方法提供了一种
    /// 便捷的方式来实现这一点。
    pub fn listener<E: ?Sized>(
        &self,
        f: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let view = self.entity().downgrade();
        move |e: &E, window: &mut Window, cx: &mut App| {
            view.update(cx, |view, cx| f(view, e, window, cx)).ok();
        }
    }

    /// 在闭包中生成视图状态的便捷方法。
    /// 有关更多详细信息，请参见 `listener`。
    pub fn processor<E, R>(
        &self,
        f: impl Fn(&mut T, E, &mut Window, &mut Context<T>) -> R + 'static,
    ) -> impl Fn(E, &mut Window, &mut App) -> R + 'static {
        let view = self.entity();
        move |e: E, window: &mut Window, cx: &mut App| {
            view.update(cx, |view, cx| f(view, e, window, cx))
        }
    }

    /// 在实体和上下文上运行某个操作，当返回的结构体被丢弃时执行
    pub fn on_drop(
        &self,
        f: impl FnOnce(&mut T, &mut Context<T>) + 'static,
    ) -> crate::rgpui_util::Deferred<impl FnOnce()> {
        let this = self.weak_entity();
        let mut cx = self.to_async();
        crate::defer(move || {
            this.update(&mut cx, f).ok();
        })
    }

    /// 聚焦给定视图，假设视图类型实现了 [`Focusable`]。
    pub fn focus_view<W: Focusable>(&mut self, view: &Entity<W>, window: &mut Window) {
        window.focus(&view.focus_handle(self), self);
    }

    /// 在下一帧运行给定的回调。
    pub fn on_next_frame(
        &self,
        window: &mut Window,
        f: impl FnOnce(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) where
        T: 'static,
    {
        let view = self.entity();
        window.on_next_frame(move |window, cx| view.update(cx, |view, cx| f(view, window, cx)));
    }

    /// 将给定函数安排在当前效果周期结束时运行，允许当前在栈上的
    /// 实体被返回给应用。
    pub fn defer_in(
        &mut self,
        window: &Window,
        f: impl FnOnce(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) {
        let view = self.weak_entity();
        let entity_id = self.entity_id();
        self.ensure_window(entity_id, window.handle.id);
        self.app.defer(move |cx| {
            cx.with_window(entity_id, |window, cx| {
                view.update(cx, |view, cx| f(view, window, cx)).ok();
            });
        });
    }

    /// 观察另一个实体的状态变化，由 [`Context::notify`] 跟踪。
    pub fn observe_in<V2>(
        &mut self,
        observed: &Entity<V2>,
        window: &mut Window,
        mut on_notify: impl FnMut(&mut T, Entity<V2>, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        V2: 'static,
        T: 'static,
    {
        let observed_id = observed.entity_id();
        let observed = observed.downgrade();
        let observer = self.weak_entity();
        let observer_id = self.entity_id();
        self.ensure_window(observer_id, window.handle.id);
        self.new_observer(
            observed_id,
            Box::new(move |cx| {
                let Some((observer, observed)) = observer.upgrade().zip(observed.upgrade()) else {
                    return false;
                };
                cx.with_window(observer_id, |window, cx| {
                    observer.update(cx, |observer, cx| {
                        on_notify(observer, observed, window, cx);
                    });
                });
                true
            }),
        )
    }

    /// 订阅另一个实体发出的事件。
    /// 你订阅的实体必须实现 [`EventEmitter`] trait。
    /// 回调会接收当前视图的引用、发出事件的 `Entity` 句柄、事件、
    /// `Window` 的可变引用以及实体的上下文。
    pub fn subscribe_in<Emitter, Evt>(
        &mut self,
        emitter: &Entity<Emitter>,
        window: &Window,
        mut on_event: impl FnMut(&mut T, &Entity<Emitter>, &Evt, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        Emitter: EventEmitter<Evt>,
        Evt: 'static,
    {
        let emitter = emitter.downgrade();
        let subscriber = self.weak_entity();
        let subscriber_id = self.entity_id();
        self.ensure_window(subscriber_id, window.handle.id);
        self.new_subscription(
            emitter.entity_id(),
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    let Some((subscriber, emitter)) = subscriber.upgrade().zip(emitter.upgrade())
                    else {
                        return false;
                    };
                    let event = event.downcast_ref().expect("invalid event type");
                    cx.with_window(subscriber_id, |window, cx| {
                        subscriber.update(cx, |subscriber, cx| {
                            on_event(subscriber, &emitter, event, window, cx);
                        });
                    });
                    true
                }),
            ),
        )
    }

    /// 注册回调，在视图被释放时被调用。
    ///
    /// 回调接收视图窗口的句柄。如果窗口在视图释放之前已关闭，
    /// 此句柄可能无效。
    pub fn on_release_in(
        &mut self,
        window: &Window,
        on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let entity = self.entity();
        self.app.observe_release_in(&entity, window, on_release)
    }

    /// 注册回调，在给定实体被释放时被调用。
    pub fn observe_release_in<T2>(
        &self,
        observed: &Entity<T2>,
        window: &Window,
        mut on_release: impl FnMut(&mut T, &mut T2, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription
    where
        T: 'static,
        T2: 'static,
    {
        let observer = self.weak_entity();
        self.app
            .observe_release_in(observed, window, move |observed, window, cx| {
                observer
                    .update(cx, |observer, cx| {
                        on_release(observer, observed, window, cx)
                    })
                    .ok();
            })
    }

    /// 注册回调，在窗口大小改变时被调用。
    pub fn observe_window_bounds(
        &self,
        window: &mut Window,
        mut callback: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let (subscription, activate) = window.bounds_observers.insert(
            (),
            Box::new(move |window, cx| {
                view.update(cx, |view, cx| callback(view, window, cx))
                    .is_ok()
            }),
        );
        activate();
        subscription
    }

    /// 注册回调，在窗口被激活或停用时被调用。
    pub fn observe_window_activation(
        &self,
        window: &mut Window,
        mut callback: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let (subscription, activate) = window.activation_observers.insert(
            (),
            Box::new(move |window, cx| {
                view.update(cx, |view, cx| callback(view, window, cx))
                    .is_ok()
            }),
        );
        activate();
        subscription
    }

    /// 注册回调，在窗口外观改变时被调用。
    pub fn observe_window_appearance(
        &self,
        window: &mut Window,
        mut callback: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let (subscription, activate) = window.appearance_observers.insert(
            (),
            Box::new(move |window, cx| {
                view.update(cx, |view, cx| callback(view, window, cx))
                    .is_ok()
            }),
        );
        activate();
        subscription
    }

    /// 注册回调，在窗口按钮布局改变时被调用。
    pub fn observe_button_layout_changed(
        &self,
        window: &mut Window,
        mut callback: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let (subscription, activate) = window.button_layout_observers.insert(
            (),
            Box::new(move |window, cx| {
                view.update(cx, |view, cx| callback(view, window, cx))
                    .is_ok()
            }),
        );
        activate();
        subscription
    }

    /// 注册回调，在应用程序的任何窗口接收到按键时被调用。
    /// 注意此回调在所有其他操作和事件机制解析之后才触发，
    /// 如果事件的传播被停止则不会调用此 API。
    pub fn observe_keystrokes(
        &mut self,
        mut f: impl FnMut(&mut T, &KeystrokeEvent, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_observers: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_observers.insert((), handler);
            activate();
            subscription
        }

        let view = self.weak_entity();
        inner(
            &self.keystroke_observers,
            Box::new(move |event, window, cx| {
                if let Some(view) = view.upgrade() {
                    view.update(cx, |view, cx| f(view, event, window, cx));
                    true
                } else {
                    false
                }
            }),
        )
    }

    /// 注册回调，在窗口的待处理输入改变时被调用。
    pub fn observe_pending_input(
        &self,
        window: &mut Window,
        mut callback: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let (subscription, activate) = window.pending_input_observers.insert(
            (),
            Box::new(move |window, cx| {
                view.update(cx, |view, cx| callback(view, window, cx))
                    .is_ok()
            }),
        );
        activate();
        subscription
    }

    /// 注册监听器，在给定的焦点句柄获得焦点时被调用。
    /// 返回订阅并在订阅被 drop 之前持续有效。
    pub fn on_focus(
        &mut self,
        handle: &FocusHandle,
        window: &mut Window,
        mut listener: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let focus_id = handle.id;
        let (subscription, activate) =
            window.new_focus_listener(Box::new(move |event, window, cx| {
                view.update(cx, |view, cx| {
                    if event.previous_focus_path.last() != Some(&focus_id)
                        && event.current_focus_path.last() == Some(&focus_id)
                    {
                        listener(view, window, cx)
                    }
                })
                .is_ok()
            }));
        self.defer(|_| activate());
        subscription
    }

    /// 注册监听器，在给定的焦点句柄或其某个子节点获得焦点时被调用。
    /// 如果给定的焦点句柄或其子节点之前已获得焦点则不会触发。
    /// 返回订阅并在订阅被 drop 之前持续有效。
    pub fn on_focus_in(
        &mut self,
        handle: &FocusHandle,
        window: &mut Window,
        mut listener: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let focus_id = handle.id;
        let (subscription, activate) =
            window.new_focus_listener(Box::new(move |event, window, cx| {
                view.update(cx, |view, cx| {
                    if event.is_focus_in(focus_id) {
                        listener(view, window, cx)
                    }
                })
                .is_ok()
            }));
        self.defer(|_| activate());
        subscription
    }

    /// 注册监听器，在给定的焦点句柄失去焦点时被调用。
    /// 返回订阅并在订阅被 drop 之前持续有效。
    pub fn on_blur(
        &mut self,
        handle: &FocusHandle,
        window: &mut Window,
        mut listener: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let focus_id = handle.id;
        let (subscription, activate) =
            window.new_focus_listener(Box::new(move |event, window, cx| {
                view.update(cx, |view, cx| {
                    if event.previous_focus_path.last() == Some(&focus_id)
                        && event.current_focus_path.last() != Some(&focus_id)
                    {
                        listener(view, window, cx)
                    }
                })
                .is_ok()
            }));
        self.defer(|_| activate());
        subscription
    }

    /// 注册监听器，在窗口中没有任何元素拥有焦点时被调用。
    /// 通常在之前获得焦点的节点从树中移除时发生，
    /// 此回调允许你选择恢复用户焦点的默认位置。
    /// 返回订阅并在订阅被 drop 之前持续有效。
    pub fn on_focus_lost(
        &mut self,
        window: &mut Window,
        mut listener: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let (subscription, activate) = window.focus_lost_listeners.insert(
            (),
            Box::new(move |window, cx| {
                view.update(cx, |view, cx| listener(view, window, cx))
                    .is_ok()
            }),
        );
        self.defer(|_| activate());
        subscription
    }

    /// 注册监听器，在给定的焦点句柄或其某个子节点失去焦点时被调用。
    /// 返回订阅并在订阅被 drop 之前持续有效。
    pub fn on_focus_out(
        &mut self,
        handle: &FocusHandle,
        window: &mut Window,
        mut listener: impl FnMut(&mut T, FocusOutEvent, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let view = self.weak_entity();
        let focus_id = handle.id;
        let (subscription, activate) =
            window.new_focus_listener(Box::new(move |event, window, cx| {
                view.update(cx, |view, cx| {
                    if let Some(blurred_id) = event.previous_focus_path.last().copied()
                        && event.is_focus_out(focus_id)
                    {
                        let event = FocusOutEvent {
                            blurred: WeakFocusHandle {
                                id: blurred_id,
                                handles: Arc::downgrade(&cx.focus_handles),
                            },
                        };
                        listener(view, event, window, cx)
                    }
                })
                .is_ok()
            }));
        self.defer(|_| activate());
        subscription
    }

    /// 调度一个未来异步运行。
    /// 给定的回调接收 [`WeakEntity<V>`] 以避免在长时间运行的进程中泄漏实体。
    /// 还接收 [`AsyncWindowContext`]，可用于在 await 点之间访问实体的状态。
    /// 返回的未来将在主线程上被轮询。
    #[track_caller]
    pub fn spawn_in<AsyncFn, R>(&self, window: &Window, f: AsyncFn) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(WeakEntity<T>, &mut AsyncWindowContext) -> R + 'static,
    {
        let view = self.weak_entity();
        window.spawn(self, async move |cx| f(view, cx).await)
    }

    /// 调度一个未来按给定优先级异步运行。
    /// 给定的回调接收 [`WeakEntity<V>`] 以避免在长时间运行的进程中泄漏实体。
    /// 还接收 [`AsyncWindowContext`]，可用于在 await 点之间访问实体的状态。
    /// 返回的未来将在主线程上被轮询。
    #[track_caller]
    pub fn spawn_in_with_priority<AsyncFn, R>(
        &self,
        priority: Priority,
        window: &Window,
        f: AsyncFn,
    ) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(WeakEntity<T>, &mut AsyncWindowContext) -> R + 'static,
    {
        let view = self.weak_entity();
        window.spawn_with_priority(priority, self, async move |cx| f(view, cx).await)
    }

    /// 注册回调，在给定的全局状态改变时被调用。
    pub fn observe_global_in<G: Global>(
        &mut self,
        window: &Window,
        mut f: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Subscription {
        let window_handle = window.handle;
        let view = self.weak_entity();
        let (subscription, activate) = self.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                // If the entity has been dropped, remove this observer.
                if view.upgrade().is_none() {
                    return false;
                }
                // If the window is unavailable (e.g. temporarily taken during a
                // nested update, or already closed), skip this notification but
                // keep the observer alive so it can fire on future changes.
                let Ok(entity_alive) = window_handle.update(cx, |_, window, cx| {
                    view.update(cx, |view, cx| f(view, window, cx)).is_ok()
                }) else {
                    return true;
                };
                entity_alive
            }),
        );
        self.defer(move |_| activate());
        subscription
    }

    /// 注册回调，在给定的操作类型被分派到窗口时被调用。
    pub fn on_action(
        &mut self,
        action_type: TypeId,
        window: &mut Window,
        listener: impl Fn(&mut T, &dyn Any, DispatchPhase, &mut Window, &mut Context<T>) + 'static,
    ) {
        let handle = self.weak_entity();
        window.on_action(action_type, move |action, phase, window, cx| {
            handle
                .update(cx, |view, cx| {
                    listener(view, action, phase, window, cx);
                })
                .ok();
        });
    }

    /// 将焦点移动到当前视图，假设视图类型实现了 [`Focusable`]。
    pub fn focus_self(&mut self, window: &mut Window)
    where
        T: Focusable,
    {
        let view = self.entity();
        window.defer(self, move |window, cx| {
            view.read(cx).focus_handle(cx).focus(window, cx)
        })
    }
}

impl<T> Context<'_, T> {
    /// 发射指定类型的事件，可由其他通过各自上下文 `subscribe` 方法订阅的实体处理。
    pub fn emit<Evt>(&mut self, event: Evt)
    where
        T: EventEmitter<Evt>,
        Evt: 'static,
    {
        let event = self
            .event_arena
            .alloc(|| event)
            .map(|it| it as &mut dyn Any);
        self.app.pending_effects.push_back(Effect::Emit {
            emitter: self.entity_state.entity_id,
            event_type: TypeId::of::<Evt>(),
            event,
        });
    }
}

impl<T> AppContext for Context<'_, T> {
    #[inline]
    fn new<U: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<U>) -> U) -> Entity<U> {
        self.app.new(build_entity)
    }

    #[inline]
    fn reserve_entity<U: 'static>(&mut self) -> Reservation<U> {
        self.app.reserve_entity()
    }

    #[inline]
    fn insert_entity<U: 'static>(
        &mut self,
        reservation: Reservation<U>,
        build_entity: impl FnOnce(&mut Context<U>) -> U,
    ) -> Entity<U> {
        self.app.insert_entity(reservation, build_entity)
    }

    #[inline]
    fn update_entity<U: 'static, R>(
        &mut self,
        handle: &Entity<U>,
        update: impl FnOnce(&mut U, &mut Context<U>) -> R,
    ) -> R {
        self.app.update_entity(handle, update)
    }

    #[inline]
    fn as_mut<'a, E>(&'a mut self, handle: &Entity<E>) -> super::GpuiBorrow<'a, E>
    where
        E: 'static,
    {
        self.app.as_mut(handle)
    }

    #[inline]
    fn read_entity<U, R>(&self, handle: &Entity<U>, read: impl FnOnce(&U, &App) -> R) -> R
    where
        U: 'static,
    {
        self.app.read_entity(handle, read)
    }

    #[inline]
    fn update_window<R, F>(&mut self, window: AnyWindowHandle, update: F) -> Result<R>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> R,
    {
        self.app.update_window(window, update)
    }

    #[inline]
    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        self.app.with_window(entity_id, f)
    }

    #[inline]
    fn read_window<U, R>(
        &self,
        window: &WindowHandle<U>,
        read: impl FnOnce(Entity<U>, &App) -> R,
    ) -> Result<R>
    where
        U: 'static,
    {
        self.app.read_window(window, read)
    }

    #[inline]
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.app.background_executor.spawn(future)
    }

    #[inline]
    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        self.app.read_global(callback)
    }
}

impl<T> Borrow<App> for Context<'_, T> {
    fn borrow(&self) -> &App {
        self.app
    }
}

impl<T> BorrowMut<App> for Context<'_, T> {
    fn borrow_mut(&mut self) -> &mut App {
        self.app
    }
}
