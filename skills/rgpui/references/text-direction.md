# 文字方向与东亚断行（bidi / line_break）

**目录：** [概述](#概述) · [TextDirection](#textdirection段落基准方向) · [强方向与语言查表](#强方向字符与语言查表) · [TextAlign 的 start/end](#textalign-的-start--end) · [东亚断行禁则](#东亚断行禁则line_break) · [与文本系统的接线](#与文本系统的接线) · [实践要点](#实践要点)

## 概述

两个核心模块（均无 feature 门控，走模块路径访问、不在 crate 根重导出）：

- `rgpui::bidi`（`crates/rgpui/src/bidi.rs`，`rgpui.rs:318`）——段落**基准方向**：
  回答「这一段文字属于 LTR 还是 RTL」。字形级重排不归它管：cosmic-text 后端内部
  已用 `unicode-bidi` 重排，Windows DirectWrite 后端把整行交给 `IDWriteTextLayout`
  自行处理 bidi。
- `rgpui::line_break`（`crates/rgpui/src/line_break.rs`，`rgpui.rs:321`）——CJK
  逐字换行时的**禁则（kinsoku）**与胶连字符判定。

示例：`cargo run -p showcase --bin intl_text`（`examples/showcase/src/bin/intl_text.rs`）。

## TextDirection：段落基准方向

`TextDirection`（`bidi.rs:33`）：变体 `Ltr`（`#[default]`）/ `Rtl`，实现
`Copy + Serialize + Deserialize + JsonSchema`。

| 方法 | 签名 | 说明 |
|------|------|------|
| `resolve` | `(text: &str) -> Self`（`:46`） | HTML `dir=auto` 口径（UBA P2/P3）：取**首个强方向字符**；全段无强方向字符回退 `Ltr`。纯 ASCII 走快速路径 |
| `is_rtl` | `(self) -> bool`（`:56`） | 是否为 `Rtl` |
| `for_locale` | `(locale: &str) -> Self`（`:64`） | 语言标签 → 基准方向（`"ar-EG"` → RTL），供没有具体文本可判定的场合（空输入框、外壳翻转） |

数字是弱方向字符，不决定方向——`resolve("123456") == Ltr`、
`resolve("٣٤٥") == Ltr`、`resolve("123 שלום") == Rtl`（跳过数字继续找），与浏览器一致。

```rust
use rgpui::bidi::TextDirection;

assert_eq!(TextDirection::resolve("שלום עולם"), TextDirection::Rtl);
assert_eq!(TextDirection::for_locale("zh-CN"), TextDirection::Ltr);
```

## 强方向字符与语言查表

- `strong_direction(c: char) -> Option<TextDirection>`（`bidi.rs:79`）：按
  `unicode_bidi::bidi_class` 归类，`L` → `Some(Ltr)`，`R` / `AL` → `Some(Rtl)`，
  弱方向与中性字符返回 `None`。方向标记 LRM/RLM/ALM 在 UBA 中即强方向，无需特例。
- `is_rtl_char(c: char) -> bool`（`:88`）：字符是否属 `R` / `AL` 类。
- `primary_language(locale: &str) -> &str`（`:96`）：取主语言子标签
  （`"ar-EG"` / `"ar_EG"` → `"ar"`）。不解析书写系统子标签（`"az-Arab"` 仍按 `"az"` 判 LTR）。
- `is_rtl_locale(locale: &str) -> bool`（`:106`）：CLDR 常用 RTL 语言表（非穷举，
  含 `ar` `he` `fa` `ur` `ckb` `ps` `sd` `ug` `dv` `syr` `nqo` `yi` 等，历史代码
  `iw` / `ji` 一并认），只看主语言子标签、大小写不敏感（`"AR_eg"` 也是 RTL）。

`rgpui::i18n` 的 `I18nManager::direction()`（`i18n/mod.rs:180`）/ `is_rtl()`（`:185`）
即 `TextDirection::for_locale` 的包装。

## TextAlign 的 Start / End

`TextAlign`（`crates/rgpui/src/style.rs:431`）共五个变体：`Left`（`#[default]`）、
`Center`、`Right`、`Start`、`End`。`TextAlign::resolve(self, direction: TextDirection)`
（`style.rs:459`）把 `Start`/`End` 折成绝对方向（`Start` 在 RTL 下为 `Right`），
`Left`/`Center`/`Right` 原样返回。

**解析发生在绘制入口，不在设置样式时**：`ShapedLine::paint` / `paint_background`
（`text_system/line.rs:82` / `:110`）与 `WrappedLine::paint` / `paint_background`
（`:298` / `:331`）里逐行执行
`align.resolve(TextDirection::resolve(&self.text))`（`:92` / `:120` / `:313` / `:346`）
——同一份样式设在希伯来文行上自动靠右、设在拉丁文行上自动靠左。

```rust
use rgpui::{TextAlign, bidi::TextDirection};

assert_eq!(TextAlign::Start.resolve(TextDirection::Rtl), TextAlign::Right);
assert_eq!(TextAlign::End.resolve(TextDirection::Ltr), TextAlign::Right);
```

HTML/CSS 透传（均为序列化侧）：`rich_text` 导出 HTML 时 `paragraph_css`
（`rich_text.rs:1987`，`Start`/`End` 分支在 `:1993-1994`，经 `to_html`（`:1156`）
的渲染路径调用）把段落写成 `text-align:start` / `text-align:end`；DOM 后端
`text_align_to_css`（`crates/rgpui-dom/src/css.rs:187`）同样原样输出，交给浏览器
按方向解析。注意 `from_html` 侧的段落 CSS 解析 `apply_paragraph_css`
（`rich_text.rs:2103`）只认 `center` / `right`，`start` / `end` 回读会降级为
`Left`，**往返不保留**。

## 东亚断行禁则（line_break）

| 符号 | 位置 | 说明 |
|------|------|------|
| `BreakClass` | `line_break.rs:32` | `Ideographic` / `Kana` / `Hangul` / `SmallKana` / `ProlongedSoundMark` / `ClosePunctuation` / `OpenPunctuation` / `Word` / `Space` / `Other` |
| `break_class(c) -> BreakClass` | `:65` | 字符断行类别 |
| `is_line_start_prohibited(c)` | `:95` | 行首禁则：`ClosePunctuation`（`。、）」…` 等）+ `SmallKana`（`ぁっャ` 等）+ `ProlongedSoundMark`（`ー々〜` 等）不得起行 |
| `is_line_end_prohibited(c)` | `:103` | 行末禁则：`OpenPunctuation`（`（「『【` 等）不得收行 |
| `is_no_break(c)` | `:108` | 胶连字符：NBSP `U+00A0`、数字空格 `U+2007`、不换行连字符 `U+2011`、窄空格 `U+202F`、ZWNJ/ZWJ `U+200C`/`U+200D`、词连接符 `U+2060`、`U+FEFF`；零宽空格 `U+200B` **不是**胶连字符 |
| `can_break_between(prev, next) -> bool` | `:127` | 四个条件全过才可断：两侧非胶连、`prev` 非行末禁则、`next` 非行首禁则 |
| `is_ideographic(c)` | `:135` | 汉字（基本区 + 扩展 A–G + 兼容表意文字） |
| `uax14_break_indices(text) -> impl Iterator<Item = usize>` | `:283` | UAX #14 完整断行机会（ICU `LineSegmenter`，LB1–LB31：标点类组合、货币/数字、URL 连接符等）；产出**允许断**的字节偏移，含换行后与串尾强制断点 |

```rust
use rgpui::line_break::can_break_between;

assert!(can_break_between('你', '好'));    // 汉字之间可断
assert!(!can_break_between('好', '。'));   // 句号不可起行
assert!(!can_break_between('「', '好'));   // 开引号不可收行
assert!(can_break_between('a', ')'));      // 半角 ASCII 行为不变
```

`can_break_between` 是「东亚禁则 + 胶连」的局部否决；需要**严格 UAX #14**
口径（完整规则表）时用 `uax14_break_indices` 取候选断点集合。两者在重叠规则
上口径一致（禁则否决的位置 UAX #14 同样不产出断点，见互验测试）。

## 与文本系统的接线（B3 已接线，UAX #14 口径）

两条换行路径的候选断点都改为 **UAX #14 断点集合查询**（`LineWrapper::break_indices`
与 `compute_wrap_boundaries` 内的 `uax14_break_indices`），不再使用逐字符的
`is_word_char`（已删除）与逐对 `can_break_between` 否决：

- `LineWrapper::wrap_line` / `wrap_line_measured`（`text_system/line_wrapper.rs`）：
  先把 fragment 拼成行文本（内联元素占位为等长 `a`，与相邻单词粘结），
  一次性算断点集合；候选必须同时是**字素边界**（emoji ZWJ 序列、肤色修饰、
  组合标记不被切开），超宽字素整体换行；
- `LineLayout::compute_wrap_boundaries`（`text_system/line_layout.rs`）：
  断点 ∩「shaping 未拆开的字素起点」（字形起点恰为断点），避免把连字 /
  组合标记切断。

`can_break_between` 仍是公开的局部判定助手（东亚禁则 + 胶连），但**软换行不再
逐对调用它**——UAX #14 的 LB13 等规则已涵盖禁则（互验测试保证两者在重叠规则上
一致）。

行为变化（对齐上游 `f0210f1621`）：连字符后允许断行（LB21）、URL / 路径中的
`/` `?` 可断、emoji 序列不再被拆开；半角 ASCII 标点的旧“单词字符”豁免随之取消。
