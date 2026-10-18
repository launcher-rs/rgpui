# 对话 / AI-Chat（Chat）

**目录：** [概述](#概述) · [消息与视图](#消息与视图) · [Sender / Prompts / Suggestion](#sender--prompts--suggestion) · [思维链](#思维链) · [附件 / 引用 / 操作行](#附件--引用--操作行) · [MessageScroller / Bubble / Marker](#messagescroller--bubble--marker) · [最佳实践](#最佳实践)

## 概述

AI-Chat 组件位于 `crates/rgpui/src/chat/`，`pub mod chat;`（`rgpui.rs:275`，
**不是** `pub use chat::*`）。文件：`mod.rs`（`Message`/`MessageGroup`/`ChatState`/
`ChatView`/`MessageType`）、`actions.rs`、`attachments.rs`、`bubble.rs`、`marker.rs`、
`prompts.rs`、`scroller.rs`、`sender.rs`、`sources.rs`、`suggestion.rs`、`thought_chain.rs`。
各子模块在 `chat/mod.rs:141-161` 内 `pub use` → 用完整路径
`rgpui::chat::{Sender, Prompts, ThoughtChain, ...}`。

**prelude 只重导出 5 个**（`prelude.rs:60`）：`ChatState`、`ChatView`、`Message`、
`MessageGroup`、`MessageType`。其余须 `rgpui::chat::` 前缀。

## 消息与视图

`MessageType`（`chat/mod.rs:10`）：`Text(String)` / `CodeBlock { language, code }`。

`Message`（`:24`）：`text(content)`（`:33`）、`code_block(language, code)`（`:41`）。
字段 `content: MessageType`、`id: String`。

`MessageGroup`（`:54`）：字段 `sender: SharedString`、`messages: Vec<Message>`。

`ChatState`（`:63`，`Default`）：字段 `groups: Vec<MessageGroup>`、`input_text: String`；
方法 `add_group(MessageGroup)`（`:72`）、`clear()`（`:77`）。

`ChatView`（`:83`）：`new(Entity<ChatState>)`（`:89`），`impl Render`。

```rust
let state = cx.new(|_| {
    let mut s = ChatState::default();
    s.add_group(MessageGroup {
        sender: "用户".into(),
        messages: vec![Message::text("你好")],
    });
    s
});
// 渲染：ChatView::new(state)
```

## Sender / Prompts / Suggestion

**Sender**（`sender.rs`）：`SenderState::new(window, cx)`（`:33`），
`Sender::on_send(f)`（`:56`）需要 `Entity<SenderState>`，`input() -> &Entity<InputState>`（`:65`）。

**Prompts**（`prompts.rs`）：`Prompts::new(Vec<PromptItem>)`（`:40`）、`on_pick`（`:49`）。
`PromptItem::new(title, description)`（`:19`）。

**Suggestion**（`suggestion.rs`）：`Suggestion::new(Vec<SharedString>)`（`:22`）、
`on_pick`（`:31`）。

## 思维链

`ThoughtChain`（`thought_chain.rs:50`）：`new(Vec<ThoughtStep>)`（`:63`）、
`expanded(bool)`（`:73`）、`on_toggle`（`:79`）。
`ThoughtStep`（`:24`）：`new(title, detail, ThoughtStatus)`（`:35`）。
`ThoughtStatus`（`:10`）：`Pending`(默认)/`Running`/`Done`/`Failed`。

## 附件 / 引用 / 操作行

**Attachments**（`attachments.rs:119`）：`new(Vec<AttachmentItem>)`（`:128`）、`on_remove`（`:136`）。
`AttachmentItem`（`:10`）：`new(name)`（`:19`）、`size(SharedString)`（`:27`）。
`FileCard`（`:35`）：`new(AttachmentItem)`（`:46`）、`on_remove`（`:55`）。

**Sources**（`sources.rs:35`）：`new(Vec<SourceItem>)`（`:46`）、`on_open`（`:55`）。
`SourceItem`（`:10`）：`new(title)`（`:19`）、`url(SharedString)`（`:27`）。

**Actions**（`actions.rs:23`）：`new()`（`:32`）、`actions(Vec<MessageAction>)`（`:44`）、
`on_action`（`:50`）。`MessageAction`（`:10`）：`Copy`/`Like`/`Dislike`/`Retry`。

## MessageScroller / Bubble / Marker

**MessageScroller**（`scroller.rs:51`）：`new(Entity<MessageScrollerState>)`（`:59`）。
状态（`:9`）：`new()`（`:17`）、`scroll_handle() -> &ScrollHandle`（`:26`）、
`unread() -> usize`（`:31`）、`scroll_to_bottom(cx)`（`:36`）。

**Bubble**（`bubble.rs:20`）：`new()`（`:29`）、`side(BubbleSide)`（`:39`）、
`avatar(impl IntoElement)`（`:45`）。`BubbleSide`（`:10`）。

**Marker**（`marker.rs:9`）：`new(label)`（`:17`）、`highlight(bool)`（`:26`）。

## 最佳实践

- **消息用 `Message`/`MessageGroup`/`ChatState`**，视图直接 `ChatView::new(state)`。
- **发送框是状态式**：`SenderState` 实体 + `Sender::on_send`。
- **流式/思维过程**用 `ThoughtChain` + `ThoughtStatus`。
- **滚动定位**用 `MessageScroller`（含未读数、滚动到底部）。
- **导入**：`ChatState`/`ChatView`/`Message`/`MessageGroup`/`MessageType` 已在 prelude；
  其余（`Sender`/`Prompts`/`ThoughtChain` 等）用 `rgpui::chat::`。
