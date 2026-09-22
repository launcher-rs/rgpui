# `cargo rgpui` CLI 工具计划

## 目标

为 rgpui 移动端开发提供一站式 CLI 工具，将 20+ 文件的手动配置和多步构建流程自动化为单命令操作。

## 子命令设计

```bash
cargo rgpui doctor                      # 环境检查
cargo rgpui new <name>                   # 创建新项目
cargo rgpui build [--release] [--target android|ios]  # 构建
cargo rgpui install [--target android]   # 构建 + 安装到设备
cargo rgpui run [--target android]       # 构建 + 安装 + 启动 + logcat
```

## 1. `cargo rgpui doctor` — 环境检查

逐项检查并输出状态表：

| # | 检查项 | 命令 | 通过条件 |
|---|--------|------|---------|
| 1 | JDK | `java -version` | ≥ 17 |
| 2 | ANDROID_HOME | 环境变量 | 已设置且目录存在 |
| 3 | Android SDK platform-tools | `$ANDROID_HOME/platform-tools/adb` | 文件存在 |
| 4 | Android SDK platforms | `$ANDROID_HOME/platforms/android-34` | 目录存在 |
| 5 | Android SDK build-tools | `$ANDROID_HOME/build-tools/34.0.0` | 目录存在 |
| 6 | Android NDK | `$ANDROID_NDK_HOME` 或 `$ANDROID_HOME/ndk/` | 存在且 ≥ r25 |
| 7 | Rust target | `rustup target list --installed` | 包含 `aarch64-linux-android` |
| 8 | cargo-ndk | `cargo ndk --version` | 已安装 |
| 9 | LLVM strip | NDK 中的 `llvm-strip` | 文件存在 |
| 10 | 设备连接 | `adb devices` | 至少一台设备 |

输出格式：
```
🔍 检查 rgpui 移动端开发环境...

  ✅ JDK 17.0.2
  ✅ Android SDK: C:\Users\xxx\AppData\Local\Android\Sdk
  ✅ platform-tools (adb)
  ✅ platforms;android-34
  ✅ build-tools;34.0.0
  ✅ NDK r27.0.12077973
  ✅ Rust target: aarch64-linux-android
  ✅ cargo-ndk 4.0.2
  ✅ llvm-strip (NDK)
  ✅ 设备: Samsung SM-N9500 (ce091719f04ed022)

全部通过！可以开始移动开发。
```

失败项标 ❌ 并给出修复建议命令。

## 2. `cargo rgpui new <name>` — 创建项目

### 输入
- 项目名（kebab-case，如 `my_app`）
- 可选：包名（默认 `com.example.<name>`）

### 生成 20 个文件

```
<name>/
├── Cargo.toml              # rgpui + android 依赖（git 依赖）
├── src/
│   ├── lib.rs              # 共享 UI + android_main 入口
│   └── main.rs             # 桌面入口
└── android/
    ├── build.gradle.kts    # AGP 版本
    ├── settings.gradle.kts # 仓库 + 项目名
    ├── gradlew             # Unix wrapper（从 hello_mobile 复制）
    ├── gradlew.bat         # Windows wrapper
    ├── .gitignore          # Android 排除项
    ├── keystore.properties.example
    ├── gradle/wrapper/
    │   ├── gradle-wrapper.jar    # 二进制（从 hello_mobile 复制）
    │   └── gradle-wrapper.properties
    └── app/
        ├── build.gradle.kts      # namespace + applicationId + abiFilters
        └── src/main/
            ├── AndroidManifest.xml  # NativeActivity + lib_name
            └── res/
                ├── values/colors.xml
                ├── drawable/ic_launcher_foreground.xml
                └── mipmap-anydpi-v26/
                    ├── ic_launcher.xml
                    └── ic_launcher_round.xml
```

### Cargo.toml 模板（独立项目，git 依赖）

```toml
[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[lib]
name = "{name}"
crate-type = ["cdylib", "staticlib", "rlib"]

[[bin]]
name = "{name}"

[dependencies]
rgpui = { git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }
rgpui-platform = { git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }
log = "0.4"

[target.'cfg(target_os = "android")'.dependencies]
rgpui-android = { git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }
android-activity = { version = "0.6", features = ["native-activity"] }
android_logger = "0.15"

[target.'cfg(target_os = "ios")'.dependencies]
rgpui-ios = { git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }
```

### 关键防错（生成时验证）
- `lib_name` (Cargo.toml `[lib] name`) == `android:value` (AndroidManifest meta-data)
- `namespace` == `applicationId` (build.gradle.kts)
- `abiFilters` = `["arm64-v8a"]`
- `minSdk` = 26

## 3. `cargo rgpui build` — 构建

流程（3 步自动化）：
```
1. cargo ndk -t arm64-v8a -P 31 -o android/app/src/main/jniLibs build [--release] -p <name>
2. llvm-strip --strip-debug android/app/src/main/jniLibs/arm64-v8a/lib<name>.so
3. cd android && ./gradlew assembleDebug (或 assembleRelease)
```

### Strip 自动化
从 `$ANDROID_NDK_HOME` 或 `$ANDROID_HOME/ndk/*/` 自动找 `llvm-strip`。

### Release 模式
检测 `android/keystore.properties` 存在时用 release 签名，否则 fallback 到 debug 签名。

## 4. `cargo rgpui install` — 构建 + 安装

在 build 基础上：
```
4. adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

## 5. `cargo rgpui run` — 构建 + 安装 + 运行 + 日志

在 install 基础上：
```
5. adb shell am force-stop <package_name>
6. adb shell am start -n <package_name>/android.app.NativeActivity
7. adb logcat -s <name>    # 实时输出，Ctrl+C 停止
```

## 技术实现

### crate 结构

```
crates/rgpui-cli/
├── Cargo.toml
├── src/
│   ├── main.rs           # clap 入口 + 子命令分发
│   ├── doctor.rs         # 环境检查逻辑
│   ├── new.rs            # 项目生成（模板引擎）
│   ├── build.rs          # 构建编排（cargo ndk + strip + gradlew）
│   ├── install.rs        # adb install
│   ├── run.rs            # adb start + logcat
│   └── templates/        # 内嵌模板文件
│       ├── Cargo.toml.template
│       ├── lib.rs.template
│       ├── main.rs.template
│       ├── AndroidManifest.xml.template
│       ├── build.gradle.kts.template
│       ├── app/build.gradle.kts.template
│       ├── settings.gradle.kts.template
│       ├── colors.xml.template
│       ├── ic_launcher_foreground.xml.template
│       ├── ic_launcher.xml.template
│       └── .gitignore.template
```

### 依赖

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1.0"
console = "0.15"        # 彩色输出
dialoguer = "0.11"      # 交互式输入（new 子命令）
include_dir = "0.7"     # 内嵌模板文件
tempfile = "3"          # 临时目录（编译时用）
```

### 注意事项

- `gradle-wrapper.jar` 是二进制文件（~60KB），用 `include_dir!` 从 hello_mobile 复制内嵌
- `gradlew` / `gradlew.bat` 同理
- `ic_launcher_foreground.xml` 是固定模板，用户后续自己替换图标
- git 分支名硬编码为 `feat/1.4.0`（发布后改为 `main`）
- Windows 上 `gradlew.bat`，Unix 上 `./gradlew`，CLI 自动选

## 实现优先级

| 阶段 | 子命令 | 工作量 |
|------|--------|--------|
| P0 | `doctor` | 小（纯检查，无文件生成） |
| P0 | `new` | 中（20 个模板 + 防错校验） |
| P0 | `build` | 小（3 条命令编排） |
| P1 | `install` | 极小（build + 1 条命令） |
| P1 | `run` | 小（install + logcat） |

## 测试

1. `cargo rgpui doctor` 在本机跑通，输出全部 ✅
2. `cargo rgpui new test_app` 在临时目录创建项目，验证 `cargo check` 通过
3. `cargo rgpui build` 在 test_app 中构建成功
4. `cargo rgpui install` 安装到真机
5. `cargo rgpui run` 安装 + 启动 + 看到日志

## workspace 变更

- 新增 `crates/rgpui-cli` 到 `[workspace] members`
- `Cargo.toml` 新增 `[[bin]]` section（name = "rgpui"）
