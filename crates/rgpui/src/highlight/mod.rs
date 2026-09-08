//! 语法高亮 trait 定义。
//!
//! 本模块提供语法高亮的抽象层，允许编辑器组件支持任意语法解析器
//! （tree-sitter、syntect 等），而无需关心具体实现细节。
//!
//! # 架构
//!
//! ```text
//! Highlighter（trait）
//!   ├── tree-sitter 实现
//!   └── syntect 实现（可选）
//!
//! HighlightStyle（样式映射）
//!   └── 语法 token → 渲染样式
//! ```

use std::ops::Range;

use ropey::Rope;

use crate::{App, HighlightStyle, SharedString, theme::ActiveTheme};

/// tree-sitter 高亮/折叠后端（Rust 单语言）。
///
/// 编译门控：`--features tree-sitter`（默认关闭），wasm 目标下不编译。
#[cfg(all(not(target_family = "wasm"), feature = "tree-sitter"))]
pub mod tree_sitter;

#[cfg(all(not(target_family = "wasm"), feature = "tree-sitter"))]
pub use tree_sitter::{TreeSitterHighlighter, rust_highlighter};

/// 语法高亮 trait。
///
/// 实现此 trait 即可为编辑器提供语法高亮能力。
/// 每种语言对应一个高亮器实现。
pub trait Highlighter {
    /// 获取语言名称（如 "rust"、"javascript"）。
    fn language(&self) -> SharedString;

    /// 更新高亮状态。
    ///
    /// 当文档内容变更时调用，用于增量解析。
    ///
    /// # 参数
    /// * `edit` - 文本编辑信息（插入/删除范围）
    /// * `text` - 完整文档内容
    /// * `folding` - 是否启用代码折叠
    /// * `window` - 窗口引用
    /// * `cx` - 应用上下文
    fn update(
        &mut self,
        edit: Option<TextEdit>,
        text: &Rope,
        folding: bool,
        window: &mut crate::Window,
        cx: &mut App,
    );

    /// 获取指定范围的高亮样式。
    ///
    /// 返回有序、不重叠的样式运行，完全覆盖指定范围。
    /// 无语义样式的文本使用 `HighlightStyle::default()`。
    ///
    /// # 参数
    /// * `range` - 字节范围
    /// * `resolver` - 样式解析器
    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)>;

    /// 获取代码折叠范围。
    fn fold_ranges(&self, text: &Rope) -> Vec<FoldRange>;

    /// 获取代码折叠范围（增量）。
    fn fold_ranges_for_edit(&self, range: Range<usize>, text: &Rope) -> Vec<FoldRange> {
        let _ = range;
        self.fold_ranges(text)
    }

    /// 获取文档符号大纲（函数/结构体/枚举等顶层条目，供跳转与面包屑）。
    ///
    /// 默认空实现；tree-sitter 后端按语法树返回。范围为 UTF-8 字节偏移。
    fn document_symbols(&self, _text: &Rope) -> Vec<DocumentSymbol> {
        Vec::new()
    }
}

/// 文本编辑信息。
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// 编辑前的范围。
    pub old_range: Range<usize>,
    /// 编辑后的新文本长度。
    pub new_len: usize,
}

/// 代码折叠范围。
#[derive(Debug, Clone)]
pub struct FoldRange {
    /// 折叠起始行号（0-based）。
    pub start: usize,
    /// 折叠结束行号（0-based）。
    pub end: usize,
    /// 是否默认折叠。
    pub default_folded: bool,
}

/// 文档符号种类（大纲/面包屑展示用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// 函数（含关联函数，暂不区分方法）。
    Function,
    /// 结构体。
    Struct,
    /// 枚举。
    Enum,
    /// Trait。
    Trait,
    /// `impl` 块（名称为被实现的类型，无名时为 `"impl"`）。
    Impl,
    /// 模块。
    Module,
    /// 常量。
    Const,
    /// 静态项。
    Static,
}

/// 文档符号（单个大纲条目）。
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    /// 符号种类。
    pub kind: SymbolKind,
    /// 符号名称。
    pub name: SharedString,
    /// 符号整体范围（UTF-8 字节偏移，含函数体）。
    pub range: Range<usize>,
    /// 符号起始行号（0-based，跳转列表展示用，免去调用方换算）。
    pub start_row: usize,
}

/// 高亮样式解析器 trait。
///
/// 将语义高亮名称（如 "keyword"、"function"）解析为可渲染的样式。
pub trait HighlightStyleResolver: Send + Sync {
    /// 根据名称解析样式。
    fn style(&self, name: &str) -> Option<HighlightStyle>;
}

/// 无高亮样式（默认实现）。
#[derive(Default)]
pub struct NoHighlightStyles;

impl HighlightStyleResolver for NoHighlightStyles {
    fn style(&self, _: &str) -> Option<HighlightStyle> {
        None
    }
}

/// 主题高亮解析器：捕获名（如 `keyword`）→ 当前主题 `SyntaxColors` 样式。
///
/// 供 `Highlighter::styles` 使用，主题切换后重建即可生效。
#[derive(Debug, Clone, Default)]
pub struct ThemeHighlightResolver {
    colors: crate::theme::SyntaxColors,
}

impl ThemeHighlightResolver {
    /// 由主题语法颜色表创建。
    pub fn new(colors: crate::theme::SyntaxColors) -> Self {
        Self { colors }
    }

    /// 由当前应用主题创建。
    pub fn from_app(cx: &App) -> Self {
        Self::new(cx.theme().highlight_theme.style.syntax.clone())
    }
}

impl HighlightStyleResolver for ThemeHighlightResolver {
    fn style(&self, name: &str) -> Option<HighlightStyle> {
        self.colors.style(name)
    }
}

/// 高亮器工厂（注册表用；`Send + Sync` 以便进全局注册表）。
pub type HighlighterFactory = Box<dyn Fn(&str) -> Option<Box<dyn Highlighter>> + Send + Sync>;

/// 全局语言注册表（语言名 → 构造器；后注册覆盖先注册）。
static HIGHLIGHTER_REGISTRY: std::sync::LazyLock<
    std::sync::RwLock<std::collections::HashMap<&'static str, HighlighterFactory>>,
> = std::sync::LazyLock::new(|| std::sync::RwLock::new(std::collections::HashMap::new()));

/// 注册语言高亮器（`EditorState::set_language` 查表用）。
///
/// 新语言三步（grammar 包本身不进 1.2，按需加）：1. 加 `tree-sitter-xxx`
/// 可选依赖 + feature 门；2. 写 `Highlighter` 实现（query 随 grammar 版本保证
/// 有效，抄 `tree_sitter.rs` 的 Rust 实现）；3. 本函数一行注册。
/// wasm 下 tree-sitter 整体不可用，注册了也降级（见 `highlighter_for`）。
pub fn register_highlighter(language: &'static str, factory: HighlighterFactory) {
    if let Ok(mut registry) = HIGHLIGHTER_REGISTRY.write() {
        registry.insert(language, factory);
    }
}

/// 按语言取高亮器（注册表优先 → Rust 内置（tree-sitter feature 门）→ `None`）。
///
/// 未注册静默降级（不 panic、不编译失败；wasm 下 Rust 内置同样不可用，
/// 直接 `None`）。`supported_languages` 是候选名录（文档用），可用性只看本函数。
pub fn highlighter_for(language: &str) -> Option<Box<dyn Highlighter>> {
    if let Ok(registry) = HIGHLIGHTER_REGISTRY.read() {
        if let Some(factory) = registry.get(language) {
            return factory(language);
        }
    }
    #[cfg(all(not(target_family = "wasm"), feature = "tree-sitter"))]
    if language == "rust" {
        return Some(rust_highlighter());
    }
    None
}

/// 支持的语言列表（候选名录：文档/UI 展示用，可用性以 [`highlighter_for`] 为准，
/// 1.2 只实现 Rust，其余语言包不进）。
pub fn supported_languages() -> Vec<&'static str> {
    vec![
        "rust",
        "javascript",
        "typescript",
        "python",
        "go",
        "java",
        "c",
        "cpp",
        "c_sharp",
        "ruby",
        "php",
        "swift",
        "kotlin",
        "scala",
        "html",
        "css",
        "json",
        "yaml",
        "toml",
        "markdown",
        "sql",
        "bash",
        "dockerfile",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试桩高亮器（注册表用）。
    struct StubHighlighter;

    impl Highlighter for StubHighlighter {
        fn language(&self) -> SharedString {
            "test-only-x".into()
        }

        fn update(
            &mut self,
            _edit: Option<TextEdit>,
            _text: &Rope,
            _folding: bool,
            _window: &mut crate::Window,
            _cx: &mut App,
        ) {
        }

        fn styles(
            &self,
            range: &Range<usize>,
            _resolver: &dyn HighlightStyleResolver,
        ) -> Vec<(Range<usize>, HighlightStyle)> {
            vec![(range.clone(), HighlightStyle::default())]
        }

        fn fold_ranges(&self, _text: &Rope) -> Vec<FoldRange> {
            Vec::new()
        }
    }

    /// 未注册语言静默降级（不 panic）。
    #[test]
    fn unknown_language_returns_none() {
        assert!(highlighter_for("cobol-xyz-not-registered").is_none());
    }

    /// 注册后查表命中（后注册覆盖；唯一名避免并行测试串扰）。
    #[test]
    fn registry_override_hits() {
        register_highlighter(
            "test-only-x",
            Box::new(|_| Some(Box::new(StubHighlighter) as Box<dyn Highlighter>)),
        );
        let highlighter = highlighter_for("test-only-x").expect("刚注册必须命中");
        assert_eq!(highlighter.language().to_string(), "test-only-x");
    }

    /// Rust 内置（tree-sitter feature 门控）。
    #[cfg(all(not(target_family = "wasm"), feature = "tree-sitter"))]
    #[test]
    fn rust_builtin_available() {
        let highlighter = highlighter_for("rust").expect("tree-sitter 下 Rust 内置可用");
        assert_eq!(highlighter.language().to_string(), "rust");
    }
}
