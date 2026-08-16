//! 拖拽选择时的定时自动滚动。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{AsyncApp, Bounds, Context, Pixels, Point, Task, WeakEntity, px};

/// 管理拖拽选择期间基于定时器的自动滚动。
///
/// 增量约定：正值 = 向底部，负值 = 向顶部。
pub struct AutoScroll {
    /// 主线程与后台任务共享。写入 `None` 即停止信号；任务在下一个 tick 退出。
    shared: Arc<Mutex<Option<Pixels>>>,
    task: Option<Task<()>>,
    /// 上次拖拽位置，用于每次滚动后重新扩展选区。
    pub last_drag_position: Option<Point<Pixels>>,
}

impl Default for AutoScroll {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(None)),
            task: None,
            last_drag_position: None,
        }
    }
}

impl AutoScroll {
    /// 当前滚动增量。正值 = 向底部。
    pub fn delta(&self) -> Option<Pixels> {
        *self.shared.lock().unwrap()
    }

    /// 计算给定视口边界内鼠标 Y 位置的滚动增量。
    /// 靠近底部边缘返回正值，靠近顶部返回负值，死区内返回 `None`。
    pub fn compute_delta(y: Pixels, bounds: Bounds<Pixels>) -> Option<Pixels> {
        const MIN_SPEED: f32 = 12.0;
        const MAX_SPEED: f32 = 64.0;
        // 触发区从边界内这么远开始，使全屏下鼠标无法远离元素也能滚动。
        const INNER_ZONE: f32 = 16.0;
        // 从边界到达到 MAX_SPEED 的距离。
        // 总斜坡 = INNER_ZONE + OUTER_RAMP，形成单条平滑曲线，
        // 没有平台段或不连续。
        const OUTER_RAMP: f32 = 80.0;

        let bottom_trigger = bounds.bottom() - px(INNER_ZONE);
        let top_trigger = bounds.top() + px(INNER_ZONE);

        if y > bottom_trigger {
            let t = ((y - bottom_trigger) / px(INNER_ZONE + OUTER_RAMP)).min(1.0);
            Some(px(MIN_SPEED + t * (MAX_SPEED - MIN_SPEED)))
        } else if y < top_trigger {
            let t = ((top_trigger - y) / px(INNER_ZONE + OUTER_RAMP)).min(1.0);
            Some(px(-(MIN_SPEED + t * (MAX_SPEED - MIN_SPEED))))
        } else {
            None
        }
    }

    /// 更新滚动增量并按需（重新）启动后台任务。
    ///
    /// `tick` 每帧（约 60fps）以当前增量调用。
    /// 它应对该实体执行实际的滚动动作。
    pub fn set<T, F>(&mut self, delta: Option<Pixels>, cx: &mut Context<T>, tick: F)
    where
        T: 'static,
        F: Fn(Pixels, &mut T, &mut Context<T>) + Send + 'static,
    {
        let was_idle = self.task.is_none();
        *self.shared.lock().unwrap() = delta;

        if delta.is_none() {
            self.task = None;
            return;
        }

        if was_idle {
            let shared = Arc::clone(&self.shared);
            self.task = Some(cx.spawn(Self::task_loop(shared, tick)));
        }
    }

    fn task_loop<T, F>(
        shared: Arc<Mutex<Option<Pixels>>>,
        tick: F,
    ) -> impl AsyncFnOnce(WeakEntity<T>, &mut AsyncApp) + 'static
    where
        T: 'static,
        F: Fn(Pixels, &mut T, &mut Context<T>) + Send + 'static,
    {
        async move |this: WeakEntity<T>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let Some(d) = *shared.lock().unwrap() else {
                    break;
                };
                let alive = this
                    .update(cx, |state, cx| {
                        tick(d, state, cx);
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        }
    }

    /// 当前是否处于自动滚动状态。
    pub fn is_active(&self) -> bool {
        self.delta().is_some()
    }

    /// 停止自动滚动并清除状态。
    pub fn stop(&mut self) {
        *self.shared.lock().unwrap() = None;
        self.task = None;
        self.last_drag_position = None;
    }
}
