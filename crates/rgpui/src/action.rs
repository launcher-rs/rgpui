use crate::collections::HashMap;
use anyhow::{Context as _, Result};
pub use no_action::{NoAction, Unbind, is_no_action, is_unbind};
pub use rgpui_macros::Action;
use serde_json::json;
use std::{
    any::{Any, TypeId},
    fmt::Display,
};

/// 定义并注册可用作操作的单元结构体。对于更复杂的数据类型，请派生 `Action`。
///
/// 例如：
///
/// ```
/// use rgpui::actions;
/// actions!(editor, [MoveUp, MoveDown, MoveLeft, MoveRight, Newline]);
/// ```
///
/// 这将创建名称为 `editor::MoveUp`、`editor::MoveDown` 等的操作。
///
/// 命名空间参数 `editor` 也可以省略，尽管 Zed 操作需要它。
#[macro_export]
macro_rules! actions {
    ($namespace:path, [ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default, ::std::fmt::Debug, rgpui::Action)]
            #[action(namespace = $namespace)]
            $(#[$attr])*
            pub struct $name;
        )*
    };
    ([ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default, ::std::fmt::Debug, rgpui::Action)]
            $(#[$attr])*
            pub struct $name;
        )*
    };
}

/// Action trait - GPUI 中所有用户交互动作的基础 trait。
///
/// Action 是 GPUI 的核心概念之一，用于实现键盘驱动的 UI：
/// 1. 定义 Action 类型（通过 `actions!` 宏或 `#[derive(Action)]`）
/// 2. 在元素树中注册 Action 监听器
/// 3. 在快捷键映射中将按键绑定到 Action
///
/// # 创建 Action
///
/// 简单的单元结构体 Action 可以用 `actions!` 宏快速创建：
///
/// ```rust,ignore
/// actions!(editor, [MoveUp, MoveDown, MoveLeft, MoveRight, Newline]);
/// ```
///
/// 带数据的复杂 Action 使用 `#[derive(Action)]`：
///
/// ```rust,ignore
/// #[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
/// #[action(namespace = editor)]
/// pub struct SelectNext {
///     pub replace_newest: bool,
/// }
/// ```
///
/// # 序列化支持
///
/// Action 需要实现 `Clone` 和 `PartialEq`。默认还需要 `serde::Deserialize` 和
/// `schemars::JsonSchema`（用于从 JSON 加载快捷键映射），
/// 可通过 `#[action(no_json)]` 禁用。
pub trait Action: Any + Send {
    /// 将 Action 克隆到一个新的 Box 中（类型擦除的克隆）
    fn boxed_clone(&self) -> Box<dyn Action>;

    /// 对此 Action 和另一个 Action 进行部分相等性比较
    fn partial_eq(&self, action: &dyn Action) -> bool;

    /// 获取此 Action 的名称（用于在 UI 中显示）
    fn name(&self) -> &'static str;

    /// 获取此 Action 类型的名称（静态方法）
    fn name_for_type() -> &'static str
    where
        Self: Sized;

    /// 从 JSON 值构建此 Action。用于从快捷键映射中构造 Action。
    /// 没有参数的 Action 会传入 `{}`。
    fn build(value: serde_json::Value) -> Result<Box<dyn Action>>
    where
        Self: Sized;

    /// Action 输入数据的可选 JSON Schema
    fn action_json_schema(_: &mut schemars::SchemaGenerator) -> Option<schemars::Schema>
    where
        Self: Sized,
    {
        None
    }

    /// 此 Action 的已弃用别名列表。这些旧名称仍可用于调用此 Action。
    fn deprecated_aliases() -> &'static [&'static str]
    where
        Self: Sized,
    {
        &[]
    }

    /// 返回此 Action 的弃用消息（如果有）
    fn deprecation_message() -> Option<&'static str>
    where
        Self: Sized,
    {
        None
    }

    /// 此 Action 的文档（如果有）。使用 derive 宏时会自动生成。
    fn documentation() -> Option<&'static str>
    where
        Self: Sized,
    {
        None
    }
}

impl std::fmt::Debug for dyn Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("dyn Action")
            .field("name", &self.name())
            .finish()
    }
}

impl dyn Action {
    /// Type-erase Action type.
    pub fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

/// Error type for `Keystroke::parse`. This is used instead of `anyhow::Error` so that Zed can use
/// markdown to display it.
#[derive(Debug)]
pub enum ActionBuildError {
    /// Indicates that an action with this name has not been registered.
    NotFound {
        /// Name of the action that was not found.
        name: String,
    },
    /// Indicates that an error occurred while building the action, typically a JSON deserialization
    /// error.
    BuildError {
        /// Name of the action that was attempting to be built.
        name: String,
        /// Error that occurred while building the action.
        error: anyhow::Error,
    },
}

impl std::error::Error for ActionBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ActionBuildError::NotFound { .. } => None,
            ActionBuildError::BuildError { error, .. } => error.source(),
        }
    }
}

impl Display for ActionBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionBuildError::NotFound { name } => {
                write!(f, "Didn't find an action named \"{name}\"")
            }
            ActionBuildError::BuildError { name, error } => {
                write!(f, "Error while building action \"{name}\": {error}")
            }
        }
    }
}

type ActionBuilder = fn(json: serde_json::Value) -> anyhow::Result<Box<dyn Action>>;

/// Action 注册表 - 管理所有已注册的 Action 类型。
///
/// 在应用启动时，所有 Action 通过 `register_action!` 宏注册到此表中。
/// 注册表提供了：
/// - 按名称查找 Action 并构建实例
/// - 按 TypeId 查找 Action 名称
/// - 获取所有已注册 Action 的名称列表
/// - 管理弃用别名和弃用消息
pub(crate) struct ActionRegistry {
    /// 按名称存储的 Action 数据（包含构建函数和元数据）
    by_name: HashMap<&'static str, ActionData>,
    /// TypeId -> Action 名称的映射
    names_by_type_id: HashMap<TypeId, &'static str>,
    /// 所有已注册 Action 的名称列表（用于返回静态切片）
    all_names: Vec<&'static str>,
    /// 弃用别名映射：旧名称 -> 推荐名称
    deprecated_aliases: HashMap<&'static str, &'static str>,
    /// 弃用消息映射：Action 名称 -> 弃用消息
    deprecation_messages: HashMap<&'static str, &'static str>,
    /// Action 文档映射：Action 名称 -> 文档字符串
    documentation: HashMap<&'static str, &'static str>,
}

impl Default for ActionRegistry {
    fn default() -> Self {
        let mut this = ActionRegistry {
            by_name: Default::default(),
            names_by_type_id: Default::default(),
            documentation: Default::default(),
            all_names: Default::default(),
            deprecated_aliases: Default::default(),
            deprecation_messages: Default::default(),
        };

        this.load_actions();

        this
    }
}

struct ActionData {
    pub build: ActionBuilder,
    pub json_schema: fn(&mut schemars::SchemaGenerator) -> Option<schemars::Schema>,
}

/// This type must be public so that our macros can build it in other crates.
/// But this is an implementation detail and should not be used directly.
#[doc(hidden)]
pub struct MacroActionBuilder(pub fn() -> MacroActionData);

/// This type must be public so that our macros can build it in other crates.
/// But this is an implementation detail and should not be used directly.
#[doc(hidden)]
pub struct MacroActionData {
    pub name: &'static str,
    pub type_id: TypeId,
    pub build: ActionBuilder,
    pub json_schema: fn(&mut schemars::SchemaGenerator) -> Option<schemars::Schema>,
    pub deprecated_aliases: &'static [&'static str],
    pub deprecation_message: Option<&'static str>,
    pub documentation: Option<&'static str>,
}

inventory::collect!(MacroActionBuilder);

impl ActionRegistry {
    /// Load all registered actions into the registry.
    pub(crate) fn load_actions(&mut self) {
        for builder in inventory::iter::<MacroActionBuilder> {
            let action = builder.0();
            self.insert_action(action);
        }
    }

    fn insert_action(&mut self, action: MacroActionData) {
        let name = action.name;
        if self.by_name.contains_key(name) {
            panic!(
                "Action with name `{name}` already registered \
                (might be registered in `#[action(deprecated_aliases = [...])]`."
            );
        }
        self.by_name.insert(
            name,
            ActionData {
                build: action.build,
                json_schema: action.json_schema,
            },
        );
        for &alias in action.deprecated_aliases {
            if self.by_name.contains_key(alias) {
                panic!(
                    "Action with name `{alias}` already registered. \
                    `{alias}` is specified in `#[action(deprecated_aliases = [...])]` for action `{name}`."
                );
            }
            self.by_name.insert(
                alias,
                ActionData {
                    build: action.build,
                    json_schema: action.json_schema,
                },
            );
            self.deprecated_aliases.insert(alias, name);
            self.all_names.push(alias);
        }
        self.names_by_type_id.insert(action.type_id, name);
        self.all_names.push(name);
        if let Some(deprecation_msg) = action.deprecation_message {
            self.deprecation_messages.insert(name, deprecation_msg);
        }
        if let Some(documentation) = action.documentation {
            self.documentation.insert(name, documentation);
        }
    }

    /// Construct an action based on its name and optional JSON parameters sourced from the keymap.
    pub fn build_action_type(&self, type_id: &TypeId) -> Result<Box<dyn Action>> {
        let name = self
            .names_by_type_id
            .get(type_id)
            .with_context(|| format!("no action type registered for {type_id:?}"))?;

        Ok(self.build_action(name, None)?)
    }

    pub(crate) fn try_resolve_action(&self, type_id: &TypeId) -> Option<&'static str> {
        self.names_by_type_id.get(type_id).copied()
    }

    /// Construct an action based on its name and optional JSON parameters sourced from the keymap.
    pub fn build_action(
        &self,
        name: &str,
        params: Option<serde_json::Value>,
    ) -> std::result::Result<Box<dyn Action>, ActionBuildError> {
        let build_action = self
            .by_name
            .get(name)
            .ok_or_else(|| ActionBuildError::NotFound {
                name: name.to_owned(),
            })?
            .build;
        (build_action)(params.unwrap_or_else(|| json!({}))).map_err(|e| {
            ActionBuildError::BuildError {
                name: name.to_owned(),
                error: e,
            }
        })
    }

    pub fn all_action_names(&self) -> &[&'static str] {
        self.all_names.as_slice()
    }

    pub fn action_schemas(
        &self,
        generator: &mut schemars::SchemaGenerator,
    ) -> Vec<(&'static str, Option<schemars::Schema>)> {
        // Use the order from all_names so that the resulting schema has sensible order.
        self.all_names
            .iter()
            .map(|name| {
                let action_data = self
                    .by_name
                    .get(name)
                    .expect("All actions in all_names should be registered");
                (*name, (action_data.json_schema)(generator))
            })
            .collect::<Vec<_>>()
    }

    pub fn action_schema_by_name(
        &self,
        name: &str,
        generator: &mut schemars::SchemaGenerator,
    ) -> Option<Option<schemars::Schema>> {
        self.by_name
            .get(name)
            .map(|action_data| (action_data.json_schema)(generator))
    }

    pub fn deprecated_aliases(&self) -> &HashMap<&'static str, &'static str> {
        &self.deprecated_aliases
    }

    pub fn deprecation_messages(&self) -> &HashMap<&'static str, &'static str> {
        &self.deprecation_messages
    }

    pub fn documentation(&self) -> &HashMap<&'static str, &'static str> {
        &self.documentation
    }
}

/// Generate a list of all the registered actions.
/// Useful for transforming the list of available actions into a
/// format suited for static analysis such as in validating keymaps, or
/// generating documentation.
pub fn generate_list_of_all_registered_actions() -> impl Iterator<Item = MacroActionData> {
    inventory::iter::<MacroActionBuilder>
        .into_iter()
        .map(|builder| builder.0())
}

mod no_action {
    use crate as rgpui;
    use schemars::JsonSchema;
    use serde::Deserialize;

    actions!(
        zed,
        [
            /// Action with special handling which unbinds the keybinding this is associated with,
            /// if it is the highest precedence match.
            NoAction
        ]
    );

    /// Action with special handling which unbinds later bindings for the same keystrokes when they
    /// dispatch the named action, regardless of that action's context.
    ///
    /// In keymap JSON this is written as:
    ///
    /// `["zed::Unbind", "editor::NewLine"]`
    #[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, rgpui::Action)]
    #[action(namespace = zed)]
    pub struct Unbind(pub rgpui::SharedString);

    /// Returns whether or not this action represents a removed key binding.
    pub fn is_no_action(action: &dyn rgpui::Action) -> bool {
        action.as_any().is::<NoAction>()
    }

    /// Returns whether or not this action represents an unbind marker.
    pub fn is_unbind(action: &dyn rgpui::Action) -> bool {
        action.as_any().is::<Unbind>()
    }
}
