//! Action 系统 —— 定义和管理 UI 动作的注册、分发与序列化机制。

use crate::collections::{HashMap, TypeIdHashMap};
use anyhow::{Context as _, Result};
pub use no_action::{NoAction, Unbind, is_no_action, is_unbind};
pub use rgpui_macros::Action;
use serde_json::json;
use std::{
    any::{Any, TypeId},
    fmt::Display,
};

/// 定义并注册可用作动作的单元结构体。对于更复杂的数据类型，请派生 `Action`。
///
/// 例如：
///
/// ```
/// use rgpui::actions;
/// actions!(editor, [MoveUp, MoveDown, MoveLeft, MoveRight, Newline]);
/// ```
///
/// 这将创建名为 `editor::MoveUp`、`editor::MoveDown` 等的动作。
///
/// 命名空间参数 `editor` 也可以省略，但对于 Zed 动作是必需的。
#[macro_export]
macro_rules! actions {
    ($namespace:path, [ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default, ::std::fmt::Debug, rgpui::Action)]
            #[action(namespace = $namespace)]
            $(#[$attr])*
            #[doc = concat!("`", stringify!($name), "` 动作。")]
            pub struct $name;
        )*
    };
    ([ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default, ::std::fmt::Debug, rgpui::Action)]
            $(#[$attr])*
            #[doc = concat!("`", stringify!($name), "` 动作。")]
            pub struct $name;
        )*
    };
}

/// 动作用于实现键盘驱动的 UI。声明动作后，可以在键映射中将按键绑定到该动作，
/// 并在元素树中为该动作设置监听器。
///
/// 要声明一组简单动作，可以使用 actions! 宏，它为给定命名空间中列出的每个动作名称
/// 定义一个简单的单元结构体动作。
///
/// ```
/// use rgpui::actions;
/// actions!(editor, [MoveUp, MoveDown, MoveLeft, MoveRight, Newline]);
/// ```
///
/// 注册同名动作会导致 `App` 创建时 panic。
///
/// # 派生宏
///
/// 更复杂的数据类型也可以是动作，通过为 `Action` 使用派生宏：
///
/// ```
/// use rgpui::Action;
/// #[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
/// #[action(namespace = editor)]
/// pub struct SelectNext {
///     pub replace_newest: bool,
/// }
/// ```
///
/// `Action` 的派生宏要求类型实现 `Clone` 和 `PartialEq`。它还要求
/// `serde::Deserialize` 和 `schemars::JsonSchema`，除非指定了 `#[action(no_json)]`。
/// 在 Zed 中，这些 trait 实现用于从 JSON 加载键映射。
///
/// `#[action(...)]` 中可以指定多个以逗号分隔的参数：
///
/// - `namespace = some_namespace` 设置命名空间。在 Zed 中这是必需的。
///
/// - `name = "ActionName"` 覆盖动作名称。不能包含 `::`。
///
/// - `no_json` 使 `build` 方法始终报错且 `action_json_schema` 返回 `None`，
///   并允许动作不实现 `serde::Serialize` 和 `schemars::JsonSchema`。
///
/// - `no_register` 跳过注册动作。这在实现 `Action` trait 时不支持按名称调用
///   或 JSON 反序列化时很有用。
///
/// - `deprecated_aliases = ["editor::SomeAction"]` 指定动作的已弃用旧名称。
///   这些动作名称*不应*对应任何已注册的动作。这些旧名称仍可用于引用调用此动作。
///   在 Zed 中，键映射 JSON 模式将接受这些旧名称并提供警告。
///
/// - `deprecated = "关于此动作弃用原因的消息"` 指定弃用消息。
///   在 Zed 中，键映射 JSON 模式会将其显示为警告。
///
/// # 手动实现
///
/// 如果想手动控制 action trait 的行为，可以使用更低级的
/// `#[register_action]` 宏，它只生成在 `main` 之前注册动作所需的代码。
///
/// ```
/// use rgpui::{SharedString, register_action};
/// #[derive(Clone, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
/// pub struct Paste {
///     pub content: SharedString,
/// }
///
/// impl rgpui::Action for Paste {
///     # fn boxed_clone(&self) -> Box<dyn rgpui::Action> { unimplemented!()}
///     # fn partial_eq(&self, other: &dyn rgpui::Action) -> bool { unimplemented!() }
///     # fn name(&self) -> &'static str { "Paste" }
///     # fn name_for_type() -> &'static str { "Paste" }
///     # fn build(value: serde_json::Value) -> anyhow::Result<Box<dyn rgpui::Action>> {
///     #     unimplemented!()
///     # }
/// }
///
/// register_action!(Paste);
/// ```
pub trait Action: Any + Send {
    /// 将动作克隆到新的 Box 中
    fn boxed_clone(&self) -> Box<dyn Action>;

    /// 对此动作与另一个动作进行部分相等性检查
    fn partial_eq(&self, action: &dyn Action) -> bool;

    /// 获取此动作的名称，用于在 UI 中显示
    fn name(&self) -> &'static str;

    /// 获取此动作类型的名称（静态）
    fn name_for_type() -> &'static str
    where
        Self: Sized;

    /// 从 JSON 值构建此动作。用于从键映射构造动作。
    /// 对于没有参数的动作，将传递 `{}` 值。
    fn build(value: serde_json::Value) -> Result<Box<dyn Action>>
    where
        Self: Sized;

    /// 动作输入数据的可选 JSON 模式。
    fn action_json_schema(_: &mut schemars::SchemaGenerator) -> Option<schemars::Schema>
    where
        Self: Sized,
    {
        None
    }

    /// 此动作的替代已弃用名称列表。这些名称仍可用于调用该动作。
    /// 在 Zed 中，键映射 JSON 模式将接受这些旧名称并提供警告。
    fn deprecated_aliases() -> &'static [&'static str]
    where
        Self: Sized,
    {
        &[]
    }

    /// 返回此动作的弃用消息（如果有）。在 Zed 中，键映射 JSON 模式会将其显示为警告。
    fn deprecation_message() -> Option<&'static str>
    where
        Self: Sized,
    {
        None
    }

    /// 此动作的文档（如果有）。使用动作的派生宏时，
    /// 这将从动作结构体上的文档注释自动生成。
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
    /// 类型擦除 Action 类型。
    pub fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

/// `Keystroke::parse` 的错误类型。使用此类型而非 `anyhow::Error`，以便 Zed 可以使用 markdown 来显示它。
#[derive(Debug)]
pub enum ActionBuildError {
    /// 表示未注册具有此名称的动作。
    NotFound {
        /// 未找到的动作名称。
        name: String,
    },
    /// 表示构建动作时发生错误，通常是 JSON 反序列化错误。
    BuildError {
        /// 正在尝试构建的动作名称。
        name: String,
        /// 构建动作时发生的错误。
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

pub(crate) struct ActionRegistry {
    by_name: HashMap<&'static str, ActionData>,
    names_by_type_id: TypeIdHashMap<&'static str>,
    all_names: Vec<&'static str>, // So we can return a static slice.
    deprecated_aliases: HashMap<&'static str, &'static str>, // deprecated name -> preferred name
    deprecation_messages: HashMap<&'static str, &'static str>, // action name -> deprecation message
    documentation: HashMap<&'static str, &'static str>, // action name -> documentation
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

/// 此类型必须是公共的，以便我们的宏可以在其他 crate 中构建它。
/// 但这是实现细节，不应直接使用。
#[doc(hidden)]
pub struct MacroActionBuilder(pub fn() -> MacroActionData);

/// 此类型必须是公共的，以便我们的宏可以在其他 crate 中构建它。
/// 但这是实现细节，不应直接使用。
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
    /// 将所有已注册的动作加载到注册表中。
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

    /// 根据动作名称和可选的 JSON 参数（来源于键映射）构建动作。
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

    /// 根据动作名称和可选的 JSON 参数（来源于键映射）构建动作。
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

/// 生成所有已注册动作的列表。
/// 适用于将可用动作列表转换为适合静态分析的格式，
/// 例如验证键映射或生成文档。
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
        rgpui,
        [
            /// 具有特殊处理的动作，如果它是最高优先级匹配项，
            /// 则会解除关联的键绑定。
            NoAction
        ]
    );

    /// 具有特殊处理的动作，当同一按键序列触发指定动作时，
    /// 它会解除后续绑定，无论该动作的上下文如何。
    ///
    /// 在键映射 JSON 中写作：
    ///
    /// `["rgpui::Unbind", "editor::NewLine"]`
    #[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, rgpui::Action)]
    #[action(namespace = rgpui)]
    pub struct Unbind(pub rgpui::SharedString);

    /// 返回此动作是否表示已移除的键绑定。
    pub fn is_no_action(action: &dyn rgpui::Action) -> bool {
        action.as_any().is::<NoAction>()
    }

    /// 返回此动作是否表示解除绑定标记。
    pub fn is_unbind(action: &dyn rgpui::Action) -> bool {
        action.as_any().is::<Unbind>()
    }
}
