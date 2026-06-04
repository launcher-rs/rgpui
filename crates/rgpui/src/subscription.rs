//! # 订阅系统
//!
//! GPUI 的订阅系统基于观察者模式，提供了高效的事件通知机制。
//! 核心组件包括：
//! - [`SubscriberSet`] - 管理一组订阅者的容器
//! - [`Subscription`] - 订阅句柄，Drop 时自动取消订阅
//!
//! ## 设计要点
//!
//! 1. **延迟激活**：新插入的订阅默认处于非活跃状态，需要通过返回的闭包手动激活
//! 2. **安全取消**：在回调执行期间取消订阅是安全的，不会导致迭代器失效
//! 3. **RAII 模式**：`Subscription` 的 `drop` 方法会自动调用取消订阅逻辑

use crate::collections::BTreeMap;
use crate::post_inc;
use std::{
    cell::{Cell, RefCell},
    fmt::Debug,
    rc::Rc,
};

/// 订阅者集合 - 管理一组以 `EmitterKey` 为键的订阅者。
///
/// `EmitterKey` 通常是 `EntityId`（实体事件）或 `TypeId`（全局事件），
/// `Callback` 是事件处理闭包的类型。
///
/// # 线程安全
///
/// 内部使用 `Rc<RefCell<...>>`，因此只能在单线程中使用。
pub(crate) struct SubscriberSet<EmitterKey, Callback>(
    Rc<RefCell<SubscriberSetState<EmitterKey, Callback>>>,
);

impl<EmitterKey, Callback> Clone for SubscriberSet<EmitterKey, Callback> {
    fn clone(&self) -> Self {
        SubscriberSet(self.0.clone())
    }
}

/// 订阅者集合的内部状态
struct SubscriberSetState<EmitterKey, Callback> {
    /// 订阅者映射表：键 -> (订阅者ID -> 订阅者)
    /// `Option<BTreeMap>` 用于在回调遍历期间临时取出整个 map，避免借用冲突
    subscribers: BTreeMap<EmitterKey, Option<BTreeMap<usize, Subscriber<Callback>>>>,
    /// 下一个可用的订阅者 ID（单调递增）
    next_subscriber_id: usize,
}

/// 单个订阅者的包装结构
struct Subscriber<Callback> {
    /// 订阅是否已激活（新插入的订阅默认未激活）
    active: Rc<Cell<bool>>,
    /// 订阅是否已被标记为丢弃（在回调执行期间取消的订阅）
    dropped: Rc<Cell<bool>>,
    /// 事件处理回调
    callback: Callback,
}

impl<EmitterKey, Callback> SubscriberSet<EmitterKey, Callback>
where
    EmitterKey: 'static + Ord + Clone + Debug,
    Callback: 'static,
{
    /// 创建一个新的空订阅者集合
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(SubscriberSetState {
            subscribers: Default::default(),
            next_subscriber_id: 0,
        })))
    }

    /// 为给定的 `emitter_key` 插入一个新的订阅。
    ///
    /// 新插入的订阅默认处于**非活跃状态**，不会在 `remove` 或 `retain` 中被处理。
    /// 需要调用返回的闭包来激活订阅。
    ///
    /// # 返回值
    ///
    /// 返回一个元组 `(_, activate)`：
    /// - 第一个元素是 [`Subscription`] 句柄，Drop 时自动取消订阅
    /// - 第二个是激活闭包，调用后订阅才会真正生效
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let (subscription, activate) = subscriber_set.insert(key, callback);
    /// activate(); // 激活订阅
    /// ```
    pub fn insert(
        &self,
        emitter_key: EmitterKey,
        callback: Callback,
    ) -> (Subscription, impl FnOnce() + use<EmitterKey, Callback>) {
        let active = Rc::new(Cell::new(false));
        let dropped = Rc::new(Cell::new(false));
        let mut lock = self.0.borrow_mut();
        // 分配一个唯一的订阅者 ID
        let subscriber_id = post_inc(&mut lock.next_subscriber_id);
        lock.subscribers
            .entry(emitter_key.clone())
            .or_default()
            .get_or_insert_with(Default::default)
            .insert(
                subscriber_id,
                Subscriber {
                    active: active.clone(),
                    dropped: dropped.clone(),
                    callback,
                },
            );
        let this = self.0.clone();

        // 创建订阅句柄，Drop 时从集合中移除自身
        let subscription = Subscription {
            unsubscribe: Some(Box::new(move || {
                // 标记为已丢弃，防止在回调遍历期间再次被处理
                dropped.set(true);

                let mut lock = this.borrow_mut();
                let Some(subscribers) = lock.subscribers.get_mut(&emitter_key) else {
                    return;
                };

                if let Some(subscribers) = subscribers {
                    subscribers.remove(&subscriber_id);
                    // 如果该 emitter 的所有订阅者都已移除，清理整个条目
                    if subscribers.is_empty() {
                        lock.subscribers.remove(&emitter_key);
                    }
                }
            })),
        };
        // 返回激活闭包
        (subscription, move || active.set(true))
    }

    /// 移除给定 emitter 的所有订阅，并返回它们的回调。
    ///
    /// 只有已激活的订阅会被返回，未激活的会被静默丢弃。
    pub fn remove(
        &self,
        emitter: &EmitterKey,
    ) -> impl IntoIterator<Item = Callback> + use<EmitterKey, Callback> {
        let subscribers = self.0.borrow_mut().subscribers.remove(emitter);
        subscribers
            .unwrap_or_default()
            .map(|s| s.into_values())
            .into_iter()
            .flatten()
            .filter_map(|subscriber| {
                if subscriber.active.get() {
                    Some(subscriber.callback)
                } else {
                    None
                }
            })
    }

    /// 遍历给定 emitter 的所有活跃订阅者。
    ///
    /// 如果回调返回 `false`，对应的订阅者将被移除。
    /// 此方法在遍历期间是安全的：即使回调中取消订阅或添加新订阅，
    /// 也不会导致迭代器失效。
    pub fn retain<F>(&self, emitter: &EmitterKey, mut f: F)
    where
        F: FnMut(&mut Callback) -> bool,
    {
        // 临时取出订阅者 map，避免在回调期间持有借用
        let Some(mut subscribers) = self
            .0
            .borrow_mut()
            .subscribers
            .get_mut(emitter)
            .and_then(|s| s.take())
        else {
            return;
        };

        subscribers.retain(|_, subscriber| {
            // 跳过未激活的订阅者（保持它们不被处理）
            if !subscriber.active.get() {
                return true;
            }
            // 在回调执行期间被取消的订阅者
            if subscriber.dropped.get() {
                return false;
            }
            let keep = f(&mut subscriber.callback);
            // 再次检查 dropped，因为回调执行期间可能触发了取消
            keep && !subscriber.dropped.get()
        });
        let mut lock = self.0.borrow_mut();

        // 合并在回调执行期间新添加的订阅者
        if let Some(Some(new_subscribers)) = lock.subscribers.remove(emitter) {
            subscribers.extend(new_subscribers);
        }

        if !subscribers.is_empty() {
            lock.subscribers.insert(emitter.clone(), Some(subscribers));
        }
    }
}

/// 订阅句柄 - GPUI 中事件订阅的 RAII 管理器。
///
/// 当 `Subscription` 被 Drop 时，对应的订阅会自动取消，
/// 回调将不再被调用。此设计确保了：
/// - 组件销毁时自动清理事件订阅，避免悬空回调
/// - 作用域结束时自动取消订阅，无需手动管理生命周期
///
/// # 示例
///
/// ```rust,ignore
/// // 订阅会在 subscription 离开作用域时自动取消
/// let subscription = cx.subscribe(&entity, |entity, event, cx| {
///     // 处理事件
/// });
///
/// // 如果需要手动取消
/// drop(subscription);
///
/// // 如果需要保持订阅活跃（即使句柄被丢弃）
/// subscription.detach();
/// ```
#[must_use]
pub struct Subscription {
    /// 取消订阅的回调，`None` 表示已分离（detach）
    unsubscribe: Option<Box<dyn FnOnce() + 'static>>,
}

impl Subscription {
    /// 创建一个新的订阅句柄，当其被 Drop 时调用 `unsubscribe` 回调。
    pub fn new(unsubscribe: impl 'static + FnOnce()) -> Self {
        Self {
            unsubscribe: Some(Box::new(unsubscribe)),
        }
    }

    /// 将订阅从句柄中分离。
    ///
    /// 分离后，即使句柄被 Drop，订阅仍会保持活跃，
    /// 直到所订阅的实体被销毁。
    /// 适用于需要"永久"监听事件的场景。
    pub fn detach(mut self) {
        self.unsubscribe.take();
    }

    /// 将两个订阅合并为一个。
    ///
    /// 返回的新订阅在 Drop 时会同时取消两个原始订阅。
    /// 调用 `detach` 会同时分离两个内部订阅。
    pub fn join(mut subscription_a: Self, mut subscription_b: Self) -> Self {
        let a_unsubscribe = subscription_a.unsubscribe.take();
        let b_unsubscribe = subscription_b.unsubscribe.take();
        Self {
            unsubscribe: Some(Box::new(move || {
                if let Some(self_unsubscribe) = a_unsubscribe {
                    self_unsubscribe();
                }
                if let Some(other_unsubscribe) = b_unsubscribe {
                    other_unsubscribe();
                }
            })),
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // Drop 时执行取消订阅逻辑
        if let Some(unsubscribe) = self.unsubscribe.take() {
            unsubscribe();
        }
    }
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Global, TestApp};

    #[test]
    fn test_unsubscribe_during_callback_with_insert() {
        struct TestGlobal;
        impl Global for TestGlobal {}

        let mut app = TestApp::new();
        app.set_global(TestGlobal);

        let observer_a_count = Rc::new(Cell::new(0usize));
        let observer_b_count = Rc::new(Cell::new(0usize));

        let sub_a: Rc<RefCell<Option<Subscription>>> = Default::default();
        let sub_b: Rc<RefCell<Option<Subscription>>> = Default::default();

        // Observer A fires first (lower subscriber_id). It drops itself and
        // inserts a new observer for the same global.
        *sub_a.borrow_mut() = Some(app.update({
            let count = observer_a_count.clone();
            let sub_a = sub_a.clone();
            move |cx| {
                cx.observe_global::<TestGlobal>(move |cx| {
                    count.set(count.get() + 1);
                    sub_a.borrow_mut().take();
                    cx.observe_global::<TestGlobal>(|_| {}).detach();
                })
            }
        }));

        // Observer B fires second. It just drops itself.
        *sub_b.borrow_mut() = Some(app.update({
            let count = observer_b_count.clone();
            let sub_b = sub_b.clone();
            move |cx| {
                cx.observe_global::<TestGlobal>(move |_cx| {
                    count.set(count.get() + 1);
                    sub_b.borrow_mut().take();
                })
            }
        }));

        // Both fire once.
        app.update(|cx| cx.set_global(TestGlobal));
        assert_eq!(observer_a_count.get(), 1);
        assert_eq!(observer_b_count.get(), 1);

        // Neither should fire again — both dropped their subscriptions.
        app.update(|cx| cx.set_global(TestGlobal));
        assert_eq!(observer_a_count.get(), 1);
        assert_eq!(observer_b_count.get(), 1, "orphaned subscriber fired again");
    }

    #[test]
    fn test_callback_dropped_by_earlier_callback_does_not_fire() {
        struct TestGlobal;
        impl Global for TestGlobal {}

        let mut app = TestApp::new();
        app.set_global(TestGlobal);

        let observer_b_count = Rc::new(Cell::new(0usize));
        let sub_b: Rc<RefCell<Option<Subscription>>> = Default::default();

        // Observer A fires first and drops B's subscription.
        app.update({
            let sub_b = sub_b.clone();
            move |cx| {
                cx.observe_global::<TestGlobal>(move |_cx| {
                    sub_b.borrow_mut().take();
                })
                .detach();
            }
        });

        // Observer B fires second — but A already dropped it.
        *sub_b.borrow_mut() = Some(app.update({
            let count = observer_b_count.clone();
            move |cx| {
                cx.observe_global::<TestGlobal>(move |_cx| {
                    count.set(count.get() + 1);
                })
            }
        }));

        app.update(|cx| cx.set_global(TestGlobal));
        assert_eq!(
            observer_b_count.get(),
            0,
            "B should not fire — A dropped its subscription"
        );
    }

    #[test]
    fn test_self_drop_during_callback() {
        struct TestGlobal;
        impl Global for TestGlobal {}

        let mut app = TestApp::new();
        app.set_global(TestGlobal);

        let count = Rc::new(Cell::new(0usize));
        let sub: Rc<RefCell<Option<Subscription>>> = Default::default();

        *sub.borrow_mut() = Some(app.update({
            let count = count.clone();
            let sub = sub.clone();
            move |cx| {
                cx.observe_global::<TestGlobal>(move |_cx| {
                    count.set(count.get() + 1);
                    sub.borrow_mut().take();
                })
            }
        }));

        app.update(|cx| cx.set_global(TestGlobal));
        assert_eq!(count.get(), 1);

        app.update(|cx| cx.set_global(TestGlobal));
        assert_eq!(count.get(), 1, "should not fire after self-drop");
    }

    #[test]
    fn test_subscription_drop() {
        struct TestGlobal;
        impl Global for TestGlobal {}

        let mut app = TestApp::new();
        app.set_global(TestGlobal);

        let count = Rc::new(Cell::new(0usize));

        let subscription = app.update({
            let count = count.clone();
            move |cx| {
                cx.observe_global::<TestGlobal>(move |_cx| {
                    count.set(count.get() + 1);
                })
            }
        });

        drop(subscription);

        app.update(|cx| cx.set_global(TestGlobal));
        assert_eq!(count.get(), 0, "should not fire after drop");
    }
}
