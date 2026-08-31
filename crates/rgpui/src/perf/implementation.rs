//! 本 crate 的实现保存在单独的模块中，
//! 以便于将其作为 RGPUI 依赖的一部分发布

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{num::NonZero, time::Duration};

type HashMap<K, V> = FxHashMap<K, V>;

pub mod consts {
    //! 预设标识符和常量，使分析器和过程宏在通信协议上保持一致。

    /// 实际测试函数上的后缀。
    pub const SUF_NORMAL: &str = "__ZED_PERF_FN";
    /// 用于向 stdout 打印测试元数据的额外函数的后缀。
    pub const SUF_MDATA: &str = "__ZED_PERF_MDATA";
    /// 用于向测试传递迭代次数的环境变量。
    pub const ITER_ENV_VAR: &str = "ZED_PERF_ITER";
    /// 所有基准测试元数据行的前缀，用于将其与测试框架本身的可能输出区分开。
    pub const MDATA_LINE_PREF: &str = "ZED_MDATA_";
    /// 测试元数据函数返回数据的版本号。
    /// 在非向后兼容的更改时递增。
    pub const MDATA_VER: u32 = 0;
    /// 默认权重，如果未指定。
    pub const WEIGHT_DEFAULT: u8 = 50;
    /// 测试必须运行多长时间才被认为是可靠的。
    pub const NOISE_CUTOFF: std::time::Duration = std::time::Duration::from_millis(250);

    /// 测试元数据中迭代次数的标识符。
    pub const ITER_COUNT_LINE_NAME: &str = "iter_count";
    /// 测试元数据中权重的标识符。
    pub const WEIGHT_LINE_NAME: &str = "weight";
    /// 测试元数据中重要性的标识符。
    pub const IMPORTANCE_LINE_NAME: &str = "importance";
    /// 测试元数据版本的标识符。
    pub const VERSION_LINE_NAME: &str = "version";

    /// 保存 json 运行信息的位置。
    pub const RUNS_DIR: &str = ".perf-runs";
}

/// 基准测试的相关程度。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Importance {
    /// 除非有充分理由，否则不应接受回归。
    Critical = 4,
    /// 应额外关注回归。
    Important = 3,
    /// 不应对回归给予额外关注，但它们仍可能表明发生了某些事情。
    #[default]
    Average = 2,
    /// 不清楚回归是否有意义，但仍值得留意。
    /// 分析器默认检查的最低级别。
    Iffy = 1,
    /// 回归可能是虚假的或不影响核心功能。
    /// 仅在大量回归发生时相关，或作为更高重要性基准测试回归的补充证据。
    /// 默认不检查。
    Fluff = 0,
}

impl std::fmt::Display for Importance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Importance::Critical => f.write_str("critical"),
            Importance::Important => f.write_str("important"),
            Importance::Average => f.write_str("average"),
            Importance::Iffy => f.write_str("iffy"),
            Importance::Fluff => f.write_str("fluff"),
        }
    }
}

/// 为什么或何时此测试失败？
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FailKind {
    /// 在分类以确定迭代次数时失败。
    Triage,
    /// 在分析时失败。
    Profile,
    /// 由于测试版本不兼容而失败。
    VersionMismatch,
    /// 无法解析测试的元数据。
    BadMetadata,
    /// 由于应用了 perf 运行的过滤器而跳过。
    Skipped,
}

impl std::fmt::Display for FailKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailKind::Triage => f.write_str("errored in triage"),
            FailKind::Profile => f.write_str("errored while profiling"),
            FailKind::VersionMismatch => f.write_str("test version mismatch"),
            FailKind::BadMetadata => f.write_str("bad test metadata"),
            FailKind::Skipped => f.write_str("skipped"),
        }
    }
}

/// 给定 perf 测试的信息。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestMdata {
    /// 测试生成时的版本号。如果大于此测试处理器期望的版本，
    /// 将以未指定的方式发生以下情况之一：
    /// - 测试被静默跳过。
    /// - 处理器退出并显示指示版本不匹配或无法解析元数据的错误消息。
    ///
    /// 不变量：如果 `version` <= `MDATA_VER`，此工具*必须*能够
    /// 正确解析此测试的输出。
    pub version: u32,
    /// 如果是预设的，通过此测试需要多少次迭代，
    /// 或者如果在运行时确定，测试最终运行了多少次迭代。
    pub iterations: Option<NonZero<usize>>,
    /// 此特定测试的重要性。详见 `Importance` 的文档。
    pub importance: Importance,
    /// 此特定测试在其重要性类别中的权重。用于跨运行比较时。
    pub weight: u8,
}

/// 测试的实际计时，由 Hyperfine 测量。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timings {
    /// 此测试运行 `self.iter_total` 次的平均运行时间。
    pub mean: Duration,
    /// 上述的标准偏差。
    pub stddev: Duration,
}

impl Timings {
    /// 此测试每秒似乎执行多少次迭代？
    #[expect(
        clippy::cast_precision_loss,
        reason = "We only care about a couple sig figs anyways"
    )]
    #[must_use]
    pub fn iters_per_sec(&self, total_iters: NonZero<usize>) -> f64 {
        (1000. / self.mean.as_millis() as f64) * total_iters.get() as f64
    }
}

/// 聚合结果，用于给定的重要性类别。每个测试名称对应其基准测试结果、
/// 迭代次数和权重。
type CategoryInfo = HashMap<String, (Timings, NonZero<usize>, u8)>;

/// 此处理器运行的所有测试的聚合输出。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Output {
    /// 测试输出列表。格式为 `(test_name, mdata, timings)`。
    /// 后者为 `Ok(_)` 表示测试成功。
    ///
    /// 不变量：如果测试成功，第二个字段为 `Some(mdata)` 且
    /// `mdata.iterations` 为 `Some(_)`。
    tests: Vec<(String, Option<TestMdata>, Result<Timings, FailKind>)>,
}

impl Output {
    /// 实例化空的"输出"。用于合并。
    #[must_use]
    pub fn blank() -> Self {
        Output { tests: Vec::new() }
    }

    /// 报告成功并将其添加到此运行的 `Output` 中。
    pub fn success(
        &mut self,
        name: impl AsRef<str>,
        mut mdata: TestMdata,
        iters: NonZero<usize>,
        timings: Timings,
    ) {
        mdata.iterations = Some(iters);
        self.tests
            .push((name.as_ref().to_string(), Some(mdata), Ok(timings)));
    }

    /// 报告失败并将其添加到此运行的 `Output` 中。如果此测试以某些
    /// 迭代次数尝试过（即这不是版本不匹配或跳过的测试），也应报告。
    ///
    /// 使用 `fail!()` 宏通常更方便。
    pub fn failure(
        &mut self,
        name: impl AsRef<str>,
        mut mdata: Option<TestMdata>,
        attempted_iters: Option<NonZero<usize>>,
        kind: FailKind,
    ) {
        if let Some(ref mut mdata) = mdata {
            mdata.iterations = attempted_iters;
        }
        self.tests
            .push((name.as_ref().to_string(), mdata, Err(kind)));
    }

    /// 此次运行是否没有测试执行。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }

    /// 按我们希望的打印顺序对输出中的运行进行排序。
    pub fn sort(&mut self) {
        self.tests.sort_unstable_by(|a, b| match (a, b) {
            // Tests where we got no metadata go at the end.
            ((_, Some(_), _), (_, None, _)) => std::cmp::Ordering::Greater,
            ((_, None, _), (_, Some(_), _)) => std::cmp::Ordering::Less,
            // Then sort by importance, then weight.
            ((_, Some(a_mdata), _), (_, Some(b_mdata), _)) => {
                let c = a_mdata.importance.cmp(&b_mdata.importance);
                if matches!(c, std::cmp::Ordering::Equal) {
                    a_mdata.weight.cmp(&b_mdata.weight)
                } else {
                    c
                }
            }
            // Lastly by name.
            ((a_name, ..), (b_name, ..)) => a_name.cmp(b_name),
        });
    }

    /// 合并两次运行的输出，为新运行的结果添加前缀。
    /// 应与 `Output::blank()` 结合使用，否则只有部分测试会设置前缀。
    pub fn merge<'a>(&mut self, other: Self, pref_other: impl Into<Option<&'a str>>) {
        let pref = if let Some(pref) = pref_other.into() {
            "crates/".to_string() + pref + "::"
        } else {
            String::new()
        };
        self.tests = std::mem::take(&mut self.tests)
            .into_iter()
            .chain(
                other
                    .tests
                    .into_iter()
                    .map(|(name, md, tm)| (pref.clone() + &name, md, tm)),
            )
            .collect();
    }

    /// 评估 `self` 相对于 `baseline` 的性能。后者作为比较点，
    /// 即正的 `PerfReport` 结果表示 `self` 表现更好。
    ///
    /// # Panics
    /// 假设 `self` 和 `baseline` 的所有 `TestMdata` 上的迭代次数字段
    /// 在 `TestMdata` 本身存在时设置为 `Some(_)`。
    #[must_use]
    pub fn compare_perf(self, baseline: Self) -> PerfReport {
        let self_categories = self.collapse();
        let mut other_categories = baseline.collapse();

        let deltas = self_categories
            .into_iter()
            .filter_map(|(cat, self_data)| {
                // Only compare categories where both           meow
                // runs have data.                              /
                let mut other_data = other_categories.remove(&cat)?;
                let mut max = f64::MIN;
                let mut min = f64::MAX;

                // Running totals for averaging out tests.
                let mut r_total_numerator = 0.;
                let mut r_total_denominator = 0;
                // Yeah this is O(n^2), but realistically it'll hardly be a bottleneck.
                for (name, (s_timings, s_iters, weight)) in self_data {
                    // Only use the new weights if they conflict.
                    let Some((o_timings, o_iters, _)) = other_data.remove(&name) else {
                        continue;
                    };
                    let shift =
                        (o_timings.iters_per_sec(o_iters) / s_timings.iters_per_sec(s_iters)) - 1.;
                    if shift > max {
                        max = shift;
                    }
                    if shift < min {
                        min = shift;
                    }
                    r_total_numerator += shift * f64::from(weight);
                    r_total_denominator += u32::from(weight);
                }
    // There were no runs here!
    if r_total_denominator == 0 {
                    None
                } else {
                    let mean = r_total_numerator / f64::from(r_total_denominator);
                    // TODO: also aggregate standard deviation? That's harder to keep
                    // meaningful, though, since we dk which tests are correlated.
                    Some((cat, PerfDelta { max, mean, min }))
                }
            })
            .collect();

        PerfReport { deltas }
    }

    /// 将 `PerfReport` 折叠为按 `Importance` 分组的 `HashMap`，
    /// 每个重要性类别包含其测试。
    fn collapse(self) -> HashMap<Importance, CategoryInfo> {
        let mut categories = HashMap::<Importance, HashMap<String, _>>::default();
        for entry in self.tests {
            if let Some(mdata) = entry.1
                && let Ok(timings) = entry.2
            {
                if let Some(handle) = categories.get_mut(&mdata.importance) {
                    handle.insert(entry.0, (timings, mdata.iterations.unwrap(), mdata.weight));
                } else {
                    let mut new = HashMap::default();
                    new.insert(entry.0, (timings, mdata.iterations.unwrap(), mdata.weight));
                    categories.insert(mdata.importance, new);
                }
            }
        }

        categories
    }
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Don't print the header for an empty run.
        if self.tests.is_empty() {
            return Ok(());
        }

        // We want to print important tests at the top, then alphabetical.
        let mut sorted = self.clone();
        sorted.sort();
        // Markdown header for making a nice little table :>
        writeln!(
            f,
            "| Command | Iter/sec | Mean [ms] | SD [ms] | Iterations | Importance (weight) |",
        )?;
        writeln!(f, "|:---|---:|---:|---:|---:|---:|")?;
        for (name, metadata, timings) in &sorted.tests {
            match metadata {
                Some(metadata) => match timings {
                    // Happy path.
                    Ok(timings) => {
                        // If the test succeeded, then metadata.iterations is Some(_).
                        writeln!(
                            f,
                            "| {} | {:.2} | {} | {:.2} | {} | {} ({}) |",
                            name,
                            timings.iters_per_sec(metadata.iterations.unwrap()),
                            {
                                // Very small mean runtimes will give inaccurate
                                // results. Should probably also penalise weight.
                                let mean = timings.mean.as_secs_f64() * 1000.;
                                if mean < consts::NOISE_CUTOFF.as_secs_f64() * 1000. / 8. {
                                    format!("{mean:.2} (unreliable)")
                                } else {
                                    format!("{mean:.2}")
                                }
                            },
                            timings.stddev.as_secs_f64() * 1000.,
                            metadata.iterations.unwrap(),
                            metadata.importance,
                            metadata.weight,
                        )?;
                    }
                    // We have (some) metadata, but the test errored.
                    Err(err) => writeln!(
                        f,
                        "| ({}) {} | N/A | N/A | N/A | {} | {} ({}) |",
                        err,
                        name,
                        metadata
                            .iterations
                            .map_or_else(|| "N/A".to_owned(), |i| format!("{i}")),
                        metadata.importance,
                        metadata.weight
                    )?,
                },
                // No metadata, couldn't even parse the test output.
                None => writeln!(
                    f,
                    "| ({}) {} | N/A | N/A | N/A | N/A | N/A |",
                    timings.as_ref().unwrap_err(),
                    name
                )?,
            }
        }
        Ok(())
    }
}

/// 给定重要性类别中两次运行之间的性能差异。
struct PerfDelta {
    /// 最大改进 / 最小回归。
    max: f64,
    /// 测试时间的加权平均变化。
    mean: f64,
    /// 最大回归 / 最小改进。
    min: f64,
}

/// 报告所有重要性类别性能差异的垫片类型。
pub struct PerfReport {
    /// 内部（组，差异）配对。
    deltas: HashMap<Importance, PerfDelta>,
}

impl std::fmt::Display for PerfReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.deltas.is_empty() {
            return write!(f, "(no matching tests)");
        }
        let sorted = self.deltas.iter().collect::<Vec<_>>();
        writeln!(f, "| Category | Max | Mean | Min |")?;
        // We don't want to print too many newlines at the end, so handle newlines
        // a little jankily like this.
        write!(f, "|:---|---:|---:|---:|")?;
        for (cat, delta) in sorted.into_iter().rev() {
            const SIGN_POS: &str = "↑";
            const SIGN_NEG: &str = "↓";
            const SIGN_NEUTRAL_POS: &str = "±↑";
            const SIGN_NEUTRAL_NEG: &str = "±↓";

            let prettify = |time: f64| {
                let sign = if time > 0.05 {
                    SIGN_POS
                } else if time > 0. {
                    SIGN_NEUTRAL_POS
                } else if time > -0.05 {
                    SIGN_NEUTRAL_NEG
                } else {
                    SIGN_NEG
                };
                format!("{} {:.1}%", sign, time.abs() * 100.)
            };

            // Pretty-print these instead of just using the float display impl.
            write!(
                f,
                "\n| {cat} | {} | {} | {} |",
                prettify(delta.max),
                prettify(delta.mean),
                prettify(delta.min)
            )?;
        }
        Ok(())
    }
}
