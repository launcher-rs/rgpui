mod binding;
mod context;

pub use binding::*;
pub use context::*;

use crate::{Action, AsKeystroke, Keystroke, Unbind, is_no_action, is_unbind};
use crate::collections::{HashMap, HashSet};
use smallvec::SmallVec;
use std::any::TypeId;

/// 当前活动的键映射版本的不透明标识符。
/// 每当添加或删除绑定时，键映射的版本都会更改。
#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub struct KeymapVersion(usize);

/// 用户应用程序的键绑定集合。
#[derive(Default)]
pub struct Keymap {
    bindings: Vec<KeyBinding>,
    binding_indices_by_action_id: HashMap<TypeId, SmallVec<[usize; 3]>>,
    disabled_binding_indices: Vec<usize>,
    version: KeymapVersion,
}

/// 键映射内绑定的索引。
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct BindingIndex(usize);

fn disabled_binding_matches_context(disabled_binding: &KeyBinding, binding: &KeyBinding) -> bool {
    match (
        &disabled_binding.context_predicate,
        &binding.context_predicate,
    ) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(disabled_predicate), Some(predicate)) => disabled_predicate.is_superset(predicate),
    }
}

fn binding_is_unbound(disabled_binding: &KeyBinding, binding: &KeyBinding) -> bool {
    disabled_binding.keystrokes == binding.keystrokes
        && disabled_binding
            .action()
            .as_any()
            .downcast_ref::<Unbind>()
            .is_some_and(|unbind| unbind.0.as_ref() == binding.action.name())
}

impl Keymap {
    /// 创建具有给定绑定的新键映射。
    pub fn new(bindings: Vec<KeyBinding>) -> Self {
        let mut this = Self::default();
        this.add_bindings(bindings);
        this
    }

    /// 获取键映射的当前版本。
    pub fn version(&self) -> KeymapVersion {
        self.version
    }

    /// 向键映射添加更多绑定。
    pub fn add_bindings<T: IntoIterator<Item = KeyBinding>>(&mut self, bindings: T) {
        for binding in bindings {
            let action_id = binding.action().as_any().type_id();
            if is_no_action(&*binding.action) || is_unbind(&*binding.action) {
                self.disabled_binding_indices.push(self.bindings.len());
            } else {
                self.binding_indices_by_action_id
                    .entry(action_id)
                    .or_default()
                    .push(self.bindings.len());
            }
            self.bindings.push(binding);
        }

        self.version.0 += 1;
    }

    /// 将此键映射重置为初始状态。
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.binding_indices_by_action_id.clear();
        self.disabled_binding_indices.clear();
        self.version.0 += 1;
    }

    /// 遍历所有绑定，按添加顺序。
    pub fn bindings(&self) -> impl DoubleEndedIterator<Item = &KeyBinding> + ExactSizeIterator {
        self.bindings.iter()
    }

    /// 遍历给定操作的所有绑定，按添加顺序。对于显示，
    /// 最后的绑定应优先。
    pub fn bindings_for_action<'a>(
        &'a self,
        action: &'a dyn Action,
    ) -> impl 'a + DoubleEndedIterator<Item = &'a KeyBinding> {
        let action_id = action.type_id();
        let binding_indices = self
            .binding_indices_by_action_id
            .get(&action_id)
            .map_or(&[] as _, SmallVec::as_slice)
            .iter();

        binding_indices.filter_map(|ix| {
            let binding = &self.bindings[*ix];
            if !binding.action().partial_eq(action) {
                return None;
            }

            for disabled_ix in &self.disabled_binding_indices {
                if disabled_ix > ix {
                    let disabled_binding = &self.bindings[*disabled_ix];
                    if disabled_binding.keystrokes != binding.keystrokes {
                        continue;
                    }

                    if is_no_action(&*disabled_binding.action) {
                        if disabled_binding_matches_context(disabled_binding, binding) {
                            return None;
                        }
                    } else if is_unbind(&*disabled_binding.action)
                        && disabled_binding_matches_context(disabled_binding, binding)
                        && binding_is_unbound(disabled_binding, binding)
                    {
                        return None;
                    }
                }
            }

            Some(binding)
        })
    }

    /// 返回可能与输入匹配的所有绑定，不检查上下文。返回的绑定
    /// 按优先级顺序排列（与添加到键映射的顺序相反）。
    pub fn all_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        self.bindings()
            .rev()
            .filter(|binding| {
                binding
                    .match_keystrokes(input)
                    .is_some_and(|pending| !pending)
            })
            .cloned()
            .collect()
    }

    /// 返回与给定输入匹配的绑定列表，以及一个布尔值，指示如果输入更长是否可能有更多匹配。绑定按优先级
    /// 顺序返回（较高优先级在前，与添加到键映射的顺序相反）。
    ///
    /// 优先级由树中的深度定义（Editor 上的匹配优先于
    /// Pane 上的匹配，然后是 Workspace 等）。没有上下文的绑定被视为与
    /// 最深上下文相同。
    ///
    /// 如果在相同深度有多个绑定，则稍后添加到键映射的绑定
    /// 优先。用户绑定在内置绑定之后添加，以便它们优先。
    ///
    /// 如果用户使用 `"x": null` 禁用了绑定，它将不会返回。禁用的绑定
    /// 使用相同的优先级规则进行评估，因此你可以在给定上下文中
    /// 仅禁用规则。
    pub fn bindings_for_input(
        &self,
        input: &[impl AsKeystroke],
        context_stack: &[KeyContext],
    ) -> (SmallVec<[KeyBinding; 1]>, bool) {
        let mut matched_bindings = SmallVec::<[(usize, BindingIndex, &KeyBinding); 1]>::new();
        let mut pending_bindings = SmallVec::<[(BindingIndex, &KeyBinding); 1]>::new();

        for (ix, binding) in self.bindings().enumerate().rev() {
            let Some(depth) = self.binding_enabled(binding, context_stack) else {
                continue;
            };
            let Some(pending) = binding.match_keystrokes(input) else {
                continue;
            };

            if !pending {
                matched_bindings.push((depth, BindingIndex(ix), binding));
            } else {
                pending_bindings.push((BindingIndex(ix), binding));
            }
        }

        matched_bindings.sort_by(|(depth_a, ix_a, _), (depth_b, ix_b, _)| {
            depth_b.cmp(depth_a).then(ix_b.cmp(ix_a))
        });

        let mut bindings: SmallVec<[_; 1]> = SmallVec::new();
        let mut first_binding_index = None;
        let mut unbound_bindings: Vec<&KeyBinding> = Vec::new();

        for (_, ix, binding) in matched_bindings {
            if is_no_action(&*binding.action) {
                // Only break if this is a user-defined NoAction binding
                // This allows user keymaps to override base keymap NoAction bindings
                if let Some(meta) = binding.meta {
                    if meta.0 == 0 {
                        break;
                    }
                } else {
                    // If no meta is set, assume it's a user binding for safety
                    break;
                }
                // For non-user NoAction bindings, continue searching for user overrides
                continue;
            }

            if is_unbind(&*binding.action) {
                unbound_bindings.push(binding);
                continue;
            }

            if unbound_bindings
                .iter()
                .any(|disabled_binding| binding_is_unbound(disabled_binding, binding))
            {
                continue;
            }

            bindings.push(binding.clone());
            first_binding_index.get_or_insert(ix);
        }

        let mut pending = HashSet::default();
        for (ix, binding) in pending_bindings.into_iter().rev() {
            if let Some(binding_ix) = first_binding_index
                && binding_ix > ix
            {
                continue;
            }
            if is_no_action(&*binding.action) || is_unbind(&*binding.action) {
                pending.remove(&&binding.keystrokes);
                continue;
            }
            pending.insert(&binding.keystrokes);
        }

        (bindings, !pending.is_empty())
    }
    /// 检查给定绑定在给定键上下文下是否启用。
    /// 返回绑定匹配的最深深度，如果不匹配则返回 None。
    fn binding_enabled(&self, binding: &KeyBinding, contexts: &[KeyContext]) -> Option<usize> {
        if let Some(predicate) = &binding.context_predicate {
            predicate.depth_of(contexts)
        } else {
            Some(contexts.len())
        }
    }

    /// 查找可以跟随当前输入序列的绑定。
    pub fn possible_next_bindings_for_input(
        &self,
        input: &[Keystroke],
        context_stack: &[KeyContext],
    ) -> Vec<KeyBinding> {
        let mut bindings = self
            .bindings()
            .enumerate()
            .rev()
            .filter_map(|(ix, binding)| {
                let depth = self.binding_enabled(binding, context_stack)?;
                let pending = binding.match_keystrokes(input);
                match pending {
                    None => None,
                    Some(is_pending) => {
                        if !is_pending
                            || is_no_action(&*binding.action)
                            || is_unbind(&*binding.action)
                        {
                            return None;
                        }
                        Some((depth, BindingIndex(ix), binding))
                    }
                }
            })
            .collect::<Vec<_>>();

        bindings.sort_by(|(depth_a, ix_a, _), (depth_b, ix_b, _)| {
            depth_b.cmp(depth_a).then(ix_b.cmp(ix_a))
        });

        bindings
            .into_iter()
            .map(|(_, _, binding)| binding.clone())
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as rgpui;
    use rgpui::{NoAction, Unbind};

    actions!(
        test_only,
        [ActionAlpha, ActionBeta, ActionGamma, ActionDelta,]
    );

    #[test]
    fn test_keymap() {
        let bindings = [
            KeyBinding::new("ctrl-a", ActionAlpha {}, None),
            KeyBinding::new("ctrl-a", ActionBeta {}, Some("pane")),
            KeyBinding::new("ctrl-a", ActionGamma {}, Some("editor && mode==full")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings.clone());

        // global bindings are enabled in all contexts
        assert_eq!(keymap.binding_enabled(&bindings[0], &[]), Some(0));
        assert_eq!(
            keymap.binding_enabled(&bindings[0], &[KeyContext::parse("terminal").unwrap()]),
            Some(1)
        );

        // contextual bindings are enabled in contexts that match their predicate
        assert_eq!(
            keymap.binding_enabled(&bindings[1], &[KeyContext::parse("barf x=y").unwrap()]),
            None
        );
        assert_eq!(
            keymap.binding_enabled(&bindings[1], &[KeyContext::parse("pane x=y").unwrap()]),
            Some(1)
        );

        assert_eq!(
            keymap.binding_enabled(&bindings[2], &[KeyContext::parse("editor").unwrap()]),
            None
        );
        assert_eq!(
            keymap.binding_enabled(
                &bindings[2],
                &[KeyContext::parse("editor mode=full").unwrap()]
            ),
            Some(1)
        );
    }

    #[test]
    fn test_depth_precedence() {
        let bindings = [
            KeyBinding::new("ctrl-a", ActionBeta {}, Some("pane")),
            KeyBinding::new("ctrl-a", ActionGamma {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-a").unwrap()],
            &[
                KeyContext::parse("pane").unwrap(),
                KeyContext::parse("editor").unwrap(),
            ],
        );

        assert!(!pending);
        assert_eq!(result.len(), 2);
        assert!(result[0].action.partial_eq(&ActionGamma {}));
        assert!(result[1].action.partial_eq(&ActionBeta {}));
    }

    #[test]
    fn test_keymap_disabled() {
        let bindings = [
            KeyBinding::new("ctrl-a", ActionAlpha {}, Some("editor")),
            KeyBinding::new("ctrl-b", ActionAlpha {}, Some("editor")),
            KeyBinding::new("ctrl-a", NoAction {}, Some("editor && mode==full")),
            KeyBinding::new("ctrl-b", NoAction {}, None),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // binding is only enabled in a specific context
        assert!(
            keymap
                .bindings_for_input(
                    &[Keystroke::parse("ctrl-a").unwrap()],
                    &[KeyContext::parse("barf").unwrap()],
                )
                .0
                .is_empty()
        );
        assert!(
            !keymap
                .bindings_for_input(
                    &[Keystroke::parse("ctrl-a").unwrap()],
                    &[KeyContext::parse("editor").unwrap()],
                )
                .0
                .is_empty()
        );

        // binding is disabled in a more specific context
        assert!(
            keymap
                .bindings_for_input(
                    &[Keystroke::parse("ctrl-a").unwrap()],
                    &[KeyContext::parse("editor mode=full").unwrap()],
                )
                .0
                .is_empty()
        );

        // binding is globally disabled
        assert!(
            keymap
                .bindings_for_input(
                    &[Keystroke::parse("ctrl-b").unwrap()],
                    &[KeyContext::parse("barf").unwrap()],
                )
                .0
                .is_empty()
        );
    }

    #[test]
    /// Tests for https://github.com/zed-industries/zed/issues/30259
    fn test_multiple_keystroke_binding_disabled() {
        let bindings = [
            KeyBinding::new("space w w", ActionAlpha {}, Some("workspace")),
            KeyBinding::new("space w w", NoAction {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let space = || Keystroke::parse("space").unwrap();
        let w = || Keystroke::parse("w").unwrap();

        let space_w = [space(), w()];
        let space_w_w = [space(), w(), w()];

        let workspace_context = || [KeyContext::parse("workspace").unwrap()];

        let editor_workspace_context = || {
            [
                KeyContext::parse("workspace").unwrap(),
                KeyContext::parse("editor").unwrap(),
            ]
        };

        // Ensure `space` results in pending input on the workspace, but not editor
        let space_workspace = keymap.bindings_for_input(&[space()], &workspace_context());
        assert!(space_workspace.0.is_empty());
        assert!(space_workspace.1);

        let space_editor = keymap.bindings_for_input(&[space()], &editor_workspace_context());
        assert!(space_editor.0.is_empty());
        assert!(!space_editor.1);

        // Ensure `space w` results in pending input on the workspace, but not editor
        let space_w_workspace = keymap.bindings_for_input(&space_w, &workspace_context());
        assert!(space_w_workspace.0.is_empty());
        assert!(space_w_workspace.1);

        let space_w_editor = keymap.bindings_for_input(&space_w, &editor_workspace_context());
        assert!(space_w_editor.0.is_empty());
        assert!(!space_w_editor.1);

        // Ensure `space w w` results in the binding in the workspace, but not in the editor
        let space_w_w_workspace = keymap.bindings_for_input(&space_w_w, &workspace_context());
        assert!(!space_w_w_workspace.0.is_empty());
        assert!(!space_w_w_workspace.1);

        let space_w_w_editor = keymap.bindings_for_input(&space_w_w, &editor_workspace_context());
        assert!(space_w_w_editor.0.is_empty());
        assert!(!space_w_w_editor.1);

        // Now test what happens if we have another binding defined AFTER the NoAction
        // that should result in pending
        let bindings = [
            KeyBinding::new("space w w", ActionAlpha {}, Some("workspace")),
            KeyBinding::new("space w w", NoAction {}, Some("editor")),
            KeyBinding::new("space w x", ActionAlpha {}, Some("editor")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let space_editor = keymap.bindings_for_input(&[space()], &editor_workspace_context());
        assert!(space_editor.0.is_empty());
        assert!(space_editor.1);

        // Now test what happens if we have another binding defined BEFORE the NoAction
        // that should result in pending
        let bindings = [
            KeyBinding::new("space w w", ActionAlpha {}, Some("workspace")),
            KeyBinding::new("space w x", ActionAlpha {}, Some("editor")),
            KeyBinding::new("space w w", NoAction {}, Some("editor")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let space_editor = keymap.bindings_for_input(&[space()], &editor_workspace_context());
        assert!(space_editor.0.is_empty());
        assert!(space_editor.1);

        // Now test what happens if we have another binding defined at a higher context
        // that should result in pending
        let bindings = [
            KeyBinding::new("space w w", ActionAlpha {}, Some("workspace")),
            KeyBinding::new("space w x", ActionAlpha {}, Some("workspace")),
            KeyBinding::new("space w w", NoAction {}, Some("editor")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let space_editor = keymap.bindings_for_input(&[space()], &editor_workspace_context());
        assert!(space_editor.0.is_empty());
        assert!(space_editor.1);
    }

    #[test]
    fn test_override_multikey() {
        let bindings = [
            KeyBinding::new("ctrl-w left", ActionAlpha {}, Some("editor")),
            KeyBinding::new("ctrl-w", NoAction {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // Ensure `space` results in pending input on the workspace, but not editor
        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-w").unwrap()],
            &[KeyContext::parse("editor").unwrap()],
        );
        assert!(result.is_empty());
        assert!(pending);

        let bindings = [
            KeyBinding::new("ctrl-w left", ActionAlpha {}, Some("editor")),
            KeyBinding::new("ctrl-w", ActionBeta {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // Ensure `space` results in pending input on the workspace, but not editor
        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-w").unwrap()],
            &[KeyContext::parse("editor").unwrap()],
        );
        assert_eq!(result.len(), 1);
        assert!(!pending);
    }

    #[test]
    fn test_simple_disable() {
        let bindings = [
            KeyBinding::new("ctrl-x", ActionAlpha {}, Some("editor")),
            KeyBinding::new("ctrl-x", NoAction {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // Ensure `space` results in pending input on the workspace, but not editor
        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x").unwrap()],
            &[KeyContext::parse("editor").unwrap()],
        );
        assert!(result.is_empty());
        assert!(!pending);
    }

    #[test]
    fn test_fail_to_disable() {
        // disabled at the wrong level
        let bindings = [
            KeyBinding::new("ctrl-x", ActionAlpha {}, Some("editor")),
            KeyBinding::new("ctrl-x", NoAction {}, Some("workspace")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // Ensure `space` results in pending input on the workspace, but not editor
        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x").unwrap()],
            &[
                KeyContext::parse("workspace").unwrap(),
                KeyContext::parse("editor").unwrap(),
            ],
        );
        assert_eq!(result.len(), 1);
        assert!(!pending);
    }

    #[test]
    fn test_disable_deeper() {
        let bindings = [
            KeyBinding::new("ctrl-x", ActionAlpha {}, Some("workspace")),
            KeyBinding::new("ctrl-x", NoAction {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // Ensure `space` results in pending input on the workspace, but not editor
        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x").unwrap()],
            &[
                KeyContext::parse("workspace").unwrap(),
                KeyContext::parse("editor").unwrap(),
            ],
        );
        assert_eq!(result.len(), 0);
        assert!(!pending);
    }

    #[test]
    fn test_pending_match_enabled() {
        let bindings = [
            KeyBinding::new("ctrl-x", ActionBeta, Some("vim_mode == normal")),
            KeyBinding::new("ctrl-x 0", ActionAlpha, Some("Workspace")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let matched = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x")].map(Result::unwrap),
            &[
                KeyContext::parse("Workspace"),
                KeyContext::parse("Pane"),
                KeyContext::parse("Editor vim_mode=normal"),
            ]
            .map(Result::unwrap),
        );
        assert_eq!(matched.0.len(), 1);
        assert!(matched.0[0].action.partial_eq(&ActionBeta));
        assert!(matched.1);
    }

    #[test]
    fn test_pending_match_enabled_extended() {
        let bindings = [
            KeyBinding::new("ctrl-x", ActionBeta, Some("vim_mode == normal")),
            KeyBinding::new("ctrl-x 0", NoAction, Some("Workspace")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let matched = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x")].map(Result::unwrap),
            &[
                KeyContext::parse("Workspace"),
                KeyContext::parse("Pane"),
                KeyContext::parse("Editor vim_mode=normal"),
            ]
            .map(Result::unwrap),
        );
        assert_eq!(matched.0.len(), 1);
        assert!(matched.0[0].action.partial_eq(&ActionBeta));
        assert!(!matched.1);
        let bindings = [
            KeyBinding::new("ctrl-x", ActionBeta, Some("Workspace")),
            KeyBinding::new("ctrl-x 0", NoAction, Some("vim_mode == normal")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let matched = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x")].map(Result::unwrap),
            &[
                KeyContext::parse("Workspace"),
                KeyContext::parse("Pane"),
                KeyContext::parse("Editor vim_mode=normal"),
            ]
            .map(Result::unwrap),
        );
        assert_eq!(matched.0.len(), 1);
        assert!(matched.0[0].action.partial_eq(&ActionBeta));
        assert!(!matched.1);
    }

    #[test]
    fn test_overriding_prefix() {
        let bindings = [
            KeyBinding::new("ctrl-x 0", ActionAlpha, Some("Workspace")),
            KeyBinding::new("ctrl-x", ActionBeta, Some("vim_mode == normal")),
        ];
        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let matched = keymap.bindings_for_input(
            &[Keystroke::parse("ctrl-x")].map(Result::unwrap),
            &[
                KeyContext::parse("Workspace"),
                KeyContext::parse("Pane"),
                KeyContext::parse("Editor vim_mode=normal"),
            ]
            .map(Result::unwrap),
        );
        assert_eq!(matched.0.len(), 1);
        assert!(matched.0[0].action.partial_eq(&ActionBeta));
        assert!(!matched.1);
    }

    #[test]
    fn test_context_precedence_with_same_source() {
        // Test case: User has both Workspace and Editor bindings for the same key
        // Editor binding should take precedence over Workspace binding
        let bindings = [
            KeyBinding::new("cmd-r", ActionAlpha {}, Some("Workspace")),
            KeyBinding::new("cmd-r", ActionBeta {}, Some("Editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        // Test with context stack: [Workspace, Editor] (Editor is deeper)
        let (result, _) = keymap.bindings_for_input(
            &[Keystroke::parse("cmd-r").unwrap()],
            &[
                KeyContext::parse("Workspace").unwrap(),
                KeyContext::parse("Editor").unwrap(),
            ],
        );

        // Both bindings should be returned, but Editor binding should be first (highest precedence)
        assert_eq!(result.len(), 2);
        assert!(result[0].action.partial_eq(&ActionBeta {})); // Editor binding first
        assert!(result[1].action.partial_eq(&ActionAlpha {})); // Workspace binding second
    }

    #[test]
    fn test_bindings_for_action() {
        let bindings = [
            KeyBinding::new("ctrl-a", ActionAlpha {}, Some("pane")),
            KeyBinding::new("ctrl-b", ActionBeta {}, Some("editor && mode == full")),
            KeyBinding::new("ctrl-c", ActionGamma {}, Some("workspace")),
            KeyBinding::new("ctrl-a", NoAction {}, Some("pane && active")),
            KeyBinding::new("ctrl-b", NoAction {}, Some("editor")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        assert_bindings(&keymap, &ActionAlpha {}, &["ctrl-a"]);
        assert_bindings(&keymap, &ActionBeta {}, &[]);
        assert_bindings(&keymap, &ActionGamma {}, &["ctrl-c"]);

        #[track_caller]
        fn assert_bindings(keymap: &Keymap, action: &dyn Action, expected: &[&str]) {
            let actual = keymap
                .bindings_for_action(action)
                .map(|binding| binding.keystrokes[0].inner().unparse())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{:?}", action);
        }
    }

    #[test]
    fn test_targeted_unbind_ignores_target_context() {
        let bindings = [
            KeyBinding::new("tab", ActionAlpha {}, Some("Editor")),
            KeyBinding::new("tab", ActionBeta {}, Some("Editor && showing_completions")),
            KeyBinding::new(
                "tab",
                Unbind("test_only::ActionAlpha".into()),
                Some("Editor && edit_prediction"),
            ),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        let (result, pending) = keymap.bindings_for_input(
            &[Keystroke::parse("tab").unwrap()],
            &[KeyContext::parse("Editor showing_completions edit_prediction").unwrap()],
        );

        assert!(!pending);
        assert_eq!(result.len(), 1);
        assert!(result[0].action.partial_eq(&ActionBeta {}));
    }

    #[test]
    fn test_bindings_for_action_keeps_binding_for_narrower_targeted_unbind() {
        let bindings = [
            KeyBinding::new("tab", ActionAlpha {}, Some("Editor")),
            KeyBinding::new(
                "tab",
                Unbind("test_only::ActionAlpha".into()),
                Some("Editor && edit_prediction"),
            ),
            KeyBinding::new("tab", ActionBeta {}, Some("Editor && showing_completions")),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        assert_bindings(&keymap, &ActionAlpha {}, &["tab"]);
        assert_bindings(&keymap, &ActionBeta {}, &["tab"]);

        #[track_caller]
        fn assert_bindings(keymap: &Keymap, action: &dyn Action, expected: &[&str]) {
            let actual = keymap
                .bindings_for_action(action)
                .map(|binding| binding.keystrokes[0].inner().unparse())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{:?}", action);
        }
    }

    #[test]
    fn test_bindings_for_action_removes_binding_for_broader_targeted_unbind() {
        let bindings = [
            KeyBinding::new("tab", ActionAlpha {}, Some("Editor && edit_prediction")),
            KeyBinding::new(
                "tab",
                Unbind("test_only::ActionAlpha".into()),
                Some("Editor"),
            ),
        ];

        let mut keymap = Keymap::default();
        keymap.add_bindings(bindings);

        assert!(keymap.bindings_for_action(&ActionAlpha {}).next().is_none());
    }

    #[test]
    fn test_source_precedence_sorting() {
        // KeybindSource precedence: User (0) > Vim (1) > Base (2) > Default (3)
        // Test that user keymaps take precedence over default keymaps at the same context depth
        let mut keymap = Keymap::default();

        // Add a default keymap binding first
        let mut default_binding = KeyBinding::new("cmd-r", ActionAlpha {}, Some("Editor"));
        default_binding.set_meta(KeyBindingMetaIndex(3)); // Default source
        keymap.add_bindings([default_binding]);

        // Add a user keymap binding
        let mut user_binding = KeyBinding::new("cmd-r", ActionBeta {}, Some("Editor"));
        user_binding.set_meta(KeyBindingMetaIndex(0)); // User source
        keymap.add_bindings([user_binding]);

        // Test with Editor context stack
        let (result, _) = keymap.bindings_for_input(
            &[Keystroke::parse("cmd-r").unwrap()],
            &[KeyContext::parse("Editor").unwrap()],
        );

        // User binding should take precedence over default binding
        assert_eq!(result.len(), 2);
        assert!(result[0].action.partial_eq(&ActionBeta {}));
        assert!(result[1].action.partial_eq(&ActionAlpha {}));
    }
}
