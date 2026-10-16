use super::{Theme, ThemeColor, ThemeConfig, ThemeMode, ThemeSet};
use crate::theme::highlight::HighlightTheme;
use rgpui::{App, Global, SharedString};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, LazyLock},
    time::Duration,
};

const DEFAULT_THEME: &str = include_str!("./default-theme.json");

/// 主题目录默认相对路径（存在时启动即加载，并启动热重载监视）。
const DEFAULT_THEMES_DIR: &str = "./themes";

/// 主题目录热重载的轮询间隔。
const THEMES_DIR_POLL_INTERVAL: Duration = Duration::from_millis(700);

// 内嵌额外主题表（默认关闭，feature = "bundled-themes" 时启用）。
// 主题来源于原 rgpui-component 的 themes/ 目录，整合为单个 ThemeSet。
#[cfg(feature = "bundled-themes")]
const EXTRA_THEMES: &str = include_str!("./themes/extra-themes.json");
/// 默认主题颜色与高亮主题映射（按亮/暗模式）。
pub static DEFAULT_THEME_COLORS: LazyLock<
    HashMap<ThemeMode, (Arc<ThemeColor>, Arc<HighlightTheme>)>,
> = LazyLock::new(|| {
    let mut colors = HashMap::new();

    let themes: Vec<ThemeConfig> = serde_json::from_str::<ThemeSet>(DEFAULT_THEME)
        .expect("Failed to parse themes/default.json")
        .themes;

    for theme in themes {
        let mut theme_color = ThemeColor::default();
        theme_color.apply_config(&theme, &ThemeColor::default());

        let highlight_theme = HighlightTheme {
            name: theme.name.to_string(),
            appearance: theme.mode,
            style: theme.highlight.unwrap_or_default(),
        };

        colors.insert(
            theme.mode,
            (Arc::new(theme_color), Arc::new(highlight_theme)),
        );
    }

    colors
});

pub(super) fn init(cx: &mut App) {
    cx.set_global(ThemeRegistry::default());
    ThemeRegistry::global_mut(cx).init_default_themes();

    // Observe changes to the theme registry to apply changes to the active theme
    cx.observe_global::<ThemeRegistry>(|cx| {
        let mode = Theme::global(cx).mode;
        let light_theme = Theme::global(cx).light_theme.name.clone();
        let dark_theme = Theme::global(cx).dark_theme.name.clone();

        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&light_theme)
            .cloned()
        {
            Theme::global_mut(cx).light_theme = theme;
        }
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&dark_theme).cloned() {
            Theme::global_mut(cx).dark_theme = theme;
        }

        let theme_name = if mode.is_dark() {
            dark_theme
        } else {
            light_theme
        };

        tracing::info!("Reload active theme: {:?}...", theme_name);
        Theme::change(mode, None, cx);
        cx.refresh_windows();
    })
    .detach();

    // 存在主题目录时立即加载自定义主题，并启动热重载监视。
    let themes_dir = ThemeRegistry::global(cx).themes_dir.clone();
    if themes_dir.is_dir() {
        let loaded = ThemeRegistry::global_mut(cx).load_themes_from_dir(&themes_dir);
        tracing::info!("从 {} 加载 {loaded} 个自定义主题", themes_dir.display());
        spawn_themes_dir_watcher(cx, themes_dir);
    }
}

/// 内部实现：监视主题目录，轮询目录签名，变化时重载主题。
///
/// 重载经 `set_global` 触发 `ThemeRegistry` 的全局观察者，从而重新应用当前主题并
/// 刷新所有窗口。Web 平台无文件系统，此函数为空实现。
#[cfg(target_family = "wasm")]
fn spawn_themes_dir_watcher(_cx: &mut App, _dir: PathBuf) {}

/// 内部实现：监视主题目录，轮询目录签名，变化时重载主题。
#[cfg(not(target_family = "wasm"))]
fn spawn_themes_dir_watcher(cx: &mut App, dir: PathBuf) {
    cx.spawn(async move |cx| {
        let mut last = themes_dir_signature(&dir);
        loop {
            cx.background_executor()
                .timer(THEMES_DIR_POLL_INTERVAL)
                .await;

            let scan_dir = dir.clone();
            let signature = cx
                .background_executor()
                .spawn(async move { themes_dir_signature(&scan_dir) })
                .await;
            if signature == last {
                continue;
            }
            last = signature;

            let load_dir = dir.clone();
            let loaded = cx.update(|cx| {
                // `set_global` 触发全局观察者重新应用主题；先取出注册表再放回。
                let mut registry = std::mem::take(ThemeRegistry::global_mut(cx));
                let loaded = registry.load_themes_from_dir(&load_dir);
                cx.set_global(registry);
                loaded
            });
            tracing::info!("主题目录 {} 变化，重新加载 {loaded} 个主题", dir.display());
        }
    })
    .detach();
}

/// 是否为可加载的主题文件（`*.json`，大小写不敏感）。
fn is_theme_json(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

/// 计算主题目录签名（文件路径 + 修改时间 + 大小），用于变更检测。
#[cfg(not(target_family = "wasm"))]
fn themes_dir_signature(dir: &Path) -> Vec<(PathBuf, Option<std::time::SystemTime>, u64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut signature = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !is_theme_json(&path) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some((path, metadata.modified().ok(), metadata.len()))
        })
        .collect::<Vec<_>>();
    signature.sort();
    signature
}

/// 主题注册表，管理默认主题与自定义主题。
#[derive(Default, Debug)]
pub struct ThemeRegistry {
    themes_dir: PathBuf,
    default_themes: HashMap<ThemeMode, Rc<ThemeConfig>>,
    themes: HashMap<SharedString, Rc<ThemeConfig>>,
    has_custom_themes: bool,
}

impl Global for ThemeRegistry {}

impl ThemeRegistry {
    /// 返回全局主题注册表引用。
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// 返回全局主题注册表可变引用。
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// 启动主题目录热重载监视（监视 [`themes_dir`](Self::themes_dir)）。
    ///
    /// 目录尚不存在时也可以调用，便于运行中创建；创建后新增/修改的 `*.json` 主题
    /// 会被自动加载，并重新应用当前主题、刷新所有窗口。重复调用会启动多个监视任务，
    /// 一般只需调用一次（`theme::init` 在目录已存在时已自动启动）。
    pub fn watch_themes_dir(cx: &mut App) {
        let dir = ThemeRegistry::global(cx).themes_dir.clone();
        spawn_themes_dir_watcher(cx, dir);
    }

    /// Returns a reference to the map of themes (including default themes).
    pub fn themes(&self) -> &HashMap<SharedString, Rc<ThemeConfig>> {
        &self.themes
    }

    /// Returns a sorted list of themes.
    pub fn sorted_themes(&self) -> Vec<&Rc<ThemeConfig>> {
        let mut themes = self.themes.values().collect::<Vec<_>>();
        // sort by is_default true first, then light first dark later, then by name case-insensitive
        themes.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then(a.mode.cmp(&b.mode))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        themes
    }

    /// Returns a reference to the map of default themes.
    pub fn default_themes(&self) -> &HashMap<ThemeMode, Rc<ThemeConfig>> {
        &self.default_themes
    }

    /// 返回默认亮色主题。
    pub fn default_light_theme(&self) -> &Rc<ThemeConfig> {
        &self.default_themes[&ThemeMode::Light]
    }

    /// 返回默认暗色主题。
    pub fn default_dark_theme(&self) -> &Rc<ThemeConfig> {
        &self.default_themes[&ThemeMode::Dark]
    }

    /// 主题目录路径（默认为 `./themes`）。
    pub fn themes_dir(&self) -> &Path {
        &self.themes_dir
    }

    /// 设置主题目录（仅更新路径，不立即加载）。
    pub fn set_themes_dir(&mut self, dir: impl Into<PathBuf>) {
        self.themes_dir = dir.into();
    }

    /// 从 JSON 字符串加载主题集，名称冲突时跳过。
    pub fn load_themes_from_str(&mut self, content: &str) -> anyhow::Result<()> {
        self.load_themes(content, false).map(|_| ())
    }

    /// 从目录加载全部 `*.json` 主题集，同名主题以文件内容覆盖，返回加载数量。
    ///
    /// 解析失败的文件会被跳过并记录警告，不影响其余文件。
    pub fn load_themes_from_dir(&mut self, dir: &Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut files = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| is_theme_json(path))
            .collect::<Vec<_>>();
        files.sort();

        let mut loaded = 0;
        for file in files {
            match std::fs::read_to_string(&file) {
                Ok(content) => match self.load_themes(&content, true) {
                    Ok(count) => loaded += count,
                    Err(error) => {
                        tracing::warn!("跳过无法解析的主题文件 {}：{error}", file.display())
                    }
                },
                Err(error) => tracing::warn!("读取主题文件 {} 失败：{error}", file.display()),
            }
        }
        loaded
    }

    /// 从当前主题目录重新加载（供热重载使用），返回加载数量。
    pub fn reload_themes_from_dir(&mut self) -> usize {
        let dir = self.themes_dir.clone();
        self.load_themes_from_dir(&dir)
    }

    /// 解析主题集并合并到注册表；`overwrite` 为真时同名主题以新内容覆盖。
    fn load_themes(&mut self, content: &str, overwrite: bool) -> anyhow::Result<usize> {
        let theme_set = serde_json::from_str::<ThemeSet>(content)?;
        let mut loaded = 0;
        for theme in theme_set.themes {
            if overwrite || !self.themes.contains_key(&theme.name) {
                let theme_name = theme.name.clone();
                self.themes.insert(theme_name, Rc::new(theme));
                self.has_custom_themes = true;
                loaded += 1;
            }
        }
        Ok(loaded)
    }

    fn init_default_themes(&mut self) {
        let default_themes: Vec<ThemeConfig> = serde_json::from_str::<ThemeSet>(DEFAULT_THEME)
            .expect("failed to parse default theme.")
            .themes;
        for theme in default_themes.into_iter() {
            if theme.mode.is_dark() {
                self.default_themes.insert(ThemeMode::Dark, Rc::new(theme));
            } else {
                self.default_themes.insert(ThemeMode::Light, Rc::new(theme));
            }
        }
        self.themes_dir = PathBuf::from(DEFAULT_THEMES_DIR);
        self.themes = self
            .default_themes
            .values()
            .map(|theme| {
                let name = theme.name.clone();
                (name, Rc::clone(theme))
            })
            .collect();

        #[cfg(feature = "bundled-themes")]
        {
            // 加载内置额外主题，名称冲突时保留已有主题。
            if let Err(e) = self.load_themes_from_str(EXTRA_THEMES) {
                tracing::error!("Failed to load bundled extra themes: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "bundled-themes")]
    #[test]
    fn test_bundled_themes_parse_and_load() {
        // 验证额外主题 JSON 可解析且全部主题加载成功。
        let mut registry = ThemeRegistry::default();
        registry
            .load_themes_from_str(EXTRA_THEMES)
            .expect("bundled extra themes should parse");

        // 37 套额外主题全部加载
        assert_eq!(
            registry.themes().len(),
            37,
            "themes: {:?}",
            registry.themes().keys()
        );
    }

    /// 目录加载：读取 `*.json` 主题集，忽略非 json 文件与子目录。
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn test_load_themes_from_dir_reads_json_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pack.json"),
            r#"{"name":"pack","themes":[
                {"name":"Pack Dark","mode":"dark"},
                {"name":"Pack Light","mode":"light"}
            ]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a theme").unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        let mut registry = ThemeRegistry::default();
        let loaded = registry.load_themes_from_dir(dir.path());

        assert_eq!(loaded, 2);
        assert!(registry.themes().contains_key("Pack Dark"));
        assert!(registry.themes().contains_key("Pack Light"));
    }

    /// 重载：同名主题以文件的新内容覆盖。
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn test_reload_themes_from_dir_overwrites_same_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pack.json");
        std::fs::write(
            &file,
            r#"{"name":"pack","themes":[{"name":"Pack Dark","mode":"dark"}]}"#,
        )
        .unwrap();

        let mut registry = ThemeRegistry::default();
        registry.set_themes_dir(dir.path());
        registry.load_themes_from_dir(dir.path());
        assert_eq!(registry.themes()["Pack Dark"].mode, ThemeMode::Dark);

        // 覆盖为亮色后重载。
        std::fs::write(
            &file,
            r#"{"name":"pack","themes":[{"name":"Pack Dark","mode":"light"}]}"#,
        )
        .unwrap();
        let loaded = registry.reload_themes_from_dir();

        assert_eq!(loaded, 1);
        assert_eq!(registry.themes()["Pack Dark"].mode, ThemeMode::Light);
    }

    /// 解析失败的文件被跳过，不影响同目录其余主题文件。
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn test_load_themes_from_dir_skips_broken_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("broken.json"), "{ not json").unwrap();
        std::fs::write(
            dir.path().join("good.json"),
            r#"{"name":"pack","themes":[{"name":"Only Theme","mode":"light"}]}"#,
        )
        .unwrap();

        let mut registry = ThemeRegistry::default();
        let loaded = registry.load_themes_from_dir(dir.path());

        assert_eq!(loaded, 1);
        assert!(registry.themes().contains_key("Only Theme"));
    }
}
