//! Markdown 流式增量状态。
//!
//! [`MarkdownState`] 持有源码与最近一次解析结果。`append` / `set_source` 只更新
//! 源码并防抖触发后台整篇解析；解析完成前渲染仍使用旧块列表，避免流式输出时
//! 每来一个 chunk 就重建整篇文档、卡住主线程。

use crate::markdown::{ParsedMarkdown, parse_markdown_with_urls};
use rgpui::{Context, Task};
use std::sync::Arc;
use std::time::Duration;

/// 追加源码后的解析防抖间隔：约一帧，聚合密集 chunk。
const PARSE_DEBOUNCE: Duration = Duration::from_millis(16);

/// 可增量追加的 Markdown 状态。
pub struct MarkdownState {
    source: String,
    /// 每次调度解析都递增；后台结果回来时比对，过期结果直接丢弃。
    generation: u64,
    parsed: ParsedMarkdown,
    parsing: bool,
    /// 当前防抖任务；再次调度时丢弃旧任务即取消旧解析。
    parse_task: Option<Task<()>>,
}

impl MarkdownState {
    /// 用初始源码创建状态，并同步完成首次解析（首帧即有内容）。
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let parsed = Arc::new(parse_markdown_with_urls(&source));
        Self {
            source,
            generation: 0,
            parsed,
            parsing: false,
            parse_task: None,
        }
    }

    /// 追加一段 Markdown 源码（流式输出的一个 chunk）。
    pub fn append(&mut self, delta: &str, cx: &mut Context<Self>) {
        if delta.is_empty() {
            return;
        }
        self.source.push_str(delta);
        self.schedule_parse(cx);
    }

    /// 整体替换源码。
    pub fn set_source(&mut self, source: impl Into<String>, cx: &mut Context<Self>) {
        self.source = source.into();
        self.schedule_parse(cx);
    }

    /// 当前完整源码。
    pub fn source(&self) -> &str {
        &self.source
    }

    /// 最近一次解析结果（解析进行中时仍是上一版）。
    pub fn parsed(&self) -> ParsedMarkdown {
        self.parsed.clone()
    }

    /// 是否有后台解析尚未完成。
    pub fn is_parsing(&self) -> bool {
        self.parsing
    }

    /// 防抖后在后台整篇重解析。
    fn schedule_parse(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.parsing = true;
        self.parse_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PARSE_DEBOUNCE).await;
            let Ok(source) = this.update(cx, |state, _| state.source.clone()) else {
                return;
            };
            let parsed = cx
                .background_executor()
                .spawn(async move { Arc::new(parse_markdown_with_urls(&source)) })
                .await;
            _ = this.update(cx, |state, cx| {
                if state.generation != generation {
                    return;
                }
                state.parsed = parsed;
                state.parsing = false;
                cx.notify();
            });
        }));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgpui::{AppContext, Entity, TestAppContext};

    fn new_state(cx: &mut TestAppContext, source: &str) -> Entity<MarkdownState> {
        cx.update(|cx| cx.new(|_| MarkdownState::new(source)))
    }

    /// 连续追加只在防抖窗口结束后解析一次，且解析期间继续提供旧块列表。
    #[test]
    fn append_debounces_then_publishes_parsed_blocks() {
        let mut cx = TestAppContext::single();
        let state = new_state(&mut cx, "# 标题\n");
        assert_eq!(state.read_with(&cx, |state, _| state.parsed().0.len()), 1);

        state.update(&mut cx, |state, cx| {
            state.append("\n段落 A\n", cx);
            state.append("\n段落 B\n", cx);
        });
        assert!(state.read_with(&cx, |state, _| state.is_parsing()));

        // 防抖窗口内不解析：仍显示旧内容。
        cx.background_executor
            .advance_clock(Duration::from_millis(8));
        cx.run_until_parked();
        assert_eq!(state.read_with(&cx, |state, _| state.parsed().0.len()), 1);
        assert!(state.read_with(&cx, |state, _| state.is_parsing()));

        // 越过防抖窗口后发布新结果。
        cx.background_executor
            .advance_clock(Duration::from_millis(20));
        cx.run_until_parked();
        assert!(!state.read_with(&cx, |state, _| state.is_parsing()));
        assert_eq!(state.read_with(&cx, |state, _| state.parsed().0.len()), 3);
        assert_eq!(
            state.read_with(&cx, |state, _| state.source().to_owned()),
            "# 标题\n\n段落 A\n\n段落 B\n"
        );
    }

    /// 同屏两条流式消息各自持有解析结果，追加一条不影响另一条。
    #[test]
    fn independent_states_do_not_reparse_each_other() {
        let mut cx = TestAppContext::single();
        let first = new_state(&mut cx, "# 第一条\n");
        let second = new_state(&mut cx, "# 第二条\n");

        first.update(&mut cx, |state, cx| state.append("新段落\n", cx));
        cx.background_executor
            .advance_clock(Duration::from_millis(20));
        cx.run_until_parked();

        assert_eq!(first.read_with(&cx, |state, _| state.parsed().0.len()), 2);
        assert_eq!(second.read_with(&cx, |state, _| state.parsed().0.len()), 1);
        assert_eq!(
            second.read_with(&cx, |state, _| state.source().to_owned()),
            "# 第二条\n"
        );
    }

    /// `set_source` 整体替换后同样经过防抖并发布新内容。
    #[test]
    fn set_source_replaces_and_reparses() {
        let mut cx = TestAppContext::single();
        let state = new_state(&mut cx, "# A\n");
        state.update(&mut cx, |state, cx| {
            state.set_source("段落一\n\n段落二\n", cx)
        });
        cx.background_executor
            .advance_clock(Duration::from_millis(20));
        cx.run_until_parked();
        assert_eq!(state.read_with(&cx, |state, _| state.parsed().0.len()), 2);
        assert_eq!(
            state.read_with(&cx, |state, _| state.source().to_owned()),
            "段落一\n\n段落二\n"
        );
    }
}
