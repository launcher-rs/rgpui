//! macOS 状态栏图标（菜单栏右上角）：`NSStatusItem` + `NSMenu`。
//!
//! 与 Windows 的 `Shell_NotifyIconW` 对齐的行为：
//! - `set_tray_icon(Some(bytes))` 装图标（PNG 等 `NSImage` 支持的格式），`None` 移除图标；
//! - `set_tray_tooltip` 设置悬停提示；
//! - `set_tray_menu` 设置点击后弹出的原生菜单，菜单项按 id 回传（`TrayMenuItem::Action`
//!   / `Toggle` / `Submenu` / `Separator` 全部支持，`Toggle` 显示勾选态）；
//! - 点击图标派发 `TrayIconEvent`（左键 / 右键 / 双击），随后弹菜单 —— 菜单挂在按钮的
//!   action 上而不是 `NSStatusItem.setMenu:`，所以事件与菜单可以同时工作。
//!
//! 状态栏图标的尺寸按 macOS 习惯固定为 18×18 点（Retina 下由 AppKit 自动放大）。

use std::{ffi::c_void, sync::OnceLock};

use cocoa::{
    base::{BOOL, NO, YES, id, nil},
    foundation::{NSAutoreleasePool, NSSize, NSUInteger},
};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use rgpui::{SharedString, TrayIconEvent, TrayMenuItem};

use crate::{ns_string, platform::MacPlatform};

/// 状态栏按钮的 target 对象：把原生点击与菜单项动作转回 rgpui。
static STATUS_TARGET_CLASS: OnceLock<&'static Class> = OnceLock::new();

/// 平台指针在 target 对象上的 ivar 名（与 AppDelegate 同一约定）。
const MAC_PLATFORM_IVAR: &str = "platform";

/// 状态栏图标的标准高度（点）。
const STATUS_ICON_SIZE: f64 = 18.0;

/// 按需注册 `GPUIStatusItemTarget` 类。
fn target_class() -> &'static Class {
    STATUS_TARGET_CLASS.get_or_init(|| {
        let mut decl = ClassDecl::new("GPUIStatusItemTarget", class!(NSObject))
            .expect("状态栏 target 类重复注册");
        unsafe {
            decl.add_ivar::<*mut c_void>(MAC_PLATFORM_IVAR);
            decl.add_method(
                sel!(trayClicked:),
                tray_clicked as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(trayMenuItem:),
                tray_menu_item_clicked as extern "C" fn(&mut Object, Sel, id),
            );
        }
        decl.register()
    })
}

/// 从 target 对象上取回平台引用。
///
/// 平台对象由 `Arc<MacPlatform>` 持有，生命周期长于状态栏图标，因此这里用裸指针回读。
unsafe fn platform_from(target: &Object) -> Option<&'static MacPlatform> {
    let pointer: *mut c_void = unsafe { *target.get_ivar(MAC_PLATFORM_IVAR) };
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { &*(pointer as *const MacPlatform) })
    }
}

/// 状态栏按钮被点击：派发托盘事件，然后弹菜单（菜单在平台侧统一处理）。
extern "C" fn tray_clicked(target: &mut Object, _: Sel, _sender: id) {
    let Some(platform) = (unsafe { platform_from(target) }) else {
        return;
    };

    unsafe {
        let application: id = msg_send![class!(NSApplication), sharedApplication];
        let event: id = msg_send![application, currentEvent];
        let event = if event == nil {
            TrayIconEvent::LeftClick
        } else {
            let event_type: i64 = msg_send![event, type];
            let click_count: i64 = msg_send![event, clickCount];
            match event_type {
                // NSEventTypeRightMouseDown / RightMouseUp
                3 | 4 => TrayIconEvent::RightClick,
                _ if click_count >= 2 => TrayIconEvent::DoubleClick,
                _ => TrayIconEvent::LeftClick,
            }
        };
        platform.handle_tray_icon_event(event);
    }
}

/// 菜单项被点击：把 `representedObject`（菜单项 id）交给应用。
extern "C" fn tray_menu_item_clicked(target: &mut Object, _: Sel, sender: id) {
    let Some(platform) = (unsafe { platform_from(target) }) else {
        return;
    };

    unsafe {
        // 子菜单标题项没有 representedObject，点它只是展开子菜单
        let represented: id = msg_send![sender, representedObject];
        if represented == nil {
            return;
        }
        let utf8: *const std::os::raw::c_char = msg_send![represented, UTF8String];
        if utf8.is_null() {
            return;
        }
        let id = std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned();
        platform.handle_tray_menu_action(SharedString::from(id));
    }
}

/// 状态栏图标本体。
pub(crate) struct MacTray {
    /// 按钮与菜单项共用的 target 对象（alloc/init 保留，Drop 时释放）
    target: id,
    /// 状态栏图标，`None` 表示当前没装图标
    status_item: Option<id>,
    /// 点击后弹出的菜单（同上，`None` 表示没有菜单）
    menu: Option<id>,
}

impl MacTray {
    /// 创建托盘对象；`platform` 是回传给点击处理器的平台指针。
    pub(crate) fn new(platform: *mut c_void) -> Self {
        unsafe {
            let target: id = msg_send![target_class(), alloc];
            let target: id = msg_send![target, init];
            (&mut *target).set_ivar(MAC_PLATFORM_IVAR, platform);
            Self {
                target,
                status_item: None,
                menu: None,
            }
        }
    }

    /// 装图标；`None` 把图标从菜单栏移除。
    ///
    /// 图标字节直接交给 `NSImage`，PNG 最稳妥；macOS 不认 ICO，解码失败时记一条警告并清空图像。
    pub(crate) fn set_icon(&mut self, icon: Option<&[u8]>) {
        let Some(icon) = icon else {
            self.remove_status_item();
            return;
        };

        self.ensure_status_item();
        let Some(status_item) = self.status_item else {
            return;
        };

        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let data: id = msg_send![
                class!(NSData),
                dataWithBytes: icon.as_ptr() as *const c_void
                length: icon.len() as NSUInteger
            ];
            let image: id = msg_send![class!(NSImage), alloc];
            let image: id = msg_send![image, initWithData: data];
            let button: id = msg_send![status_item, button];

            if image == nil {
                log::warn!(
                    "托盘图标解码失败：macOS 只接受 NSImage 支持的格式（PNG/PDF 等），ICO 不可用"
                );
                let _: () = msg_send![button, setImage: nil];
            } else {
                let size = NSSize {
                    width: STATUS_ICON_SIZE,
                    height: STATUS_ICON_SIZE,
                };
                let _: () = msg_send![image, setSize: size];
                let _: () = msg_send![button, setImage: image];
                let _: () = msg_send![image, release];
            }

            pool.drain();
        }
    }

    /// 设置悬停提示文本。
    pub(crate) fn set_tooltip(&mut self, tooltip: &str) {
        self.ensure_status_item();
        let Some(status_item) = self.status_item else {
            return;
        };

        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let button: id = msg_send![status_item, button];
            let _: () = msg_send![button, setToolTip: ns_string(tooltip)];
            pool.drain();
        }
    }

    /// 设置点击后弹出的菜单；空列表表示不弹菜单。
    ///
    /// 注意不调用 `NSStatusItem.setMenu:` —— 那会把按钮的 action 吃掉，点击事件就没了。
    pub(crate) fn set_menu(&mut self, items: Vec<TrayMenuItem>) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            if let Some(menu) = self.menu.take() {
                let _: () = msg_send![menu, release];
            }

            if items.is_empty() {
                pool.drain();
                return;
            }

            let menu: id = msg_send![class!(NSMenu), alloc];
            let menu: id = msg_send![menu, init];
            let _: () = msg_send![menu, setAutoenablesItems: NO];

            for item in &items {
                let menu_item = create_menu_item(item, self.target);
                let _: () = msg_send![menu, addItem: menu_item];
                // addItem: 会 retain，create_menu_item 返回的是 +1 引用
                let _: () = msg_send![menu_item, release];
            }

            self.menu = Some(menu);
            pool.drain();
        }
    }

    /// 弹出菜单所需的两样原生对象。
    ///
    /// 弹菜单会进入 AppKit 的模态菜单循环，期间菜单项回调会再进平台层（取锁），
    /// 所以调用方必须**在锁外**拿这两个 id 再调用 [`pop_up_menu`]。
    pub(crate) fn menu_anchor(&self) -> Option<(id, id)> {
        Some((self.status_item?, self.menu?))
    }

    /// 确保状态栏里有按钮，并把点击 action 接到 target 上。
    fn ensure_status_item(&mut self) {
        if self.status_item.is_some() {
            return;
        }

        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
            // NSVariableStatusItemLength == -1：宽度随图标变化
            let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0f64];
            // 状态栏只持有弱引用，自己 retain 住
            let _: id = msg_send![status_item, retain];

            let button: id = msg_send![status_item, button];
            let _: () = msg_send![button, setTarget: self.target];
            let _: () = msg_send![button, setAction: sel!(trayClicked:)];
            // NSEventMaskLeftMouseUp(1<<2) | NSEventMaskRightMouseUp(1<<4)
            let _: i64 = msg_send![button, sendActionOn: (1u64 << 2) | (1u64 << 4)];
            // NSImageOnly == 1：只显示图像，不留标题
            let _: () = msg_send![button, setImagePosition: 1u64];
            // 只设了菜单没设图标时给个默认齿轮图标，否则状态栏里是个看不见的空位
            set_default_symbol_image(button);

            self.status_item = Some(status_item);
            pool.drain();
        }
    }

    /// 把图标从菜单栏移除并释放。
    fn remove_status_item(&mut self) {
        let Some(status_item) = self.status_item.take() else {
            return;
        };
        unsafe {
            let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
            let _: () = msg_send![status_bar, removeStatusItem: status_item];
            // 抵掉 ensure_status_item 里的 retain
            let _: () = msg_send![status_item, release];
        }
    }
}

impl Drop for MacTray {
    fn drop(&mut self) {
        self.remove_status_item();
        unsafe {
            if let Some(menu) = self.menu.take() {
                let _: () = msg_send![menu, release];
            }
            let _: () = msg_send![self.target, release];
        }
    }
}

/// 弹出状态栏菜单（调用方保证此时没有持有平台锁）。
pub(crate) fn pop_up_menu(status_item: id, menu: id) {
    unsafe {
        let _: () = msg_send![status_item, popUpMenu: menu];
    }
}

/// 没有显式图标时，给状态栏按钮放一个默认齿轮符号（SF Symbol）。
///
/// `imageWithSystemSymbolName:accessibilityDescription:` 需要 macOS 11+，先用
/// `respondsToSelector:` 探一下；旧系统上保持空图像，不影响菜单与点击事件。
unsafe fn set_default_symbol_image(button: id) {
    unsafe {
        let responds: BOOL = msg_send![
            class!(NSImage),
            respondsToSelector: sel!(imageWithSystemSymbolName:accessibilityDescription:)
        ];
        if responds == NO {
            return;
        }

        let image: id = msg_send![
            class!(NSImage),
            imageWithSystemSymbolName: ns_string("gearshape")
            accessibilityDescription: nil
        ];
        if image == nil {
            return;
        }

        let size = NSSize {
            width: STATUS_ICON_SIZE,
            height: STATUS_ICON_SIZE,
        };
        let _: () = msg_send![image, setSize: size];
        // 模板图像会跟随菜单栏明暗自动着色
        let _: () = msg_send![image, setTemplate: YES];
        let _: () = msg_send![button, setImage: image];
    }
}

/// 递归构建原生菜单项，返回 +1 引用（调用方负责释放）。
unsafe fn create_menu_item(item: &TrayMenuItem, target: id) -> id {
    unsafe {
        match item {
            TrayMenuItem::Separator => {
                let separator: id = msg_send![class!(NSMenuItem), separatorItem];
                let _: id = msg_send![separator, retain];
                separator
            }
            TrayMenuItem::Action { label, id } => {
                let menu_item: id = msg_send![class!(NSMenuItem), alloc];
                let menu_item: id = msg_send![
                    menu_item,
                    initWithTitle: ns_string(label)
                    action: sel!(trayMenuItem:)
                    keyEquivalent: ns_string("")
                ];
                let _: () = msg_send![menu_item, setTarget: target];
                let _: () = msg_send![menu_item, setRepresentedObject: ns_string(id)];
                menu_item
            }
            TrayMenuItem::Toggle { label, checked, id } => {
                let menu_item: id = msg_send![class!(NSMenuItem), alloc];
                let menu_item: id = msg_send![
                    menu_item,
                    initWithTitle: ns_string(label)
                    action: sel!(trayMenuItem:)
                    keyEquivalent: ns_string("")
                ];
                let _: () = msg_send![menu_item, setTarget: target];
                let _: () = msg_send![menu_item, setRepresentedObject: ns_string(id)];
                let _: () = msg_send![menu_item, setState: if *checked { 1i64 } else { 0i64 }];
                menu_item
            }
            TrayMenuItem::Submenu { label, items } => {
                let submenu: id = msg_send![class!(NSMenu), alloc];
                let submenu: id = msg_send![submenu, init];
                let _: () = msg_send![submenu, setAutoenablesItems: NO];
                for sub_item in items {
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
