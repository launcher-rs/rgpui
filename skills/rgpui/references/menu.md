# 菜单与通知（Menu）

**目录：** [概述](#概述) · [PopupMenu](#popupmenu) · [ContextMenu](#contextmenu) · [DropdownMenu](#dropdownmenu) · [Popover/Popconfirm/HoverCard](#popoverpopconfirmhovercard) · [AppMenuBar](#appmenubar) · [Notification](#notification) · [全局状态](#全局状态) · [最佳实践](#最佳实践)

## 概述

菜单子系统位于 `crates/rgpui/src/menu/`，`pub mod menu;`（`rgpui.rs:211`）+
`pub use menu::*`（`rgpui.rs:530`）→ 既可 `rgpui::menu::PopupMenu` 也可
`rgpui::PopupMenu`。**与 `dialog` 一样均不在 prelude。**

导出（`menu/mod.rs:15-24`）：`actions::*`、`AppMenuBar`、`ContextMenu`/`ContextMenuExt`/
`ContextMenuState`、`DropdownMenu`、`GlobalState`、`HoverCard`/`HoverCardState`、
`Notification`/`NotificationId`/`NotificationList`/`NotificationType`、`Popconfirm`、
`Popover`、`PopupMenu`/`PopupMenuItem`。

初始化：`rgpui::menu::init(cx)`（`menu/mod.rs:27`）= `GlobalState::new()` +
`popup_menu::init` + `app_menu_bar::init` + `popover::init`。

> **纠偏：无 `menu/global.rs`**（全局状态在 `menu/global_state.rs`）。
> **无 `menu::MenuBar`** —— 唯一菜单栏组件是 `menu::AppMenuBar`（`app_menu_bar.rs:27`）;
> `app.rs:2486` 注释里的「`menu::MenuBar`」是过时措辞。**Toast 不在 menu**：
> `Toast`/`ToastStack`/`ToastPosition` 在 `elements/toast.rs`。

## PopupMenu

`PopupMenu`（`popup_menu.rs:318`）。构建入口：

```rust
PopupMenu::build(window, cx, |menu, window, cx| {
    menu.item(PopupMenuItem::new("新建").action(Box::new(NewFile)))
        .separator()
        .submenu("导出", export_menu)
}) // -> Entity<PopupMenu>
```

`build(window, cx, builder)`（`:381`）。> `PopupMenu::new` 是 `pub(crate)`（`:357`），
应用层只能走 `build`。

**构建器方法**（`PopupMenu`）：`item(impl Into<PopupMenuItem>)`（`:744`）、
`menu(label, action)`（`:481`）、`menu_with_enable`（`:486`）、`menu_with_disabled`（`:497`）、
`menu_with_icon`（`:559`）、`menu_with_icon_and_disabled`（`:569`）、
`menu_with_check`（`:581`）、`menu_with_check_and_disabled`（`:591`）、
`menu_element(action, builder)`（`:603`）及其 `_with_disabled`/`_with_icon`/`_with_check` 变体、
`label`（`:508`）、`link(label, href)`（`:514`）及 `link_with_disabled`/`link_with_icon`、
`separator()`（`:696`）、`submenu(label, entity)`（`:710`）、`submenu_with_icon`（`:721`）、
`action_context(FocusHandle)`（`:394`）、`min_w(Pixels)`（`:433`）、`max_w`（`:439`）、
`max_h`（`:455`）、`scrollable(bool)`（`:463`）、`check_side(Side)`（`:469`）、
`external_link_icon(bool)`（`:475`）、`rebuild(...)`（`:751`）、`is_empty()`（`:841`）。
`impl Focusable`/`EventEmitter<DismissEvent>`/`FluentBuilder`。

`PopupMenuItem`（`:33`）：`new(label)`（`:91`）、`element(builder)`（`:106`）、
`submenu(label, Entity<PopupMenu>)`（`:123`）、`separator()`（`:134`）、
`label(label)`（`:140`）、`icon(impl Into<Icon>)`（`:147`）、`action(Box<dyn Action>)`（`:166`）、
`disabled(bool)`（`:182`）、`checked(bool)`（`:201`）、`danger(bool)`（`:217`）、
`on_click(handler)`（`:230`）、`link(label, href)`（`:248`）、`on_hover`（`menu_item.rs:71`）。

## ContextMenu

`ContextMenuExt`（`context_menu.rs:15`）为任意 `InteractiveElement + ParentElement + Styled` 提供：

```rust
div().context_menu(|menu, window, cx| {
    menu.item(PopupMenuItem::new("复制").action(Box::new(Copy)))
})
```

`context_menu(f)`（`:21`，`#[track_caller]`）→ `ContextMenu<Self>`。
`ContextMenu::new(id, element)`（`:54`，底层）。id 跨渲染稳定（否则打开的菜单会丢）。

## DropdownMenu

`DropdownMenu`（`dropdown_menu.rs:13`）为 `Button` 等提供：

```rust
Button::new("more").label("更多").dropdown_menu(|menu, window, cx| menu.item(/* ... */))
```

`dropdown_menu(f)`（`:15`，锚 TopLeft）、`dropdown_menu_with_anchor(anchor, f)`（`:23`）。
返回 `DropdownMenuPopover`（`:39`）：`anchor`（`:70`）、`match_trigger_width(bool)`（`:78`）。
`impl DropdownMenu for Button {}`（`:35`）。

## Popover/Popconfirm/HoverCard

**Popover**（`popover.rs:24`）：`new(impl Into<ElementId>)`（`:56`）、`anchor`（`:81`）、
`mouse_button(MouseButton)`（`:87`）、`trigger(T)`（`:93`）、`default_open(bool)`（`:109`）、
`open(bool)`（`:119`）、`on_open_change(callback)`（`:129`）、`overlay_closable(bool)`（`:144`）、
`match_trigger_width(bool)`（`:150`）、`content(builder)`（`:159`）、`appearance(bool)`（`:176`）、
`track_focus(&FocusHandle)`（`:186`）。`PopoverState`（`:228`）：`new(default_open, cx)`（`:242`）、
`is_open()`（`:256`）、`dismiss`（`:268`）、`show`（`:275`）。

**Popconfirm**（`popconfirm.rs:11`）：`new(trigger_label, message)`（`:30`）、`id`（`:42`）、
`on_confirm`（`:48`）、`on_cancel`（`:57`）。

**HoverCard**（`hover_card.rs:17`）：`new(id)`（`:37`）、`anchor`（`:53`）、`trigger`（`:59`）、
`content`（`:70`）、`open_delay(Duration)`（`:82`）、`close_delay(Duration)`（`:88`）、
`appearance`（`:94`）、`on_open_change`（`:100`）。`HoverCardState`（`:122`）：`is_open()`（`:158`）。

## AppMenuBar

`AppMenuBar`（`app_menu_bar.rs:27`）：`new(cx) -> Entity<Self>`（`:35`）、`reload(cx)`（`:48`），
`impl Render`（`:120`）。从 `GlobalState::app_menus()` 取菜单渲染为应用内菜单栏
（走常规 action 派发）。

**原生菜单栏**（平台层）走 `App`：
- `App::set_menus(menus)`（`app.rs:2471`）—— 替换应用菜单栏；
- `App::get_menus()`（`:2477`）；
- `App::on_app_menu_action(impl FnMut(&dyn Action, &mut App))`（`:2487`）—— 平台原生
  菜单项被选中时回调（**注意需与 `menu::AppMenuBar` 区分**）。

## Notification

`Notification`（`notification.rs:79`）：`new()`（`:137`）、`message`（`:158`）、
`info`/`success`/`warning`/`error(msg)`（`:164`/`:171`/`:178`/`:185`）、`id::<T>()`（`:197`）、
`id1::<T>(key)`（`:203`）、`title`（`:211`）、`icon`（`:219`）、`with_type(NotificationType)`（`:225`）、
`autohide(bool)`（`:231`）、`on_click`（`:237`）、`on_close`（`:249`）、`action(f)`（`:260`）。
`NotificationType`（`:24`）。`impl EventEmitter<DismissEvent>`/`FluentBuilder`/`Styled`/`Render`。

`NotificationList`（`:455`）：`new(window, cx)`（`:464`）、`push(notification, window, cx)`（`:473`）、
`close`（`:514`）、`close_by_type`（`:528`）、`clear`（`:544`）、`notifications()`（`:550`）。
id 相同会去重；`autohide` 默认 5 秒后自动隐藏（`:497`）。

## 全局状态

`GlobalState`（`global_state.rs:8`，`impl Global` `:23`）：`global(cx)`（`:38`）、
`global_mut(cx)`（`:46`）、`app_menus() -> &[OwnedMenu]`（`:66`）、
`set_app_menus(Vec<OwnedMenu>)`（`:71`）、`suppress_text_selection(cx)`（`:76`）、
`clear_suppress_text_selection`（`:81`）、`is_suppress_text_selection(cx)`（`:86`）。

菜单 action（`actions.rs:14`）：`Confirm{secondary}`（`:9`）与
`Cancel`/`SelectUp`/`SelectDown`/`SelectLeft`/`SelectRight`/`SelectFirst`/`SelectLast`/
`SelectPrevColumn`/`SelectNextColumn`/`SelectPageUp`/`SelectPageDown`。

## 最佳实践

- **`PopupMenu` 只能 `build`**；`new` 是 crate 内 API。
- **`ContextMenu`/`DropdownMenu` 走扩展 trait**：元素上直接 `.context_menu(...)` /
  `.dropdown_menu(...)`，无需手动搭。
- **菜单 id 跨渲染稳定**：`context_menu` 已自动处理；手动 `ContextMenu::new` 需注意。
- **原生 vs 应用内菜单**：应用内用 `AppMenuBar`（普通 action），原生用
  `App::set_menus` + `on_app_menu_action`。
- **通知去重靠 id**：`Notification::id::<T>()`，同 id 覆盖。
- **注意区分**：`menu::Notification`（应用内通知条，`NotificationList` 管理）与
  `components::NotificationCenterState`/`NotificationItem`（通知中心，见
  `components/notification_center.rs`）、`elements::Toast`（轻提示，见 `elements/toast.rs`）。
