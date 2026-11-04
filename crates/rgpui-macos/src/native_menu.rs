//! macOS 原生弹出菜单：`NSMenu` + `popUpMenuPositioningItem:atLocation:inView:`。
//!
//! 与状态栏托盘菜单（`tray.rs`）共用同一套构造方式：菜单项把各自的 id 放在
//! `representedObject` 里，被点时由 target 对象上的动作回传。区别在于这里不走应用回调，
//! 弹菜单的调用方在模态循环结束后直接从共享的选中槽里取结果 —— 调用因此是同步的，
//! 由 `window.rs` 放进前台执行器的任务里执行。
//!
//! 菜单挂在窗口的内容视图上（不是 `NSStatusItem`），位置按框架给的客户区坐标换算：
//! 框架原点在主区**左上**，AppKit 视图原点在**左下**，所以纵坐标要按视图高度翻转。

use std::{ffi::c_void, sync::OnceLock};

use cocoa::{
    base::{BOOL, NO, id, nil},
    foundation::{NSAutoreleasePool, NSPoint, NSRect},
};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use rgpui::{NativeMenu, NativeMenuItem, Pixels, Point, SharedString};

use crate::ns_string;

/// 菜单项动作的 target 类，首次使用时注册。
static MENU_TARGET_CLASS: OnceLock<&'static Class> = OnceLock::new();

/// 选中槽在 target 对象上的 ivar 名。
const SELECTION_IVAR: &str = "selection";

/// 按需注册 `GPUINativeMenuTarget` 类。
fn target_class() -> &'static Class {
    MENU_TARGET_CLASS.get_or_init(|| {
        let mut decl = ClassDecl::new("GPUINativeMenuTarget", class!(NSObject))
            .expect("原生菜单 target 类重复注册");
        unsafe {
            decl.add_ivar::<*mut c_void>(SELECTION_IVAR);
            decl.add_method(
                sel!(nativeMenuItemSelected:),
                menu_item_selected as extern "C" fn(&mut Object, Sel, id),
            );
        }
        decl.register()
    })
}

/// 菜单项被点击：把 `representedObject`（菜单项 id）写进选中槽。
///
/// 子菜单标题没有 `representedObject`，点它只是展开子菜单，这里直接返回。
extern "C" fn menu_item_selected(target: &mut Object, _: Sel, sender: id) {
    unsafe {
        let represented: id = msg_send![sender, representedObject];
        if represented == nil {
            return;
        }
        let utf8: *const std::os::raw::c_char = msg_send![represented, UTF8String];
        if utf8.is_null() {
            return;
        }
        let selected = SharedString::from(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        );
        let slot = *target.get_ivar::<*mut c_void>(SELECTION_IVAR) as *mut Option<SharedString>;
        if !slot.is_null() {
            *slot = Some(selected);
        }
    }
}

/// 在窗口内容视图的指定位置弹出原生菜单，返回被选中项的 id。
///
/// `position` 是框架坐标（原点在主区左上，单位逻辑点）。必须在主线程且**没有持有平台锁**
/// 时调用：`popUpMenuPositioningItem:` 进入 AppKit 的模态菜单循环，期间的菜单项回调会再
/// 进平台层取锁。
pub(crate) fn pop_up_menu(
    native_window: id,
    position: Point<Pixels>,
    menu: NativeMenu,
) -> Option<SharedString> {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);

        let content_view: id = msg_send![native_window, contentView];
        if content_view == nil {
            pool.drain();
            return None;
        }
        let frame: NSRect = msg_send![content_view, frame];

        // 选中槽放在堆上：模态循环里的回调按指针写入，这里在循环结束后取回并释放。
        let slot = Box::into_raw(Box::new(None::<SharedString>));

        let target: id = msg_send![target_class(), alloc];
        let target: id = msg_send![target, init];
        (&mut *target).set_ivar(SELECTION_IVAR, slot as *mut c_void);

        let ns_menu: id = msg_send![class!(NSMenu), alloc];
        let ns_menu: id = msg_send![ns_menu, init];
        // 关掉自动启用：禁用态由我们自己标，否则 AppKit 会按「找不到响应者」把整菜单置灰。
        let _: () = msg_send![ns_menu, setAutoenablesItems: NO];
        for item in menu.items() {
            let menu_item = create_menu_item(item, target);
            // addItem: 会 retain，create_menu_item 返回的是 +1 引用
            let _: () = msg_send![ns_menu, addItem: menu_item];
            let _: () = msg_send![menu_item, release];
        }

        let location = NSPoint {
            x: position.x.as_f32() as f64,
            y: (frame.size.height - position.y.as_f32() as f64) as f64,
        };
        // `item: nil` 表示菜单的左上角落在 location 上。
        let _: BOOL = msg_send![
            ns_menu,
            popUpMenuPositioningItem: nil
            atLocation: location
            inView: content_view
        ];

        let _: () = msg_send![ns_menu, release];
        let _: () = msg_send![target, release];
        pool.drain();

        // 收回选中槽（抵掉前面的 `Box::into_raw`），模态循环已经结束。
        *Box::from_raw(slot)
    }
}

/// 由 [`NativeMenuItem`] 造一个 `NSMenuItem`，返回 +1 引用（调用方加入菜单后释放）。
unsafe fn create_menu_item(item: &NativeMenuItem, target: id) -> id {
    unsafe {
        match item {
            NativeMenuItem::Separator => {
                let separator: id = msg_send![class!(NSMenuItem), separatorItem];
                let _: id = msg_send![separator, retain];
                separator
            }
            NativeMenuItem::Action {
                label,
                id,
                disabled,
                checked,
            } => {
                let menu_item: id = msg_send![class!(NSMenuItem), alloc];
                let menu_item: id = msg_send![
                    menu_item,
                    initWithTitle: ns_string(label)
                    action: sel!(nativeMenuItemSelected:)
                    keyEquivalent: ns_string("")
                ];
                let _: () = msg_send![menu_item, setTarget: target];
                let _: () = msg_send![menu_item, setRepresentedObject: ns_string(id)];
                // NSControlStateValueOn == 1 / Off == 0
                let _: () = msg_send![menu_item, setState: if *checked { 1i64 } else { 0i64 }];
                if *disabled {
                    let _: () = msg_send![menu_item, setEnabled: NO];
                }
                menu_item
            }
            NativeMenuItem::Submenu {
                label,
                items: sub_items,
            } => {
                let submenu: id = msg_send![class!(NSMenu), alloc];
                let submenu: id = msg_send![submenu, init];
                let _: () = msg_send![submenu, setAutoenablesItems: NO];
                for sub_item in sub_items {
                    let sub_menu_item = create_menu_item(sub_item, target);
                    let _: () = msg_send![submenu, addItem: sub_menu_item];
                    let _: () = msg_send![sub_menu_item, release];
                }

                let menu_item: id = msg_send![class!(NSMenuItem), alloc];
                let menu_item: id = msg_send![
                    menu_item,
                    initWithTitle: ns_string(label)
                    action: nil
                    keyEquivalent: ns_string("")
                ];
                let _: () = msg_send![menu_item, setSubmenu: submenu];
                let _: () = msg_send![submenu, release];
                menu_item
            }
        }
    }
}
