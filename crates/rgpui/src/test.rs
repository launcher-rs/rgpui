//! RGPUI 的测试支持。
//!
//! RGPUI 为测试提供一流支持，包括一个运行依赖上下文的测试的宏，
//! 以及 `ForegroundExecutor` 和 `BackgroundExecutor` 的测试实现，
//! 确保即使在任意并行性的情况下，您的测试也能确定性地运行。
//!
//! `rgpui::test` 宏的输出被其他 rust 测试运行器理解，因此您可以
//! 将其与 `cargo test` 或 `cargo-nextest` 或您选择的其他运行器一起使用。
//!
//! 为了使测试协作用户界面（如 Zed）成为可能，您可以根据需要请求
//! 多个不同的上下文。
//!
//! ## 示例
//!
//! ```
//! use rgpui;
//!
//! #[rgpui::test]
//! async fn test_example(cx: &TestAppContext) {
//!   assert!(true)
//! }
//!
//! #[rgpui::test]
//! async fn test_collaboration_example(cx_a: &TestAppContext, cx_b: &TestAppContext) {
//!   assert!(true)
//! }
//! ```
use crate::{Entity, Subscription, TestAppContext, TestDispatcher};
use futures::StreamExt as _;
use proptest::prelude::{Just, Strategy, any};
use std::{
    env,
    panic::{self, RefUnwindSafe, UnwindSafe},
    pin::Pin,
};

/// 注入到 `#[rgpui::property_test]` 测试中的策略，用于控制传递给
/// 调度器的种子。不进行收缩，因为所有调度器种子在复杂性上都是
/// 等效的。如果设置了 `$SEED`，则始终使用该值。
///
/// 注意：此函数不打算直接使用。而是公开的，以便可以从
/// `property_test` 宏中使用。
pub fn seed_strategy() -> impl Strategy<Value = u64> {
    match std::env::var("SEED") {
        Ok(val) => Just(val.parse().unwrap()).boxed(),
        Err(_) => any::<u64>().no_shrink().boxed(),
    }
}

/// 将固定的 RNG 种子应用于 proptest 配置，使用例生成
/// 是确定性的。如果设置了 `$SEED`，则使用它，否则默认为 `0`。
/// 这将 RGPUI `SEED` 环境变量桥接到 proptest 的 RNG 种子，以便
/// 单个变量控制调度器种子和用例生成。
///
/// 注意：此函数不打算直接使用。而是公开的，以便可以从
/// `property_test` 宏中使用。
pub fn apply_seed_to_proptest_config(
    mut config: proptest::test_runner::Config,
) -> proptest::test_runner::Config {
    let seed = env::var("SEED")
        .ok()
        .and_then(|val| val.parse::<u64>().ok())
        .unwrap_or(0);
    config.rng_seed = proptest::test_runner::RngSeed::Fixed(seed);
    config
}

/// 类似于 [`run_test`]，但只运行一次回调，允许
/// [`FnOnce`] 回调。这旨在与 `rgpui::property_test` 宏一起使用，
/// 通常不应直接使用。
///
/// 不支持 [`run_test`] 的许多功能，因为这些由 proptest 提供。
pub fn run_test_once<R>(
    seed: u64,
    test_fn: Box<dyn UnwindSafe + FnOnce(TestDispatcher) -> R>,
) -> R {
    let result = panic::catch_unwind(|| {
        let dispatcher = TestDispatcher::new(seed);
        let scheduler = dispatcher.scheduler().clone();
        let res = test_fn(dispatcher);
        scheduler.end_test();
        res
    });

    match result {
        Ok(r) => r,
        Err(e) => panic::resume_unwind(e),
    }
}

/// 使用配置的参数运行给定的测试函数。
/// 这旨在与 `rgpui::test` 宏一起使用，
/// 通常不应直接使用。
pub fn run_test(
    num_iterations: usize,
    explicit_seeds: &[u64],
    max_retries: usize,
    test_fn: &mut (dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
    on_fail_fn: Option<fn()>,
) {
    let (seeds, is_multiple_runs) = calculate_seeds(num_iterations as u64, explicit_seeds);

    for seed in seeds {
        let mut attempt = 0;
        loop {
            if is_multiple_runs {
                eprintln!("seed = {seed}");
            }
            let result = panic::catch_unwind(|| {
                let dispatcher = TestDispatcher::new(seed);
                let scheduler = dispatcher.scheduler().clone();
                test_fn(dispatcher, seed);
                scheduler.end_test();
            });

            match result {
                Ok(_) => break,
                Err(error) => {
                    if attempt < max_retries {
                        println!("attempt {} failed, retrying", attempt);
                        attempt += 1;
                        // The panic payload might itself trigger an unwind on drop:
                        // https://doc.rust-lang.org/std/panic/fn.catch_unwind.html#notes
                        std::mem::forget(error);
                    } else {
                        if is_multiple_runs {
                            eprintln!("failing seed: {seed}");
                            eprintln!(
                                "You can rerun from this seed by setting the environmental variable SEED to {seed}"
                            );
                        }
                        if let Some(on_fail_fn) = on_fail_fn {
                            on_fail_fn()
                        }
                        panic::resume_unwind(error);
                    }
                }
            }
        }
    }
}

fn calculate_seeds(
    iterations: u64,
    explicit_seeds: &[u64],
) -> (impl Iterator<Item = u64> + '_, bool) {
    let iterations = env::var("ITERATIONS")
        .ok()
        .map(|var| var.parse().expect("invalid ITERATIONS variable"))
        .unwrap_or(iterations);

    let env_num = env::var("SEED")
        .map(|seed| seed.parse().expect("invalid SEED variable as integer"))
        .ok();

    let empty_range = || 0..0;

    let iter = {
        let env_range = if let Some(env_num) = env_num {
            env_num..env_num + 1
        } else {
            empty_range()
        };

        // if `iterations` is 1 and !(`explicit_seeds` is non-empty || `SEED` is set), then add     the run `0`
        // if `iterations` is 1 and  (`explicit_seeds` is non-empty || `SEED` is set), then discard the run `0`
        // if `iterations` isn't 1 and `SEED` is set, do `SEED..SEED+iterations`
        // otherwise, do `0..iterations`
        let iterations_range = match (iterations, env_num) {
            (1, None) if explicit_seeds.is_empty() => 0..1,
            (1, None) | (1, Some(_)) => empty_range(),
            (iterations, Some(env)) => env..env + iterations,
            (iterations, None) => 0..iterations,
        };

        // if `SEED` is set, ignore `explicit_seeds`
        let explicit_seeds = if env_num.is_some() {
            &[]
        } else {
            explicit_seeds
        };

        env_range
            .chain(iterations_range)
            .chain(explicit_seeds.iter().copied())
    };
    let is_multiple_runs = iter.clone().nth(1).is_some();
    (iter, is_multiple_runs)
}

/// 一个将观察回调转换为流的测试结构体。
pub struct Observation<T> {
    rx: Pin<Box<async_channel::Receiver<T>>>,
    _subscription: Subscription,
}

impl<T: 'static> futures::Stream for Observation<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_next_unpin(cx)
    }
}

/// observe 返回给定 `Entity` 的变更事件流
pub fn observe<T: 'static>(entity: &Entity<T>, cx: &mut TestAppContext) -> Observation<()> {
    let (tx, rx) = async_channel::unbounded();
    let _subscription = cx.update(|cx| {
        cx.observe(entity, move |_, _| {
            let _ = rgpui::block_on(tx.send(()));
        })
    });
    let rx = Box::pin(rx);

    Observation { rx, _subscription }
}
