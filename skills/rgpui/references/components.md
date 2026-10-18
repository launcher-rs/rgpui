# 扩展组件（Components）

**目录：** [概述](#概述) · [导入与初始化](#导入与初始化) · [选择类](#选择类) · [日期/颜色/命令](#日期颜色命令) · [布局与导览](#布局与导览) · [数据展示](#数据展示) · [输入类](#输入类) · [反馈/动画](#反馈动画) · [feature 门控组件](#feature-门控组件) · [最佳实践](#最佳实践)

## 概述

扩展组件库位于 `crates/rgpui/src/components/`，原独立 `rgpui-ui` crate 并入核心
（`components/mod.rs:1-8`）。`pub mod components;`（`rgpui.rs:246`）→ 可用
`rgpui::components::X`，多数经 `pub use components::*`（`rgpui.rs:525`）也在 crate 根；
**prelude 含 `pub use crate::components::*`**（`prelude.rs:21`），故多数可直接用。

约 60 个组件。feature 门控：`charts`（图表，`mod.rs:86-87`/`167-168`）、
`effects`（aurora/confetti/marquee/particle_emitter/ripple/shimmer/pulse_indicator）、
`qr-code`、`dom-backend`（dom_svg）。

## 导入与初始化

`components::init(cx)`（`mod.rs:171`）注册 inline_edit / otp_input / image_viewer /
tree 的快捷键。由统一 `init.rs:30` 调用。

## 选择类

**Select**（`select.rs:11`）：`new(Vec<SharedString>)`（`:30`）、`id`（`:42`）、
`selected(Option<usize>)`（`:48`）、`placeholder`（`:54`）、`on_change`（`:60`），`impl Styled`。
下拉（DropdownMenu）里 `match_trigger_width(true)` 对齐宽度。

**Combobox**（`combobox.rs:14`）：`new(window, cx)`（`:37`）、`items(Vec<SharedString>)`（`:73`）、
`multiple(bool)`（`:81`）、`on_change`（`:87`）、`input() -> &Entity<InputState>`（`:96`）、
`selected() -> &[usize]`（`:101`），`impl Render`（`:143`）。

**SegmentedNav**（`segmented_nav.rs:105`）：`new(id, Entity<SegmentedNavState>)`（`:124`）、
`item(id, label)`（`:137`）、`size(SegmentedNavSize)`（`:146`）、`duration(Duration)`（`:152`）、
`on_change`（`:158`）。状态 `SegmentedNavState::new(active)`（`:72`）。

**NavigationMenu**（`navigation_menu.rs:71`）：`new()`（`:90`）、`orientation(...)`（`:103`）、
`item(NavigationMenuItem<T>)`（`:109`）/`items`（`:115`）、`selected_id(T)`（`:121`）、
`expanded_ids(Vec<T>)`（`:127`）、`on_change`（`:133`）、`on_toggle`（`:142`）。
`NavigationMenuItem<T>`（`:20`）：`new(id, label)`（`:35`）、`with_icon`（`:46`）、
`disabled`（`:52`）、`with_children`（`:58`）。

**TagInput**（`tag_input.rs:147`）：`new(Entity<TagInputState>)`（`:164`）、`placeholder`（`:176`）、
`disabled`（`:182`）、`suggestions`（`:188`）、`on_change`（`:194`）。
状态 `TagInputState`（`:9`）：`new`（`:20`）、`with_tags`（`:30`）、`tags`（`:44`）、
`set_tags`（`:49`）、`add_tag`（`:55`）、`remove_tag`（`:74`）、`clear_tags`（`:90`）、
`set_max_tags`（`:120`）。

## 日期/颜色/命令

**DatePicker**（`date_picker.rs:17`）：`new()`（`:30`）、`select(NaiveDate)`（`:41`）、
`on_change`（`:48`）、`selected() -> Option<NaiveDate>`（`:57`）。

**ColorPicker**（`color_picker.rs:14`）：`new(window, cx)`（`:35`）、`color(Hsla)`（`:90`）、
`on_change`（`:96`）、`color_value() -> Hsla`（`:105`）。

**CommandPalette**（`command_palette.rs:220`）：`new(window, cx, Vec<Command>)`（`:235`）、
`on_close`（`:264`）。`CommandPaletteState`（`:130`）：`new(commands)`（`:145`）、
`update_search(String)`（`:158`）、`select_previous`/`select_next`（`:179`/`:186`）、
`execute_selected(window, cx) -> bool`（`:193`）、`filtered_commands()`（`:208`）、
`selected_index()`（`:213`）。`Command`（`:17`）：`new(id, name)`（`:38`）、`description`（`:56`）、
`icon`（`:64`）、`category`（`:70`）、`shortcut`（`:76`）、`on_select`（`:82`）。

## 布局与导览

**SplitPane**（`split_pane.rs`）：`SplitPane::new(Entity<SplitPaneState>)`（`:271`）、
`show_collapse_buttons(bool)`、`on_resize`；状态 `SplitPaneState::new(cx)`（`:80`）。
枚举 `SplitDirection`（`:15`）、`SplitPaneEvent`（`:25`）、`CollapsiblePane`（`:45`）。
`impl EventEmitter<SplitPaneEvent>`（`:242`）。

**Resizable**（`resizable.rs`）：`ResizablePanelGroup`（`:285`）：`axis(Axis)`（`:308`）、
`group(ResizablePanelGroup)`（`:329`）；`ResizablePanel`（`:394`）：`child`（`:429`）、
`visible(bool)`（`:435`）、`size`（`:441`）、`size_range`（`:447`）、`min_size`/`max_size`（`:453`/`:459`）。
状态 `ResizableState::new(cx)`（`:58`）：`insert_panel`（`:69`）、`remove_panel`（`:92`）、
`sizes`（`:112`）、`total_size`（`:117`）。`ResizablePanelEvent`（`:31`）。

**DockArea**（`dock.rs:321`）：`new(Entity<DockAreaState>)`（`:328`）。状态
`DockAreaState`（`:97`）：`new`（`:108`）、`add_panel`（`:120`）、`remove_panel`（`:137`）、
`move_panel`（`:159`）、`set_active`（`:169`）、`set_visible`（`:181`）、`is_visible`（`:194`）、
`set_left_width`/`set_right_width`/`set_bottom_height`（`:201`/`:207`/`:213`）、
`layout_json()`（`:240`）、`restore_layout(json, cx)`（`:270`）。
`DockPanel::new(...)`（`:74`）、`closable(bool)`（`:90`）。`DockPosition`（`:14`）。

**Sidebar**（`sidebar.rs:94`）：`new()`（`:118`）。`SidebarSection::new(title, Vec<SidebarItem>)`（`:77`）。
`SidebarItem::new(id, label)`（`:28`）。

**ViewRouter**（`view_router.rs:182`）：`new(id, Entity<ViewRouterState>)`（`:195`）、
`transition(PageTransition)`（`:205`）。状态（`:48`）：`new`（`:67`）、`push`（`:80`）、
`pop`（`:97`）、`replace`（`:108`）、`current_id`（`:155`）、`can_go_back`（`:160`）、
`depth`（`:165`）。`PageTransition`（`:10`）。

**BottomSheet**（`bottom_sheet.rs:37`）：`new()`（`:62`）、`size(BottomSheetSize)`（`:78`）、
`height`（`:84`）、`title`（`:91`）、`description`（`:97`）、`content`（`:103`）、
`actions`（`:109`）、`show_drag_handle`（`:115`）、`close_on_backdrop_click`（`:121`）、
`on_close`（`:127`）。

**DrawerNavigation**（`drawer_navigation.rs`）、**AppMenu**（`components/app_menu.rs`，
`AppMenuBar::new()` `:13`，与 `menu::AppMenuBar` 区分）、**Tree**（`rgpui::tree`，见下）。

## 数据展示

**Tree**（`pub mod tree;` `rgpui.rs:260`）：自由函数 `tree(state, render_item)`（`tree.rs:57`）、
`Tree::new(state, render_item)`（`:574`）。`TreeState::new(cx)`（`:236`）、
`TreeItem::new(id, label)`（`:150`）、`TreeEvent`（`:127`）、`TreeEntry`（`:83`）。
初始化 `tree::init(cx)`（`:26`）。

**StatusBar**（`status_bar.rs:103`）：`new()`（`:111`）。`StatusBarItem::new(label)`（`:41`）。

**Steps**（`steps.rs:34`）：`new(Vec<StepItem>, current)`（`:45`）。`StepItem::new(title)`（`:18`）。

**Timeline**（`timeline.rs:46`）：`new(Vec<TimelineItem>)`（`:55`）。
`TimelineItem::new(time, title)`（`:22`）。

**DescriptionList**（`description_list.rs:112`）：`new()`（`:131`）、`vertical()`（`:144`）、
`horizontal()`（`:149`）、`label_width`（`:154`）、`columns(usize)`（`:172`）、
`item`（`:178`）、`child`/`children`（`:193`/`:199`）、`separator()`（`:209`）。

**GroupBox**（`group_box.rs:43`）：`new()`（`:64`）、`title`（`:84`）、`footer`（`:102`）。
`GroupBoxVariant`（`:10`）。

**EmptyState**（`empty_state.rs:58`）：`new(id, title)`（`:85`）、`icon`（`:99`）、
`description`（`:105`）、`action`（`:111`）、`secondary_action`（`:121`）、`size`（`:131`）。

**ImageViewer**（`image_viewer.rs:214`）：`new(Entity<ImageViewerState>, cx)`（`:237`）、
`on_close`（`:252`）、`close_on_backdrop_click`（`:258`）、`close_on_escape`（`:264`）、
`show_controls`（`:270`）、`show_counter`（`:276`）、`show_thumbnails`（`:282`）。
状态（`:77`）：`new(Vec<ImageItem>)`（`:100`）、`go_to`/`next`/`prev`（`:122`/`:130`/`:138`）、
`zoom_in`/`zoom_out`/`reset_zoom`（`:146`/`:151`/`:156`）、`set_zoom`（`:162`）、
`toggle_thumbnails`（`:167`）。`ImageItem::new(src)`（`:34`）。

**SearchPanelState**（`search_panel.rs:444`）：`new(window, cx)`（`:479`）、
`search_only(window, cx)`（`:541`）、`on_navigate`（`:585`）、`on_replace`（`:594`）、
`on_replace_all`（`:603`）、`on_close`（`:612`）、`set_source(String, cx)`（`:637`）、
`attach_editor(&Entity<InputState>, cx)`（`:656`）、`set_highlight_colors`（`:693`）、
`focus_search_input`（`:706`）、`set_show_replace`（`:721`）、`search_input()`（`:762`）、
`state()`（`:767`）。配套 `SearchState`（`:40`）、`SearchOptions`（`:17`）、
`SearchMatch`（`:28`）、`SearchHighlight`（`:301`）。

**MermaidDiagram**（`mermaid.rs:552`）：`new(source)`（`:559`）。
**SVGRenderer**（`svg_renderer.rs:162`）：`new()`（`:179`）。
**Sparkline**（`sparkline.rs:182`）：`new(Vec<f64>)`（`:209`）。
**Waveform**（`waveform.rs:18`）：`new()`（`:37`）。
**NumberTicker**（`number_ticker.rs:11`）：`new(id, value: i64)`（`:23`）。
**Countdown**（`countdown.rs:343`）：`new(id, Entity<CountdownState>)`（`:355`）。
**Carousel**（`carousel.rs:158`）：`new(Entity<CarouselState>)`（`:168`）、`arrows`（`:179`）、
`dots`（`:185`）。状态（`:12`）：`new`（`:31`）、`autoplay(Duration)`（`:44`）、`looping`（`:50`）、
`next`/`prev`/`go_to`（`:70`/`:83`/`:96`）。
**Rate**（`rate.rs:10`）：`new()`（`:31`）。
**Pagination**（`pagination.rs:11`）：`new(current, total)`（`:29`）。
**Clipboard**（`clipboard.rs:12`）：`new(id)`（`:31`）。

## 输入类

**OTPInput**（`otp_input.rs:344`）：`new(&Entity<OTPState>)`（`:367`）、`size(OTPInputSize)`（`:382`）、
`disabled`（`:388`）、`masked`（`:394`）、`separator`（`:400`）、`separator_position`（`:406`）、
`on_change`（`:412`）、`on_complete`（`:421`）。状态 `OTPState`（`:76`）：`new(cx, digit_count)`（`:97`）、
`digit_count`（`:113`）、`masked`（`:121`）、`value`（`:127`）、`is_complete`（`:132`）、
`set_value`（`:137`）、`clear`（`:156`）、`set_state(OTPInputState)`（`:164`）、`set_error`/`set_success`（`:170`/`:176`）。
`OTPInputEvent`（`:40`）、`OTPInputSize`（`:53`）、`OTPInputState`（`:65`）。
action：`OTPBackspace`/`OTPDelete`/`OTPLeft`/`OTPRight`/`OTPHome`/`OTPEnd`/`OTPPaste`/`OTPEscape`。

**InlineEdit**（`inline_edit.rs:808`）：`new(Entity<InlineEditState>)`（`:827`）、`placeholder`（`:840`）、
`disabled`（`:846`）、`trigger(InlineEditTrigger)`（`:852`）、`on_save`（`:858`）、`on_cancel`（`:867`）。
状态 `InlineEditState`（`:39`）：`new(cx)`（`:64`）、`with_value`（`:80`）、`value`（`:97`）、
`set_value`（`:102`）、`is_editing`（`:109`）、`start_editing`（`:114`）、`save`（`:127`）、`cancel`（`:138`）。
`InlineEditBlurBehavior`（`:30`）。

**HotkeyInput**（`hotkey_input.rs:209`）：`new(Entity<HotkeyInputState>)`（`:226`）、`placeholder`（`:238`）、
`recording_text`（`:244`）、`disabled`（`:250`）、`on_change`（`:256`）。
状态（`:109`）：`new(cx)`（`:120`）、`with_hotkey`（`:129`）、`hotkey`（`:138`）、
`set_hotkey`（`:143`）、`start_recording`（`:155`）、`stop_recording`（`:161`）、`clear`（`:167`）。
`HotkeyValue`（`:9`）：`new(key, Modifiers)`（`:18`）、`from_keystroke`（`:26`）、`format_display`（`:47`）。

## 反馈/动画

**InfiniteScroll**（`infinite_scroll.rs:92`）：`new(Entity<InfiniteScrollState>)`（`:114`）、
`threshold(f32)`（`:137`）、`on_load_more`（`:143`）。状态（`:22`）：`set_loading`（`:55`）、
`set_loaded`（`:60`）、`set_error`（`:67`）、`set_end_reached`（`:72`）。`LoadingState`（`:8`）。

**SortableList**（`sortable_list.rs:92`）：`on_reorder`（`:124`）、`direction(Axis)`（`:133`）、
`gap`（`:139`）。状态 `SortableListState<T>`（`:50`）。

**DragDrop**（`drag_drop.rs`）：`Draggable<T>::new(id, DragData<T>)`（`:141`）、
`DropZone<T>::new(id)`（`:250`）。`DragData<T>::new(data)`（`:42`）、`DropZoneStyle`（`:218`）。

**AnimatedXxx**：`AnimatedCollapsible`、`AnimatedCounter`、`AnimatedList`、`AnimatedPresence`、
`AnimatedProgress`、`AnimatedSwitch`、`AnimatedText`（各 `components/animated_*.rs`）。
**LayoutTransition**（`layout_transition.rs:26`）：`new(id)`（`:45`），`LayoutAnimation`（`:10`）。
**TypeWriter**（`type_writer.rs:129`）：`new(id, Entity<TypeWriterState>)`（`:139`）；
状态 `new(text)`（`:20`）。**TextReveal**（`text_reveal.rs:20`）：`new(id, text)`（`:35`），
`RevealMode`（`:9`）。**Spotlight**（`spotlight.rs:20`）：`new(id, Entity<SpotlightState>)`（`:39`）、
`color`（`:52`）、`size`（`:58`）、`intensity`（`:64`）。**CanvasComponent**（`canvas_component.rs:13`）：
`new(id)`（`:26`）。**NotificationCenter**（`notification_center.rs`）：`NotificationCenterState`（`:133`）：
`new(cx)`（`:142`）、`add`（`:150`）、`remove`（`:156`）、`mark_read`（`:162`）；
`NotificationItem`（`:54`）、`NotificationVariant`（`:9`）。

## feature 门控组件

| feature | 组件 | 位置 |
|---------|------|------|
| `charts` | 图表 11 个（`charts` 子模块） | `components/charts/` |
| `effects` | `Aurora`、`Confetti`、`Marquee`、`ParticleEmitter`、`Ripple`、`Shimmer`、`PulseIndicator` | `components/{aurora,confetti,marquee,particle_emitter,ripple,shimmer,pulse_indicator}.rs` |
| `qr-code` | `QrCode` | `components/qr_code.rs` |
| `dom-backend` | `dom_svg`（内部） | `components/dom_svg.rs` |

启用示例：`cargo run -p showcase --all-features --bin charts` / `--bin effects` / `--bin qr_code`。

## 最佳实践

- **多数组件在 prelude**（`components::*`）；个别未 glob 的用 `rgpui::components::X`。
- **状态式组件用实体**：`cx.new(|cx| XxxState::new(...))`，元素持 `&Entity<...>`。
- **feature 门控组件**（charts/effects/qr-code）需在依赖开对应 feature。
- **区分同名**：`components::AppMenuBar`（纯应用内）vs `menu::AppMenuBar`（读
  `GlobalState`）；`components::NotificationCenterState` vs `menu::Notification`。
- **图表/特效独立参考**：见 `feature-components.md`。
