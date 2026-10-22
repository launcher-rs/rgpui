mod assertions;
mod marked_text;

pub use assertions::*;
pub use marked_text::*;

#[cfg(feature = "test-support")]
use git2;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// 内部不变量：值为 `None` 说明前提被破坏，必须立刻暴露。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant<T>(value: Option<T>, message: &str) -> T {
    #[allow(clippy::panic)] // 契约校验：JSON 输入非法，必须立刻暴露
    match value {
        Some(value) => value,
        None => panic!("{message}"),
    }
}

/// 根据 JSON 描述创建临时目录树。
pub struct TempTree {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl TempTree {
    pub fn new(tree: serde_json::Value) -> Self {
        let dir = invariant(TempDir::new().ok(), "temp dir must be creatable");
        let path = invariant(
            std::fs::canonicalize(dir.path()).ok(),
            "temp dir must exist",
        );
        write_tree(path.as_path(), tree);

        Self {
            _temp_dir: dir,
            path,
        }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

// 将 JSON 树结构写入文件系统。
fn write_tree(path: &Path, tree: serde_json::Value) {
    use serde_json::Value;
    #[cfg(all(not(target_family = "wasm"), feature = "test-support"))]
    use std::ffi::OsStr;
    use std::fs;

    if let Value::Object(map) = tree {
        for (name, contents) in map {
            let mut path = PathBuf::from(path);
            path.push(name);
            match contents {
                Value::Object(_) => {
                    invariant(fs::create_dir(&path).ok(), "dir must be creatable");

                    #[cfg(all(not(target_family = "wasm"), feature = "test-support"))]
                    if path.file_name() == Some(OsStr::new(".git")) {
                        invariant(
                            git2::Repository::init(path.parent().unwrap_or(Path::new("."))).ok(),
                            "git repo must be creatable",
                        );
                    }

                    write_tree(&path, contents);
                }
                Value::Null => {
                    invariant(fs::create_dir(&path).ok(), "dir must be creatable");
                }
                Value::String(contents) => {
                    invariant(fs::write(&path, contents).ok(), "file must be writable");
                }
                _ => {
                    panic!("JSON 对象必须只包含对象、字符串或 null");
                }
            }
        }
    } else {
        panic!("你必须向此辅助函数传递一个 JSON 对象")
    }
}

/// 生成指定行数、列数并以指定字符开头的示例文本。
pub fn sample_text(rows: usize, cols: usize, start_char: char) -> String {
    let mut text = String::new();
    for row in 0..rows {
        let c: char = (start_char as u32 + row as u32) as u8 as char;
        let mut line = c.to_string().repeat(cols);
        if row < rows - 1 {
            line.push('\n');
        }
        text += &line;
    }
    text
}
