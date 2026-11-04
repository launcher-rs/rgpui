//! 内置 Toast 通知系统
//!
//! 提供可堆叠、自动消失的 Toast 通知组件

use crate::{
    AnyElement, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement, Pixels,
    Render, Role, SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use std::time::Duration;

/// Toast 位置枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastPosition {
    /// 右上角
    #[default]
    TopRight,
    /// 右下角
    BottomRight,
    /// 顶部居中
    TopCenter,
}

/// Toast 通知结构体
#[derive(Clone)]
pub struct Toast {
    /// Toast 标题
    pub title: SharedString,
    /// Toast 内容（可选）
    pub body: Option<SharedString>,
    /// 自动消失时间（默认 3 秒）
    pub duration: Duration,
    /// Toast 位置
    pub position: ToastPosition,
}

impl Toast {
    /// 创建新的 Toast，仅包含标题
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: None,
            duration: Duration::from_secs(3),
            position: ToastPosition::default(),
        }
    }

    /// 设置 Toast 内容
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// 设置自动消失时间
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置 Toast 位置
    pub fn position(mut self, position: ToastPosition) -> Self {
        self.position = position;
        self
    }
}

/// Toast 堆栈中的单个条目
struct ToastEntry {
    toast: Toast,
}

/// Toast 通知堆栈实体
///
/// 用于管理和显示多个 Toast 通知
pub struct ToastStack {
    toasts: Vec<ToastEntry>,
    position: ToastPosition,
}

impl ToastStack {
    /// 创建新的 Toast 堆栈
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            position: ToastPosition::default(),
        }
    }

    /// 设置 Toast 堆栈的位置
    pub fn with_position(mut self, position: ToastPosition) -> Self {
        self.position = position;
        self
    }

    /// 添加一个新的 Toast 通知
    ///
    /// # 参数
    /// * `toast` - Toast 通知
    /// * `window` - 窗口引用
    /// * `cx` - 上下文
    pub fn push(&mut self, toast: Toast, window: &mut Window, cx: &mut Context<Self>) {
        let duration = toast.duration;
        let index = self.toasts.len();

        // 创建自动消失任务
        let weak_self = cx.entity().downgrade();
        let _ = cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(duration).await;
            if let Some(stack) = weak_self.upgrade() {
                let _ = stack.update(cx, |stack, cx| {
                    if index < stack.toasts.len() {
                        stack.toasts.remove(index);
                        cx.notify();
                    }
                });
            }
        });

        self.toasts.push(ToastEntry { toast });
        cx.notify();
    }

    /// 清除所有 Toast 通知
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.toasts.clear();
        cx.notify();
    }

    /// 获取 Toast 数量
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// 计算 Toast 堆栈的偏移位置
    fn offset_for_index(&self, index: usize) -> Pixels {
        // 每个 Toast 高度约 80px，间距 8px
        px(88.0 * index as f32)
    }
}

impl Render for ToastStack {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let position = self.position;

        // 创建容器 div
        let mut container = div().absolute().flex().flex_col().gap_2().p_4();

        // 根据位置设置容器定位
        container = match position {
            ToastPosition::TopRight => container.top_0().right_0(),
            ToastPosition::BottomRight => container.bottom_0().right_0(),
            ToastPosition::TopCenter => container.top_0().left(px(50.0)).ml(-px(100.0)),
        };

        // 添加所有 Toast
        let toasts: Vec<AnyElement> = self
            .toasts
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                render_toast(&entry.toast, index, position, self.offset_for_index(index))
            })
            .collect();

        container.children(toasts)
    }
}

/// 渲染单个 Toast 通知
fn render_toast(
    toast: &Toast,
    index: usize,
    position: ToastPosition,
    offset: Pixels,
) -> AnyElement {
    let mut toast_div = div()
        // 有 ID 才会进无障碍树；同一条位置复用 ID，内容变化即成为一次实时播报。
        .id(ElementId::Name(format!("toast-{index}").into()))
        // Toast 是自动出现又消失的消息：按 `status` 语义 + 礼貌播报，
        // 不打断辅助技术正在朗读的内容。
        .role(Role::Status)
        .aria_live(accesskit::Live::Polite)
        .aria_label(toast.title.clone())
        .absolute()
        .w(px(320.0))
        .rounded_lg()
        .shadow_md()
        .p_4()
        .bg(crate::rgb(0x1e1e1e))
        .text_color(crate::white())
        .child(div().text_sm().child(toast.title.clone()));

    // 如果有内容，添加内容文本
    if let Some(ref body) = toast.body {
        toast_div = toast_div.child(
            div()
                .mt_1()
                .text_xs()
                .text_color(crate::hsla(0.0, 0.0, 1.0, 0.7))
                .child(body.clone()),
        );
    }

    // 根据位置设置偏移
    toast_div = match position {
        ToastPosition::TopRight | ToastPosition::TopCenter => toast_div.top(offset),
        ToastPosition::BottomRight => toast_div.bottom(offset),
    };

    toast_div.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppContext as _, Entity, Render, TestAppContext};

    /// 承载 Toast 堆栈的窗口视图。
    struct StackProbe {
        stack: Entity<ToastStack>,
    }

    impl Render for StackProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.stack.clone())
        }
    }

    /// Toast 必须是礼貌实时区域：它自己出现自己消失，用户无法主动去「按」它。
    #[rgpui::test]
    fn toast_is_a_polite_live_region(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let stack = cx.update(|cx| cx.new(|_| ToastStack::new()));
        let (_handle, mut cx) = cx.add_window_view(move |window, cx| {
            stack.update(cx, |stack, cx| {
                stack.push(Toast::new("已保存").body("3 个文件"), window, cx);
            });
            StackProbe { stack }
        });
        cx.update(|window, cx| {
            window.force_a11y_active_for_test();
            let _ = window.draw(cx);
        });
        let json = cx
            .update(|window, _| window.debug_a11y_tree_json())
            .expect("无障碍树未捕获");
        let tree: serde_json::Value = serde_json::from_str(&json).expect("树 JSON 必可解析");
        let nodes = tree["nodes"].as_object().expect("nodes 必为对象");

        let node = nodes
            .values()
            .find(|node| node["aria"]["role"] == "Status")
            .expect("Toast 必须以 Status 角色出现在无障碍树里");
        assert_eq!(
            node["aria"]["live"], "Polite",
            "Toast 要声明礼貌实时区域，通知到达时才会被读出"
        );
        assert_eq!(node["aria"]["label"], "已保存");
    }
}
