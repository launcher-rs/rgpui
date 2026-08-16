use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{AsyncApp, Bounds, Context, Pixels, Point, Task, WeakEntity, px};

/// 自动滚动 - 在拖拽选择期间管理基于定时器的自动滚动。
///
/// 增量约定：正数 = 向底部滚动，负数 = 向顶部滚动。
pub struct AutoScroll {
    /// 在主线程和后台任务之间共享。
    /// 写入 `None` 是停止信号；任务在下一个 tick 退出。
    shared: Arc<Mutex<Option<Pixels>>>,
    task: Option<Task<()>>,
    /// 最后的拖拽位置，用于在每个滚动步骤后重新扩展选择。
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
    /// 当前的滚动增量。正数 = 向底部。
    pub fn delta(&self) -> Option<Pixels> {
        *self.shared.lock().unwrap()
    }

    /// 计算鼠标 Y 位置在给定视口边界内的滚动增量。
    /// 接近底部边缘时返回正数，接近顶部边缘时为负数，死区内返回 `None`。
    pub fn compute_delta(y: Pixels, bounds: Bounds<Pixels>) -> Option<Pixels> {
        const MIN_SPEED: f32 = 12.0;
        const MAX_SPEED: f32 = 64.0;
        // 触发点从边界内部这么远的距离开始，
        // 这样即使在全屏模式下鼠标无法远离元素也能滚动。
        const INNER_ZONE: f32 = 16.0;
        // 从边界边缘到达最大速度的距离。
        // 总坡道 = INNER_ZONE + OUTER_RAMP，形成一条平滑曲线，
        // 没有平坦段或不连续点。
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

    /// 更新滚动增量并在需要时（重新）启动后台任务。
    ///
    /// `tick` 每帧（约 60 fps）以当前增量被调用一次。
    /// 它应执行此实体的实际滚动操作。
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

    /// 判断自动滚动是否处于活动状态。
    pub fn is_active(&self) -> bool {
        self.delta().is_some()
    }

    /// 停止自动滚动。
    pub fn stop(&mut self) {
        *self.shared.lock().unwrap() = None;
        self.task = None;
        self.last_drag_position = None;
    }
}