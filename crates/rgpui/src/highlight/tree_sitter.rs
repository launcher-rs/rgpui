//! tree-sitter 语法高亮/折叠后端（Rust/JSON/TOML + 多语言包，各独立 feature 门）。
//!
//! 编译门控：`--features tree-sitter`（Rust，地基，默认关闭），
//! `--features tree-sitter-json` / `tree-sitter-toml` / `tree-sitter-javascript` /
//! `tree-sitter-typescript` / `tree-sitter-python` / `tree-sitter-bash` /
//! `tree-sitter-go` / `tree-sitter-c` / `tree-sitter-cpp` / `tree-sitter-c-sharp` /
//! `tree-sitter-java` / `tree-sitter-ruby` / `tree-sitter-php` / `tree-sitter-css` /
//! `tree-sitter-html` / `tree-sitter-yaml`（各蕴含地基）；
//! wasm 目标下不编译（tree-sitter 含 C 运行时，见 `crates/rgpui/Cargo.toml` 说明）。
//! 策略为全量重解析（tree-sitter 增量足够快，典型文件亚毫秒级）；
//! 超大文件如有卡顿，后续再接 `TextEdit` 做增量。

use std::ops::Range;

use ropey::Rope;
use tree_sitter::{Language, Parser, Tree};
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent};

/// 内部不变量：结果为 `Err` 说明前提被破坏（内嵌高亮查询构建期必有效），必须立刻暴露。
///
/// 统一走这里而不是散落的 `unwrap`：语义与原先一致（同样 panic），但把「这是不变量
/// 断言」的意图写进调用点，模块级 `deny` 仍能拦住随手写的 unwrap。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant_result<T, E>(result: Result<T, E>, message: &str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("{message}"),
    }
}

use crate::highlight::{
    DocumentSymbol, FoldRange, HighlightStyle, HighlightStyleResolver, Highlighter, SymbolKind,
    TextEdit,
};
use crate::theme::highlight::HIGHLIGHT_NAMES;
use crate::{App, SharedString};

/// 可折叠的 Rust 语法节点（多行才折叠）。
const RUST_FOLDABLE_KINDS: [&str; 12] = [
    "function_item",
    "struct_item",
    "enum_item",
    "impl_item",
    "mod_item",
    "trait_item",
    "block",
    "match_block",
    "array",
    "arguments",
    "parameters",
    "field_declaration_list",
];

/// 可折叠的 JSON 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-json")]
const JSON_FOLDABLE_KINDS: [&str; 2] = ["object", "array"];

/// 可折叠的 TOML 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-toml")]
const TOML_FOLDABLE_KINDS: [&str; 3] = ["table", "table_array_element", "array"];

/// 可折叠的 JavaScript 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-javascript")]
const JAVASCRIPT_FOLDABLE_KINDS: [&str; 8] = [
    "statement_block",
    "object",
    "array",
    "class_body",
    "switch_body",
    "arguments",
    "formal_parameters",
    "template_substitution",
];

/// 可折叠的 TypeScript 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-typescript")]
const TYPESCRIPT_FOLDABLE_KINDS: [&str; 10] = [
    "statement_block",
    "object",
    "array",
    "class_body",
    "interface_body",
    "enum_body",
    "switch_body",
    "arguments",
    "formal_parameters",
    "type_parameters",
];

/// 可折叠的 Python 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-python")]
const PYTHON_FOLDABLE_KINDS: [&str; 4] =
    ["block", "parameters", "argument_list", "parenthesized_list"];

/// 可折叠的 Bash 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-bash")]
const BASH_FOLDABLE_KINDS: [&str; 2] = ["compound_statement", "subshell"];

/// 可折叠的 Go 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-go")]
const GO_FOLDABLE_KINDS: [&str; 5] = [
    "block",
    "struct_type",
    "interface_type",
    "parameter_list",
    "argument_list",
];

/// 可折叠的 C 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-c")]
const C_FOLDABLE_KINDS: [&str; 5] = [
    "compound_statement",
    "struct_specifier",
    "union_specifier",
    "enum_specifier",
    "declaration_list",
];

/// 可折叠的 C++ 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-cpp")]
const CPP_FOLDABLE_KINDS: [&str; 6] = [
    "compound_statement",
    "class_specifier",
    "namespace_definition",
    "template_declaration",
    "field_declaration_list",
    "argument_list",
];

/// 可折叠的 C# 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-c-sharp")]
const C_SHARP_FOLDABLE_KINDS: [&str; 8] = [
    "block",
    "class_declaration",
    "struct_declaration",
    "interface_declaration",
    "enum_declaration",
    "namespace_declaration",
    "constructor_declaration",
    "file_scoped_namespace_declaration",
];

/// 可折叠的 Java 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-java")]
const JAVA_FOLDABLE_KINDS: [&str; 6] = [
    "block",
    "class_body",
    "interface_body",
    "enum_body",
    "constructor_body",
    "argument_list",
];

/// 可折叠的 Ruby 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-ruby")]
const RUBY_FOLDABLE_KINDS: [&str; 6] = [
    "body_statement",
    "class",
    "module",
    "method",
    "do_block",
    "begin",
];

/// 可折叠的 PHP 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-php")]
const PHP_FOLDABLE_KINDS: [&str; 5] = [
    "compound_statement",
    "class_declaration",
    "interface_declaration",
    "trait_declaration",
    "enum_declaration",
];

/// 可折叠的 CSS 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-css")]
const CSS_FOLDABLE_KINDS: [&str; 3] = ["block", "keyframe_block", "media_statement"];

/// 可折叠的 HTML 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-html")]
const HTML_FOLDABLE_KINDS: [&str; 4] = ["element", "script_element", "style_element", "fragment"];

/// 可折叠的 YAML 语法节点（多行才折叠）。
#[cfg(feature = "tree-sitter-yaml")]
const YAML_FOLDABLE_KINDS: [&str; 3] = ["block_mapping", "block_sequence", "block_node"];

/// tree-sitter 高亮器（`Highlighter` trait 实现； per-language 经构造器区分）。
pub struct TreeSitterHighlighter {
    /// 语言名（`language()` 返回）。
    name: &'static str,
    language: Language,
    config: HighlightConfiguration,
    engine: tree_sitter_highlight::Highlighter,
    source: String,
    runs: Vec<(Range<usize>, Option<String>)>,
    tree: Option<Tree>,
    /// 本语言可折叠节点（多行才折叠）。
    foldable: &'static [&'static str],
    /// 语法节点种类转符号种类（非符号节点返回 `None`）。
    symbol_kind: fn(&str) -> Option<SymbolKind>,
    /// 符号名提取（字节切片；取不到返回 `None`，调用方回退种类名）。
    symbol_name: fn(&tree_sitter::Node, &[u8]) -> Option<String>,
}

impl TreeSitterHighlighter {
    /// 通用构造（各语言构造器经此实现；内置查询随 grammar 版本保证有效）。
    fn new(
        name: &'static str,
        language: Language,
        highlights_query: &str,
        injections_query: &str,
        locals_query: &str,
        foldable: &'static [&'static str],
        symbol_kind: fn(&str) -> Option<SymbolKind>,
        symbol_name: fn(&tree_sitter::Node, &[u8]) -> Option<String>,
    ) -> Self {
        // 内嵌高亮查询构建期必有效
        let mut config = invariant_result(
            HighlightConfiguration::new(
                language.clone(),
                name,
                &rewrite_captures(highlights_query),
                &rewrite_captures(injections_query),
                &rewrite_captures(locals_query),
            ),
            "内置查询必须有效",
        );
        config.configure(&HIGHLIGHT_NAMES);
        Self {
            name,
            language,
            config,
            engine: tree_sitter_highlight::Highlighter::new(),
            source: String::new(),
            runs: Vec::new(),
            tree: None,
            foldable,
            symbol_kind,
            symbol_name,
        }
    }

    /// 创建 Rust 高亮器（内置查询随 `tree-sitter-rust` 版本保证有效）。
    pub fn rust() -> Self {
        Self::new(
            "rust",
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "", // tree-sitter-rust 未提供 locals 查询，传空
            &RUST_FOLDABLE_KINDS,
            rust_symbol_kind,
            rust_symbol_name,
        )
    }

    /// 创建 JSON 高亮器（O6；内置查询随 `tree-sitter-json` 版本保证有效）。
    #[cfg(feature = "tree-sitter-json")]
    pub fn json() -> Self {
        Self::new(
            "json",
            tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
            &JSON_FOLDABLE_KINDS,
            json_symbol_kind,
            json_symbol_name,
        )
    }

    /// 创建 TOML 高亮器（O6；内置查询随 `tree-sitter-toml-ng` 版本保证有效）。
    #[cfg(feature = "tree-sitter-toml")]
    pub fn toml() -> Self {
        Self::new(
            "toml",
            tree_sitter_toml_ng::LANGUAGE.into(),
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
            &TOML_FOLDABLE_KINDS,
            toml_symbol_kind,
            toml_symbol_name,
        )
    }

    /// 创建 JavaScript 高亮器（内置查询随 `tree-sitter-javascript` 版本保证有效）。
    #[cfg(feature = "tree-sitter-javascript")]
    pub fn javascript() -> Self {
        Self::new(
            "javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
            &JAVASCRIPT_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 TypeScript 高亮器（内置查询随 `tree-sitter-typescript` 版本保证有效）。
    #[cfg(feature = "tree-sitter-typescript")]
    pub fn typescript() -> Self {
        Self::new(
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_typescript::LOCALS_QUERY,
            &TYPESCRIPT_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 TSX 高亮器（TypeScript + JSX 语法）。
    #[cfg(feature = "tree-sitter-typescript")]
    pub fn tsx() -> Self {
        Self::new(
            "tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_typescript::LOCALS_QUERY,
            &TYPESCRIPT_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 Python 高亮器（内置查询随 `tree-sitter-python` 版本保证有效）。
    #[cfg(feature = "tree-sitter-python")]
    pub fn python() -> Self {
        Self::new(
            "python",
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
            &PYTHON_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 Bash 高亮器（内置查询随 `tree-sitter-bash` 版本保证有效）。
    #[cfg(feature = "tree-sitter-bash")]
    pub fn bash() -> Self {
        Self::new(
            "bash",
            tree_sitter_bash::LANGUAGE.into(),
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
            &BASH_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 Go 高亮器（内置查询随 `tree-sitter-go` 版本保证有效）。
    #[cfg(feature = "tree-sitter-go")]
    pub fn go() -> Self {
        Self::new(
            "go",
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
            &GO_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 C 高亮器（内置查询随 `tree-sitter-c` 版本保证有效）。
    #[cfg(feature = "tree-sitter-c")]
    pub fn c() -> Self {
        Self::new(
            "c",
            tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
            &C_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 C++ 高亮器（内置查询随 `tree-sitter-cpp` 版本保证有效）。
    #[cfg(feature = "tree-sitter-cpp")]
    pub fn cpp() -> Self {
        Self::new(
            "cpp",
            tree_sitter_cpp::LANGUAGE.into(),
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            "",
            "",
            &CPP_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 C# 高亮器（内置查询随 `tree-sitter-c-sharp` 版本保证有效）。
    #[cfg(feature = "tree-sitter-c-sharp")]
    pub fn c_sharp() -> Self {
        Self::new(
            "c_sharp",
            tree_sitter_c_sharp::LANGUAGE.into(),
            tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
            "",
            "",
            &C_SHARP_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 Java 高亮器（内置查询随 `tree-sitter-java` 版本保证有效）。
    #[cfg(feature = "tree-sitter-java")]
    pub fn java() -> Self {
        Self::new(
            "java",
            tree_sitter_java::LANGUAGE.into(),
            tree_sitter_java::HIGHLIGHTS_QUERY,
            "",
            "",
            &JAVA_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 Ruby 高亮器（内置查询随 `tree-sitter-ruby` 版本保证有效）。
    #[cfg(feature = "tree-sitter-ruby")]
    pub fn ruby() -> Self {
        Self::new(
            "ruby",
            tree_sitter_ruby::LANGUAGE.into(),
            tree_sitter_ruby::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_ruby::LOCALS_QUERY,
            &RUBY_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }

    /// 创建 PHP 高亮器（内置查询随 `tree-sitter-php` 版本保证有效）。
    #[cfg(feature = "tree-sitter-php")]
    pub fn php() -> Self {
        Self::new(
            "php",
            tree_sitter_php::LANGUAGE_PHP.into(),
            tree_sitter_php::HIGHLIGHTS_QUERY,
            tree_sitter_php::INJECTIONS_QUERY,
            "",
            &PHP_FOLDABLE_KINDS,
            common_symbol_kind,
            common_symbol_name,
        )
    }
    /// 创建 CSS 高亮器（内置查询随 `tree-sitter-css` 版本保证有效）。
    #[cfg(feature = "tree-sitter-css")]
    pub fn css() -> Self {
        Self::new(
            "css",
            tree_sitter_css::LANGUAGE.into(),
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
            "",
            &CSS_FOLDABLE_KINDS,
            no_symbol_kind,
            no_symbol_name,
        )
    }

    /// 创建 HTML 高亮器（内置查询随 `tree-sitter-html` 版本保证有效）。
    #[cfg(feature = "tree-sitter-html")]
    pub fn html() -> Self {
        Self::new(
            "html",
            tree_sitter_html::LANGUAGE.into(),
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
            "",
            &HTML_FOLDABLE_KINDS,
            no_symbol_kind,
            no_symbol_name,
        )
    }

    /// 创建 YAML 高亮器（内置查询随 `tree-sitter-yaml` 版本保证有效）。
    #[cfg(feature = "tree-sitter-yaml")]
    pub fn yaml() -> Self {
        Self::new(
            "yaml",
            tree_sitter_yaml::LANGUAGE.into(),
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
            &YAML_FOLDABLE_KINDS,
            no_symbol_kind,
            no_symbol_name,
        )
    }

    /// 设置全文并重解析（无 UI 上下文时可单测；`update` 经此实现）。
    pub fn set_source(&mut self, text: &str) {
        self.source = text.to_string();
        self.reparse();
    }

    /// 全量重解析并缓存高亮运行与语法树。
    fn reparse(&mut self) {
        let mut parser = Parser::new();
        if parser.set_language(&self.language).is_err() {
            return;
        }
        self.tree = parser.parse(&self.source, None);
        self.runs.clear();
        let Ok(events) = self
            .engine
            .highlight(&self.config, self.source.as_bytes(), None, |_| None)
        else {
            return;
        };
        // 注意：`Highlight(ix)` 的下标指向 `configure` 传入的 recognized 名表
        //（`HIGHLIGHT_NAMES`），而非 `config.names()`（那是 query 捕获名表，
        // 两张表错配会全成无名运行）。recognized 侧做最长部件匹配，
        // 如 `string.special.key` → `string.special`。
        let mut stack: Vec<usize> = Vec::new();
        for event in events {
            let Ok(event) = event else {
                continue;
            };
            match event {
                HighlightEvent::HighlightStart(Highlight(ix)) => stack.push(ix),
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
                HighlightEvent::Source { start, end } => {
                    let name = stack
                        .last()
                        .and_then(|ix| HIGHLIGHT_NAMES.get(*ix))
                        .map(|s| s.to_string());
                    // 合并相邻同名运行，保持有序不重叠。
                    if let Some((range, last)) = self.runs.last_mut() {
                        if *last == name && range.end == start {
                            range.end = end;
                            continue;
                        }
                    }
                    self.runs.push((start..end, name));
                }
            }
        }
    }

    /// 遍历语法树收集多行可折叠节点。
    fn collect_folds(&self) -> Vec<FoldRange> {
        let Some(ref tree) = self.tree else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            let start = node.start_position().row;
            let end = node.end_position().row;
            if end > start + 1 && self.foldable.contains(&node.kind()) {
                out.push(FoldRange {
                    start,
                    end,
                    default_folded: false,
                });
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
        out.sort_by_key(|r| (r.start, r.end));
        out.dedup_by_key(|r| (r.start, r.end));
        out
    }
}

impl Highlighter for TreeSitterHighlighter {
    fn language(&self) -> SharedString {
        self.name.into()
    }

    fn update(
        &mut self,
        _edit: Option<TextEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut crate::Window,
        _cx: &mut App,
    ) {
        self.set_source(&text.to_string());
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut out = Vec::new();
        let mut cursor = range.start;
        for (run_range, name) in &self.runs {
            if run_range.end <= range.start {
                continue;
            }
            if run_range.start >= range.end {
                break;
            }
            let start = run_range.start.max(range.start);
            let end = run_range.end.min(range.end);
            if cursor < start {
                out.push((cursor..start, HighlightStyle::default()));
            }
            let style = name
                .as_deref()
                .and_then(|name| resolver.style(name))
                .unwrap_or_default();
            out.push((start..end, style));
            cursor = end;
        }
        if cursor < range.end {
            out.push((cursor..range.end, HighlightStyle::default()));
        }
        out
    }

    fn fold_ranges(&self, _text: &Rope) -> Vec<FoldRange> {
        self.collect_folds()
    }

    fn document_symbols(&self, text: &Rope) -> Vec<DocumentSymbol> {
        let Some(ref tree) = self.tree else {
            return Vec::new();
        };
        let source = text.to_string();
        let bytes = source.as_bytes();
        let mut out = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if let Some(kind) = (self.symbol_kind)(node.kind()) {
                let name = (self.symbol_name)(&node, bytes)
                    // 残缺代码里名字可能缺失，回退到语法种类名。
                    .unwrap_or_else(|| node.kind().to_string());
                // tree-sitter 给的是字节偏移，中文下直接 `source[..start]`
                // 可能切在多字节中间而 panic；统一吸附到字符边界。
                let start = source.floor_char_boundary(node.start_byte().min(source.len()));
                let end = source
                    .floor_char_boundary(node.end_byte().min(source.len()))
                    .max(start);
                out.push(DocumentSymbol {
                    kind,
                    name: name.into(),
                    range: start..end,
                    start_row: source.get(..start).unwrap_or("").matches('\n').count(),
                });
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
        out.sort_by_key(|symbol| symbol.range.start);
        out
    }
}

/// 捕获名改写表：各语言查询里的非标准捕获名 → 主题名表（`HIGHLIGHT_NAMES`）里的名字。
///
/// 主题名表按「点分部件」匹配（`string.escape` 能命中 `string.escape.special`），
/// 但裸名（`escape`）命中不了多段名；这里显式改写，避免新语言包出现无色片段。
const CAPTURE_ALIASES: [(&str, &str); 10] = [
    ("escape", "string.escape"),
    ("delimiter", "punctuation.delimiter"),
    ("module", "type"),
    ("module.builtin", "type"),
    ("charset", "keyword"),
    ("import", "keyword"),
    ("keyframes", "keyword"),
    ("media", "keyword"),
    ("namespace", "keyword"),
    ("supports", "keyword"),
];

/// 把查询里的捕获名改写到主题名表（未列出的捕获名按 tree-sitter-highlight 的
/// 前缀匹配规则回退：能命中就着色，命中不了就不着色）。
///
/// 字符串字面量（`"…"`）、字符类（`[…]`）与注释（`;…`）内的 `@` 不是捕获名，
/// 原样保留（CSS 的 `"@media"` 之类）。
fn rewrite_captures(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut chars = query.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // 注释：整行复制。
            ';' => {
                out.push(ch);
                for ch in chars.by_ref() {
                    out.push(ch);
                    if ch == '\n' {
                        break;
                    }
                }
            }
            // 字符串字面量：到配对的引号为止。
            '"' => {
                out.push(ch);
                let mut escaped = false;
                for ch in chars.by_ref() {
                    out.push(ch);
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        break;
                    }
                }
            }
            // 字符类：到配对的右方括号为止。
            '[' => {
                out.push(ch);
                for ch in chars.by_ref() {
                    out.push(ch);
                    if ch == ']' {
                        break;
                    }
                }
            }
            // 捕获名：命中改写表则替换。
            '@' => {
                let rest: String = chars.clone().collect();
                let mut replaced = false;
                for (from, to) in CAPTURE_ALIASES {
                    if !rest.starts_with(from) {
                        continue;
                    }
                    // 后面紧跟标识符字符时说明是别的捕获名（如 `escape` vs `escape.x`）。
                    let is_other_name = rest[from.len()..]
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.');
                    if is_other_name {
                        continue;
                    }
                    out.push('@');
                    out.push_str(to);
                    for _ in 0..from.len() {
                        chars.next();
                    }
                    replaced = true;
                    break;
                }
                if !replaced {
                    out.push('@');
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Rust 语法节点种类转符号种类（非符号节点返回 `None`）。
fn rust_symbol_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_item" => Some(SymbolKind::Function),
        "struct_item" => Some(SymbolKind::Struct),
        "enum_item" => Some(SymbolKind::Enum),
        "trait_item" => Some(SymbolKind::Trait),
        "impl_item" => Some(SymbolKind::Impl),
        "mod_item" => Some(SymbolKind::Module),
        "const_item" => Some(SymbolKind::Const),
        "static_item" => Some(SymbolKind::Static),
        _ => None,
    }
}

/// Rust 符号名：`name` 字段；`impl` 取被实现类型名；其余取不到返回 `None`。
fn rust_symbol_name(node: &tree_sitter::Node, bytes: &[u8]) -> Option<String> {
    if let Some(name) = node
        .child_by_field_name("name")
        .and_then(|name| bytes.get(name.start_byte()..name.end_byte()))
        .and_then(|slice| std::str::from_utf8(slice).ok())
    {
        return Some(name.to_string());
    }
    if node.kind() == "impl_item" {
        return Some(impl_name(node, bytes));
    }
    None
}

/// JSON 语法节点种类转符号种类（键值对为条目）。
#[cfg(feature = "tree-sitter-json")]
fn json_symbol_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "pair" => Some(SymbolKind::Const),
        _ => None,
    }
}

/// JSON 符号名：`key` 字段字符串去引号。
#[cfg(feature = "tree-sitter-json")]
fn json_symbol_name(node: &tree_sitter::Node, bytes: &[u8]) -> Option<String> {
    let key = node.child_by_field_name("key")?;
    let text = bytes.get(key.start_byte()..key.end_byte())?;
    let text = std::str::from_utf8(text).ok()?;
    Some(strip_quotes(text).to_string())
}

/// TOML 语法节点种类转符号种类（表为分组，键值对为条目）。
#[cfg(feature = "tree-sitter-toml")]
fn toml_symbol_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "table" | "table_array_element" => Some(SymbolKind::Module),
        "pair" => Some(SymbolKind::Const),
        _ => None,
    }
}

/// TOML 符号名：表取头部连续键（`[a.b]` → `a.b`），键值对取首键。
#[cfg(feature = "tree-sitter-toml")]
fn toml_symbol_name(node: &tree_sitter::Node, bytes: &[u8]) -> Option<String> {
    const KEY_KINDS: [&str; 3] = ["bare_key", "dotted_key", "quoted_key"];
    // 只看命名子节点（`[`/`=` 等匿名符号跳过，否则首个即 break）。
    let mut cursor = node.walk();
    let mut parts = Vec::new();
    for child in node.named_children(&mut cursor) {
        if !KEY_KINDS.contains(&child.kind()) {
            break;
        }
        let text = bytes.get(child.start_byte()..child.end_byte())?;
        let text = std::str::from_utf8(text).ok()?;
        parts.push(strip_quotes(text).to_string());
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("."))
}

/// 去一层引号（`"…"`/`'…'`；JSON 键恒为双引号，TOML 有单双两种）。
#[cfg(any(feature = "tree-sitter-json", feature = "tree-sitter-toml"))]
fn strip_quotes(text: &str) -> &str {
    let bytes = text.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &text[1..text.len() - 1]
    } else {
        text
    }
}
/// 通用符号种类映射（各语言按 grammar 节点名列举；无对应节点返回 `None`）。
#[cfg(any(
    feature = "tree-sitter-javascript",
    feature = "tree-sitter-typescript",
    feature = "tree-sitter-python",
    feature = "tree-sitter-bash",
    feature = "tree-sitter-go",
    feature = "tree-sitter-c",
    feature = "tree-sitter-cpp",
    feature = "tree-sitter-c-sharp",
    feature = "tree-sitter-java",
    feature = "tree-sitter-ruby",
    feature = "tree-sitter-php",
))]
fn common_symbol_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_declaration"
        | "function_definition"
        | "function_item"
        | "function_signature"
        | "method"
        | "method_definition"
        | "method_declaration"
        | "constructor_declaration"
        | "singleton_method" => Some(SymbolKind::Function),
        "class_declaration" | "class_definition" | "class_specifier" | "class" | "struct_item"
        | "struct_declaration" | "record_declaration" => Some(SymbolKind::Struct),
        "enum_declaration" | "enum_item" | "enum_specifier" => Some(SymbolKind::Enum),
        "interface_declaration" | "trait_item" | "trait_declaration" | "protocol_declaration" => {
            Some(SymbolKind::Trait)
        }
        "impl_item" | "impl_declaration" => Some(SymbolKind::Impl),
        "module"
        | "mod_item"
        | "namespace_definition"
        | "namespace_declaration"
        | "package_declaration" => Some(SymbolKind::Module),
        "const_declaration" | "const_item" | "constant" => Some(SymbolKind::Const),
        "static_item" | "static_declaration" => Some(SymbolKind::Static),
        _ => None,
    }
}

/// 通用符号名：取 `name` 字段文本（各语言 grammar 约定一致）。
#[cfg(any(
    feature = "tree-sitter-javascript",
    feature = "tree-sitter-typescript",
    feature = "tree-sitter-python",
    feature = "tree-sitter-bash",
    feature = "tree-sitter-go",
    feature = "tree-sitter-c",
    feature = "tree-sitter-cpp",
    feature = "tree-sitter-c-sharp",
    feature = "tree-sitter-java",
    feature = "tree-sitter-ruby",
    feature = "tree-sitter-php",
))]
fn common_symbol_name(node: &tree_sitter::Node, bytes: &[u8]) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    let text = bytes.get(name.start_byte()..name.end_byte())?;
    Some(std::str::from_utf8(text).ok()?.to_string())
}

/// 无符号语言（CSS / HTML / YAML）的符号种类（恒为 `None`）。
#[cfg(any(
    feature = "tree-sitter-css",
    feature = "tree-sitter-html",
    feature = "tree-sitter-yaml",
))]
fn no_symbol_kind(_kind: &str) -> Option<SymbolKind> {
    None
}

/// 无符号语言（CSS / HTML / YAML）的符号名（恒为 `None`）。
#[cfg(any(
    feature = "tree-sitter-css",
    feature = "tree-sitter-html",
    feature = "tree-sitter-yaml",
))]
fn no_symbol_name(_node: &tree_sitter::Node, _bytes: &[u8]) -> Option<String> {
    None
}

/// `impl` 块名称：被实现的类型名，取不到时为 `"impl"`。
fn impl_name(node: &tree_sitter::Node, bytes: &[u8]) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" || child.kind() == "generic_type" {
            if let Some(slice) = bytes.get(child.start_byte()..child.end_byte())
                && let Ok(name) = std::str::from_utf8(slice)
            {
                // 泛型取主名（`Foo<T>` → `Foo`）。
                return name.split('<').next().unwrap_or("impl").to_string();
            }
        }
    }
    "impl".to_string()
}

/// 创建 Rust 高亮器（`InputState::set_highlighter` 直接消费）。
pub fn rust_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::rust())
}

/// 创建 JSON 高亮器（O6，`tree-sitter-json` feature 门控）。
#[cfg(feature = "tree-sitter-json")]
pub fn json_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::json())
}

/// 创建 TOML 高亮器（O6，`tree-sitter-toml` feature 门控）。
#[cfg(feature = "tree-sitter-toml")]
pub fn toml_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::toml())
}

/// 创建 JavaScript 高亮器（`tree-sitter-javascript` feature 门控）。
#[cfg(feature = "tree-sitter-javascript")]
pub fn javascript_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::javascript())
}

/// 创建 TypeScript 高亮器（`tree-sitter-typescript` feature 门控）。
#[cfg(feature = "tree-sitter-typescript")]
pub fn typescript_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::typescript())
}

/// 创建 TSX 高亮器（`tree-sitter-typescript` feature 门控）。
#[cfg(feature = "tree-sitter-typescript")]
pub fn tsx_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::tsx())
}

/// 创建 Python 高亮器（`tree-sitter-python` feature 门控）。
#[cfg(feature = "tree-sitter-python")]
pub fn python_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::python())
}

/// 创建 Bash 高亮器（`tree-sitter-bash` feature 门控）。
#[cfg(feature = "tree-sitter-bash")]
pub fn bash_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::bash())
}

/// 创建 Go 高亮器（`tree-sitter-go` feature 门控）。
#[cfg(feature = "tree-sitter-go")]
pub fn go_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::go())
}

/// 创建 C 高亮器（`tree-sitter-c` feature 门控）。
#[cfg(feature = "tree-sitter-c")]
pub fn c_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::c())
}

/// 创建 C++ 高亮器（`tree-sitter-cpp` feature 门控）。
#[cfg(feature = "tree-sitter-cpp")]
pub fn cpp_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::cpp())
}

/// 创建 C# 高亮器（`tree-sitter-c-sharp` feature 门控）。
#[cfg(feature = "tree-sitter-c-sharp")]
pub fn c_sharp_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::c_sharp())
}

/// 创建 Java 高亮器（`tree-sitter-java` feature 门控）。
#[cfg(feature = "tree-sitter-java")]
pub fn java_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::java())
}

/// 创建 Ruby 高亮器（`tree-sitter-ruby` feature 门控）。
#[cfg(feature = "tree-sitter-ruby")]
pub fn ruby_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::ruby())
}

/// 创建 PHP 高亮器（`tree-sitter-php` feature 门控）。
#[cfg(feature = "tree-sitter-php")]
pub fn php_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::php())
}

/// 创建 CSS 高亮器（`tree-sitter-css` feature 门控）。
#[cfg(feature = "tree-sitter-css")]
pub fn css_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::css())
}

/// 创建 HTML 高亮器（`tree-sitter-html` feature 门控）。
#[cfg(feature = "tree-sitter-html")]
pub fn html_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::html())
}

/// 创建 YAML 高亮器（`tree-sitter-yaml` feature 门控）。
#[cfg(feature = "tree-sitter-yaml")]
pub fn yaml_highlighter() -> Box<dyn Highlighter> {
    Box::new(TreeSitterHighlighter::yaml())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::NoHighlightStyles;

    /// 断言高亮器产出了命名运行（语言包可用性检查）。
    fn assert_named_runs(highlighter: &mut TreeSitterHighlighter) {
        assert!(
            highlighter.runs.iter().any(|(_, name)| name.is_some()),
            "{} 未产出命名运行",
            highlighter.language()
        );
    }

    /// 各语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-javascript")]
    #[test]
    fn javascript_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::javascript();
        highlighter.set_source("function add(a, b) {\n  return a + b;\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// TypeScript 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-typescript")]
    #[test]
    fn typescript_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::typescript();
        highlighter.set_source("interface Point {\n  x: number;\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// TSX 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-typescript")]
    #[test]
    fn tsx_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::tsx();
        highlighter.set_source("const App = () => <div>hi</div>;\n");
        assert_named_runs(&mut highlighter);
    }

    /// Python 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-python")]
    #[test]
    fn python_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::python();
        highlighter.set_source("def add(a, b):\n    return a + b\n");
        assert_named_runs(&mut highlighter);
    }

    /// Bash 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-bash")]
    #[test]
    fn bash_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::bash();
        highlighter.set_source("function add() {\n  echo 1\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// Go 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-go")]
    #[test]
    fn go_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::go();
        highlighter.set_source("func add(a int, b int) int {\n\treturn a + b\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// C 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-c")]
    #[test]
    fn c_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::c();
        highlighter.set_source("int add(int a, int b) {\n  return a + b;\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// C++ 语言包：能构造高亮器并产出命名运行。
    ///
    /// C++ 的内置查询较稀疏（关键字表只含 C++ 专有词），样例取含 `class` 的片段。
    #[cfg(feature = "tree-sitter-cpp")]
    #[test]
    fn cpp_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::cpp();
        highlighter.set_source("class Point {\npublic:\n  int x;\n};\n");
        assert_named_runs(&mut highlighter);
    }

    /// C# 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-c-sharp")]
    #[test]
    fn c_sharp_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::c_sharp();
        highlighter.set_source("class Point {\n  public int X;\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// Java 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-java")]
    #[test]
    fn java_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::java();
        highlighter.set_source("class Point {\n  int x;\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// Ruby 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-ruby")]
    #[test]
    fn ruby_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::ruby();
        highlighter.set_source("def add(a, b)\n  a + b\nend\n");
        assert_named_runs(&mut highlighter);
    }

    /// PHP 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-php")]
    #[test]
    fn php_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::php();
        highlighter.set_source("<?php\nfunction add($a, $b) { return $a + $b; }\n");
        assert_named_runs(&mut highlighter);
    }

    /// CSS 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-css")]
    #[test]
    fn css_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::css();
        highlighter.set_source(".app {\n  color: red;\n}\n");
        assert_named_runs(&mut highlighter);
    }

    /// HTML 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-html")]
    #[test]
    fn html_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::html();
        highlighter.set_source("<div class=\"app\">\n  <span>hi</span>\n</div>\n");
        assert_named_runs(&mut highlighter);
    }

    /// YAML 语言包：能构造高亮器并产出命名运行。
    #[cfg(feature = "tree-sitter-yaml")]
    #[test]
    fn yaml_package_highlights() {
        let mut highlighter = TreeSitterHighlighter::yaml();
        highlighter.set_source("name: rgpui\nversion: 1\n");
        assert_named_runs(&mut highlighter);
    }

    /// 捕获名改写：非标准名映射到主题名表，标识符相邻时视为别的捕获名。
    #[test]
    fn capture_aliases_are_rewritten() {
        let rewritten = rewrite_captures("(escape) @escape (x) @escape_seq @module.builtin");
        assert!(rewritten.contains("@string.escape (x)"));
        assert!(rewritten.contains("@escape_seq"));
        assert!(rewritten.contains("@type"));
    }

    /// 关键字被识别为命名运行（非默认样式槽位）。
    #[test]
    fn rust_keyword_is_captured() {
        let mut highlighter = TreeSitterHighlighter::rust();
        highlighter.set_source("fn main() {\n    let x = 1;\n}\n");
        assert!(!highlighter.runs.is_empty());
        // 关键字 `fn` 应落在命名运行内，且按 recognized 名表解析为 `keyword`
        //（`Highlight(ix)` 下标指向名表，不是 query 捕获表）。
        let fn_run = highlighter
            .runs
            .iter()
            .find(|(range, _)| range.start == 0 && range.end == 2);
        assert_eq!(
            fn_run.map(|(_, name)| name.clone()),
            Some(Some("keyword".to_string()))
        );
    }

    /// styles 全覆盖查询区间（含无名间隙的默认样式）。
    #[test]
    fn styles_cover_full_range() {
        let mut highlighter = TreeSitterHighlighter::rust();
        highlighter.set_source("fn main() {}\n");
        let resolver = NoHighlightStyles;
        let covered = highlighter.styles(&(0..13), &resolver);
        assert!(!covered.is_empty());
        assert_eq!(covered.first().unwrap().0.start, 0);
        assert_eq!(covered.last().unwrap().0.end, 13);
        // 有序不重叠。
        for pair in covered.windows(2) {
            assert!(pair[0].0.end <= pair[1].0.start);
        }
    }

    /// 多行函数产生折叠区间。
    #[test]
    fn multiline_function_folds() {
        use ropey::Rope;
        let mut highlighter = TreeSitterHighlighter::rust();
        highlighter.set_source("fn main() {\n    let x = 1;\n    let y = 2;\n}\n");
        let text = Rope::from_str("fn main() {\n    let x = 1;\n    let y = 2;\n}\n");
        let folds = highlighter.fold_ranges(&text);
        assert!(folds.iter().any(|r| r.start == 0 && r.end >= 3));
    }

    /// 函数/结构体/模块被收进大纲（按起始偏移排序）。
    #[test]
    fn rust_items_collected_as_symbols() {
        use ropey::Rope;
        let source = "struct Point {\n    x: f32,\n}\n\nfn main() {\n    println!(\"hi\");\n}\n";
        let mut highlighter = TreeSitterHighlighter::rust();
        highlighter.set_source(source);
        let text = Rope::from_str(source);
        let symbols = highlighter.document_symbols(&text);
        let names: Vec<(String, usize)> = symbols
            .iter()
            .map(|symbol| (symbol.name.to_string(), symbol.start_row))
            .collect();
        assert!(names.contains(&("Point".to_string(), 0)));
        assert!(names.contains(&("main".to_string(), 4)));
        assert!(
            symbols
                .iter()
                .find(|symbol| symbol.name.as_ref() == "main")
                .is_some_and(|symbol| symbol.kind == SymbolKind::Function)
        );
        // 有序。
        for pair in symbols.windows(2) {
            assert!(pair[0].range.start <= pair[1].range.start);
        }
    }

    /// JSON 对象键被收进大纲（去引号，按起始偏移排序）。
    #[cfg(feature = "tree-sitter-json")]
    #[test]
    fn json_keys_collected_as_symbols() {
        use ropey::Rope;
        let source = "{\n  \"name\": \"rgpui\",\n  \"nested\": {\n    \"x\": 1\n  }\n}\n";
        let mut highlighter = TreeSitterHighlighter::json();
        assert_eq!(highlighter.language().to_string(), "json");
        highlighter.set_source(source);
        let text = Rope::from_str(source);
        let symbols = highlighter.document_symbols(&text);
        let names: Vec<(String, usize)> = symbols
            .iter()
            .map(|symbol| (symbol.name.to_string(), symbol.start_row))
            .collect();
        assert!(names.contains(&("name".to_string(), 1)));
        assert!(names.contains(&("nested".to_string(), 2)));
        assert!(names.contains(&("x".to_string(), 3)));
        assert!(
            symbols
                .iter()
                .all(|symbol| symbol.kind == SymbolKind::Const)
        );
        // 多行对象可折叠。
        let folds = highlighter.fold_ranges(&text);
        assert!(folds.iter().any(|r| r.start == 0 && r.end >= 4));
        // 高亮按 recognized 名表解析（上游模糊最长匹配；键与值都落在 `string` 系）。
        let named: Vec<&str> = highlighter
            .runs
            .iter()
            .filter_map(|(_, name)| name.as_deref())
            .collect();
        assert!(named.contains(&"string"));
        assert!(named.contains(&"number"));
    }

    /// TOML 表头与键被收进大纲（表为分组，键为条目）。
    #[cfg(feature = "tree-sitter-toml")]
    #[test]
    fn toml_tables_and_keys_collected_as_symbols() {
        use ropey::Rope;
        let source = "title = \"demo\"\n\n[server]\nhost = \"x\"\n\n[server.tls]\nenabled = true\n";
        let mut highlighter = TreeSitterHighlighter::toml();
        assert_eq!(highlighter.language().to_string(), "toml");
        highlighter.set_source(source);
        let text = Rope::from_str(source);
        let symbols = highlighter.document_symbols(&text);
        let names: Vec<(String, usize)> = symbols
            .iter()
            .map(|symbol| (symbol.name.to_string(), symbol.start_row))
            .collect();
        assert!(names.contains(&("title".to_string(), 0)));
        assert!(names.contains(&("server".to_string(), 2)));
        assert!(names.contains(&("host".to_string(), 3)));
        assert!(names.contains(&("server.tls".to_string(), 5)));
        let table = symbols
            .iter()
            .find(|symbol| symbol.name.as_ref() == "server")
            .unwrap();
        assert_eq!(table.kind, SymbolKind::Module);
        // 高亮按 recognized 名表解析（上游模糊最长匹配；键与值都落在合法主题名）。
        let named: Vec<&str> = highlighter
            .runs
            .iter()
            .filter_map(|(_, name)| name.as_deref())
            .collect();
        assert!(named.contains(&"property"));
        assert!(named.contains(&"string"));
    }
}
