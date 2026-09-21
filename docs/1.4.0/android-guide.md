# Android 接入指南（1.4.0，M1）

> 面向：第一次把 rgpui 应用装到 Android 真机的开发者。
> 前置：Windows + PowerShell 7（`AGENTS.md` 编码约定：中文命令先切 UTF-8）。
> 示例工程：`examples/hello_mobile/`（`android/` 子目录即最小宿主，可直接抄）。
> 本文档覆盖 M1 全部能力；标注 M2/M3 的能力会说明当前状态，不让读者踩空。

## 0. 版本状态（先说明能做什么）

| 能力 | 状态 |
|------|------|
| 出包 + 安装 + 启动不崩 + `logcat` 日志 | ✅ M1 已有 |
| 首帧渲染 / 触摸 / 按键明文 / 安全区 / 深浅色 | ✅ M2 代码已落地，**待真机验证**（本机有设备按 §8–§9 走一遍） |
| 状态栏样式 / 剪贴板 JNI / 通知 / 文件选择 | 部分落地：`SystemChromeStyle` + 剪贴板 + `open_url` ✅（M3 前半，见 §10.1）；通知/文件选择待办 |
| IME 组合串（预编辑/上屏回调） | 未接（M3 后半，随自定义 Activity；M2 只有按键明文） |
| `cargo check --target aarch64-linux-android`（CI） | ✅ |
| `cargo check --target ... --tests`（含真机模块单测编译检查） | ✅ 本地可跑，CI 待 M2-6 一并接入 |

## 1. 环境准备（一次配好）

| 组件 | 要求 | 说明 |
|------|------|------|
| JDK | 17 | AGP 8 要求；`java -version` 确认 |
| Android SDK | cmdline-tools + platform-tools + platforms;android-34 + build-tools;34 | 可用 Android Studio 代替手动装 |
| Android NDK | r25+（推荐 r26 LTS） | `rgpui-android` 按 NDK r25  API 行为编写 |
| Rust target | `aarch64-linux-android` | `rustup target add aarch64-linux-android` |
| cargo-ndk | 最新 | `cargo install cargo-ndk` |
| Gradle | 8.x（或 Android Studio 自带） | 示例按 AGP 8.5.2 编写 |
| 真机 | Android 8.0+（API 26+），arm64 | 开“开发者选项 → USB 调试” |

环境变量（Windows 示例，按实际路径改）：

```powershell
$env:ANDROID_HOME = "D:\Android\Sdk"
$env:ANDROID_NDK_HOME = "D:\Android\Sdk\ndk\26.1.10909125"
```

`cargo-ndk` 认 `ANDROID_NDK_HOME`；没有它会报 `NDK not found`。
本机如果装了 Android Studio，SDK 默认在 `%LOCALAPPDATA%\Android\Sdk`。

SDK 包（无 Android Studio 时手动装）：

```powershell
sdkmanager "platform-tools" "platforms;android-34" "build-tools;34.0.0" "ndk;26.1.10909125"
```

## 2. 依赖配置（Cargo.toml 三段式）

以 `examples/hello_mobile/Cargo.toml` 为准，抄三段：

```toml
[lib]
name = "hello_mobile"
# cdylib 给系统 dlopen，staticlib 给以后自写 Activity 静态链，rlib 供 bin 复用。
crate-type = ["cdylib", "staticlib", "rlib"]

[dependencies]
rgpui.workspace = true
rgpui-platform.workspace = true   # 唯一门面：application()/current_platform()
log.workspace = true

[target.'cfg(target_os = "android")'.dependencies]
rgpui-android.workspace = true
android-activity = { version = "0.6", features = ["native-activity"] }
android_logger = "0.15"
```

- `rgpui-platform` 是唯一对外门面，应用层不写 `#[cfg]`（除入口函数）。
- `android-activity` 的 `native-activity` 特性提供 `ANativeActivity_onCreate`
  胶水，负责调起下节的 `android_main`；M1 只用它做入口，M2 的事件循环也基于它。
- `android_logger` 把 `log::xxx!` 转到 `logcat`，`adb logcat -s <tag>` 可见。
- 版本钉死说明：`android-activity 0.6` + `ndk 0.9` + `jni 0.22`
  是互相兼容的一组（见开发文档 §5 风险表），升级要三个一起升，单列 PR。

## 3. Rust 入口（`android_main`）

```rust
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(...);      // ① 日志先行，否则崩了都看不见
    let platform = rgpui_android::current_platform(false);
    rgpui::Application::with_platform(platform).run(|cx| {
        open_main_window(cx);            // ② 建窗口/挂视图（与桌面同一套）
    });
}
```

三条铁律：

1. **日志与 panic hook 最先初始化**（`bridge::install_panic_hook`，
   崩了直接进 `logcat`，不再静默 abort）。
2. **`Application` 与平台同线程构造**：`android_main` 所在线程就是
   `ALooper` 线程，`AndroidDispatcher` 以它为“主线程”；换线程构造会触发
   `App` 的主线程断言（见开发文档 §5）。`shared_platform()` 拿的正是
   全局单例（`SharedPlatform` 转发），不是第二个实例。
3. **窗口由系统创建**：`Platform::run` 阻塞跑 `bridge::run_event_loop`，
   到 `INIT_WINDOW` 之后才调 `finish_launching` 回调并开窗；
   应用层不要在启动期假设窗口已存在。

模块位置（都在 `crates/rgpui-android/src/`）：`bridge.rs`（入口/循环/JNI）、
`window.rs`（surface 三态 + 输入桥）、`dispatcher.rs`（ALooper 双队列）、
`frame_source.rs` + `frame_pacer.rs`（vsync 按需帧）、`display.rs`、
`keyboard.rs`、`fling_guard.rs`、`platform.rs`。

## 4. `AndroidManifest.xml` 全解

文件：`android/app/src/main/AndroidManifest.xml`。逐项说明：

- `android.app.NativeActivity`：系统直接加载 `.so`，**零 Java 代码**；
  `android.app.lib_name` 的值必须等于 lib 的 `name`（`hello_mobile`），
  对不上就是启动即崩（`Unable to load native library`）。
- `android:exported="true"` + `MAIN`/`LAUNCHER`：桌面图标入口，缺一不可。
- `configChanges="orientation|keyboardHidden|screenSize|...|uiMode"`：
  旋转/深浅色切换时不重建 Activity，由 Rust 侧 `handle_resize` 处理；
  不写的话一转屏就重建，原生窗口丢了要走 `TERM/INIT` 全套（M2 才完整支持）。
- `launchMode="singleTask"`：桌面图标二次点击复用实例，不起第二个进程。
- `minSdk` 不在 manifest 写，在 `app/build.gradle.kts` 的 `minSdk = 26`。
- 权限按需加（`INTERNET` 等），M1 示例零权限；M3 包生态用到时再加，
  加一个权限就要在文档里写清用途（上架审核要求）。

## 5. Gradle 工程全解

`android/` 下：`settings.gradle.kts`（仓库源 + `:app`）、`build.gradle.kts`
（AGP 版本集中管理）、`app/build.gradle.kts`（包名/ABI/签名）、宿主资源。
全套 Kotlin DSL（`.kts`）：注意 `java.util.Properties` 要写成顶部
`import`（`java` 是脚本保留接收器），AGP 8 已删 `jniDebuggable`。

`app/build.gradle.kts` 关键项：

- `namespace`/`applicationId = "com.example.hellomobile"`：自有应用改掉，
  与签名证书的归属一致（见 §7）。
- `minSdk = 26`：与 `rgpui-android` 最低要求对齐；`targetSdk = 34` 跟随当年
  Play 要求（每年 8 月会涨，按构建时最新改）。
- `abiFilters = ["arm64-v8a"]`：真机只打 arm64，包最小；要兼容老 32 位设备
  加 `armeabi-v7a`，要在模拟器跑加 `x86_64`（对应 `cargo ndk -t` 参数，见 §8）。
- `jniLibs.srcDirs("src/main/jniLibs")`：`cargo ndk -o` 输出目录，AGP 自动打包，
  不要手抄 `.so`。
- `signingConfigs.release` 读 `keystore.properties`（见 §7），文件不存在时
  自动回退 debug 签名，保证 CI 也能出包。

Gradle Wrapper：`gradlew`/`gradlew.bat` + `gradle/wrapper/`（含 jar，
按惯例提交）已在仓库里，`./gradlew` 开箱即用；首次运行自动下载
Gradle 8.7 发行版（约 130MB，需联网）。本机另装了一份 Gradle
在 `D:\language\gradle`（仅用于生成 wrapper，日常用 wrapper 即可）。

## 6. 图标

示例已内置**纯 XML 自适应图标**（无 PNG，开箱即用）：

- `res/values/colors.xml`：背景色 `#101418`。
- `res/drawable/ic_launcher_foreground.xml`：矢量前景（占位 R 形）。
- `res/mipmap-anydpi-v26/ic_launcher.xml` / `ic_launcher_round.xml`：组装前后景。
- manifest 里 `android:icon` + `android:roundIcon` 已引用。

正式图标两条路：

1. **Android Studio 右键 `res` → New → Image Asset**：上传 512 PNG，
   自动生成 `mipmap-mdpi~xxxhdpi` 全套 PNG（含圆形），覆盖示例的 XML 即可。
2. **纯手工**：保留 adaptive-icon XML，只替换 foreground 矢量（需转成
   `vectorDrawable` 路径，SVG 可经 Studio 的 Vector Asset 导入）。

注意：`mipmap-anydpi-v26` 只在 API 26+ 生效，恰好等于我们的 `minSdk`，
所以不需要再补 `mipmap-*-v25` 兜底；如果将来降 `minSdk`，必须补 PNG 套。

## 7. 签名证书

### debug（开发期）

AGP 自动用 `~/.android/debug.keystore` 签名，`assembleDebug` 零配置。
卸载重装、换电脑（debug.keystore 变了）会报签名冲突，卸载重装即可。
**debug 包禁止上架**，Play 会直接拒收。

### release（发布）

1. 生成证书（一次，JDK 自带 `keytool`）：

```powershell
keytool -genkeypair -v -keystore hello_mobile-release.keystore `
  -alias hello_mobile -keyalg RSA -keysize 2048 -validity 9125
```

`-validity 9125`（25 年）：Play 要求新应用证书至少 2033 年后过期，
一次给够。口令记进密码管理器，**丢了就永远发不了更新包**。

2. 复制 `android/keystore.properties.example` 为 `android/keystore.properties`
   并填真实值（该文件已进 `.gitignore`，切勿提交；CI 用 secrets 另写一份）。

3. 打包验证：

```powershell
cd android
./gradlew assembleRelease
apksigner verify --print-certs app/build/outputs/apk/release/app-release.apk
```

`assembleRelease` 在没有 `keystore.properties` 时自动用 debug 签名兜底
（方便 CI 出包验证流程），但这种包**不能上架**，`apksigner` 会显示
`CN=Android Debug`，发布前务必确认是正式证书。

4. 上架说明：Play 要求 **AAB + 64 位 + App Signing**：
   `./gradlew bundleRelease` 出 `.aab`，上传后 Google 管签名密钥；
   本地 keystore 叫“上传密钥”，丢了可找 Google 重置（debug 可没这待遇）。

## 8. 编译（debug / release）

```powershell
# Rust .so（debug，真机 arm64；cargo-ndk 4.x 平台 flag 是大写 -P，
# 小写 -p 会被当成 cargo 包名而报 unknown package）
cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build -p hello_mobile

# 打包 + 安装 + 看日志
cd android; ./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb logcat -s hello_mobile
```

- `-P 31` 是 NDK 平台 API（链接基线），不是 `minSdk`：参考实现血泪教训——
  直接链高版本符号（如 `ANativeWindow_setFrameRate`）会导致低版本系统
  `dlopen` 失败，所以 `rgpui-android` 对这类符号一律 `dlsym` 动态查
  （M2 照搬）。应用层自己加 NDK 调用时同理：高版本 API 先判 `android_get_device_api_level()`。
- 多 ABI：`cargo ndk -t arm64-v8a -t armeabi-v7a ...` 一次全出，
  与 `abiFilters` 保持一致，多出的 ABI 只增包体积。
- release：Rust 侧在包的 `[profile.release]` 加
  `opt-level = "z" lto = true strip = true`（示例已可按需加），
  然后 `cargo ndk ... build --release -p hello_mobile` +
  `./gradlew assembleRelease`（`minifyEnabled=false`，Rust 符号不受 R8 影响，
  开了也减不了体积，反而可能误杀 NativeActivity 引用，保持关闭）。
- 体积预期：debug APK 约 500MB（`.so` 未 strip，含 wgpu 全符号），
  仅用于本机调试；release + strip 后回落到正常量级，不要被 debug 包吓到。

## 9. 安装运行与日志

```powershell
adb devices                              # 确认真机在线
adb install -r <apk>                     # -r 覆盖安装保留数据
adb logcat -s hello_mobile               # 只看本应用 tag
adb logcat *:E                           # 全局错误兜底
adb shell am start -n com.example.hellomobile/.debug/android.app.NativeActivity  # 手动拉起（applicationIdSuffix 别漏）
```

崩溃三板斧：先看 `logcat` 的 `FATAL` + `backtrace` 行（M2 的 panic hook
会把 Rust panic 打出来）；`dlopen failed: library not found` 多半是
`lib_name` 与 crate 名对不上或 ABI 目录放错；`SIGSEGV` 看 `tombstone`
（`adb logcat -b crash`）。

## 10. 调用平台底层 API（给进阶开发者）

约束先行：**所有 GPUI 状态只碰 render 线程**（即 `android_main` 线程）；
Java UI 线程只做“收事件 → 进队列”，渲染线程 draining（双入口的
`host` 模式在 M3 候选，M1/M2 只有 `android-activity` 单路径）。

- 拿 `AndroidApp`：`bridge::android_app()`（`OnceLock` 存一份，
  应用层不要自己再存）。JVM/Activity 指针同理（`java_vm()` /
  `activity_as_ptr()`），`platform.rs` 的 JNI 调用都经这几个口。
- Rust 调 Java（JNI）：经 `bridge::with_env` 拿 `Env`，
  从 UI 线程调应用类必须走 `Activity.getClassLoader().loadClass()`
  （`FindClass` 用系统类加载器，找不到应用类，参考实现已踩坑）。
  光标锚点（`update_ime_position`）是现成的完整例子，直接看
  `window.rs` 的 `CursorAnchorInfo` 四步调用。
- 常用系统服务（M3 前半已落地）：剪贴板（`ClipboardManager`，见 §10.1）、
  打开链接（`ACTION_VIEW` Intent，见 §10.1）、文件选择（`ACTION_OPEN_DOCUMENT`，
  先拷到 cache 再回文件路径，不要直接吃 content URI，待办）、通知（待办）。
- 主线程规则：`getWindow/setStatusBarColor` 这类 View 操作会跟 UI 线程抢锁，
  高频调用前先做“值未变跳过”缓存（参考 `LAST_CHROME_STYLE` 写法）。

### 10.1 开箱即用的三个调用（M3 前半已落地）

应用层（`target_os = "android"` 门控，桌面端同名调用为空操作）：

```rust
// 状态栏深底白字 + 导航栏深底（颜色 0xRRGGBB，None 表不动）
rgpui_android::set_system_chrome(&rgpui_android::SystemChromeStyle {
    status_bar_color: Some(0x101418),
    status_bar_style: rgpui_android::StatusBarContentStyle::Light,
    navigation_bar_color: Some(0x101418),
});

// 剪贴板走平台即可（真机经 JNI，桌面走各平台实现）
cx.write_to_clipboard(rgpui::ClipboardItem::new_string("hello".into()));

// 浏览器打开链接（真机 ACTION_VIEW，桌面默认浏览器）
cx.open_url("https://example.com");
```

注意：`set_system_chrome` 内部有同值跳过缓存，每帧调也无妨；
剪贴板读在真机失败时自动回退进程内缓存（读自己刚写的值一定命中）。

## 11. 移动端 UI 约束（cookbook，写应用前读一遍）

- 无悬停：`hover` 样式不能是唯一信息通道；点击目标 ≥ 44pt。
- 键盘：`Input` 获焦即弹键盘，用 `keyboard_height()` 给布局让位，
  不要写死底部边距。显隐 API（`show_keyboard` / `hide_keyboard`）M2 已有；
  获焦自动弹（焦点 hook）随自定义 Activity 记 M3，M2 由应用层按需调用。
- 安全区：状态栏/刘海/导航栏区域用 `AndroidWindow::safe_area_insets_logical()`
  避让，不要写死 dp；深浅色读 `window_appearance()`（`uiMode_night`），
  两者 M2 都有了。
- 滚动：以平台 `ScrollPhysics` 为准（M3 对齐 `ScrollPhysics::android`），
  应用层不重写摩擦系数；滚动条默认自动隐藏。

## 12. FAQ

- **CI 能验什么？** `mobile-android` job（ubuntu）在
  `aarch64-linux-android` 上 `cargo check`（`.github/workflows/ci.yml`），
  只查编译不跑 GPU 单测（软件渲染 runner 跑真 GPU case 会挂，见 1.3.0 已知问题）。
  APK 组装（`cargo-ndk` + Gradle）记 M2，本地真机验证通过后才进 CI。
- **Play 要求 64 位？** 2019 年起新应用必须含 64 位 `.so`，`arm64-v8a` 必打，
  只打 `armeabi-v7a` 会被拒。
- **`adb install` 报签名冲突？** 换了电脑/删了 debug.keystore，卸载重装。
- **Windows 长路径/空格？** 工程放短英文路径（如 `D:\work\rgpui`），
  Gradle 遇空格与超长路径会出玄学错误。
- **转屏黑屏？** 先确认 manifest 的 `configChanges` 含 `orientation|screenSize`；
  M1 无渲染所以是正常现象，M2 的 `TERM/INIT` 保图集逻辑落地后再验。
