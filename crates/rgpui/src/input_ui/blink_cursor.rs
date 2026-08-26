use rgpui::{Context, Pixels, Task, px};
use std::time::Duration;

static INTERVAL: Duration = Duration::from_millis(500);
static PAUSE_DELAY: Duration = Duration::from_millis(300);

// 在 Windows、Linux 上使用整数宽度以避免光标模糊。
#[cfg(not(target_os = "macos"))]
pub(super) const CURSOR_WIDTH: Pixels = px(2.);
#[cfg(target_os = "macos")]
pub(super) const CURSOR_WIDTH: Pixels = px(1.5);

/// 管理输入光标的闪烁。
///
/// 以 500ms 的间隔开始闪烁。
/// 每次循环都会通知视图更新 `visible`，Input 会观察此更新以触发重绘。
///
/// 输入绘制器会检查此可见状态，然后绘制光标。
pub(crate) struct BlinkCursor {
    visible: bool,
    paused: bool,
    epoch: usize,

    _task: Task<()>,
}

impl BlinkCursor {
    pub fn new() -> Self {
        Self {
            visible: false,
            paused: false,
            epoch: 0,
            _task: Task::ready(()),
        }
    }

    /// 开始闪烁
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.blink(self.epoch, cx);
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.epoch = 0;
        cx.notify();
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch += 1;
        self.epoch
    }

    fn blink(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if self.paused || epoch != self.epoch {
            self.visible = true;
            return;
        }

        self.visible = !self.visible;
        cx.notify();

        // 安排下一次闪烁
        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(INTERVAL).await;
            if let Some(this) = this.upgrade() {
                cx.update_entity_reentrant(&this, move |this, cx| this.blink(epoch, cx));
            }
        });
    }

    pub fn visible(&self) -> bool {
        // 暂停时保持显示光标
        self.paused || self.visible
    }

    /// 暂停闪烁，并在 500ms 后恢复闪烁。
    pub fn pause(&mut self, cx: &mut Context<Self>) {
        self.paused = true;
        self.visible = true;
        cx.notify();

        // 延迟 500ms 开始闪烁
        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PAUSE_DELAY).await;
            if let Some(this) = this.upgrade() {
                cx.update_entity_reentrant(&this, move |this, cx| {
                    this.paused = false;
                    this.blink(epoch, cx);
                });
            }
        });
    }
}
