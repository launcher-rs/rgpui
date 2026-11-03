# 基础组件（Widgets）

**目录：** [概述](#概述) · [导入约定](#导入约定) · [Button](#button) · [ButtonIcon](#buttonicon) · [选择类](#选择类) · [Slider](#slider) · [显示类](#显示类) · [Typography](#typography) · [Icon/Img/Svg](#iconimgswg) · [文本元素](#文本元素) · [辅助 trait](#辅助-trait) · [最佳实践](#最佳实践)

## 概述

基础组件全部定义在 `crates/rgpui/src/elements/`，经 `pub use elements::*`
（`rgpui.rs:391`）在 crate 根导出，如 `rgpui::Button`。
**注意：** `mod elements;` 是私有（`rgpui.rs:85`），`rgpui::elements::Button` 非法；
且 **基础组件不在 prelude**（`prelude.rs` 不 glob elements），必须显式
`use rgpui::{Button, Checkbox, ...};`。

示例：`cargo run -p showcase --bin component_gallery`
（`examples/showcase/src/bin/component_gallery.rs`）。

## 导入约定

```rust
// 显式导入（prelude 不含这些）
use rgpui::{
    Button, ButtonVariants, Checkbox, Radio, RadioGroup, Switch, Slider, Toggle,
    Badge, Tag, Separator, Kbd, Spinner, Skeleton, Alert, AlertVariant,
    Avatar, AvatarStatus, Breadcrumb, BreadcrumbItem, Card,
    Title, TitleLevel, Paragraph, TextVariant, Link, Icon, IconName, img, svg,
    Sizable, Selectable, Disableable, ElementSize,
};
```

`Sizable` / `Selectable` / `Disableable` / `StyleSized`（`rgpui.rs:518`）、
`ElementSize`（`rgpui.rs:535`）、`ComponentText`、`ColorName` 均在根导出。

## Button

`Button`（`elements/button.rs:210`），`impl IntoElement`。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new(id: impl Into<ElementId>) -> Self` | `:251` |
| `label` | `fn label(impl Into<SharedString>)` | `:313` |
| `aria_label` | `fn aria_label(impl Into<SharedString>)` | `:322` |
| `icon` | `fn icon(impl Into<ButtonIcon>)` | `:328` |
| `tooltip` | `fn tooltip(impl Into<SharedString>)` | `:334` |
| `tooltip_with_action` | `fn tooltip_with_action(tooltip, action: &dyn Action, context: Option<&str>)` | `:340` |
| `loading` | `fn loading(bool)` | `:357` |
| `loading_icon` | `fn loading_icon(impl Into<Icon>)` | `:386` |
| `compact` | `fn compact()` | `:363` |
| `outline` | `fn outline()` | `:292` |
| `rounded` | `fn rounded(impl Into<ButtonRounded>)` | `:298` |
| `border_corners` | `fn border_corners(Corners<bool>)` | `:307` |
| `dropdown_caret` | `fn dropdown_caret(bool)` | `:408` |
| `toggled` | `fn toggled(bool)` | `:417` |
| `on_click` | `fn on_click(impl Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync + 'static)` | `:369` |
| `on_hover` | `fn on_hover(impl Fn(&bool, &mut Window, &mut App) + 'static)` | `:378` |
| `tab_index` / `tab_stop` | `fn tab_index(isize)` / `fn tab_stop(bool)` | `:394` / `:402` |

变体经 **`ButtonVariants`** trait（`button.rs:50`）链式设置：
`with_variant(ButtonVariant)` / `primary()` / `secondary()` / `danger()` /
`warning()` / `success()` / `info()` / `ghost()` / `link()` / `text()` / `custom(...)`。

`ButtonVariant`（`:150`）：`Default` / `Primary` / `Secondary` / `Danger` /
`Info` / `Success` / `Warning` / `Ghost` / `Link` / `Text` / `Custom(ButtonCustomVariant)`。
`ButtonRounded`（`:19`）：`None` / `Small` / `Default` / `Full`，`impl From<Pixels>`。
`ButtonCustomVariant`（`:41`）：`new(&App)` + `color/foreground/hover/active/shadow`。

```rust
Button::new("save").primary().label("保存").icon(IconName::Check)
    .on_click(cx.listener(|_, _, window, cx| { /* ... */ }))
```

## ButtonIcon

`ButtonIcon`（`button_icon.rs:11`）：`new(impl Into<ButtonIconVariant>)`（`:29`）。
`ButtonIconVariant`（`:61`）：`Icon(Icon)` / `Empty` / `Custom(AnyElement)`，
`impl From<Icon/IconName/Spinner/SharedString/AnyElement>`。`impl Sizable`。

## 选择类

### Checkbox

`Checkbox`（`checkbox.rs:15`）：`new(impl Into<ElementId>)`（`:44`）、
`label(impl Into<ComponentText>)`（`:68`）、`checked(bool)`（`:74`）、
`on_change(impl Fn(&bool, &mut Window, &mut App))`（`:82`）、
`tooltip`（`:62`）、`tab_stop`/`tab_index`（`:91`/`:97`）。
实现 `Disableable`/`Selectable`/`Sizable`。

### Radio / RadioGroup

`Radio`（`radio.rs:18`）：`new(impl Into<ElementId>)`（`:47`）、`label`（`:71`）、
`checked(bool)`（`:77`）、`disabled(bool)`（`:83`）、`on_change`（`:103`）、
`tooltip`（`:65`）、`tab_index`/`tab_stop`（`:89`/`:95`）。

`RadioGroup`（`:316`）：`vertical(id)`（`:348`）/`horizontal(id)`（`:353`）、
`layout(Axis)`（`:358`）、`child(impl Into<Radio>)`（`:387`）/
`children(...)`（`:393`）、`selected_index(Option<usize>)`（`:375`）、
`disabled(bool)`（`:381`）、`on_change(impl Fn(&usize, &mut Window, &mut App))`（`:366`）。

```rust
RadioGroup::vertical("plan")
    .selected_index(Some(0))
    .on_change(cx.listener(|this, ix: &usize, _, cx| { this.plan = *ix; cx.notify(); }))
    .child(Radio::new("free").label("免费"))
    .child(Radio::new("pro").label("专业版"))
```

### Switch

`Switch`（`switch.rs:16`）：`new(impl Into<ElementId>)`（`:45`）、`checked(bool)`（`:64`）、
`label(impl Into<ComponentText>)`（`:70`）、`color(impl Into<Hsla>)`（`:85`）、
`tooltip`（`:91`）、`on_change(impl Fn(bool, &mut Window, &mut App))`（`:76`，按值传新状态）、
`tab_stop(bool)`（`:97`）、`tab_index(isize)`（`:103`）。
实现 `Sizable`/`Disableable`。
键盘：开关自身进 Tab 链，聚焦后 **空格/回车** 切换（`disabled(true)` 时不进焦点链，
并在无障碍树里报 `disabled`）。值语义走 `Toggled`（AccessKit 没有 `Checked` 属性）。

### Toggle / ToggleGroup

`Toggle`（`toggle.rs:10`）：`new(label)`（`:23`）、`pressed(bool)`（`:33`）、
`on_change(impl Fn(&bool, &mut Window, &mut App))`（`:39`）。按下态按 `aria-pressed`
口径公布（`toggled`）。

`ToggleGroup`（`:82`）：`new(Vec<SharedString>)`（`:93`）、
`selected(Option<usize>)`（`:102`）、`on_change(impl Fn(&usize, ...))`（`:108`）。

## Slider

`Slider`（`slider.rs:505`）：`new(&Entity<SliderState>)`（`:518`）、
`horizontal()`（`:528`）/`vertical()`（`:534`）、`disabled(bool)`（`:540`）。
键盘：聚焦后 **←/↓ 减、→/↑ 加**（一步）、`PageUp/PageDown` 十倍步长、`Home/End` 到端点；
范围滑块（`SliderValue::Range`）改的是**右端**且保持 `Range` 形态，右端降到起点以下时停在起点。
辅助技术动作 `Increment`/`Decrement` 与按键同一条路径。

`SliderState`（`:195`，`Clone`）：`new()`（`:216`）、`min(f32)`（`:230`）、
`max(f32)`（`:247`）、`step(f32)`（`:260`）、`scale(SliderScale)`（`:266`）、
`default_value(impl Into<SliderValue>)`（`:283`）、`set_value(...)`（`:290`）、
`value()`（`:302`）、`min_value()`/`max_value()`/`step_value()`（`:307`/`:312`/`:317`）。
`impl EventEmitter<SliderEvent>`（`:501`）。

`SliderValue`（`:56`）：`Single(f32)` / `Range(f32, f32)`，`From<f32>`/`From<(f32,f32)>`/`From<Range<f32>>`，`clamp`/`is_single`/`is_range`/`start`/`end`。
`SliderScale`（`:154`）：`Linear`（默认）/ `Logarithmic`（min 必须 > 0）。
`SliderEvent`（`:42`）：`Change(SliderValue)` / `Release(SliderValue)`。

```rust
let slider = cx.new(|_| SliderState::new().min(0.).max(100.).default_value(30.));
cx.subscribe(&slider, |this, _, ev: &SliderEvent, cx| {
    if let SliderEvent::Change(v) = ev { this.vol = v.start(); cx.notify(); }
}).detach();
// 渲染：Slider::new(&slider).horizontal()
```

## 显示类

| 组件 | 构造 | 关键方法 | 位置 |
|------|------|---------|------|
| `Spinner` | `new()` | `icon`、`color`、`ease`、`Sizable` | `spinner.rs:14/29/44/50/56` |
| `Skeleton` | `new()` | `secondary()` | `skeleton.rs:13/22/30` |
| `Badge` | `new()` | `dot()`、`count(usize)`、`icon`、`max(usize)`、`color` | `badge.rs:36/55/68/76/82/88/94` |
| `Tag` | `new()` | 见下 | `tag.rs:138` |
| `Separator` | `vertical()`/`horizontal()` | `dashed()`、`label`、`color` | `separator.rs:21/38/50/62/72/78` |
| `Alert` | `new(title)` | `body`、`variant`、`closable`、`on_close`、`id` | `alert.rs:25/45/70/64/76/82/58` |
| `Avatar` | `new(fallback)` | `src(impl Into<ImageSource>)`、`size`、`status` | `avatar.rs:23/38/49/55/61` |
| `Breadcrumb` | `new(Vec<BreadcrumbItem>)` | — | `breadcrumb.rs:38/47` |
| `Card` | `new()` | `title(impl Into<SharedString>)`、`ParentElement` | `card.rs:9/20/29` |
| `Kbd` | `new(Keystroke)` | `appearance(bool)`、`outline()`、`binding_for_action*`、`format` | `kbd.rs:11/35/45/51/57/78/95` |

**`Tag`**（`tag.rs:138`）：预设 `primary()`/`secondary()`/`danger()`/`success()`/
`warning()`/`info()`（`:167-192`）、`custom(color,fg,border)`（`:197`）、
`color(impl Into<ColorName>)`（`:206`）、`with_variant(TagVariant)`（`:211`）、
`outline()`（`:217`）、`rounded`/`rounded_full`（`:223`/`:229`）。`TagVariant`（`:12`）：
`Primary`/`Secondary`/`Danger`/`Success`/`Warning`/`Info`/`Color(ColorName)`/
`Custom{color,foreground,border}`。

`BadgeVariant`（`badge.rs:11`）：`Number`/`Dot`/`Icon`（内部，不公开）。
`AlertVariant`（`alert.rs:11`）：`Info`(默认)/`Success`/`Warning`/`Danger`。
`SeparatorStyle`（`separator.rs:11`）：`Solid`(默认)/`Dashed`。
`AvatarStatus`（`avatar.rs:9`）：`None`(默认)/`Online`/`Busy`/`Away`。
`BreadcrumbItem`（`breadcrumb.rs:10`）：`new(label)`（`:19`）、`on_click`（`:27`）。

## Typography

`Title`（`typography.rs:39`）：`new(text)`（`:48`）、`level(TitleLevel)`（`:56`）。
`TitleLevel`（`:10`）：`H1`..`H5`（默认 H2）。
`Paragraph`（`:87`）：`new(text)`（`:96`）、`variant(TextVariant)`（`:104`）。
`TextVariant`（`:73`）：`Body`(默认)/`Secondary`/`Danger`/`Warning`。
`Link`（`:125`）：`new(text)`（`:137`）、`id(impl Into<SharedString>)`（`:146`，可选）、
`on_click(impl Fn(&ClickEvent, &mut Window, &mut App))`（`:152`）。

## Icon/Img/Svg

**Icon**：`Icon`（`icon.rs:52`）：`new(impl Into<Icon>)`（`:97`）、`path(impl Into<SharedString>)`（`:110`）、
`data(&'static [u8])`（`:116`）、`transform(Transformation)`（`:127`）、`rotate(radians)`（`:138`）、
`empty()`（`:133`）、`view(&mut App) -> Entity<Icon>`（`:122`）、`impl Sizable`（`with_size` / `small()` 等）、
`impl Styled`（`text_color`）。未显式设置尺寸时取 `window.text_style()` 字号；`data` 走编译期
嵌入字节，不依赖运行时 `AssetSource`。

三种用法：

1. **内置命名图标**：`Icon::new(IconName::Check)`。`IconName` 由
   `icon_named!(IconName, "assets/icons")`（`icon.rs:29`）从 `crates/rgpui/assets/icons/*.svg`
   生成，变体名为文件名 PascalCase（`arrow-right.svg` → `ArrowRight`），SVG 字节编译期嵌入。
2. **自定义路径 + 数据**：`Icon::default().path("icons/star.svg").data(include_bytes!("…/star.svg"))`。
3. **自定义 `IconNamed`**（`icon.rs:14`）：实现 `path()` 与 `bytes()` 后 `Icon::new(MyIcon)`；
   任何 `impl IconNamed` 也可直接 `Into<Icon>` / `IntoElement` 当即插即用替代。

内置 `IconName`（与 `crates/rgpui/assets/icons` 一一对应，按变体名排序）：
`IconName::ALargeSmall`、`IconName::ArrowDown`、`IconName::ArrowLeft`、`IconName::ArrowRight`、
`IconName::ArrowUp`、`IconName::Asterisk`、`IconName::Battery`、`IconName::BatteryCharging`、
`IconName::BatteryFull`、`IconName::BatteryLow`、`IconName::BatteryMedium`、`IconName::BatteryWarning`、
`IconName::Bell`、`IconName::BookOpen`、`IconName::Bot`、`IconName::Building2`、
`IconName::Calendar`、`IconName::CaseSensitive`、`IconName::ChartPie`、`IconName::Check`、
`IconName::ChevronDown`、`IconName::ChevronLeft`、`IconName::ChevronRight`、`IconName::ChevronUp`、
`IconName::ChevronsUpDown`、`IconName::CircleCheck`、`IconName::CircleUser`、`IconName::CircleX`、
`IconName::Close`、`IconName::Copy`、`IconName::Cpu`、`IconName::Dash`、
`IconName::Delete`、`IconName::Ellipsis`、`IconName::EllipsisVertical`、`IconName::ExternalLink`、
`IconName::Eye`、`IconName::EyeOff`、`IconName::File`、`IconName::Folder`、
`IconName::FolderClosed`、`IconName::FolderOpen`、`IconName::Frame`、`IconName::GalleryVerticalEnd`、
`IconName::Github`、`IconName::Globe`、`IconName::HardDrive`、`IconName::Heart`、
`IconName::HeartOff`、`IconName::Inbox`、`IconName::Info`、`IconName::Inspector`、
`IconName::LayoutDashboard`、`IconName::Loader`、`IconName::LoaderCircle`、`IconName::Map`、
`IconName::Maximize`、`IconName::MemoryStick`、`IconName::Menu`、`IconName::Minimize`、
`IconName::Minus`、`IconName::Moon`、`IconName::Network`、`IconName::Palette`、
`IconName::PanelBottom`、`IconName::PanelBottomOpen`、`IconName::PanelLeft`、`IconName::PanelLeftClose`、
`IconName::PanelLeftOpen`、`IconName::PanelRight`、`IconName::PanelRightClose`、`IconName::PanelRightOpen`、
`IconName::Pause`、`IconName::Play`、`IconName::Plus`、`IconName::Redo`、
`IconName::Redo2`、`IconName::Replace`、`IconName::ResizeCorner`、`IconName::Search`、
`IconName::Settings`、`IconName::Settings2`、`IconName::SortAscending`、`IconName::SortDescending`、
`IconName::SquareTerminal`、`IconName::Star`、`IconName::StarFill`、`IconName::StarOff`、
`IconName::Sun`、`IconName::ThumbsDown`、`IconName::ThumbsUp`、`IconName::TriangleAlert`、
`IconName::Undo`、`IconName::Undo2`、`IconName::User`、`IconName::WindowClose`、
`IconName::WindowMaximize`、`IconName::WindowMinimize`、`IconName::WindowRestore`。

示例：`cargo run -p showcase --bin icons`；`crates/rgpui/tests/guards.rs` 会校验本清单、
`icons.rs` 与 `assets/icons` 三者一致，改图标务必同步这三处。

**Img**：自由函数 `img(source: impl Into<ImageSource>) -> Img`（`img.rs:198`）。
`ImageSource`（`:41`）`From`：`SharedUri`/`String`/`SharedString`/`&Path`/`PathBuf`/
`Arc<RenderImage>`/`Arc<Image>`。`trait StyledImage`（`:147`）：`object_fit`、
`image_cache(&Entity<I>)`。`Img` 实现 `InteractiveElement`/`StatefulInteractiveElement`，
可 `.id()` 后交互。

**Svg**：自由函数 `svg() -> Svg`（`svg.rs:33`）：`path`（`:46`）、
`external_path`（`:52`）、`data(&[u8])`（`:59`）、`with_transformation(Transformation)`（`:80`）。
`Transformation`（`:228`）：`scale`/`translate`/`rotate` + `with_scaling`/`with_translation`/`with_rotation`。

**Canvas**：`canvas<T>(prepaint, paint)`（`canvas.rs:12`），`Canvas::with_dom`（`:43`，需 `dom-backend`）。
**Surface**：`surface(impl Into<SurfaceSource>)`（`surface.rs:33`）+ `object_fit`（`:43`，平台专属）。

## 文本元素

- `Text`（`elements/text.rs:61`）——底层文本元素，`impl IntoElement`；
  大多数场景直接用 `"字符串"`，其 `impl IntoElement`（`:348`），或 `div().child(...)`。
- `StyledText`（`:445`）：`new(impl Into<SharedString>)`（`:460`）、
  `with_default_highlights`/`with_highlights`（`:478`/`:493`）、
  `with_font_family_overrides`（`:548`）、`with_runs(Vec<TextRun>)`（`:586`）。
- `InteractiveText`（`:1197`）：`new(id, StyledText)`（`:1224`）、`on_change`（`:1237`）、
  `on_hover`（`:1257`）、`tooltip`（`:1266`）——可选中/可交互文本。
- `Label`（`label.rs:58`）：`new(label)`（`:73`）、`secondary`（`:85`）、
  `masked(bool)`（`:91`）、`highlights(HighlightsMatch)`（`:97`）。

## 辅助 trait

| trait | 位置 | 提供 |
|-------|------|------|
| `Sizable` | `styled_ext.rs`（根导出 `rgpui.rs:518`） | `with_size(impl Into<ElementSize>)`、`small()/large()/xsmall()` |
| `Selectable` | 同上 | `selected(bool)` |
| `Disableable` | 同上 | `disabled(bool)` |
| `StyleSized` | 同上 | `Style` 上的尺寸辅助 |
| `ElementExt` | `element_ext.rs:44` | `ParentElement` 扩展：`child`/`children` 尺寸包装、`map`/`when`/`when_some`/`when_else`/`debug_selector`/`into_any_element` |
| `InteractiveElementExt` | `element_ext.rs:70` | `on_click`/`on_hover`/`on_mouse_down` 等便捷封装 |
| `ChildElement` | `element_ext.rs:18` | `Sizable + IntoElement` |

`ElementSize`（`rgpui.rs:535`）：`XSmall`/`Small`/`Medium`/`Large`。

## 最佳实践

- **基础组件必须显式 import**：prelude 不含 `Button`/`Checkbox`/`Badge` 等。
- **按钮变体用 `ButtonVariants` trait**：`Button::new(id).primary().label("确定")`。
- **滑块状态是实体**：`cx.new(|_| SliderState::new()...)`，事件走 `SliderEvent`。
- **图标直接用 `IconName::Check` 这类内置变体**（完整清单见上），自定义图标实现 `IconNamed`。
- **图片用 `img(source)`**，需要交互先 `.id()`。
