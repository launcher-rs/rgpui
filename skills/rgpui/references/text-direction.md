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

```rust
use rgpui::line_break::can_break_between;

assert!(can_break_between('你', '好'));    // 汉字之间可断
assert!(!can_break_between('好', '。'));   // 句号不可起行
assert!(!can_break_between('「', '好'));   // 开引号不可收行
assert!(can_break_between('a', ')'));      // 半角 ASCII 行为不变
```

## 与文本系统的接线

两条换行路径都只在**非单词字符**的候选断点处调用 `can_break_between` 否决：

- `LineWrapper::wrap_line`（`text_system/line_wrapper.rs:40`，否决点 `:78`）；
- `LineLayout::compute_wrap_boundaries`（`text_system/line_layout.rs:142`，否决点 `:194`）。

单词内部不断行由 `LineWrapper::is_word_char`（`line_wrapper.rs:451`）负责，与禁则
判定互不干涉。规则只面向 CJK 与全角标点，**半角 ASCII 标点（`)` `]` `,` `.`）
保持拉丁既有行为**。`bidi` 侧的接线在 `TextAlign`（`style.rs`）与绘制入口
（`text_system/line.rs`）；rgpui-dom 只做序列化透传。

## 实践要点

- **应用作者通常什么都不用做**：文本经过 `ShapedLine::paint` / `WrappedLine::paint`
  与两条 wrap 路径时，方向解析与禁则否决自动生效；混排 CJK 的换行不需要任何显式开关。
- **需要显式判定的场合**才直接调 API：自建 HTML-ish 渲染器（对每个块级元素跑
  `TextDirection::resolve` 决定 `start`/`end` 落点）、按语言翻转外壳（用
  `for_locale` / `I18nManager::is_rtl`，不要看具体文本）、自定义换行逻辑（在候选
  断点处用 `can_break_between` 否决，与两条内置路径同一口径）。
- **布局是逐行解析方向的**：同一份 `text_align(Start)` 样式在 LTR 行与 RTL 行上
  呈现相反对齐，这是特性而非 bug。
- `Start`/`End` 经 `to_html` 导出后 `from_html` 读不回来（降级 `Left`）；需要往返
  保真的文档模型请直接在 `ParagraphStyle.align` 上设置。
