# hello_mobile

rgpui 1.4.0 移动端最小示例：三端共用同一套视图（`src/lib.rs`），
平台差异只收敛在入口函数。Android 真机优先，iOS 预留。

完整指南（含环境准备、打包、签名、图标、底层 API 调用）见
`docs/1.4.0/android-guide.md`（Android）与 `docs/1.4.0/ios-guide.md`（iOS）。

## 桌面预览（开发期调 UI）

```powershell
cargo run -p hello_mobile
```

窗口显示当前 `TargetPlatform`；桌面仅用于预览布局，真机行为以 Android 为准。

## Android 真机（M1：出包 + 启动 + 日志）

```powershell
# 1. Rust target（一次）
rustup target add aarch64-linux-android

# 2. 编译 .so 到 jniLibs（要求 ANDROID_NDK_HOME，见完整指南）
cargo install cargo-ndk
cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p hello_mobile

# 3. 打包安装（wrapper 已提交，只需 JDK 17；首次运行自动下载 Gradle 发行版）
cd android
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk

# 前置自查：java -version（17+）、adb devices（真机在线）

# 4. 看日志
adb logcat -s hello_mobile
```

注意：`cargo-ndk 4.x` 的平台 API flag 是大写 `-P`（小写 `-p` 会被当成
cargo 包名，报 `unknown package`）；3.x 才用小写。本工程按 4.x 编写。

M2 代码已落地：`.so` 链接验证通过（`ANativeActivity_onCreate` +
`android_main` 符号已确认导出）；首帧渲染待真机验证。
签名、release、图标、ABI 取舍、疑难排查全部在完整指南里。

## iOS（M1 仅 check 通过，真机 M4）

```bash
rustup target add aarch64-apple-ios
cargo check -p hello_mobile --target aarch64-apple-ios
```

真机接线（Xcode 工程、签名、图标）见 `docs/1.4.0/ios-guide.md`。
