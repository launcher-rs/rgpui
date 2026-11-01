//! 仓库级一致性守卫（CI 与本地 `cargo test -p rgpui` 都会跑）。
//!
//! 三条规则都在对账「清单说存在」与「仓库里真的存在」：
//!
//! 1. [`doctest_fences_do_not_disable_compilation`]：`crates/**/src/**/*.rs` 的文档注释
//!    与 `skills/**/*.md` 里禁止 `ignore` / `no_run` 围栏。doctest 真正会跑的 crate 逐条
//!    校验（确需关闭的写进 [`ALLOWED_FENCES`] 并注明理由）；`[lib] doctest = false` 的
//!    crate 里围栏本来就不编译，改为整包登记到 [`NO_DOCTEST_CRATES`]（同样要理由）。
//! 2. [`showcase_covers_agented_components`]：`AGENTS.md`「组件库索引」的每一行都要在
//!    `examples/showcase/coverage.toml` 登记，且登记的源码模块与示例 bin 都真实存在。
//! 3. [`example_bin_required_features_resolve`]：`examples/*/Cargo.toml` 里 `[[bin]]` 的
//!    `required-features` 必须是本包声明的 feature，引用的 `rgpui/<f>` 必须是 rgpui 的
//!    feature；bin 的 `path` 与静态依赖 features 也一并校验。
//! 4. [`icon_catalog_matches_assets`]：`skills/rgpui/references/widgets.md` 的 `IconName`
//!    清单与 `examples/showcase/src/bin/icons.rs` 必须与 `crates/rgpui/assets/icons/*.svg`
//!    完全一致，防止文档漏写或示例出现空白格。
//!
//! 发布包里这些文件不存在时跳过；存在 `CI` 环境变量时改为失败，避免守卫静默失效。
#![cfg(not(target_family = "wasm"))]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use toml::Value;
use walkdir::WalkDir;

// ==================== 通用工具 ====================

/// 仓库根（`crates/rgpui` 的上两级）：只有同时存在 `crates/` 与 `Cargo.toml` 才认。
fn workspace_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent()?.parent()?.to_path_buf();
    (root.join("crates").is_dir() && root.join("Cargo.toml").is_file()).then_some(root)
}

/// 取仓库根；发布包场景返回 `None`，CI 里缺失则直接失败。
fn repo_root() -> Option<PathBuf> {
    let root = workspace_root();
    if root.is_none() && std::env::var_os("CI").is_some() {
        panic!("CI 中找不到仓库根：crates/rgpui 的上两级应包含 crates/ 与 Cargo.toml");
    }
    root
}

/// 校验守卫输入存在；发布包场景返回 `None`（跳过该守卫），CI 里缺失则直接失败。
fn require(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = root.join(relative);
    if path.exists() {
        Some(path)
    } else if std::env::var_os("CI").is_some() {
        panic!("CI 中缺少仓库级守卫的输入：{relative}");
    } else {
        eprintln!("跳过仓库级守卫：{relative} 不存在（发布包场景）");
        None
    }
}

/// 读 UTF-8 文本，失败时带路径。
fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("读取 {} 失败：{error}", path.display()))
}

/// 相对仓库根的路径（统一正斜杠，便于与白名单/报错对照）。
fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 路径里是否含某个目录名。
fn has_component(path: &Path, name: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str() == name)
}

/// 解析 TOML 文件。
fn parse_toml(path: &Path) -> Result<Value, String> {
    let text = read_text(path)?;
    toml::from_str::<Value>(&text).map_err(|error| format!("解析 {} 失败：{error}", path.display()))
}

/// 把错误列表拼成一条可读的失败信息。
fn join_errors(title: &str, errors: &[String]) -> String {
    let mut report = format!("{title}（{} 处）：", errors.len());
    for error in errors {
        report.push_str("\n  - ");
        report.push_str(error);
    }
    report
}

// ==================== 守卫一：doctest 围栏 ====================

/// 允许关闭编译的围栏白名单：`content_contains` 为空表示放行该文件内所有围栏。
struct AllowedFence {
    /// 相对仓库根的文件路径。
    file: &'static str,
    /// 围栏内首行必须包含的片段（防止误放行同文件的其它围栏）。
    content_contains: &'static str,
    /// 为什么必须关闭编译。
    reason: &'static str,
}

/// doctest 会实际编译、但示例只能以片段形式存在的位置。
const ALLOWED_FENCES: &[AllowedFence] = &[
    AllowedFence {
        file: "crates/rgpui-3d/src/lib.rs",
        content_contains: "use rgpui_3d::Scenix3D;",
        reason: "示例省略了场景与相机构造，无法独立编译",
    },
    AllowedFence {
        file: "crates/rgpui-term/src/terminal_element.rs",
        content_contains: "let element = TerminalElement::new(",
        reason: "片段省略了 terminal / focus handle 的构造",
    },
    AllowedFence {
        file: "crates/rgpui-term/src/terminal_view.rs",
        content_contains: "TerminalBuilder::new(...)",
        reason: "片段用 ... 省略了构造参数",
    },
    AllowedFence {
        file: "crates/rgpui-term/src/theme_manager.rs",
        content_contains: "// Initialize theme manager",
        reason: "片段省略了 ThemeManager 与配置构造",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/gpui_macros.rs",
        content_contains: "pub fn methods::<T: TheTrait + 'static>()",
        reason: "宏 crate 不能依赖 rgpui，示例是展开后的签名片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/icon_named.rs",
        content_contains: "assert_eq!(pascal_case",
        reason: "示例调用宏 crate 内部的私有函数，外部 doctest 无法访问",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/icon_named.rs",
        content_contains: "// 基本用法",
        reason: "宏 crate 不能依赖 rgpui，示例是宏调用片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[derive(Action)]",
        reason: "宏 crate 不能依赖 rgpui，示例是派生片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "impl rgpui::Action for MyAction",
        reason: "宏 crate 不能依赖 rgpui，示例是派生片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[derive(IntoElement)]",
        reason: "宏 crate 不能依赖 rgpui，示例是派生片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[derive(AppContext)]",
        reason: "宏 crate 不能依赖 rgpui，示例是派生片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[derive(VisualContext)]",
        reason: "宏 crate 不能依赖 rgpui，示例是派生片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[rgpui::test]",
        reason: "宏 crate 不能依赖 rgpui，示例是测试宏片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[rgpui::property_test]",
        reason: "宏 crate 不能依赖 rgpui，示例是属性测试片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "#[rgpui::property_test]",
        reason: "宏 crate 不能依赖 rgpui，示例是属性测试片段",
    },
    AllowedFence {
        file: "crates/rgpui-macros/src/rgpui_macros.rs",
        content_contains: "pub fn methods::<T: TheTrait + 'static>()",
        reason: "宏 crate 不能依赖 rgpui，示例是展开后的签名片段",
    },
];

/// 关闭了 doctest、文档里仍保留 `ignore` 围栏的 crate：整包放行，但必须登记理由。
///
/// 这些 crate 的围栏本来就不参与编译，`ignore` 没有隐藏任何问题；真正的风险是
/// 「悄悄关掉 doctest」，所以登记本身就是守卫的一部分。
const NO_DOCTEST_CRATES: &[(&str, &str)] = &[(
    "rgpui",
    "历史文档有大量节选示例；[lib] doctest = false 使 ignore 不隐藏编译。重新启用 doctest 前需先清理这些站点",
)];

/// 一条违规围栏。
struct FenceViolation {
    /// 开围栏所在行号（从 1 开始）。
    line: usize,
    /// 信息串原文。
    info: String,
    /// 围栏内首个非空行。
    first_content: String,
}

/// 正在扫描的围栏。
struct OpenFence {
    ch: char,
    len: usize,
    line: usize,
    info: String,
    first_content: String,
}

/// 扫描一段文本中的围栏代码块；`rust_source` 为真时只把 `///` / `//!` 行当作文档。
fn scan_fences(text: &str, rust_source: bool) -> Vec<FenceViolation> {
    let mut violations = Vec::new();
    let mut open: Option<OpenFence> = None;

    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let raw = raw.trim_end_matches('\r');
        let Some(line) = (if rust_source {
            doc_comment_body(raw)
        } else {
            Some(raw)
        }) else {
            continue;
        };
        let trimmed = line.trim();

        if let Some(fence) = open.as_mut() {
            if is_closing_fence(trimmed, fence.ch, fence.len) {
                if let Some(fence) = open.take() {
                    record_fence(fence, &mut violations);
                }
            } else if fence.first_content.is_empty() && !trimmed.is_empty() {
                fence.first_content = trimmed.to_string();
            }
            continue;
        }

        if let Some((ch, len, info)) = opening_fence(trimmed) {
            open = Some(OpenFence {
                ch,
                len,
                line: line_no,
                info,
                first_content: String::new(),
            });
        }
    }

    // 文件结尾仍未闭合的围栏：按已采集内容判定，避免漏报。
    if let Some(fence) = open {
        record_fence(fence, &mut violations);
    }
    violations
}

/// 信息串里出现 `ignore` / `no_run` 就说明围栏被排除在 doctest 之外。
fn record_fence(fence: OpenFence, violations: &mut Vec<FenceViolation>) {
    let disabled = fence
        .info
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .any(|token| token == "ignore" || token == "no_run");
    if disabled {
        violations.push(FenceViolation {
            line: fence.line,
            info: fence.info,
            first_content: fence.first_content,
        });
    }
}

/// 识别开围栏，返回（围栏字符、长度、信息串）。
fn opening_fence(trimmed: &str) -> Option<(char, usize, String)> {
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = trimmed
        .chars()
        .take_while(|candidate| *candidate == ch)
        .count();
    if len < 3 {
        return None;
    }
    let info = trimmed[len..].trim().to_string();
    Some((ch, len, info))
}

/// 闭围栏必须同字符、不短于开围栏，且其后没有内容。
fn is_closing_fence(trimmed: &str, ch: char, len: usize) -> bool {
    let count = trimmed
        .chars()
        .take_while(|candidate| *candidate == ch)
        .count();
    count >= len && trimmed[count..].trim().is_empty()
}

/// 去掉 `///` / `//!` 前缀并吃掉一个空格，返回文档注释正文。
fn doc_comment_body(raw: &str) -> Option<&str> {
    let trimmed = raw.trim_start();
    for prefix in ["///", "//!"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    None
}

/// 读取 `crates/*/Cargo.toml` 的 `[lib] doctest` 开关（缺省为 true）。
fn crate_doctest_settings(root: &Path) -> Result<BTreeMap<String, bool>, String> {
    let crates_dir = root.join("crates");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&crates_dir)
        .map_err(|error| format!("读取 {} 失败：{error}", crates_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("Cargo.toml").is_file())
        .collect();
    dirs.sort();

    let mut settings = BTreeMap::new();
    for dir in dirs {
        let name = dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("crate 目录名无法转成 UTF-8：{}", dir.display()))?
            .to_string();
        let value = parse_toml(&dir.join("Cargo.toml"))?;
        let doctest = value
            .get("lib")
            .and_then(|lib| lib.get("doctest"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        settings.insert(name, doctest);
    }
    Ok(settings)
}

/// 守卫一主逻辑。
fn check_doctest_fences(root: &Path) -> Result<(), String> {
    let doctest_settings = crate_doctest_settings(root)?;
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    // doctest 关闭的 crate：记录惰性围栏数量，用来核对登记是否还有效。
    let mut inert: BTreeMap<String, usize> = BTreeMap::new();
    let mut errors = Vec::new();

    for (crate_name, doctest_enabled) in &doctest_settings {
        let crate_dir = root.join("crates").join(crate_name);
        for entry in WalkDir::new(&crate_dir) {
            let entry =
                entry.map_err(|error| format!("遍历 {} 失败：{error}", crate_dir.display()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs")
                || !has_component(rel, "src")
            {
                continue;
            }
            if *doctest_enabled {
                files.push((path.to_path_buf(), true));
            } else {
                let count = scan_fences(&read_text(path)?, true).len();
                if count > 0 {
                    *inert.entry(crate_name.clone()).or_default() += count;
                }
            }
        }
    }
    for entry in WalkDir::new(root.join("skills")) {
        let entry = entry.map_err(|error| format!("遍历 skills 失败：{error}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push((path.to_path_buf(), false));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    // 关闭 doctest 的 crate 必须登记理由，且登记不能过期。
    let registered: BTreeSet<&str> = NO_DOCTEST_CRATES
        .iter()
        .map(|(crate_name, _)| *crate_name)
        .collect();
    for (crate_name, reason) in NO_DOCTEST_CRATES {
        match doctest_settings.get(*crate_name) {
            None => errors.push(format!(
                "NO_DOCTEST_CRATES 登记了不存在的 crate：{crate_name}"
            )),
            Some(true) => errors.push(format!(
                "NO_DOCTEST_CRATES 登记了 {crate_name}，但它的 [lib] doctest 已打开；请撤掉登记"
            )),
            Some(false) => {
                if reason.trim().is_empty() {
                    errors.push(format!("NO_DOCTEST_CRATES 的 {crate_name} 缺少理由"));
                }
                if !inert.contains_key(*crate_name) {
                    errors.push(format!(
                        "NO_DOCTEST_CRATES 的 {crate_name} 文档里已没有 ignore/no_run 围栏；请撤掉登记"
                    ));
                }
            }
        }
    }
    for (crate_name, count) in &inert {
        if !registered.contains(crate_name.as_str()) {
            errors.push(format!(
                "crate {crate_name} 关闭了 doctest，文档里还有 {count} 处 ignore/no_run 围栏；请在 NO_DOCTEST_CRATES 登记理由"
            ));
        }
    }

    let mut used_whitelist = vec![false; ALLOWED_FENCES.len()];
    for (path, rust_source) in files {
        let text = read_text(&path)?;
        let rel = relative_string(root, &path);
        for violation in scan_fences(&text, rust_source) {
            // 优先匹配尚未使用的条目：同一文件里可以有多条相同片段的围栏。
            let allowed = ALLOWED_FENCES
                .iter()
                .enumerate()
                .find(|(index, entry)| {
                    !used_whitelist[*index]
                        && entry.file == rel
                        && (entry.content_contains.is_empty()
                            || violation.first_content.contains(entry.content_contains))
                })
                .map(|(index, _)| index);
            match allowed {
                Some(index) => {
                    used_whitelist[index] = true;
                    if ALLOWED_FENCES[index].reason.trim().is_empty() {
                        errors.push(format!("{rel}:{}：白名单条目缺少理由", violation.line));
                    }
                }
                None => errors.push(format!(
                    "{rel}:{}: ```{}（围栏内首行：{}）",
                    violation.line,
                    violation.info,
                    if violation.first_content.is_empty() {
                        "空"
                    } else {
                        &violation.first_content
                    }
                )),
            }
        }
    }

    for (entry, used) in ALLOWED_FENCES.iter().zip(&used_whitelist) {
        if !used {
            errors.push(format!(
                "{}：白名单条目已失效（没有匹配的 ignore/no_run 围栏，理由：{}）",
                entry.file, entry.reason
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(join_errors(
            "文档围栏禁止用 ```ignore / ```no_run 关闭编译；确需关闭请加入 ALLOWED_FENCES 或 NO_DOCTEST_CRATES 并写明理由",
            &errors,
        ))
    }
}

// ==================== 守卫二：showcase 覆盖表 ====================

/// coverage.toml 的一条登记。
struct CoverageEntry {
    modules: Vec<String>,
    bins: Vec<String>,
}

/// 示例包 → 可用的 bin 名与清单路径。
struct PackageBins {
    manifest: PathBuf,
    bins: BTreeSet<String>,
}

/// 从 AGENTS.md 解析组件索引第一列（去掉括注）。
fn agented_subsystems(text: &str) -> Result<Vec<String>, String> {
    let mut in_section = false;
    let mut names = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### 组件库索引") {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with("`prelude`") {
            break;
        }
        if !trimmed.starts_with('|') {
            continue;
        }
        let first = trimmed
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or_default()
            .trim();
        if first == "子系统" || first.chars().all(|ch| ch == '-' || ch == ':') {
            continue;
        }
        let name = first.split('（').next().unwrap_or(first).trim();
        if !name.is_empty() {
            names.push(name.to_string());
        }
    }

    if names.is_empty() {
        Err("未能从 AGENTS.md 解析出组件库索引表格".to_string())
    } else {
        Ok(names)
    }
}

/// 解析 `examples/showcase/coverage.toml`。
fn parse_coverage(root: &Path) -> Result<BTreeMap<String, CoverageEntry>, String> {
    let path = root.join("examples/showcase/coverage.toml");
    let value = parse_toml(&path)?;
    let components = value
        .get("components")
        .and_then(Value::as_table)
        .ok_or_else(|| format!("{} 缺少 [components] 表", path.display()))?;

    let mut entries = BTreeMap::new();
    for (name, entry) in components {
        let modules = string_array(entry, "modules", &path, name)?;
        let bins = string_array(entry, "bins", &path, name)?;
        entries.insert(name.clone(), CoverageEntry { modules, bins });
    }
    Ok(entries)
}

/// 取表里的字符串数组字段，并校验非空。
fn string_array(
    entry: &Value,
    field: &str,
    path: &Path,
    name: &str,
) -> Result<Vec<String>, String> {
    let array = entry.get(field).and_then(Value::as_array).ok_or_else(|| {
        format!(
            "{} 的 [components.\"{name}\"] 缺少 {field} 数组",
            path.display()
        )
    })?;
    if array.is_empty() {
        return Err(format!(
            "{} 的 [components.\"{name}\"].{field} 不能为空",
            path.display()
        ));
    }
    array
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                format!(
                    "{} 的 [components.\"{name}\"].{field} 含非字符串项",
                    path.display()
                )
            })
        })
        .collect()
}

/// 扫描 `examples/*/Cargo.toml`，收集包名与 bin 名（兼容 autobins）。
fn example_packages(root: &Path) -> Result<BTreeMap<String, PackageBins>, String> {
    let examples_dir = root.join("examples");
    let mut manifests: Vec<PathBuf> = fs::read_dir(&examples_dir)
        .map_err(|error| format!("读取 {} 失败：{error}", examples_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();

    let mut packages = BTreeMap::new();
    for manifest in manifests {
        let value = parse_toml(&manifest)?;
        let name = value
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} 缺少 package.name", manifest.display()))?
            .to_string();

        let mut bins = BTreeSet::new();
        if let Some(entries) = value.get("bin").and_then(Value::as_array) {
            for bin in entries {
                if let Some(bin_name) = bin.get("name").and_then(Value::as_str) {
                    bins.insert(bin_name.to_string());
                }
            }
        }

        let autobins = value
            .get("autobins")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if autobins {
            let package_dir = manifest
                .parent()
                .ok_or_else(|| format!("{} 没有父目录", manifest.display()))?;
            if package_dir.join("src/main.rs").is_file() {
                bins.insert(name.clone());
            }
            if let Ok(entries) = fs::read_dir(package_dir.join("src/bin")) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                            bins.insert(stem.to_string());
                        }
                    }
                }
            }
        }

        packages.insert(name, PackageBins { manifest, bins });
    }
    Ok(packages)
}

/// 在文本里找一段片段所在行号（1 起），用于报错定位。
fn find_line(text: &str, needle: &str) -> Option<usize> {
    text.lines()
        .position(|line| line.contains(needle))
        .map(|index| index + 1)
}

/// 守卫二主逻辑。
fn check_showcase_coverage(root: &Path) -> Result<(), String> {
    let agents_path = root.join("AGENTS.md");
    let agents = read_text(&agents_path)?;
    let subsystems = agented_subsystems(&agents)?;
    let coverage_path = root.join("examples/showcase/coverage.toml");
    let coverage_text = read_text(&coverage_path)?;
    let entries = parse_coverage(root)?;
    let packages = example_packages(root)?;

    let subsystem_set: BTreeSet<&str> = subsystems.iter().map(String::as_str).collect();
    let mut errors = Vec::new();

    for name in &subsystems {
        if !entries.contains_key(name) {
            let location = find_line(&agents, &format!("| {name}"))
                .map(|line| format!("（AGENTS.md:{line}）"))
                .unwrap_or_default();
            errors.push(format!(
                "AGENTS.md 组件索引「{name}」{location}未在 examples/showcase/coverage.toml 登记"
            ));
        }
    }
    for name in entries.keys() {
        if !subsystem_set.contains(name.as_str()) {
            let location = find_line(&coverage_text, &format!("[components.\"{name}\"]"))
                .map(|line| format!("（coverage.toml:{line}）"))
                .unwrap_or_default();
            errors.push(format!(
                "coverage.toml 的「{name}」{location}在 AGENTS.md 组件索引里不存在"
            ));
        }
    }

    for (name, entry) in &entries {
        let location = find_line(&coverage_text, &format!("[components.\"{name}\"]"))
            .map(|line| format!("coverage.toml:{line}"))
            .unwrap_or_else(|| "coverage.toml".to_string());

        for module in &entry.modules {
            if !root.join(module).exists() {
                errors.push(format!(
                    "{location}「{name}」登记的模块路径不存在：{module}"
                ));
            }
        }

        for spec in &entry.bins {
            let Some((package, bin)) = spec.split_once(':') else {
                errors.push(format!(
                    "{location}「{name}」的 bin 应写成 <package>:<bin>，实际为 {spec}"
                ));
                continue;
            };
            match packages.get(package) {
                Some(package_bins) => {
                    if !package_bins.bins.contains(bin) {
                        errors.push(format!(
                            "{location}「{name}」引用不存在的 bin：{spec}（{} 未登记该 [[bin]]，src/bin/{bin}.rs 也不存在）",
                            relative_string(root, &package_bins.manifest)
                        ));
                    }
                }
                None => errors.push(format!("{location}「{name}」引用未知的示例包：{package}")),
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(join_errors(
            "showcase 覆盖表与仓库不一致；组件索引每一行都要登记源码模块与示例 bin",
            &errors,
        ))
    }
}

// ==================== 守卫三：required-features ====================

/// rgpui 包声明的 feature 名集合。
fn rgpui_features(root: &Path) -> Result<BTreeSet<String>, String> {
    let path = root.join("crates/rgpui/Cargo.toml");
    let value = parse_toml(&path)?;
    let features = value
        .get("features")
        .and_then(Value::as_table)
        .ok_or_else(|| format!("{} 缺少 [features] 表", path.display()))?;
    Ok(features.keys().cloned().collect())
}

/// 从 `rgpui/<f>` 或 `rgpui?/<f>` 里取出 `<f>`。
fn split_rgpui_feature(entry: &str) -> Option<&str> {
    entry
        .strip_prefix("rgpui/")
        .or_else(|| entry.strip_prefix("rgpui?/"))
}

/// 取依赖声明里的静态 features。
fn dependency_features(value: &Value, dependency: &str) -> Vec<String> {
    value
        .get("dependencies")
        .and_then(Value::as_table)
        .and_then(|table| table.get(dependency))
        .and_then(|entry| entry.get("features"))
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// 守卫三主逻辑。
fn check_required_features(root: &Path) -> Result<(), String> {
    let known = rgpui_features(root)?;
    let examples_dir = root.join("examples");
    let mut manifests: Vec<PathBuf> = fs::read_dir(&examples_dir)
        .map_err(|error| format!("读取 {} 失败：{error}", examples_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();

    let mut errors = Vec::new();
    for manifest in manifests {
        let text = read_text(&manifest)?;
        let relative = relative_string(root, &manifest);
        let value = parse_toml(&manifest)?;
        let features: BTreeSet<String> = value
            .get("features")
            .and_then(Value::as_table)
            .map(|table| table.keys().cloned().collect())
            .unwrap_or_default();

        // 1) 本包 [features] 里引用的 rgpui/<f> 必须真实存在。
        if let Some(table) = value.get("features").and_then(Value::as_table) {
            for (feature, definition) in table {
                let location = find_line(&text, &format!("{feature} ="))
                    .map(|line| format!("{relative}:{line}"))
                    .unwrap_or_else(|| relative.clone());
                let entries = match definition.as_array() {
                    Some(entries) => entries,
                    None => {
                        errors.push(format!("{location}：feature {feature} 不是字符串数组"));
                        continue;
                    }
                };
                for entry in entries.iter().filter_map(Value::as_str) {
                    if let Some(target) = split_rgpui_feature(entry) {
                        if !known.contains(target) {
                            errors.push(format!(
                                "{location}：feature {feature} 引用不存在的 rgpui feature：{entry}"
                            ));
                        }
                    }
                }
            }
        }

        // 2) rgpui 依赖静态 features 也要存在。
        for feature in dependency_features(&value, "rgpui") {
            if !known.contains(&feature) {
                errors.push(format!(
                    "{relative}：rgpui 依赖声明了不存在的 feature：{feature}"
                ));
            }
        }

        // 3) 每个 [[bin]] 的 required-features 必须是本包 feature，path 必须存在。
        let Some(bins) = value.get("bin").and_then(Value::as_array) else {
            continue;
        };
        let package_dir = manifest
            .parent()
            .ok_or_else(|| format!("{} 没有父目录", manifest.display()))?;
        for bin in bins {
            let name = bin
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<未命名>");
            let location = find_line(&text, &format!("name = \"{name}\""))
                .map(|line| format!("{relative}:{line}"))
                .unwrap_or_else(|| relative.clone());

            if let Some(path) = bin.get("path").and_then(Value::as_str) {
                if !package_dir.join(path).is_file() {
                    errors.push(format!("{location}：bin {name} 的 path 不存在：{path}"));
                }
            }

            let Some(required) = bin.get("required-features").and_then(Value::as_array) else {
                continue;
            };
            for feature in required.iter().filter_map(Value::as_str) {
                if !features.contains(feature) {
                    errors.push(format!(
                        "{location}：bin {name} 的 required-features 引用了本包未声明的 feature：{feature}"
                    ));
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(join_errors(
            "示例清单的 feature 门控无法解析；required-features 必须指向本包声明且能落到 rgpui 的 feature",
            &errors,
        ))
    }
}

// ==================== 守卫四：内置图标清单 ====================

/// 图标文件名 → `IconName` 变体名（与 `rgpui-macros` 的 `icon_named!` 规则一致）。
fn icon_variant_name(filename: &str) -> String {
    filename
        .strip_suffix(".svg")
        .unwrap_or(filename)
        .split(|ch: char| ch == '-' || ch == '_' || ch == '.')
        .filter(|part| !part.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) if first.is_ascii_digit() => word.to_string(),
                Some(first) => {
                    let mut result = String::with_capacity(word.len());
                    result.extend(first.to_uppercase());
                    result.push_str(&chars.as_str().to_lowercase());
                    result
                }
            }
        })
        .collect()
}

/// 收集文本里所有 `IconName::Variant` 的变体名（文档与示例共用同一扫描口径）。
fn referenced_icon_variants(text: &str) -> BTreeSet<String> {
    let marker = "IconName::";
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(index) = rest.find(marker) {
        let tail = &rest[index + marker.len()..];
        let end = tail
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .unwrap_or(tail.len());
        if end > 0 {
            names.insert(tail[..end].to_string());
        }
        rest = &tail[end..];
    }
    names
}

/// 依据 `crates/rgpui/assets/icons/*.svg` 生成期望的变体名集合。
fn expected_icon_variants(root: &Path) -> Result<BTreeSet<String>, String> {
    let dir = root.join("crates/rgpui/assets/icons");
    let entries =
        fs::read_dir(&dir).map_err(|error| format!("读取 {} 失败：{error}", dir.display()))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取 {} 条目失败：{error}", dir.display()))?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".svg") {
            names.insert(icon_variant_name(&filename));
        }
    }
    if names.is_empty() {
        return Err(format!("{} 下没有 SVG 图标", dir.display()));
    }
    Ok(names)
}

/// 对账一份清单文件与实际资产，缺漏与多余都记入错误列表。
fn compare_icon_list(
    label: &str,
    path: &Path,
    expected: &BTreeSet<String>,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    let found = referenced_icon_variants(&read_text(path)?);
    let missing: Vec<&String> = expected.difference(&found).collect();
    let extra: Vec<&String> = found.difference(expected).collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|name| name.as_str()).collect();
        errors.push(format!(
            "{label}（{}）缺少 {} 个变体：{}",
            path.display(),
            missing.len(),
            names.join(", ")
        ));
    }
    if !extra.is_empty() {
        let names: Vec<&str> = extra.iter().map(|name| name.as_str()).collect();
        errors.push(format!(
            "{label}（{}）引用了不存在的 {} 个变体：{}",
            path.display(),
            extra.len(),
            names.join(", ")
        ));
    }
    Ok(())
}

/// 校验技能文档与 icons 示例覆盖全部内置 SVG 图标。
fn check_icon_catalog(root: &Path) -> Result<(), String> {
    let expected = expected_icon_variants(root)?;
    let mut errors = Vec::new();
    compare_icon_list(
        "技能文档 Icon 清单",
        &root.join("skills/rgpui/references/widgets.md"),
        &expected,
        &mut errors,
    )?;
    compare_icon_list(
        "icons 示例",
        &root.join("examples/showcase/src/bin/icons.rs"),
        &expected,
        &mut errors,
    )?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(join_errors("内置图标清单与资产不一致", &errors))
    }
}

// ==================== 测试入口 ====================

/// 守卫一：只允许能编译的 doctest 围栏。
#[test]
fn doctest_fences_do_not_disable_compilation() {
    let Some(root) = repo_root() else { return };
    if require(&root, "crates").is_none() || require(&root, "skills").is_none() {
        return;
    }
    if let Err(report) = check_doctest_fences(&root) {
        panic!("{report}");
    }
}

/// 守卫二：AGENTS.md 组件索引 ↔ showcase 覆盖表 ↔ 实际模块和 bin。
#[test]
fn showcase_covers_agented_components() {
    let Some(root) = repo_root() else { return };
    if require(&root, "AGENTS.md").is_none()
        || require(&root, "examples/showcase/coverage.toml").is_none()
        || require(&root, "examples").is_none()
    {
        return;
    }
    if let Err(report) = check_showcase_coverage(&root) {
        panic!("{report}");
    }
}

/// 守卫三：examples 的 required-features 必须可解析到本包与 rgpui。
#[test]
fn example_bin_required_features_resolve() {
    let Some(root) = repo_root() else { return };
    if require(&root, "crates/rgpui/Cargo.toml").is_none() || require(&root, "examples").is_none() {
        return;
    }
    if let Err(report) = check_required_features(&root) {
        panic!("{report}");
    }
}

/// 守卫四：技能文档与 icons 示例覆盖全部内置 SVG 图标。
#[test]
fn icon_catalog_matches_assets() {
    let Some(root) = repo_root() else { return };
    if require(&root, "crates/rgpui/assets/icons").is_none()
        || require(&root, "skills/rgpui/references/widgets.md").is_none()
        || require(&root, "examples/showcase/src/bin/icons.rs").is_none()
    {
        return;
    }
    if let Err(report) = check_icon_catalog(&root) {
        panic!("{report}");
    }
}

/// 自测：围栏扫描器要能区分可编译围栏、关闭编译围栏与非 doctest 代码。
#[test]
fn fence_scanner_recognizes_compiled_fences() {
    let rust = "/// ```rust\n/// let x = 1;\n/// ```\n/// ```rust,no_run\n/// x;\n/// ```\n/// ```ignore\n/// x;\n/// ```\n";
    let found = scan_fences(rust, true);
    assert_eq!(found.len(), 2, "应只报出 no_run 与 ignore 两条");
    assert_eq!(found[0].line, 4);
    assert_eq!(found[1].line, 7);
    assert!(
        scan_fences("// ```ignore\n", true).is_empty(),
        "普通注释不是 doctest"
    );
    assert!(
        scan_fences("```mermaid\ngraph TD;\n```\n", false).is_empty(),
        "普通 markdown 围栏不应误报"
    );
    assert_eq!(scan_fences("~~~~no_run\nfn f() {}\n~~~~\n", false).len(), 1);
}
