//! 日志初始化入口。
//!
//! rgpui 及其平台后端（GPU 适配器探测、X11/Wayland 初始化、桌面门户调用）都用
//! `log` facade 打日志，但 `log` 在没有安装输出器时会把所有日志静默丢弃 ——
//! 应用看不到任何诊断信息。本模块提供一个零依赖的 stderr 输出器，
//! 应用在 `main` 的第一行调用 [`init_logging`] 即可拿到日志。

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Log, Metadata, Record};

/// 未设置 `RUST_LOG` 时的默认过滤级别：只保留错误，避免默认输出干扰
const DEFAULT_LEVEL: LevelFilter = LevelFilter::Error;

/// 已生效的过滤规则，初始化后不再变化
static FILTER: OnceLock<Filter> = OnceLock::new();

/// 过滤规则：一个默认级别 + 若干按 target 前缀的覆盖
struct Filter {
    /// 未命中任何覆盖时使用的级别
    default: LevelFilter,
    /// `target 前缀 -> 级别` 覆盖表
    overrides: Vec<(String, LevelFilter)>,
}

impl Filter {
    /// 解析 `RUST_LOG` 风格的过滤串，例如 `info`、`warn,rgpui_wgpu=debug`
    fn parse(spec: &str) -> Self {
        let mut filter = Filter {
            default: DEFAULT_LEVEL,
            overrides: Vec::new(),
        };

        for part in spec
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            match part.split_once('=') {
                Some((target, level)) => {
                    if !target.is_empty()
                        && let Ok(level) = level.parse::<LevelFilter>()
                    {
                        filter.overrides.push((target.to_owned(), level));
                    }
                }
                None => {
                    if let Ok(level) = part.parse::<LevelFilter>() {
                        filter.default = level;
                    }
                }
            }
        }

        filter
    }

    /// 取某个 target 应使用的级别：前缀最长的一条覆盖优先
    fn level_for(&self, target: &str) -> LevelFilter {
        let mut best: Option<(&str, LevelFilter)> = None;
        for (prefix, level) in &self.overrides {
            // 只在整个模块名段上匹配，避免 `wgpu` 命中 `wgpu_hal`
            let matched = target == prefix.as_str()
                || (target.starts_with(prefix.as_str()) && target.as_bytes()[prefix.len()] == b':');
            if matched && best.is_none_or(|(best_prefix, _)| prefix.len() > best_prefix.len()) {
                best = Some((prefix.as_str(), *level));
            }
        }

        best.map(|(_, level)| level).unwrap_or(self.default)
    }

    /// 所有规则里的最宽松级别，用作 `log` 的全局上限
    ///
    /// 全局上限决定 `log` 宏是否构造日志参数，压得太紧会让 target 覆盖收不到日志。
    fn max_level(&self) -> LevelFilter {
        self.overrides
            .iter()
            .map(|(_, level)| *level)
            .max()
            .unwrap_or(self.default)
            .max(self.default)
    }
}

/// 把日志写到 stderr 的输出器
struct StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        FILTER
            .get()
            .is_none_or(|filter| metadata.level() <= filter.level_for(metadata.target()))
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        // 只取当天时间（UTC），够用于排查帧间隔与启动时序
        let secs_of_day = now.as_secs() % 86_400;

        eprintln!(
            "[{:02}:{:02}:{:02}.{:03} {:>5} {}] {}",
            secs_of_day / 3600,
            secs_of_day % 3600 / 60,
            secs_of_day % 60,
            now.subsec_millis(),
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

/// 初始化日志输出，级别取自环境变量 `RUST_LOG`
///
/// 支持 `RUST_LOG` 的常规写法：`info` 设定全局级别，
/// `warn,rgpui_wgpu=debug` 额外为某个 crate/模块放开级别。
/// 未设置 `RUST_LOG` 时只输出 `error`。
///
/// 重复调用没有副作用（后续调用被忽略），因此可以安全地放在 `main` 开头：
///
/// ```rust,ignore
/// fn main() {
///     rgpui::init_logging();
///     rgpui_platform::application().run(|cx| { ... });
/// }
/// ```
pub fn init_logging() {
    let _ = FILTER.set(Filter::parse(
        &std::env::var("RUST_LOG").unwrap_or_else(|_| String::new()),
    ));
    // 已有输出器（例如 Web 端把日志转发到浏览器控制台）时不抢它的级别设置
    if log::set_logger(&StderrLogger).is_ok() {
        // 全局上限取规则里最宽松的一档，细筛交给 enabled()
        log::set_max_level(FILTER.get().map(Filter::max_level).unwrap_or(DEFAULT_LEVEL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 校验过滤串解析：全局级别、target 覆盖与非法项忽略
    #[test]
    fn parses_filter_spec() {
        let filter = Filter::parse("warn,rgpui_wgpu=debug,bogus,=error");
        assert_eq!(filter.default, LevelFilter::Warn);
        assert_eq!(filter.overrides.len(), 1);
        assert_eq!(filter.max_level(), LevelFilter::Debug);
    }

    /// 校验 target 前缀匹配：整段命中才算，最长前缀优先
    #[test]
    fn matches_target_prefixes() {
        let filter = Filter::parse("error,rgpui=info,rgpui_wgpu=debug");
        assert_eq!(
            filter.level_for("rgpui_wgpu::wgpu_renderer"),
            LevelFilter::Debug
        );
        assert_eq!(filter.level_for("rgpui::app"), LevelFilter::Info);
        assert_eq!(filter.level_for("wgpu_core"), LevelFilter::Error);
    }
}
