//! 源码映射支持 —— 将编辑器位置映射到源码位置。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::source_map::{SourceMap, SourceLocation};
//!
//! let source_map = SourceMap::new("input.ts", source_code);
//! let location = source_map.get_location(100); // 字节偏移
//! ```

use std::collections::HashMap;

/// 源码位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    /// 行号（1-based）。
    pub line: usize,
    /// 列号（1-based）。
    pub column: usize,
    /// 源文件路径。
    pub source_file: Option<String>,
    /// 原始名称。
    pub name: Option<String>,
}

/// 源码映射。
pub struct SourceMap {
    /// 源文件路径。
    source_file: String,
    /// 源代码行。
    lines: Vec<String>,
    /// 字节偏移到行号的映射。
    byte_to_line: Vec<usize>,
    /// 行号到字节偏移的映射。
    line_to_byte: Vec<usize>,
}

impl SourceMap {
    /// 创建新的源码映射。
    pub fn new(source_file: &str, source: &str) -> Self {
        let lines: Vec<String> = source.lines().map(String::from).collect();
        let mut byte_to_line = Vec::new();
        let mut line_to_byte = Vec::new();

        let mut byte_offset = 0;
        for (line_idx, line) in lines.iter().enumerate() {
            line_to_byte.push(byte_offset);
            for _ in 0..line.len() {
                byte_to_line.push(line_idx);
            }
            byte_offset += line.len();
            // 加上换行符
            byte_to_line.push(line_idx);
            byte_offset += 1;
        }

        Self {
            source_file: source_file.to_string(),
            lines,
            byte_to_line,
            line_to_byte,
        }
    }

    /// 获取指定字节偏移的位置。
    pub fn get_location(&self, byte_offset: usize) -> Option<SourceLocation> {
        if byte_offset >= self.byte_to_line.len() {
            return None;
        }

        let line_idx = self.byte_to_line[byte_offset];
        let line_start = self.line_to_byte.get(line_idx).copied().unwrap_or(0);
        let column = byte_offset - line_start + 1;

        Some(SourceLocation {
            line: line_idx + 1,
            column,
            source_file: Some(self.source_file.clone()),
            name: None,
        })
    }

    /// 获取指定行号的内容。
    pub fn get_line(&self, line: usize) -> Option<&str> {
        self.lines.get(line - 1).map(|s| s.as_str())
    }

    /// 获取总行数。
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 获取源文件路径。
    pub fn source_file(&self) -> &str {
        &self.source_file
    }

    /// 搜索文本并返回位置。
    pub fn search(&self, query: &str) -> Vec<SourceLocation> {
        let mut results = Vec::new();
        for (line_idx, line) in self.lines.iter().enumerate() {
            let mut col = 0;
            while let Some(pos) = line[col..].find(query) {
                results.push(SourceLocation {
                    line: line_idx + 1,
                    column: col + pos + 1,
                    source_file: Some(self.source_file.clone()),
                    name: None,
                });
                col += pos + 1;
            }
        }
        results
    }
}

/// 双向源码映射 —— 支持编译前后的位置转换。
pub struct BidirectionalSourceMap {
    /// 原始源码映射。
    original: SourceMap,
    /// 编译后源码映射。
    generated: SourceMap,
    /// 原始行号到编译后行号的映射。
    original_to_generated: HashMap<usize, usize>,
    /// 编译后行号到原始行号的映射。
    generated_to_original: HashMap<usize, usize>,
}

impl BidirectionalSourceMap {
    /// 创建新的双向源码映射。
    pub fn new(original: SourceMap, generated: SourceMap) -> Self {
        Self {
            original,
            generated,
            original_to_generated: HashMap::new(),
            generated_to_original: HashMap::new(),
        }
    }

    /// 添加行映射。
    pub fn add_mapping(&mut self, original_line: usize, generated_line: usize) {
        self.original_to_generated.insert(original_line, generated_line);
        self.generated_to_original.insert(generated_line, original_line);
    }

    /// 从原始位置获取编译后位置。
    pub fn original_to_generated_location(&self, location: &SourceLocation) -> Option<SourceLocation> {
        let generated_line = self.original_to_generated.get(&(location.line - 1))?;
        Some(SourceLocation {
            line: generated_line + 1,
            column: location.column,
            source_file: Some(self.generated.source_file().to_string()),
            name: location.name.clone(),
        })
    }

    /// 从编译后位置获取原始位置。
    pub fn generated_to_original_location(&self, location: &SourceLocation) -> Option<SourceLocation> {
        let original_line = self.generated_to_original.get(&(location.line - 1))?;
        Some(SourceLocation {
            line: original_line + 1,
            column: location.column,
            source_file: Some(self.original.source_file().to_string()),
            name: location.name.clone(),
        })
    }
}

/// 源码映射构建器。
pub struct SourceMapBuilder {
    /// 源文件路径。
    source_file: String,
    /// 源代码行。
    lines: Vec<String>,
    /// 名称映射。
    names: HashMap<String, usize>,
}

impl SourceMapBuilder {
    /// 创建新的构建器。
    pub fn new(source_file: &str, source: &str) -> Self {
        Self {
            source_file: source_file.to_string(),
            lines: source.lines().map(String::from).collect(),
            names: HashMap::new(),
        }
    }

    /// 添加名称映射。
    pub fn add_name(&mut self, name: &str) -> usize {
        let index = self.names.len();
        self.names.insert(name.to_string(), index);
        index
    }

    /// 构建源码映射。
    pub fn build(self) -> SourceMap {
        SourceMap::new(&self.source_file, &self.lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map_creation() {
        let map = SourceMap::new("test.rs", "line 1\nline 2\nline 3");
        assert_eq!(map.line_count(), 3);
        assert_eq!(map.source_file(), "test.rs");
    }

    #[test]
    fn test_get_location() {
        let map = SourceMap::new("test.txt", "hello\nworld");
        let loc = map.get_location(0).unwrap();
        assert_eq!(loc.line, 1);
        assert_eq!(loc.column, 1);
    }

    #[test]
    fn test_get_line() {
        let map = SourceMap::new("test.rs", "line 1\nline 2\nline 3");
        assert_eq!(map.get_line(1), Some("line 1"));
        assert_eq!(map.get_line(2), Some("line 2"));
        assert_eq!(map.get_line(3), Some("line 3"));
    }

    #[test]
    fn test_search() {
        let map = SourceMap::new("test.rs", "hello world\nfoo bar\nhello baz");
        let results = map.search("hello");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line, 1);
        assert_eq!(results[1].line, 3);
    }
}
