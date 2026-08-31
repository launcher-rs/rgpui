//! # RGPUI 中的无障碍支持
//!
//! "无障碍"（Accessibility）是指你的应用能否被所有用户使用，
//! 无论其是否有残障。这方面包含许多要素，都很重要，例如：
//! - 确保足够的文本对比度。
//! - 提供禁用动画的机制。
//! - 提供增大文本字号的机制。
//! - 等等。
//!
//! 本指南聚焦于**可编程无障碍**（programmatic accessibility）。它允许
//! 辅助技术（如屏幕阅读器或盲文显示器）检查并与你的应用交互。有关
//! 贡献者编写无障碍支持的文档，请参阅 `a11y` 模块的文档注释。
//!
//! RGPUI 集成了 [AccessKit] 以提供可编程无障碍功能
//! （本指南后续部分将其简称为"无障碍"）。
//!
//! 最小示例可参见 `examples/a11y` 目录。
//!
//! ## 背景
//!
//! 无障碍支持基于两项关键能力：
//! - 将当前 UI 状态的信息暴露给辅助技术。
//! - 响应辅助技术请求的操作。
//!
//! 例如，屏幕阅读器可能想要通知用户出现了一个新按钮，
//! 然后用户可以通过语音控制程序来按下该按钮。
//!
//! ### RGPUI 中的 ID — [`ElementId`] 与 [`GlobalElementId`]
//!
//! 在 RGPUI 中，每个 [`Element`] 都可以拥有一个 [`id`][Element::id]：
//! ```rust
//! # use rgpui::*;
//! let div_with_id = div().id("my-id").child(text!("hello"));
//!
//! // ID 是可选的
//! let div_without_id = div().child(text!("hello"));
//! ```
//!
//! 拥有 ID 的 [`Element`] 还会被分配一个 [`GlobalElementId`]。这个全局
//! ID 由其所有祖先的非 `None` ID 组合而成。例如：
//! ```rust
//! # use rgpui::*;
//! let inner = div().id("inner-id");
//! let middle = div().child(inner);  // 没有 ID
//! let outer = div().id("outer-id").child(middle);
//! ```
//! 在此示例中，`inner` 的全局 ID（粗略地说）是 `["outer-id",
//! "inner-id"]`。
//!
//! 由于 `middle` 本身没有 ID，因此它没有全局 ID。
//!
//! [`GlobalElementId`] 在每一帧中应当是唯一的。同一帧中重复的全局
//! ID 很可能会导致错误。
//!
//! ### ID 与无障碍
//!
//! 当 RGPUI 渲染一帧时，它会遍历你的 UI 树，找到具有全局 ID 的节点，
//! 并将这些节点的信息告知辅助技术。
//!
//! 为了使节点被报告，它们还必须拥有非 `None` 的
//! [`role`][Element::a11y_role]。这用于告知辅助技术该节点是*哪种类型*
//! （按钮、标签、表格等）。你可以使用
//! [`div().id(...).role()`][StatefulInteractiveElement::role] 来设置角色。
//!
//! *跨帧*拥有相同全局 ID 的节点被视为"同一个"节点。例如：
//! ```rust
//! # use rgpui::*;
//! // 第 1 帧的 UI
//! let frame_1 = div()
//!     .id("parent")
//!     .role(Role::Button)
//!     .child(
//!       div()
//!         .id("id-1")
//!         .role(Role::Label)
//!         .child(text!("hello"))
//!     );
//!
//! // 下一帧的 UI
//! let frame_2 = div()
//!     .id("parent")
//!     .role(Role::Button)
//!     .child(
//!       div()
//!         .id("id-2")  // <- 不同的 ID
//!         .role(Role::Label)
//!         .child(text!("hello"))
//!     );
//! ```
//! 从逻辑上看，UI 没有变化。但屏幕阅读器无法知道两个子 [`div`] 是"同一个"。
//! 因此辅助技术会将其解释为一个节点被移除、另一个节点被添加。这可能会
//! 令用户非常困惑，因为通知通常只在某些内容发生了*有意义的*变化时才会触发。
//!
//! 换句话说，通过控制元素的 ID，你可以控制对 UI 元素的更改是否被视为有意义的。
//! 你还可以通过设置 [`role`][Element::a11y_role] 来控制元素是否*完全*报告给
//! 辅助技术，因为没有角色的节点不会被报告。
//!
//! #### ID 与文本
//!
//! 处理文本时必须特别注意。
//!
//! RGPUI 提供了 [`text!`] 宏，它将字符串包装在 [`Text`] 类型中，
//! 并自动派生一个 ID。通常这就是你想要的。然而，它生成 ID 的方式
//! 可能比较微妙，甚至令人意外。
//!
//! [`text!`] 宏调用的 ID 派生自**该调用在源代码中的位置**。例如：
//!
//! ```rust
//! # use rgpui::*;
//! let a = text!("a");
//! let b = text!("b");
//!
//! // 不同的源码位置，不同的 ID
//! assert_ne!(a.id(), b.id());
//!
//! // 但是：
//!
//! fn make_text(s: &str) -> Text { text!(s) }
//!
//! let a = make_text("a");
//! let b = make_text("b");
//!
//! // `a` 和 `b` 都由同一个 `text!` 调用产生，因此 ID 相同
//! assert_eq!(a.id(), b.id());
//! ```
//! 这可能会产生令人意外的行为。例如，这个陷阱：
//! ```rust
//! # use rgpui::*;
//! let todos = vec!["eat lunch", "drink water", "go to gym"];
//! let todo_divs = todos.into_iter().map(|todo| {
//!     text!(todo)
//! });
//!
//! div()
//!     .id("todo-list")
//!     .role(Role::Document)
//!     .children(todo_divs);  // 错误：多个节点具有相同的全局 ID
//! ```
//!
//! 这里，当我们映射迭代器时，由于我们只写了一次 [`text!`]，
//! 因此只有一个 ID。由于它们拥有相同的祖先和相同的 ID，它们将具有
//! 相同的全局 ID。在 release 构建中，这将导致某些节点被静默丢弃！
//!
//! 要修复此问题，你可以设置一个 ID：
//! ```rust
//! # use rgpui::*;
//! let todos = vec!["eat lunch", "drink water", "go to gym"];
//! let todo_divs = todos.into_iter().enumerate().map(|(index, todo)| {
//!     text!(todo).with_id(index)  // 或者 `text(id = index, todo)`
//! });
//!
//! div()
//!     .id("todo-list")
//!     .role(Role::Document)
//!     .children(todo_divs);
//! ```
//! 另一种可能的解决方案是将 [`text!`] 包装在一个*具有*唯一全局 ID 的
//! 另一个节点中。例如：
//! ```rust
//! # use rgpui::*;
//! let todos = vec!["eat lunch", "drink water", "go to gym"];
//! let todo_divs = todos.into_iter().enumerate().map(|(index, todo)| {
//!     div().id(index).child(text!(todo))
//! });
//!
//! div()
//!     .id("todo-list")
//!     .role(Role::Document)
//!     .children(todo_divs);
//! ```
//! 由于 AccessKit [`NodeId`][accesskit::NodeId] 派生自全局 ID，而全局
//! ID 会考虑所有祖先的 ID，因此这种方法同样有效。
//!
//! 偶尔，你需要创建一个*没有* ID 的 [`Text`] 元素。你可以通过
//! [`Text::new_inaccessible`] 来实现。如果你正在创建自定义 UI 组件
//! （例如一个按钮），你可能需要这样做，以便在父 [`div`] 上设置
//! `label` 属性，而无需在无障碍树中重复文本。
//!
//! ### 处理操作
//!
//! 辅助技术可以向 UI 派发操作。虽然许多辅助技术用户使用传统输入设备
//! （如键盘），但有些人使用更专业的系统。例如，行动不便的用户可能会
//! 使用语音控制来与你的应用交互。
//!
//! 当用户派发一个操作时，它是*派发到特定节点*的。你有责任告知 UI
//! 元素在收到请求时应如何响应。
//!
//! 注意，这些操作与 RGPUI 的 [`Action`] trait **完全无关**。
//! AccessKit 暴露了 [`accesskit::Action`]。在 RGPUI 中，它被重新导出为
//! [`AccessibleAction`]。
//!
//! 要响应无障碍操作，请使用
//! [`div().on_a11y_action()`][InteractiveElement::on_a11y_action]：
//! ```rust,ignore
//! div()
//!     .id("my-slider")
//!     .role(Role::Slider)
//!     .on_a11y_action(AccessibleAction::Increment, |_extra, _window, _cx| {
//!         position += 1;
//!         cx.notify();
//!     })
//!     .child(my_cool_slider());
//! ```
//!
//! 注意，一些常见操作会自动注册。例如，
//! [`.on_click()`][StatefulInteractiveElement::on_click] 会添加一个
//! [`AccessibleAction::Click`] 处理器，该处理器会调用点击处理函数。
//!
//! ## 合成子节点
//!
//! 有时，一个自定义 [`Element`] 可能希望看起来像是由多个节点组成的。
//! 例如，一个完全假设的自定义文本编辑器元素可能希望拥有
//! [`Role::TextInput`]，同时呈现由 [`Role::TextRun`] 组成的子节点。
//!
//! 这可以通过 [`Element::a11y_synthetic_children`] 实现。例如：
//! ```rust,ignore
//! # use rgpui::*;
//! impl Element for MyCustomTextField {
//!
//!     // ...
//!     
//!     fn a11y_role(&self) -> Option<Role> {
//!         Some(Role::TextInput)
//!     }
//!     
//!     fn a11y_synthetic_children(
//!         &mut self,
//!         _prepaint: &mut Self::PrepaintState,
//!         builder: &mut A11ySubtreeBuilder,
//!     ) {
//!         // 创建合成子节点
//!         let mut run = accesskit::Node::new(Role::TextRun);
//!         run.set_value(self.text.clone());
//!         run.set_character_lengths(
//!             self.text.chars().map(|c| c.len_utf8() as u8).collect::<Vec<_>>(),
//!         );
//!
//!         // 将其插入为 `MyCustomTextField` 的子节点
//!         let run_id = builder.synthetic_node_id(0);
//!         builder.push_child(run_id, run);
//!
//!         // 你还可以修改父节点（即 `MyCustomTextField`）
//!         let caret = accesskit::TextPosition {
//!             node: run_id,
//!             character_index: self.cursor,
//!         };
//!         builder.parent_node().set_text_selection(accesskit::TextSelection {
//!             anchor: caret,
//!             focus: caret,
//!         });
//!     }
//! }
//! ```
//!
//! 值得注意的是，合成子节点是在元素[预绘制][Element::prepaint]
//! *之后*添加的，因此可以使用预绘制状态（例如，确定屏幕上哪些内容可见）。
//!
//! ## 延伸阅读
//!
//! 设计高质量的无障碍界面可能很有挑战性，就像设计高质量的传统界面一样。
//! 以下页面包含有用的信息：
//!
//! - [AccessKit]：RGPUI 内部使用的跨平台无障碍工具包。
//! - [MDN WAI-ARIA 基础][mdn-aria]：角色、属性和状态的介绍。
//! - [ARIA 编写实践指南][apg]：W3C 无障碍组件模式。
//!
//! 注意，虽然 RGPUI 模仿了 Web API，但它的行为不一定与 Web 浏览器
//! 在使用相同属性时*完全*一致。
//!
//! [AccessKit]: https://accesskit.dev/
//! [mdn-aria]: https://developer.mozilla.org/en-US/docs/Learn_web_development/Core/Accessibility/WAI-ARIA_basics
//! [apg]: https://www.w3.org/WAI/ARIA/apg/

#[cfg(doc)]
use crate::*; // 这样就不必限定每个类型了 :)
