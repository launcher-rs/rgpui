//! `hello_mobile` 可执行入口。
//!
//! - 桌面：经 `rgpui_platform::application()` 正常跑窗口（开发期预览 UI）。
//! - Android：真机入口是 `lib.rs` 的 `android_main`（cdylib），此 `main` 仅为
//!   `cargo check` 占位，不会被系统调用。
//! - iOS：真机入口在 M4 接线，此 `main` 仅为 check 占位。

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn main() {
    use rgpui::App;

    rgpui_platform::application().run(|cx: &mut App| {
        hello_mobile::open_main_window(cx);
    });
}

#[cfg(target_os = "android")]
fn main() {
    eprintln!("Android 真机入口为 lib.rs 的 android_main()，此二进制不会被调用。");
}

#[cfg(target_os = "ios")]
fn main() {
    eprintln!("iOS 真机入口在 M4 接线，此二进制不会被调用。");
}
