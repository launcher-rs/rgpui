//! Window 扩展 trait - 为 Window 添加对话框打开/关闭等能力。

use crate::dialog::{AlertDialog, Dialog};
use crate::{App, Root, Window};

/// 为 [`Window`] 添加对话框等功能的扩展 trait。
pub trait WindowExt: Sized {
    /// 打开一个对话框。
    fn open_dialog<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static;

    /// 打开一个警告对话框（带便捷默认值的对话框）。
    fn open_alert_dialog<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static;

    /// 返回是否存在活动对话框。
    fn has_active_dialog(&mut self, cx: &mut App) -> bool;

    /// 关闭最后一个活动对话框。
    fn close_dialog(&mut self, cx: &mut App);

    /// 关闭所有活动对话框。
    fn close_all_dialogs(&mut self, cx: &mut App);
}

impl WindowExt for Window {
    #[inline]
    fn open_dialog<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    {
        Root::update(self, cx, move |root, window, cx| {
            root.open_dialog(build, window, cx);
        })
    }

    #[inline]
    fn open_alert_dialog<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static,
    {
        self.open_dialog(cx, move |_, window, cx| {
            build(AlertDialog::new(cx), window, cx).into_dialog(window, cx)
        })
    }

    #[inline]
    fn has_active_dialog(&mut self, cx: &mut App) -> bool {
        Root::read(self, cx).active_dialogs.len() > 0
    }

    #[inline]
    fn close_dialog(&mut self, cx: &mut App) {
        Root::update(self, cx, |root, window, cx| {
            root.close_dialog(window, cx);
        })
    }

    #[inline]
    fn close_all_dialogs(&mut self, cx: &mut App) {
        Root::update(self, cx, |root, window, cx| {
            root.close_all_dialogs(window, cx);
        })
    }
}
