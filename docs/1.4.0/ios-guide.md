# iOS 接入指南（1.4.0，M1 预留，M4 实现）

> 本机无 iOS 设备：M1 只保证结构桩 + CI `check` 通过，真机实现在 M4。
> 本文档给出最小接线形状，M4 按此填实现。

## M1 状态

- `crates/rgpui-ios`：`IosPlatform` 桩（窗口/显示器/文本系统空实现），
  `rgpui-platform::current_platform()` 已有 `ios` 分支。
- `examples/hello_mobile`：`cargo check -p hello_mobile --target
  aarch64-apple-ios` 通过；iOS 真机入口符号 `rgpui_ios_register_app` 已占位。
- CI：`mobile-ios` job 在 **macOS runner** 上跑 `aarch64-apple-ios` check
 （iOS 依赖链含 `psm`，其构建脚本要调 `xcrun`，ubuntu/Windows runner 上跑不起来，
  这是已知约束不是 bug）。

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo check -p rgpui-ios --target aarch64-apple-ios
cargo check -p hello_mobile --target aarch64-apple-ios
```

## M4 接线清单（有设备时照此做）

1. **渲染**：`rgpui-wgpu` Metal 后端 + `CAMetalLayer`（参考
   `temp/gpui-mobile/src/ios/window.rs`），`IosPlatformWindow` 实现
   `PlatformWindow` 全量方法。
2. **文本**：CoreText 整形（`font-kit` 特性门，与 macOS 同栈）。
3. **Xcode 工程**：`xcodegen`（`project.yml`）或手动建 App Target，
   链接 `libhello_mobile.a`（staticlib），`main.m` 调注册符号后进 runloop。
4. **签名**：Apple Developer Team + Bundle ID（如 `com.example.hellomobile`），
   真机调试自动签名即可；上架走 Archive + App Store Connect。
5. **图标**：`Assets.xcassets` 的 AppIcon（1024 PNG 一张，Xcode 自动切尺寸）。
6. **输入**：UIKit 触摸 → 鼠标事件翻译 + 外接键盘 HID 映射（参考 `events.rs` /
   `text_input.rs`），键盘高度与安全区（`safeAreaInsets`）同 Android 语义对齐。
7. **示例**：`hello_mobile` 双端一致 + `migration-guide-1.3-to-1.4.md` 补 iOS 节。
