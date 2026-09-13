//! keymap.json 文件加载（M7）。
//!
//! 格式（数组，每项一绑定，`serde_json_lenient` 解析，注释与尾逗号可写）：
//!
//! ```json
//! [
//!   { "key": "ctrl-s", "action": "workspace::Save", "context": "Editor" },
//!   { "key": "ctrl-k", "action": null }
//! ]
//! ```
//!
//! - `key`：按键串（`KeyBinding::load` 同款；多键序列空格分隔，如 `"space w w"`）。
//! - `action`：注册动作名（`App::build_action` 解析）；`null` = 禁用（`NoAction`，
//!   只认这一种写法）。
//! - `context`：可选，上下文谓词字符串，原样透传（不校验，写错匹配不上，文档注明）。
//!
//! 语义"改键"：返回的 `Vec` 即用户层全集，调用方 `cx.bind_keys` 一次装入；
//! 缺的走默认键。未知动作名直接报错（`ActionBuildError` 原样透传，不静默跳过）。
//! 注册表在 `App` 手里，故加载需 `&mut App`（设计草图无参版据此调整）。

use std::rc::Rc;

use serde::Deserialize;

use crate::{Action, App, DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, NoAction};

/// keymap.json 单条目。
#[derive(Debug, Deserialize)]
struct KeymapEntry {
    /// 按键串。
    key: String,
    /// 注册动作名；`null` 表禁用。
    action: Option<String>,
    /// 上下文谓词（可选）。
    #[serde(default)]
    context: Option<String>,
}

/// 解析 keymap.json（数组）为绑定列表。
pub fn load_keymap_json(cx: &mut App, json: &str) -> anyhow::Result<Vec<KeyBinding>> {
    let entries: Vec<KeymapEntry> = serde_json_lenient::from_str(json)?;
    entries.iter().map(|entry| load_entry(cx, entry)).collect()
}

/// 解析单条目（未知动作/非法按键/非法谓词一律报错）。
fn load_entry(cx: &mut App, entry: &KeymapEntry) -> anyhow::Result<KeyBinding> {
    let action: Box<dyn Action> = match &entry.action {
        None => Box::new(NoAction {}),
        Some(name) => cx.build_action(name, None)?,
    };
    let predicate: Option<Rc<KeyBindingContextPredicate>> = entry
        .context
        .as_deref()
        .map(KeyBindingContextPredicate::parse)
        .transpose()?
        .map(Rc::new);
    Ok(KeyBinding::load(
        &entry.key,
        action,
        predicate,
        false,
        None,
        &DummyKeyboardMapper,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::is_no_action;

    /// 正常加载：命名动作 + 上下文 + `null` 禁用。
    #[rgpui::test]
    fn loads_bindings_and_null_disables(cx: &mut crate::TestAppContext) {
        let json = r#"[
            // 注释与尾逗号可写（lenient 解析）。
            { "key": "ctrl-s", "action": "input::Undo", "context": "Input" },
            { "key": "ctrl-k", "action": null },
        ]"#;
        let bindings = cx.update(|cx| load_keymap_json(cx, json).expect("合法 keymap 必过"));
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].action().name(), "input::Undo");
        assert!(bindings[0].predicate().is_some());
        assert!(is_no_action(bindings[1].action()));
    }

    /// 未知动作名直接报错（不静默跳过）。
    #[rgpui::test]
    fn unknown_action_errors(cx: &mut crate::TestAppContext) {
        let json = r#"[{ "key": "ctrl-s", "action": "nope::Nope" }]"#;
        cx.update(|cx| {
            assert!(load_keymap_json(cx, json).is_err());
        });
    }

    /// 非法 JSON / 非法谓词报错；空数组得空列表。
    #[rgpui::test]
    fn malformed_inputs_error(cx: &mut crate::TestAppContext) {
        cx.update(|cx| {
            assert!(load_keymap_json(cx, "not json").is_err());
            assert!(load_keymap_json(cx, r#"{"key": "a"}"#).is_err());
            let bad_predicate =
                r#"[{ "key": "ctrl-s", "action": "input::Undo", "context": "&&&" }]"#;
            assert!(load_keymap_json(cx, bad_predicate).is_err());
            let empty = load_keymap_json(cx, "[]").expect("空数组合法");
            assert!(empty.is_empty());
        });
    }
}
