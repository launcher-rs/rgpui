use anyhow::{Context, Result};
use console::style;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// rgpui 依赖仓库地址默认值。
pub const DEFAULT_GIT: &str = "https://github.com/launcher-rs/rgpui.git";
/// rgpui 依赖分支默认值（1.4.0 开发期；发布后应改为 main）。
pub const DEFAULT_BRANCH: &str = "feat/1.4.0";
/// 包名前缀默认值。
pub const DEFAULT_PACKAGE_PREFIX: &str = "com.example.";

/// 配置文件名（项目根目录）。
pub const PROJECT_CONFIG_FILE: &str = "rgpui.toml";

/// 配置三层结构：全局（`~/.config/rgpui/config.toml`）
/// < 项目（`./rgpui.toml`）< CLI 参数。后加载的覆盖先加载的。
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// 项目生成相关配置。
    pub project: ProjectConfig,
    /// Android 构建相关配置。
    pub android: AndroidConfig,
}

/// `[project]` 段：控制 `new` 生成的项目骨架。
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// rgpui git 仓库地址。
    pub git: Option<String>,
    /// rgpui git 分支（或 tag / commit）。
    pub branch: Option<String>,
    /// 生成项目的 applicationId 前缀，如 `com.example.`。
    pub package_prefix: Option<String>,
    /// 生成项目的 Rust edition。
    pub edition: Option<String>,
}

/// `[android]` 段：控制 Android 构建参数。
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AndroidConfig {
    /// 目标 ABI 列表（cargo-ndk -t 与 gradle abiFilters 共用）。
    pub abis: Option<Vec<String>>,
    /// 最低支持 API（gradle minSdk）。
    pub min_sdk: Option<u32>,
    /// 编译 API（gradle compileSdk）。
    pub compile_sdk: Option<u32>,
    /// 目标 API（gradle targetSdk）。
    pub target_sdk: Option<u32>,
    /// cargo-ndk 平台 API（-P 参数）。
    pub platform_api: Option<u32>,
    /// 是否在打包前 strip .so（减小 APK 体积）。
    pub strip: Option<bool>,
}

impl Config {
    /// rgpui git 仓库地址。
    pub fn git(&self) -> &str {
        self.project.git.as_deref().unwrap_or(DEFAULT_GIT)
    }

    /// rgpui git 分支。
    pub fn branch(&self) -> &str {
        self.project.branch.as_deref().unwrap_or(DEFAULT_BRANCH)
    }

    /// 包名前缀。
    pub fn package_prefix(&self) -> &str {
        self.project
            .package_prefix
            .as_deref()
            .unwrap_or(DEFAULT_PACKAGE_PREFIX)
    }

    /// Rust edition。
    pub fn edition(&self) -> &str {
        self.project.edition.as_deref().unwrap_or("2024")
    }

    /// ABI 列表（配置为空时回落到默认 arm64-v8a）。
    pub fn abis(&self) -> Vec<String> {
        match &self.android.abis {
            Some(list) if !list.is_empty() => list.clone(),
            _ => vec!["arm64-v8a".to_string()],
        }
    }

    /// 主 ABI（cargo-ndk 第一个 -t）。
    pub fn primary_abi(&self) -> String {
        self.abis()
            .into_iter()
            .next()
            .unwrap_or_else(|| "arm64-v8a".into())
    }

    /// 最低 API。
    pub fn min_sdk(&self) -> u32 {
        self.android.min_sdk.unwrap_or(26)
    }

    /// 编译 API。
    pub fn compile_sdk(&self) -> u32 {
        self.android.compile_sdk.unwrap_or(34)
    }

    /// 目标 API。
    pub fn target_sdk(&self) -> u32 {
        self.android.target_sdk.unwrap_or(34)
    }

    /// cargo-ndk 平台 API。
    pub fn platform_api(&self) -> u32 {
        self.android.platform_api.unwrap_or(31)
    }

    /// 是否 strip。
    pub fn strip(&self) -> bool {
        self.android.strip.unwrap_or(true)
    }

    /// 展示生效配置（`config show` 子命令）。
    pub fn print_effective(&self, sources: &[(&'static str, bool)]) {
        println!("{}", style("生效配置：").bold().cyan());
        let rows: [(&str, String); 14] = [
            ("project.git", self.git().to_string()),
            ("project.branch", self.branch().to_string()),
            ("project.package_prefix", self.package_prefix().to_string()),
            ("project.edition", self.edition().to_string()),
            ("android.abis", self.abis().join(", ")),
            ("android.min_sdk", self.min_sdk().to_string()),
            ("android.compile_sdk", self.compile_sdk().to_string()),
            ("android.target_sdk", self.target_sdk().to_string()),
            ("android.platform_api", self.platform_api().to_string()),
            ("android.strip", self.strip().to_string()),
            (
                "global",
                global_config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "未找到".into()),
            ),
            (
                "project",
                project_config_path(Path::new("."))
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "未找到".into()),
            ),
            (
                "已加载",
                sources
                    .iter()
                    .filter(|(_, loaded)| *loaded)
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            (
                "— 来源优先级 —",
                "CLI 参数 > 项目 rgpui.toml > 全局 config > 内置默认".into(),
            ),
        ];
        for (key, val) in rows {
            if key.starts_with('—') {
                println!();
            }
            println!("  {:<24} {}", style(key).dim(), val);
        }
    }

    /// 合并：`other` 中的 Some 字段覆盖 `self`。
    pub fn merge(&mut self, other: Config) {
        macro_rules! merge_opt {
            ($dst:expr, $src:expr) => {
                if $src.is_some() {
                    $dst = $src;
                }
            };
        }
        merge_opt!(self.project.git, other.project.git);
        merge_opt!(self.project.branch, other.project.branch);
        merge_opt!(self.project.package_prefix, other.project.package_prefix);
        merge_opt!(self.project.edition, other.project.edition);
        merge_opt!(self.android.abis, other.android.abis);
        merge_opt!(self.android.min_sdk, other.android.min_sdk);
        merge_opt!(self.android.compile_sdk, other.android.compile_sdk);
        merge_opt!(self.android.target_sdk, other.android.target_sdk);
        merge_opt!(self.android.platform_api, other.android.platform_api);
        merge_opt!(self.android.strip, other.android.strip);
    }
}

/// 全局配置文件路径（`$XDG_CONFIG_HOME/rgpui/config.toml` 或平台等价路径）。
pub fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("rgpui").join("config.toml"))
}

/// 项目配置文件路径（`<dir>/rgpui.toml`）。
pub fn project_config_path(dir: &Path) -> Option<PathBuf> {
    let p = dir.join(PROJECT_CONFIG_FILE);
    p.exists().then_some(p)
}

/// 按 全局 < 项目 顺序加载配置并合并。返回合并结果与已加载来源。
pub fn load(dir: &Path) -> Result<(Config, Vec<(&'static str, bool)>)> {
    let mut cfg = Config::default();
    let mut sources = vec![("global", false), ("project", false)];

    if let Some(path) = global_config_path()
        && path.exists()
    {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("读取全局配置失败: {}", path.display()))?;
        let parsed: Config = toml::from_str(&text)
            .with_context(|| format!("解析全局配置失败: {}", path.display()))?;
        cfg.merge(parsed);
        sources[0].1 = true;
    }

    if let Some(path) = project_config_path(dir) {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("读取项目配置失败: {}", path.display()))?;
        let parsed: Config = toml::from_str(&text)
            .with_context(|| format!("解析项目配置失败: {}", path.display()))?;
        cfg.merge(parsed);
        sources[1].1 = true;
    }

    Ok((cfg, sources))
}

/// 生成默认配置文件内容（`config init` 子命令）。写显式默认值而非序列化
/// 全 None 的 Config（后者会产出空表，对用户无参考价值）。
pub fn default_config_toml() -> String {
    format!(
        r##"# rgpui CLI 项目配置（`cargo rgpui config init` 生成）
# 优先级：CLI 参数 > 本文件 > 全局 ~/.config/rgpui/config.toml > 内置默认

[project]
# rgpui 依赖仓库地址
git = "{git}"
# rgpui 依赖分支（发布后改为 main）
branch = "{branch}"
# 生成项目的 applicationId 前缀
package_prefix = "{prefix}"
# 生成项目的 Rust edition
edition = "2024"

[android]
# 目标 ABI（第一个为主 ABI，控制 cargo-ndk -t 与 gradle abiFilters）
abis = ["arm64-v8a"]
# 最低支持 API（rgpui-android 要求 >= 26）
min_sdk = 26
# 编译 / 目标 API
compile_sdk = 34
target_sdk = 34
# cargo-ndk 平台 API（-P 参数）
platform_api = 31
# 打包前 strip .so（显著减小 APK 体积）
strip = true
"##,
        git = DEFAULT_GIT,
        branch = DEFAULT_BRANCH,
        prefix = DEFAULT_PACKAGE_PREFIX,
    )
}
