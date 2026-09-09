//! 编辑器扩展表 + 文本变更订阅表（O7，分拆 3.2 未落地件转正）。
//!
//! - 扩展表：全局名 → 工厂注册（高亮注册表 M6 同款），`EditorState::attach_extension`
//!   按名实例化挂载，文本变更时逐个调 `on_edit`；
//! - 订阅表：`EditorState::on_edit` 注册变更钩子（有 window，补 `subscribe`
//!   无 window 拿不到 provider 的缺口，见 M2 文档）。
//! 触发点统一在 `EditorState::new` 的 `subscribe_in` 回调（文本 `Change` 事件），
//! 与大纲刷新/自动补全同一管线。

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{LazyLock, RwLock};

use crate::{Context, SharedString, Window};

use super::state::EditorState;

/// 文本变更事件（订阅表/扩展回调参数）。
///
/// 粗粒度：新光标 + 全文长度；要读文本自取（回调内可读 `EditorState`）。
#[derive(Debug, Clone, Copy)]
pub struct EditEvent {
    /// 变更后主光标字节偏移。
    pub cursor: usize,
    /// 变更后全文 UTF-8 字节长度。
    pub text_len: usize,
}

/// 编辑器扩展（挂载后随文本变更收到 `on_edit`；装饰/行为扩展的统一入口）。
pub trait EditorExtension {
    /// 扩展名（注册/挂载/查询用）。
    fn name(&self) -> SharedString;
    /// 文本变更回调（默认空实现；只读事件，无编辑器访问权，不可重入）。
    fn on_edit(&mut self, _event: &EditEvent) {}
}

/// 扩展工厂（注册表用）。
pub type EditorExtensionFactory = Box<dyn Fn() -> Box<dyn EditorExtension> + Send + Sync>;

/// 变更钩子（订阅表用；有 window，可读 `EditorState`）。
///
/// 重入警告（E4 同款）：回调内禁止直接改文本（会重入触发管线），需延后
/// （`cx.spawn`/`defer`）或只读状态；扩展侧 `on_edit` 无此问题（无编辑器访问权）。
pub type EditHandler = Rc<dyn Fn(&EditEvent, &mut Window, &mut Context<EditorState>)>;

/// 全局扩展表（名 → 工厂；后注册覆盖先注册，高亮注册表同款语义）。
static EXTENSION_REGISTRY: LazyLock<RwLock<HashMap<&'static str, EditorExtensionFactory>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 注册编辑器扩展（`EditorState::attach_extension` 按名实例化）。
pub fn register_editor_extension(name: &'static str, factory: EditorExtensionFactory) {
    if let Ok(mut registry) = EXTENSION_REGISTRY.write() {
        registry.insert(name, factory);
    }
}

/// 按名构造扩展（未注册返回 `None`）。
pub fn editor_extension(name: &str) -> Option<Box<dyn EditorExtension>> {
    EXTENSION_REGISTRY
        .read()
        .ok()?
        .get(name)
        .map(|factory| factory())
}

/// 扩展与订阅运行时状态（`EditorState` 内嵌小字段，不另起 Entity，M 系惯例）。
pub(super) struct ExtensionsState {
    /// 已挂载扩展（按挂载顺序触发）。
    attached: Vec<Box<dyn EditorExtension>>,
    /// 直接订阅的变更钩子（id 递增）。
    handlers: Vec<(u64, EditHandler)>,
    /// 下一钩子 id。
    next_id: u64,
}

impl ExtensionsState {
    /// 空状态。
    pub(super) fn new() -> Self {
        Self {
            attached: Vec::new(),
            handlers: Vec::new(),
            next_id: 0,
        }
    }
}

impl EditorState {
    /// 按名挂载扩展（已挂载返回 `false`；未注册返回 `false`）。
    pub fn attach_extension(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        if self
            .extensions
            .attached
            .iter()
            .any(|ext| ext.name() == name)
        {
            return false;
        }
        let Some(extension) = editor_extension(name) else {
            return false;
        };
        self.extensions.attached.push(extension);
        cx.notify();
        true
    }

    /// 已挂载扩展名（检查/调试用）。
    pub fn attached_extensions(&self) -> Vec<SharedString> {
        self.extensions
            .attached
            .iter()
            .map(|ext| ext.name())
            .collect()
    }

    /// 订阅文本变更（返回钩子 id，`remove_on_edit` 可摘除）。
    ///
    /// 回调在每次文本 `Change` 后触发（大纲刷新/自动补全之后）；重入警告见 [`EditHandler`]。
    pub fn on_edit(
        &mut self,
        handler: impl Fn(&EditEvent, &mut Window, &mut Context<Self>) + 'static,
    ) -> u64 {
        let id = self.extensions.next_id;
        self.extensions.next_id = self.extensions.next_id.wrapping_add(1);
        self.extensions.handlers.push((id, Rc::new(handler)));
        id
    }

    /// 摘除变更钩子（存在返回 `true`）。
    pub fn remove_on_edit(&mut self, id: u64) -> bool {
        let before = self.extensions.handlers.len();
        self.extensions.handlers.retain(|(known, _)| *known != id);
        self.extensions.handlers.len() != before
    }

    /// 分发变更事件（`subscribe_in` 回调内调用；先扩展后直接钩子）。
    pub(super) fn fire_edit_event(
        &mut self,
        event: &EditEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for extension in &mut self.extensions.attached {
            extension.on_edit(event);
        }
        // 先克隆 Rc 再调：钩子内可摘除其他钩子（自身摘除不影响本轮）。
        let handlers: Vec<EditHandler> = self
            .extensions
            .handlers
            .iter()
            .map(|(_, hook)| hook.clone())
            .collect();
        for handler in handlers {
            handler(event, window, cx);
        }
    }

    /// 由内部输入状态组装变更事件（光标 + 全文长度）。
    pub(super) fn edit_event<C: crate::AppContext>(&self, cx: &C) -> EditEvent {
        self.input.read_with(cx, |state, _| EditEvent {
            cursor: state.cursor(),
            text_len: state.text().len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::Render;
    use std::sync::{Arc, Mutex};

    /// 持有编辑器状态的测试宿主视图。
    struct Probe {
        state: crate::Entity<EditorState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    /// 计数桩扩展（记录收到的事件；`Arc` 以满足工厂 `Send + Sync`）。
    struct CountingExtension {
        events: Arc<Mutex<Vec<EditEvent>>>,
    }

    impl EditorExtension for CountingExtension {
        fn name(&self) -> SharedString {
            "test-only-counting".into()
        }

        fn on_edit(&mut self, event: &EditEvent) {
            self.events.lock().unwrap().push(*event);
        }
    }

    /// 在内部输入上模拟一次键入（走真实 `Change` 事件）。
    fn type_text(
        editor: &crate::Entity<EditorState>,
        text: &str,
        cx: &mut crate::VisualTestContext,
    ) {
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                let input = state.input().clone();
                input.update(cx, |state, cx| {
                    crate::EntityInputHandler::replace_text_in_range(state, None, text, window, cx);
                });
            });
        });
    }

    /// 未注册名挂载失败；注册后挂载成功，重复挂载拒绝。
    #[rgpui::test]
    fn attach_extension_registry_flow(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "hi\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        // 未注册：失败。
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                assert!(!state.attach_extension("test-only-missing", cx));
            });
        });
        register_editor_extension(
            "test-only-counting",
            Box::new(|| {
                Box::new(CountingExtension {
                    events: Arc::new(Mutex::new(Vec::new())),
                }) as Box<dyn EditorExtension>
            }),
        );
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                assert!(state.attach_extension("test-only-counting", cx));
                // 重复挂载拒绝。
                assert!(!state.attach_extension("test-only-counting", cx));
            });
        });
        assert_eq!(
            editor.read_with(cx, |state, _| state.attached_extensions()),
            vec![SharedString::from("test-only-counting")]
        );
    }

    /// 键入后扩展与直接钩子都收到事件；摘除后钩子不再触发。
    #[rgpui::test]
    fn edit_event_fires_to_extension_and_handler(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "hi\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let seen: Arc<Mutex<Vec<EditEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_for_hook = seen.clone();
        cx.update(|_, cx| {
            editor.update(cx, |state, _cx| {
                // 不存在的 id 摘除失败。
                let id = state.on_edit(move |event, _, _| {
                    seen_for_hook.lock().unwrap().push(*event);
                });
                assert!(!state.remove_on_edit(id.wrapping_add(1)));
                assert!(state.remove_on_edit(id));
                // 重订一个常驻钩子，后续断言用它。
                let seen = seen.clone();
                let _ = state.on_edit(move |event, _, _| {
                    seen.lock().unwrap().push(*event);
                });
            });
        });
        // 注意：上面闭包里 `seen.clone()` 借的是外部 Arc（Fn 捕获），常驻钩子写入同一处。
        type_text(&editor, "!", cx);
        cx.run_until_parked();
        // 直接钩子收到一次（`hi\n` 末尾键入 `!`：光标 4，全文 4 字节）。
        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].cursor, 4);
        assert_eq!(events[0].text_len, 4);
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "hi\n!");
    }

    /// 挂载的扩展收到变更事件（经注册表实例化）。
    #[rgpui::test]
    fn attached_extension_receives_edit(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "hi\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let events: Arc<Mutex<Vec<EditEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_for_factory = events.clone();
        register_editor_extension(
            "test-only-wired",
            Box::new(move || {
                Box::new(CountingExtension {
                    events: events_for_factory.clone(),
                }) as Box<dyn EditorExtension>
            }),
        );
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                assert!(state.attach_extension("test-only-wired", cx));
            });
        });
        type_text(&editor, "?", cx);
        cx.run_until_parked();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].cursor, 4);
    }
}
