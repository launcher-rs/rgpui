//! 命令注册表 —— 在 Action 之上附加 UI 元数据，供功能区、命令面板、快捷键、
//! CLI、control channel、MCP 统一按 id 分发。
//!
//! ## 与 Action 的关系
//!
//! 一个「命令」= 一个可被派发的 [`Action`](crate::Action) + 一份 UI 元数据
//! [`CommandDescriptor`]。命令 id 与动作名称相同（如 `"editor::MoveUp"`），
//! 因此注册表只负责元数据与分发入口：动作的 JSON 构建、上下文路由仍由 Action 系统完成。
//!
//! ## 用法
//!
//! ```
//! use rgpui::{register_command, CommandDescriptor};
//!
//! /// 打开文件命令的描述符。
//! fn open_file() -> CommandDescriptor {
//!     CommandDescriptor::new("app::Open", "打开")
//!         .with_category("文件")
//!         .with_description("打开一个文件")
//!         .with_keystrokes(["ctrl-o"])
//! }
//!
//! register_command!(open_file);
//! ```

use crate::SharedString;
use crate::collections::HashMap;
use std::sync::OnceLock;

/// 命令的 UI 元数据。
///
/// 与同名动作配合，驱动命令面板、功能区、菜单、快捷键提示与命令搜索。
/// 仅承载展示信息，不含执行逻辑（执行走 [`App::dispatch_command`](crate::App::dispatch_command)）。
#[derive(Clone, Debug, PartialEq)]
pub struct CommandDescriptor {
    id: SharedString,
    label: SharedString,
    category: Option<SharedString>,
    description: Option<SharedString>,
    default_keystrokes: Vec<SharedString>,
    keytip: Option<SharedString>,
}

impl CommandDescriptor {
    /// 以命令 id（与动作名称一致）和显示标签创建描述符。
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category: None,
            description: None,
            default_keystrokes: Vec::new(),
            keytip: None,
        }
    }

    /// 设置命令分类（用于分组与搜索）。
    pub fn with_category(mut self, category: impl Into<SharedString>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// 设置命令描述（参与命令搜索）。
    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 追加一个默认快捷键（仅用于展示，如 `"ctrl-o"`）。
    pub fn with_keystroke(mut self, keystroke: impl Into<SharedString>) -> Self {
        self.default_keystrokes.push(keystroke.into());
        self
    }

    /// 批量设置默认快捷键（会覆盖已有值）。
    pub fn with_keystrokes<I, K>(mut self, keystrokes: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<SharedString>,
    {
        self.default_keystrokes = keystrokes.into_iter().map(Into::into).collect();
        self
    }

    /// 设置按键提示（KeyTips，如 `"P"` 或 `"HO"`）。
    ///
    /// 供功能区的 Alt/F10 按键导航（见 [`crate::ribbon`]）使用；
    /// 未设置时功能区会退化为从标签的首个 ASCII 字母派生。
    pub fn with_keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }

    /// 命令 id（与动作名称一致）。
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// 显示标签。
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// 命令分类。
    pub fn category(&self) -> Option<&SharedString> {
        self.category.as_ref()
    }

    /// 命令描述。
    pub fn description(&self) -> Option<&SharedString> {
        self.description.as_ref()
    }

    /// 默认快捷键（展示用）。
    pub fn default_keystrokes(&self) -> &[SharedString] {
        &self.default_keystrokes
    }

    /// 按键提示（KeyTips，展示用）。
    pub fn keytip(&self) -> Option<&SharedString> {
        self.keytip.as_ref()
    }
}

/// inventory 收集项：保存一个返回 [`CommandDescriptor`] 的构造函数指针。
///
/// 这是实现细节，请通过 [`register_command!`](crate::register_command) 宏使用。
#[doc(hidden)]
pub struct CommandRegistration(pub fn() -> CommandDescriptor);

inventory::collect!(CommandRegistration);

/// 登记一个命令描述符构造函数。
///
/// 参数必须是 `fn() -> CommandDescriptor` 的**函数项**（inventory 的 const 约束不接受闭包）：
///
/// ```
/// use rgpui::{register_command, CommandDescriptor};
///
/// fn save() -> CommandDescriptor {
///     CommandDescriptor::new("app::Save", "保存")
/// }
///
/// register_command!(save);
/// ```
#[macro_export]
macro_rules! register_command {
    ($constructor:path) => {
        $crate::private::inventory::submit! {
            $crate::commands::CommandRegistration($constructor)
        }
    };
}

/// 全局命令注册表：进程内单例，首次访问时从 inventory 加载。
///
/// 命令按「分类 → 标签」排序，便于功能区与命令面板稳定展示。
pub struct CommandRegistry {
    descriptors: Vec<CommandDescriptor>,
    by_id: HashMap<SharedString, usize>,
}

impl CommandRegistry {
    /// 从 inventory 加载全部登记的命令并按分类/标签排序。
    fn load() -> Self {
        let mut descriptors: Vec<CommandDescriptor> = inventory::iter::<CommandRegistration>
            .into_iter()
            .map(|registration| (registration.0)())
            .collect();

        descriptors.sort_by(|a, b| {
            a.category()
                .cmp(&b.category())
                .then_with(|| a.label().cmp(b.label()))
        });

        let mut by_id: HashMap<SharedString, usize> = HashMap::default();
        for (index, descriptor) in descriptors.iter().enumerate() {
            // 同一 id 登记两次会让功能区 / 命令面板 / 控制通道看到不一致的命令：
            // `by_id` 只认第一条（inventory 的遍历顺序不保证），重复项保留在清单里
            // 由 `validate()` 报出来 —— 启动时记一条错误，CI 上的 gate 测试会失败。
            if by_id.contains_key(&descriptor.id) {
                log::error!(
                    "命令 id `{}` 重复登记，已忽略后一条（`register_command!` 只能登记一次）",
                    descriptor.id
                );
                continue;
            }
            by_id.insert(descriptor.id.clone(), index);
        }

        Self { descriptors, by_id }
    }

    /// 获取全局命令注册表（进程内仅加载一次）。
    pub fn global() -> &'static CommandRegistry {
        static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();
        REGISTRY.get_or_init(CommandRegistry::load)
    }

    /// 按 id 查找命令描述符。
    pub fn get(&self, id: &str) -> Option<&CommandDescriptor> {
        self.by_id.get(id).map(|&index| &self.descriptors[index])
    }

    /// 是否登记了指定 id 的命令。
    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// 命令显示标签；未登记时回退为命令 id 本身。
    ///
    /// Ribbon / 快速访问工具栏 / 菜单等按 id 绑定命令的 UI 用它取默认标签。
    pub fn label_for(&self, id: &str) -> SharedString {
        self.get(id)
            .map(|descriptor| descriptor.label().clone())
            .unwrap_or_else(|| SharedString::from(id))
    }

    /// 迭代全部命令（已按分类/标签排序）。
    pub fn iter(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.descriptors.iter()
    }

    /// 全部分类（去重，按排序出现顺序）。
    pub fn categories(&self) -> Vec<&SharedString> {
        let mut categories: Vec<&SharedString> = Vec::new();
        for descriptor in &self.descriptors {
            if let Some(category) = descriptor.category() {
                if !categories.contains(&category) {
                    categories.push(category);
                }
            }
        }
        categories
    }

    /// 命令数量。
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// 注册表是否为空。
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    /// 校验注册表自洽性，返回问题清单（空表示通过）。
    ///
    /// 检查项：id 非空且唯一、标签非空、分类非空、默认快捷键可解析。
    /// 这是「命令清单」的 gate：应用在自己的测试里
    /// `assert!(CommandRegistry::global().validate().is_empty())`，
    /// 就能在 CI 上拦住「漏填分类 / 快捷键写错 / 重复登记」这类
    /// 只有点到 UI 才会暴露的问题（控制通道的 `commands.list` 复用同一份元数据）。
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for descriptor in &self.descriptors {
            let id = descriptor.id();
            if id.as_ref().is_empty() {
                problems.push("存在空命令 id".to_string());
            }
            if !seen.insert(id.clone()) {
                problems.push(format!("命令 id 重复：{id}"));
            }
            if descriptor.label().as_ref().is_empty() {
                problems.push(format!("命令 `{id}` 缺少标签"));
            }
            if descriptor
                .category()
                .is_none_or(|category| category.as_ref().is_empty())
            {
                problems.push(format!("命令 `{id}` 缺少分类"));
            }
            for keystroke in descriptor.default_keystrokes() {
                if crate::Keystroke::parse(keystroke).is_err() {
                    problems.push(format!("命令 `{id}` 的快捷键 `{keystroke}` 无法解析"));
                }
            }
        }
        problems
    }
}

impl crate::App {
    /// 按命令 id 构建并派发对应动作。
    ///
    /// 命令 id 即动作名称（如 `"editor::MoveUp"`），`params` 是动作的 JSON 参数
    /// （无参动作传 `None`）。派发走 [`App::dispatch_action`](crate::App::dispatch_action)
    /// 的常规路由：发送到当前活动窗口的焦点节点，无窗口时走全局动作分发。
    pub fn dispatch_command(
        &mut self,
        id: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let action = self.build_action(id, params)?;
        self.dispatch_action(&*action);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_open() -> CommandDescriptor {
        CommandDescriptor::new("rgpui_test::Open", "打开")
            .with_category("文件")
            .with_description("打开一个文件")
            .with_keystrokes(["ctrl-o", "ctrl-shift-o"])
    }

    fn demo_quit() -> CommandDescriptor {
        CommandDescriptor::new("rgpui_test::Quit", "退出").with_category("文件")
    }

    register_command!(demo_open);
    register_command!(demo_quit);

    #[test]
    fn registry_loads_registered_commands() {
        let registry = CommandRegistry::global();
        assert!(registry.contains("rgpui_test::Open"));
        assert!(registry.contains("rgpui_test::Quit"));

        let open = registry.get("rgpui_test::Open").expect("已登记 Open");
        assert_eq!(open.id().as_ref(), "rgpui_test::Open");
        assert_eq!(open.label().as_ref(), "打开");
        assert_eq!(open.category().map(|c| c.as_ref()), Some("文件"));
        assert_eq!(open.description().map(|d| d.as_ref()), Some("打开一个文件"));
        assert_eq!(
            open.default_keystrokes(),
            &[
                SharedString::from("ctrl-o"),
                SharedString::from("ctrl-shift-o"),
            ]
        );
    }

    #[test]
    fn registry_is_sorted_by_category_then_label() {
        let registry = CommandRegistry::global();
        let file: Vec<&str> = registry
            .iter()
            .filter(|d| d.category().map(|c| c.as_ref()) == Some("文件"))
            .map(|d| d.label().as_ref())
            .collect();
        assert_eq!(file, vec!["打开", "退出"]);
    }

    #[test]
    fn registry_reports_categories() {
        let registry = CommandRegistry::global();
        assert!(registry.categories().iter().any(|c| c.as_ref() == "文件"));
    }

    #[test]
    fn unknown_command_is_not_found() {
        let registry = CommandRegistry::global();
        assert!(registry.get("rgpui_test::Missing").is_none());
        assert!(!registry.contains("rgpui_test::Missing"));
    }

    /// gate：全局命令清单自洽（id 唯一、标签/分类非空、快捷键可解析）。
    #[test]
    fn registry_passes_validation() {
        let problems = CommandRegistry::global().validate();
        assert!(problems.is_empty(), "命令清单存在问题：{problems:?}");
    }
}
