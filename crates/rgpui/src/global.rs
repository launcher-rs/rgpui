//! # 全局状态系统
//!
//! GPUI 的全局状态系统允许你在应用级别共享数据，而无需通过实体传递。
//! 全局状态通过 [`Global`] trait 标记类型，通过 [`App`] 的方法进行访问。
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use rgpui::Global;
//!
//! /// 应用的主题配置
//! struct ThemeConfig {
//!     dark_mode: bool,
//! }
//!
//! impl Global for ThemeConfig {}
//!
//! // 在 App 中设置全局状态
//! cx.set_global(ThemeConfig { dark_mode: true });
//!
//! // 读取全局状态
//! let theme = cx.global::<ThemeConfig>();
//! ```
//!
//! # 限制访问
//!
//! 可以利用 Rust 的可见性系统限制全局状态的读写访问。
//! 例如创建私有结构体实现 `Global`，然后通过 newtype 封装暴露有限的操作。

use crate::{App, BorrowAppContext};

/// 全局状态标记 trait - 实现此 trait 的类型可以存储在 GPUI 的全局状态中。
///
/// 此 trait 确保只有实现了 `Global` 的类型才能使用全局访问方法，
/// 在编译时防止误用。
///
/// 此 trait 故意留空，仅作为标记使用。
/// 可以通过 blanket 实现附加功能到实现了 `Global` 的类型上。
pub trait Global: 'static {
    // 此 trait 故意留空，仅作为标记使用
}

/// 从上下文中读取全局值的 trait。
pub trait ReadGlobal {
    /// 返回实现类型的全局实例。
    ///
    /// 如果该类型的全局值尚未设置，会触发 panic。
    fn global(cx: &App) -> &Self;
}

impl<T: Global> ReadGlobal for T {
    fn global(cx: &App) -> &Self {
        cx.global::<T>()
    }
}

/// 在上下文中更新全局值的 trait。
pub trait UpdateGlobal {
    /// 使用提供的闭包更新全局实例。
    ///
    /// 闭包同时接收全局值的可变引用和上下文的可变引用。
    fn update_global<C, F, R>(cx: &mut C, update: F) -> R
    where
        C: BorrowAppContext,
        F: FnOnce(&mut Self, &mut C) -> R;

    /// 设置全局实例。
    fn set_global<C>(cx: &mut C, global: Self)
    where
        C: BorrowAppContext;
}

impl<T: Global> UpdateGlobal for T {
    #[track_caller]
    fn update_global<C, F, R>(cx: &mut C, update: F) -> R
    where
        C: BorrowAppContext,
        F: FnOnce(&mut Self, &mut C) -> R,
    {
        cx.update_global(update)
    }

    fn set_global<C>(cx: &mut C, global: Self)
    where
        C: BorrowAppContext,
    {
        cx.set_global(global)
    }
}
