//! Markdown 插件扩展系统 —— 支持自定义 Markdown 渲染器。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::markdown_ext::{MarkdownPlugin, MarkdownRenderer, PluginManager};
//!
//! struct CustomPlugin;
//! impl MarkdownPlugin for CustomPlugin {
//!     fn name(&self) -> &str { "custom" }
//!     fn render(&self, content: &str) -> Option<String> {
//!         Some(format!("<div class=\"custom\">{}</div>", content))
//!     }
//! }
//!
//! let mut manager = PluginManager::new();
//! manager.register(Box::new(CustomPlugin));
//! ```

use std::collections::HashMap;

use regex::Regex;

/// Markdown 插件 trait。
pub trait MarkdownPlugin {
    /// 插件名称。
    fn name(&self) -> &str;

    /// 插件描述。
    fn description(&self) -> &str {
        ""
    }

    /// 渲染内容。
    fn render(&self, content: &str) -> Option<String>;

    /// 是否处理指定语法。
    fn can_handle(&self, _syntax: &str) -> bool {
        false
    }

    /// 获取插件版本。
    fn version(&self) -> &str {
        "0.1.0"
    }
}

/// Markdown 渲染器。
pub struct MarkdownRenderer {
    /// 插件管理器。
    plugin_manager: PluginManager,
    /// 自定义样式。
    custom_styles: HashMap<String, String>,
}

impl MarkdownRenderer {
    /// 创建新的渲染器。
    pub fn new() -> Self {
        Self {
            plugin_manager: PluginManager::new(),
            custom_styles: HashMap::new(),
        }
    }

    /// 注册插件。
    pub fn register_plugin(&mut self, plugin: Box<dyn MarkdownPlugin>) {
        self.plugin_manager.register(plugin);
    }

    /// 添加自定义样式。
    pub fn add_style(&mut self, class: &str, style: &str) {
        self.custom_styles.insert(class.to_string(), style.to_string());
    }

    /// 渲染 Markdown。
    pub fn render(&self, input: &str) -> String {
        let mut output = String::new();
        let mut remaining = input;

        // 尝试使用插件渲染
        for plugin in &self.plugin_manager.plugins {
            if let Some(result) = plugin.render(remaining) {
                return result;
            }
        }

        // 默认渲染逻辑
        for line in input.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                output.push_str("<br>");
                continue;
            }

            // 标题
            if let Some(level) = trimmed.strip_prefix('#').map(|s| s.len() as u8) {
                if level <= 6 {
                    let content = trimmed.trim_start_matches('#').trim();
                    output.push_str(&format!("<h{level}>{}</h{level}>\n", self.render_inline(content)));
                    continue;
                }
            }

            // 水平线
            if trimmed.starts_with("---") || trimmed.starts_with("***") || trimmed.starts_with("___") {
                output.push_str("<hr>\n");
                continue;
            }

            // 代码块
            if trimmed.starts_with("```") {
                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                output.push_str(&format!("<pre><code class=\"language-{lang}\">"));
                continue;
            }

            // 引用块
            if let Some(content) = trimmed.strip_prefix('>') {
                output.push_str(&format!("<blockquote>{}</blockquote>\n", self.render_inline(content.trim())));
                continue;
            }

            // 段落
            output.push_str(&format!("<p>{}</p>\n", self.render_inline(trimmed)));
        }

        output
    }

    /// 渲染内联元素。
    fn render_inline(&self, input: &str) -> String {
        let mut output = input.to_string();

        // 粗体
        output = regex_replace(&output, r"\*\*(.+?)\*\*", "<strong>$1</strong>");
        output = regex_replace(&output, r"__(.+?)__", "<strong>$1</strong>");

        // 斜体
        output = regex_replace(&output, r"\*(.+?)\*", "<em>$1</em>");
        output = regex_replace(&output, r"_(.+?)_", "<em>$1</em>");

        // 行内代码
        output = regex_replace(&output, r"`(.+?)`", "<code>$1</code>");

        // 链接
        output = regex_replace(&output, r"\[(.+?)\]\((.+?)\)", "<a href=\"$2\">$1</a>");

        // 图片
        output = regex_replace(&output, r"!\[(.+?)\]\((.+?)\)", "<img src=\"$2\" alt=\"$1\">");

        output
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// 插件管理器。
pub struct PluginManager {
    /// 已注册的插件。
    plugins: Vec<Box<dyn MarkdownPlugin>>,
}

impl PluginManager {
    /// 创建新的插件管理器。
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// 注册插件。
    pub fn register(&mut self, plugin: Box<dyn MarkdownPlugin>) {
        self.plugins.push(plugin);
    }

    /// 获取所有插件。
    pub fn plugins(&self) -> &[Box<dyn MarkdownPlugin>] {
        &self.plugins
    }

    /// 按名称查找插件。
    pub fn find_plugin(&self, name: &str) -> Option<&dyn MarkdownPlugin> {
        self.plugins.iter().find(|p| p.name() == name).map(|p| p.as_ref())
    }

    /// 移除插件。
    pub fn remove(&mut self, name: &str) -> Option<Box<dyn MarkdownPlugin>> {
        if let Some(pos) = self.plugins.iter().position(|p| p.name() == name) {
            Some(self.plugins.remove(pos))
        } else {
            None
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 正则替换辅助函数。
fn regex_replace(input: &str, pattern: &str, replacement: &str) -> String {
    match Regex::new(pattern) {
        Ok(re) => re.replace_all(input, replacement).to_string(),
        Err(_) => input.to_string(),
    }
}

/// 内置插件：代码高亮。
pub struct CodeHighlightPlugin;

impl MarkdownPlugin for CodeHighlightPlugin {
    fn name(&self) -> &str {
        "code-highlight"
    }

    fn render(&self, content: &str) -> Option<String> {
        if content.starts_with("```") {
            let lang = content.trim_start_matches('`').trim();
            Some(format!("<pre><code class=\"language-{lang}\">CODE</code></pre>"))
        } else {
            None
        }
    }
}

/// 内置插件：数学公式。
pub struct MathPlugin;

impl MarkdownPlugin for MathPlugin {
    fn name(&self) -> &str {
        "math"
    }

    fn can_handle(&self, syntax: &str) -> bool {
        syntax.starts_with("$$") || syntax.starts_with("$")
    }

    fn render(&self, content: &str) -> Option<String> {
        if content.starts_with("$$") {
            let math = content.trim_start_matches('$').trim();
            Some(format!("<div class=\"math-block\">{}</div>", math))
        } else if content.starts_with('$') {
            let math = content.trim_start_matches('$').trim();
            Some(format!("<span class=\"math-inline\">{}</span>", math))
        } else {
            None
        }
    }
}

/// 内置插件：任务列表。
pub struct TaskListPlugin;

impl MarkdownPlugin for TaskListPlugin {
    fn name(&self) -> &str {
        "task-list"
    }

    fn can_handle(&self, syntax: &str) -> bool {
        syntax.contains("[ ]") || syntax.contains("[x]")
    }

    fn render(&self, content: &str) -> Option<String> {
        let content = content.replace("[ ]", "<input type=\"checkbox\" disabled>");
        let content = content.replace("[x]", "<input type=\"checkbox\" checked disabled>");
        Some(content)
    }
}
