//! tree-sitter 语法高亮/折叠后端（Rust 单语言先行）。
//!
//! 编译门控：`--features tree-sitter`（默认关闭），且 wasm 目标下不编译
//! （tree-sitter 含 C 运行时，见 `crates/rgpui/Cargo.toml` 说明）。
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
const FOLDABLE_KINDS: [&str; 12] = [
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

/// Rust 语言的 tree-sitter 高亮器（`Highlighter` trait 实现）。
pub struct TreeSitterHighlighter {
    language: Language,
    config: HighlightConfiguration,
    engine: tree_sitter_highlight::Highlighter,
    source: String,
    runs: Vec<(Range<usize>, Option<String>)>,
    tree: Option<Tree>,
}

impl TreeSitterHighlighter {
    /// 创建 Rust 高亮器（内置查询随 `tree-sitter-rust` 版本保证有效）。
    pub fn rust() -> Self {
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        let mut config = HighlightConfiguration::new(
            language.clone(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "", // tree-sitter-rust 未提供 locals 查询，传空
        )
        .expect("tree-sitter-rust 内置查询必须有效");
        config.configure(&HIGHLIGHT_NAMES);
        Self {
            language,
            config,
            engine: tree_sitter_highlight::Highlighter::new(),
            source: String::new(),
            runs: Vec::new(),
            tree: None,
        }
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
        let names = self.config.names();
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
                        .and_then(|ix| names.get(*ix))
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
            if end > start + 1 && FOLDABLE_KINDS.contains(&node.kind()) {
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
        "rust".into()
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
            if let Some(kind) = symbol_kind(node.kind()) {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|name| bytes.get(name.start_byte()..name.end_byte()))
                    .and_then(|slice| std::str::from_utf8(slice).ok())
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| {
                        if kind == SymbolKind::Impl {
                            impl_name(&node, bytes)
                        } else {
                            // 残缺代码里名字可能缺失，回退到语法种类名。
                            node.kind().to_string()
                        }
                    });
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

/// 语法节点种类转符号种类（非符号节点返回 `None`）。
fn symbol_kind(kind: &str) -> Option<SymbolKind> {
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
        // 关键字 `fn` 应落在命名运行内。
        let fn_run = highlighter
            .runs
            .iter()
            .find(|(range, _)| range.start == 0 && range.end == 2);
        assert!(fn_run.is_some());
        assert!(fn_run.unwrap().1.is_some());
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
}
