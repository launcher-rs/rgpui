use super::*;
use futures::{
    FutureExt,
    channel::{mpsc, oneshot},
    executor::block_on,
    future,
    sink::SinkExt,
    stream::{FuturesUnordered, StreamExt},
};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashSet},
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Waker},
};

#[test]
fn test_foreground_executor_spawn() {
    let result = TestScheduler::once(async |scheduler| {
        let task = scheduler.foreground().spawn(async move { 42 });
        task.await
    });
    assert_eq!(result, 42);
}

#[test]
fn test_background_executor_spawn() {
    TestScheduler::once(async |scheduler| {
        let task = scheduler.background().spawn(async move { 42 });
        let result = task.await;
        assert_eq!(result, 42);
    });
}

#[test]
fn test_foreground_ordering() {
    let mut traces = HashSet::new();

    TestScheduler::many(100, async |scheduler| {
        #[derive(Hash, PartialEq, Eq)]
        struct TraceEntry {
            session: usize,
            task: usize,
        }

        let trace = Rc::new(RefCell::new(Vec::new()));

        let foreground_1 = scheduler.foreground();
        for task in 0..10 {
            foreground_1
                .spawn({
                    let trace = trace.clone();
                    async move {
                        trace.borrow_mut().push(TraceEntry { session: 0, task });
                    }
                })
                .detach();
        }

        let foreground_2 = scheduler.foreground();
        for task in 0..10 {
            foreground_2
                .spawn({
                    let trace = trace.clone();
                    async move {
                        trace.borrow_mut().push(TraceEntry { session: 1, task });
                    }
                })
                .detach();
        }

        scheduler.run();

        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|entry| entry.session == 0)
                .map(|entry| entry.task)
                .collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|entry| entry.session == 1)
                .map(|entry| entry.task)
                .collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );

        traces.insert(trace.take());
    });

    assert!(traces.len() > 1, "Expected at least two traces");
}

#[test]
fn test_timer_ordering() {
    TestScheduler::many(1, async |scheduler| {
        let background = scheduler.background();
        let futures = FuturesUnordered::new();
        futures.push(
            async {
                background.timer(Duration::from_millis(100)).await;
                2
            }
            .boxed(),
        );
        futures.push(
            async {
                background.timer(Duration::from_millis(50)).await;
                1
            }
            .boxed(),
        );
        futures.push(
            async {
                background.timer(Duration::from_millis(150)).await;
                3
            }
            .boxed(),
        );
        assert_eq!(futures.collect::<Vec<_>>().await, vec![1, 2, 3]);
    });
}

#[test]
fn test_send_from_bg_to_fg() {
    TestScheduler::once(async |scheduler| {
        let foreground = scheduler.foreground();
        let background = scheduler.background();

        let (sender, receiver) = oneshot::channel::<i32>();

        background
            .spawn(async move {
                sender.send(42).unwrap();
            })
            .detach();

        let task = foreground.spawn(async move { receiver.await.unwrap() });
        let result = task.await;
        assert_eq!(result, 42);
    });
}

#[test]
fn test_randomize_order() {
    // 测试确定性模式：不同种子应产生相同的执行顺序
    let mut deterministic_results = HashSet::new();
    for seed in 0..10 {
        let config = TestSchedulerConfig {
            seed,
            randomize_order: false,
            ..Default::default()
        };
        let order = block_on(capture_execution_order(config));
        assert_eq!(order.len(), 6);
        deterministic_results.insert(order);
    }

    // 所有确定性运行应产生相同的结果
    assert_eq!(
        deterministic_results.len(),
        1,
        "确定性模式应始终产生相同的执行顺序"
    );

    // 测试随机化模式：不同种子可以产生不同的执行顺序
    let mut randomized_results = HashSet::new();
    for seed in 0..20 {
        let config = TestSchedulerConfig::with_seed(seed);
        let order = block_on(capture_execution_order(config));
        assert_eq!(order.len(), 6);
        randomized_results.insert(order);
    }

    // 随机化模式应产生多个不同的执行顺序
    assert!(
        randomized_results.len() > 1,
        "随机化模式应产生多个不同的顺序"
    );
}

async fn capture_execution_order(config: TestSchedulerConfig) -> Vec<String> {
    let scheduler = Arc::new(TestScheduler::new(config));
    let foreground = scheduler.foreground();
    let background = scheduler.background();

    let (sender, receiver) = mpsc::unbounded::<String>();

    // 生成前台任务
    for i in 0..3 {
        let mut sender = sender.clone();
        foreground
            .spawn(async move {
                sender.send(format!("fg-{}", i)).await.ok();
            })
            .detach();
    }

    // 生成后台任务
    for i in 0..3 {
        let mut sender = sender.clone();
        background
            .spawn(async move {
                sender.send(format!("bg-{}", i)).await.ok();
            })
            .detach();
    }

    drop(sender); // 关闭 sender 以表示没有更多消息
    scheduler.run();

    receiver.collect().await
}

#[test]
fn test_block() {
    let scheduler = Arc::new(TestScheduler::new(TestSchedulerConfig::default()));
    let (tx, rx) = oneshot::channel();

    // 生成后台任务以发送值
    let _ = scheduler
        .background()
        .spawn(async move {
            tx.send(42).unwrap();
        })
        .detach();

    // 阻塞接收值
    let result = scheduler.foreground().block_on(async { rx.await.unwrap() });
    assert_eq!(result, 42);
}

#[test]
#[should_panic(expected = "Parking forbidden. Pending traces:")]
fn test_parking_panics() {
    let config = TestSchedulerConfig {
        capture_pending_traces: true,
        ..Default::default()
    };
    let scheduler = Arc::new(TestScheduler::new(config));
    scheduler.foreground().block_on(async {
        let (_tx, rx) = oneshot::channel::<()>();
        rx.await.unwrap(); // This will never complete
    });
}

#[test]
fn test_block_with_parking() {
    let config = TestSchedulerConfig {
        allow_parking: true,
        ..Default::default()
    };
    let scheduler = Arc::new(TestScheduler::new(config));
    let (tx, rx) = oneshot::channel();

    // Spawn background task to send value
    let _ = scheduler
        .background()
        .spawn(async move {
            tx.send(42).unwrap();
        })
        .detach();

    // Block on receiving the value (will park if needed)
    let result = scheduler.foreground().block_on(async { rx.await.unwrap() });
    assert_eq!(result, 42);
}

#[test]
fn test_helper_methods() {
    // 测试 once 方法
    let result = TestScheduler::once(async |scheduler: Arc<TestScheduler>| {
        let background = scheduler.background();
        background.spawn(async { 42 }).await
    });
    assert_eq!(result, 42);

    // 测试 many 方法
    let results = TestScheduler::many(3, async |scheduler: Arc<TestScheduler>| {
        let background = scheduler.background();
        background.spawn(async { 10 }).await
    });
    assert_eq!(results, vec![10, 10, 10]);
}

#[test]
fn test_many_with_arbitrary_seed() {
    for seed in [0u64, 1, 5, 42] {
        let mut seeds_seen = Vec::new();
        let iterations = 3usize;

        for current_seed in seed..seed + iterations as u64 {
            let scheduler = Arc::new(TestScheduler::new(TestSchedulerConfig::with_seed(
                current_seed,
            )));
            let captured_seed = current_seed;
            scheduler
                .foreground()
                .block_on(async { seeds_seen.push(captured_seed) });
            scheduler.run();
        }

        assert_eq!(
            seeds_seen,
            (seed..seed + iterations as u64).collect::<Vec<_>>(),
            "Expected {iterations} iterations starting at seed {seed}"
        );
    }
}

#[test]
fn test_block_with_timeout() {
    // 测试用例：未来对象在超时内完成
    TestScheduler::once(async |scheduler| {
        let foreground = scheduler.foreground();
        let future = future::ready(42);
        let output = foreground.block_with_timeout(Duration::from_millis(100), future);
        assert_eq!(output.ok(), Some(42));
    });

    // 测试用例：未来对象超时
    TestScheduler::once(async |scheduler| {
        // 使超时行为确定性，强制超时 tick 预算恰好为 0。
        // 这防止 `block_with_timeout` 通过额外的调度器步进取得进展并
        /// 意外完成我们期望超时的工作。
        scheduler.set_timeout_ticks(0..=0);

        let foreground = scheduler.foreground();
        let future = future::pending::<()>();
        let output = foreground.block_with_timeout(Duration::from_millis(50), future);
        assert!(output.is_err(), "future should not have finished");
    });

    // 测试用例：未来对象通过计时器取得进展但仍超时
    let mut results = BTreeSet::new();
    TestScheduler::many(100, async |scheduler| {
        // 在此处保持现有的概率行为（不强制 0 tick），因为此子测试
        // 明确检查某些种子/超时可以完成而其他可以超时。
        let task = scheduler.background().spawn(async move {
            Yield { polls: 10 }.await;
            42
        });
        let output = scheduler
            .foreground()
            .block_with_timeout(Duration::from_millis(50), task);
        results.insert(output.ok());
    });
    assert_eq!(
        results.into_iter().collect::<Vec<_>>(),
        vec![None, Some(42)]
    );

    // 回归测试：
    // 超时的未来对象不得被取消。返回的未来对象仍应
    // 可轮询至完成。我们还要确保时间仅在我们
    // 显式推进时才推进（而非通过 yield）。
    TestScheduler::once(async |scheduler| {
        // 强制立即超时：超时 tick 预算为 0，因此我们不会在 `block_with_timeout` 内步进或
        // 推进计时器。
        scheduler.set_timeout_ticks(0..=0);

        let background = scheduler.background();

        // 此任务应仅在时间被显式推进后才完成。
        let task = background.spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler.timer(Duration::from_millis(100)).await;
                123
            }
        });

        // 这应在我们推进足够时间使计时器触发之前超时。
        let timed_out = scheduler
            .foreground()
            .block_with_timeout(Duration::from_millis(50), task);
        assert!(
            timed_out.is_err(),
            "expected timeout before advancing the clock enough for the timer"
        );

        // 现在显式推进时间并确保返回的未来对象可以完成。
        let mut task = timed_out.err().unwrap();
        scheduler.advance_clock(Duration::from_millis(100));
        scheduler.run();

        let output = scheduler.foreground().block_on(&mut task);
        assert_eq!(output, 123);
    });
}

// 调用 block 时，我们不应在具有相同会话 id 的前台生成的未来对象上取得进展。
#[test]
fn test_block_does_not_progress_same_session_foreground() {
    let mut task2_made_progress_once = false;
    TestScheduler::many(1000, async |scheduler| {
        let foreground1 = scheduler.foreground();
        let foreground2 = scheduler.foreground();

        let task1 = foreground1.spawn(async move {});
        let task2 = foreground2.spawn(async move {});

        foreground1.block_on(async {
            scheduler.yield_random().await;
            assert!(!task1.is_ready());
            task2_made_progress_once |= task2.is_ready();
        });

        task1.await;
        task2.await;
    });

    assert!(
        task2_made_progress_once,
        "Expected task from different foreground executor to make progress (at least once)"
    );
}

struct Yield {
    polls: usize,
}

impl Future for Yield {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls -= 1;
        if self.polls == 0 {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[test]
fn test_nondeterministic_wake_detection() {
    let config = TestSchedulerConfig {
        allow_parking: false,
        ..Default::default()
    };
    let scheduler = Arc::new(TestScheduler::new(config));

    // A future that captures its waker and sends it to an external thread
    struct SendWakerToThread {
        waker_tx: Option<std::sync::mpsc::Sender<Waker>>,
    }

    impl Future for SendWakerToThread {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if let Some(tx) = self.waker_tx.take() {
                tx.send(cx.waker().clone()).ok();
            }
            Poll::Ready(())
        }
    }

    let (waker_tx, waker_rx) = std::sync::mpsc::channel::<Waker>();

    // 通过运行发送它的未来对象获取唤醒器
    scheduler.foreground().block_on(SendWakerToThread {
        waker_tx: Some(waker_tx),
    });

    // 生成一个真实的 OS 线程，将在唤醒器上调用 wake()
    let handle = std::thread::spawn(move || {
        if let Ok(waker) = waker_rx.recv() {
            // 这应触发非确定性检测
            waker.wake();
        }
    });

    // Wait for the spawned thread to complete
    handle.join().ok();

    // The non-determinism error should be detected when end_test is called
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scheduler.end_test();
    }));
    assert!(result.is_err(), "Expected end_test to panic");
    let panic_payload = result.unwrap_err();
    let panic_message = panic_payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
        .unwrap_or("<unknown panic>");
    assert!(
        panic_message.contains("Your test is not deterministic"),
        "期望 panic 消息包含非确定性错误，得到：{}",
        panic_message
    );
}

#[test]
fn test_nondeterministic_wake_allowed_with_parking() {
    let config = TestSchedulerConfig {
        allow_parking: true,
        ..Default::default()
    };
    let scheduler = Arc::new(TestScheduler::new(config));

    // 捕获其唤醒器并将其发送到外部线程的未来对象
    struct WakeFromExternalThread {
        waker_sent: bool,
        waker_tx: Option<std::sync::mpsc::Sender<Waker>>,
    }

    impl Future for WakeFromExternalThread {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if !self.waker_sent {
                self.waker_sent = true;
                if let Some(tx) = self.waker_tx.take() {
                    tx.send(cx.waker().clone()).ok();
                }
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }
    }

    let (waker_tx, waker_rx) = std::sync::mpsc::channel::<Waker>();

    // 生成一个真实的 OS 线程，将在唤醒器上调用 wake()
    std::thread::spawn(move || {
        if let Ok(waker) = waker_rx.recv() {
            // 允许 park 时，这不应 panic
            waker.wake();
        }
    });

    // 这应在不 panic 的情况下完成
    scheduler.foreground().block_on(WakeFromExternalThread {
        waker_sent: false,
        waker_tx: Some(waker_tx),
    });
}

#[test]
fn test_nondeterministic_waker_drop_detection() {
    let config = TestSchedulerConfig {
        allow_parking: false,
        ..Default::default()
    };
    let scheduler = Arc::new(TestScheduler::new(config));

    // A future that captures its waker and sends it to an external thread
    struct SendWakerToThread {
        waker_tx: Option<std::sync::mpsc::Sender<Waker>>,
    }

    impl Future for SendWakerToThread {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if let Some(tx) = self.waker_tx.take() {
                tx.send(cx.waker().clone()).ok();
            }
            Poll::Ready(())
        }
    }

    let (waker_tx, waker_rx) = std::sync::mpsc::channel::<Waker>();

    // Get a waker by running a future that sends it
    scheduler.foreground().block_on(SendWakerToThread {
        waker_tx: Some(waker_tx),
    });

    // 生成一个真实的 OS 线程，将丢弃唤醒器而不调用 wake
    let handle = std::thread::spawn(move || {
        if let Ok(waker) = waker_rx.recv() {
            // 这应在丢弃时触发非确定性检测
            drop(waker);
        }
    });

    // Wait for the spawned thread to complete
    handle.join().ok();

    // The non-determinism error should be detected when end_test is called
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scheduler.end_test();
    }));
    assert!(result.is_err(), "Expected end_test to panic");
    let panic_payload = result.unwrap_err();
    let panic_message = panic_payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
        .unwrap_or("<unknown panic>");
    assert!(
        panic_message.contains("Your test is not deterministic"),
        "Expected panic message to contain non-determinism error, got: {}",
        panic_message
    );
}

#[test]
fn test_background_priority_scheduling() {
    use parking_lot::Mutex;

    // Run many iterations to get statistical significance
    let mut high_before_low_count = 0;
    let iterations = 100;

    for seed in 0..iterations {
        let config = TestSchedulerConfig::with_seed(seed);
        let scheduler = Arc::new(TestScheduler::new(config));
        let background = scheduler.background();

        let execution_order = Arc::new(Mutex::new(Vec::new()));

        // Spawn low priority tasks first
        for i in 0..3 {
            let order = execution_order.clone();
            background
                .spawn_with_priority(Priority::Low, async move {
                    order.lock().push(format!("low-{}", i));
                })
                .detach();
        }

        // Spawn high priority tasks second
        for i in 0..3 {
            let order = execution_order.clone();
            background
                .spawn_with_priority(Priority::High, async move {
                    order.lock().push(format!("high-{}", i));
                })
                .detach();
        }

        scheduler.run();

        // Count how many high priority tasks ran in the first half
        let order = execution_order.lock();
        let high_in_first_half = order
            .iter()
            .take(3)
            .filter(|s| s.starts_with("high"))
            .count();

        if high_in_first_half >= 2 {
            high_before_low_count += 1;
        }
    }

    // 高优先级任务应倾向于在低优先级任务之前运行
    // 权重为 60 vs 10 时，高优先级应在早期执行中占主导地位
    assert!(
        high_before_low_count > iterations / 2,
        "期望高优先级任务比低优先级任务更经常先运行。\
         在 {} 次迭代中得到 {}",
        high_before_low_count,
        iterations
    );
}
