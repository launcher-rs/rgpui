use std::rc::Rc;

use crate::{
    Action, AsKeystroke, DummyKeyboardMapper, InvalidKeystrokeError, KeyBindingContextPredicate,
    KeybindingKeystroke, Keystroke, PlatformKeyboardMapper, SharedString,
};
use smallvec::SmallVec;

/// 键绑定及其关联的元数据，来自键映射
pub struct KeyBinding {
    pub(crate) action: Box<dyn Action>,
    pub(crate) keystrokes: SmallVec<[KeybindingKeystroke; 2]>,
    pub(crate) context_predicate: Option<Rc<KeyBindingContextPredicate>>,
    pub(crate) meta: Option<KeyBindingMetaIndex>,
    /// The json input string used when building the keybinding, if any
    pub(crate) action_input: Option<SharedString>,
}

impl Clone for KeyBinding {
    fn clone(&self) -> Self {
        KeyBinding {
            action: self.action.boxed_clone(),
            keystrokes: self.keystrokes.clone(),
            context_predicate: self.context_predicate.clone(),
            meta: self.meta,
            action_input: self.action_input.clone(),
        }
    }
}

impl KeyBinding {
    /// 从给定数据构建新键绑定。解析错误时触发 panic
    pub fn new<A: Action>(keystrokes: &str, action: A, context: Option<&str>) -> Self {
        let context_predicate =
            context.map(|context| KeyBindingContextPredicate::parse(context).unwrap().into());
        Self::load(
            keystrokes,
            Box::new(action),
            context_predicate,
            false,
            None,
            &DummyKeyboardMapper,
        )
        .unwrap()
    }

    /// 从给定原始数据加载键绑定
    pub fn load(
        keystrokes: &str,
        action: Box<dyn Action>,
        context_predicate: Option<Rc<KeyBindingContextPredicate>>,
        use_key_equivalents: bool,
        action_input: Option<SharedString>,
        keyboard_mapper: &dyn PlatformKeyboardMapper,
    ) -> std::result::Result<Self, InvalidKeystrokeError> {
        let keystrokes: SmallVec<[KeybindingKeystroke; 2]> = keystrokes
            .split_whitespace()
            .map(|source| {
                let keystroke = Keystroke::parse(source)?;
                Ok(KeybindingKeystroke::new_with_mapper(
                    keystroke,
                    use_key_equivalents,
                    keyboard_mapper,
                ))
            })
            .collect::<std::result::Result<_, _>>()?;

        Ok(Self {
            keystrokes,
            action,
            context_predicate,
            meta: None,
            action_input,
        })
    }

    /// 设置此绑定的元数据
    pub fn with_meta(mut self, meta: KeyBindingMetaIndex) -> Self {
        self.meta = Some(meta);
        self
    }

    /// 设置此绑定的元数据
    pub fn set_meta(&mut self, meta: KeyBindingMetaIndex) {
        self.meta = Some(meta);
    }

    /// 检查给定的击键是否匹配此绑定
    pub fn match_keystrokes(&self, typed: &[impl AsKeystroke]) -> Option<bool> {
        if self.keystrokes.len() < typed.len() {
            return None;
        }

        for (target, typed) in self.keystrokes.iter().zip(typed.iter()) {
            if !typed.as_keystroke().should_match(target) {
                return None;
            }
        }

        Some(self.keystrokes.len() > typed.len())
    }

    /// 获取与此绑定关联的击键
    pub fn keystrokes(&self) -> &[KeybindingKeystroke] {
        self.keystrokes.as_slice()
    }

    /// 获取与此绑定关联的动作
    pub fn action(&self) -> &dyn Action {
        self.action.as_ref()
    }

    /// 获取用于匹配此绑定的谓词
    pub fn predicate(&self) -> Option<Rc<KeyBindingContextPredicate>> {
        self.context_predicate.as_ref().map(|rc| rc.clone())
    }

    /// 获取此绑定的元数据
    pub fn meta(&self) -> Option<KeyBindingMetaIndex> {
        self.meta
    }

    /// 获取此绑定动作的输入
    pub fn action_input(&self) -> Option<SharedString> {
        self.action_input.clone()
    }
}

impl std::fmt::Debug for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyBinding")
            .field("keystrokes", &self.keystrokes)
            .field("context_predicate", &self.context_predicate)
            .field("action", &self.action.name())
            .finish()
    }
}

/// 用于检索与键绑定关联元数据的唯一标识符。
/// 用作用户定义的元数据存储的索引或键，
/// 例如绑定的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBindingMetaIndex(pub u32);
