//! tree-sitter 语法高亮/折叠后端（Rust/JSON/TOML，各独立 feature 门）。
//!
//! 编译门控：`--features tree-sitter`（Rust，地基，默认关闭），
//! `--features tree-sitter-json` / `tree-sitter-toml`（各蕴含地基）；
//! wasm 目标下不编译（tree-sitter 含 C 运行时，见 `crates/rgpui/Cargo.toml` 说明）。
//! 策略为全量重解析（tree-sitter 增量足够快，典型文件亚毫秒级）；
//! 超大文件如有卡顿，后续再接 `TextEdit` 做增量。

use std::ops::Range;

use ropey::Rope;
use tree_sitter::{Language, Parser, Tree};
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent};

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
        let mut config = HighlightConfiguration::new(
            language.clone(),
            name,
            highlights_query,
            injections_query,
            locals_query,
        )
        .expect("内置查询必须有效");
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
                let start = node.start_byte().min(source.len());
                let end = node.end_byte().min(source.len()).max(start);
                out.push(DocumentSymbol {
                    kind,
                    name: name.into(),
                    range: start..end,
                    start_row: source[..start].matches('\n').count(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::NoHighlightStyles;

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
