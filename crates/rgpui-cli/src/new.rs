use anyhow::Result;
use console::style;
use std::fs;
use std::path::{Path, PathBuf};

/// `cargo rgpui new <name>` — 创建新的 rgpui 移动端项目。
pub fn run(name: &str) -> Result<()> {
    // 校验项目名
    if !is_valid_crate_name(name) {
        anyhow::bail!(
            "项目名 '{}' 不合法（只允许小写字母、数字、下划线、连字符）",
            name
        );
    }

    let project_dir = PathBuf::from(name);
    if project_dir.exists() {
        anyhow::bail!("目录 '{}' 已存在，请换个名字或先删除", name);
    }

    let package_id = format!("com.example.{}", name.replace('-', "_"));
    let lib_name = name.replace('-', "_");

    println!("{} 创建项目 {} ...", style("📦").cyan(), style(name).bold());

    // 创建目录结构
    let dirs = [
        "src",
        "android/app/src/main/res/values",
        "android/app/src/main/res/drawable",
        "android/app/src/main/res/mipmap-anydpi-v26",
        "android/gradle/wrapper",
    ];
    for d in &dirs {
        fs::create_dir_all(project_dir.join(d))?;
    }

    // 生成所有文件
    write_file(&project_dir.join("Cargo.toml"), &cargo_toml(name))?;
    write_file(&project_dir.join("src/lib.rs"), &lib_rs(name, &lib_name))?;
    write_file(&project_dir.join("src/main.rs"), &main_rs(name))?;
    write_file(
        &project_dir.join("android/build.gradle.kts"),
        &root_build_gradle(),
    )?;
    write_file(
        &project_dir.join("android/settings.gradle.kts"),
        &settings_gradle(name),
    )?;
    write_file(
        &project_dir.join("android/app/build.gradle.kts"),
        &app_build_gradle(&package_id),
    )?;
    write_file(
        &project_dir.join("android/app/src/main/AndroidManifest.xml"),
        &android_manifest(&lib_name, name),
    )?;
    write_file(
        &project_dir.join("android/app/src/main/res/values/colors.xml"),
        COLORS_XML,
    )?;
    write_file(
        &project_dir.join("android/app/src/main/res/drawable/ic_launcher_foreground.xml"),
        IC_LAUNCHER_FOREGROUND_XML,
    )?;
    write_file(
        &project_dir.join("android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml"),
        IC_LAUNCHER_XML,
    )?;
    write_file(
        &project_dir.join("android/app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml"),
        IC_LAUNCHER_ROUND_XML,
    )?;
    write_file(
        &project_dir.join("android/gradle/wrapper/gradle-wrapper.properties"),
        GRADLE_WRAPPER_PROPERTIES,
    )?;
    write_file(&project_dir.join("android/.gitignore"), ANDROID_GITIGNORE)?;
    write_file(
        &project_dir.join("android/keystore.properties.example"),
        KEYSTORE_PROPERTIES_EXAMPLE,
    )?;

    // 复制二进制模板（gradlew / gradle-wrapper.jar）
    let template_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/templates/android");
    copy_file(
        &template_dir.join("gradlew"),
        &project_dir.join("android/gradlew"),
    )?;
    copy_file(
        &template_dir.join("gradlew.bat"),
        &project_dir.join("android/gradlew.bat"),
    )?;
    copy_file(
        &template_dir.join("gradle/wrapper/gradle-wrapper.jar"),
        &project_dir.join("android/gradle/wrapper/gradle-wrapper.jar"),
    )?;

    // 防错校验
    validate_project(&project_dir, name, &lib_name, &package_id)?;

    println!(
        "\n{} 项目 {} 已创建！\n",
        style("✅").green(),
        style(name).bold().green()
    );
    println!("下一步：");
    println!("  {} cd {}", style("1.").bold(), name);
    println!("  {} cargo rgpui build", style("2.").bold());
    println!("  {} cargo rgpui run", style("3.").bold());

    Ok(())
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

fn is_valid_crate_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// 校验关键不变量
fn validate_project(dir: &Path, _name: &str, lib_name: &str, package_id: &str) -> Result<()> {
    let mut errors = Vec::new();

    // 1. lib_name 必须一致
    let cargo = fs::read_to_string(dir.join("Cargo.toml"))?;
    if !cargo.contains(&format!("name = \"{}\"", lib_name)) {
        errors.push(format!(
            "Cargo.toml [lib] name '{}' 与 lib_name '{}' 不匹配",
            lib_name, lib_name
        ));
    }

    // 2. AndroidManifest lib_name 必须一致
    let manifest = fs::read_to_string(dir.join("android/app/src/main/AndroidManifest.xml"))?;
    if !manifest.contains(&format!("android:value=\"{}\"", lib_name)) {
        errors.push(format!(
            "AndroidManifest.xml lib_name '{}' 与 Cargo.toml crate name 不匹配",
            lib_name
        ));
    }

    // 3. namespace == applicationId
    let app_gradle = fs::read_to_string(dir.join("android/app/build.gradle.kts"))?;
    if !app_gradle.contains(&format!("namespace = \"{}\"", package_id)) {
        errors.push(format!(
            "build.gradle.kts namespace '{}' 与 applicationId 不匹配",
            package_id
        ));
    }

    // 4. minSdk >= 26
    if !app_gradle.contains("minSdk = 26") {
        errors.push("minSdk 必须 >= 26（rgpui-android 要求）".into());
    }

    // 5. abiFilters 包含 arm64-v8a
    if !app_gradle.contains("arm64-v8a") {
        errors.push("abiFilters 必须包含 arm64-v8a".into());
    }

    if errors.is_empty() {
        println!(
            "  {} 防错校验通过（lib_name / packageId / minSdk / abiFilters）",
            style("✓").green()
        );
        Ok(())
    } else {
        for e in &errors {
            println!("  {} {}", style("✗").red(), e);
        }
        anyhow::bail!("项目校验失败，{} 个问题", errors.len());
    }
}

// ── 模板内容 ────────────────────────────────────────────────────────────────

fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
name = "{name}"
crate-type = ["cdylib", "staticlib", "rlib"]
path = "src/lib.rs"

[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
rgpui = {{ git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }}
rgpui-platform = {{ git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }}
log = "0.4"

[target.'cfg(target_os = "android")'.dependencies]
rgpui-android = {{ git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }}
android-activity = {{ version = "0.6", features = ["native-activity"] }}
android_logger = "0.15"

[target.'cfg(target_os = "ios")'.dependencies]
rgpui-ios = {{ git = "https://github.com/launcher-rs/rgpui.git", branch = "feat/1.4.0" }}
"#
    )
}

fn lib_rs(name: &str, lib_name: &str) -> String {
    let display_name = name
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        r#"//! `{name}` — rgpui 移动端应用。
//!
//! 桌面 / Android / iOS 共用同一套视图代码，平台差异收敛在入口函数。

use rgpui::{{App, Context, Window, div, prelude::*, px, rgb}};

/// 应用主视图。
pub struct AppView;

impl Render for AppView {{
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {{
        div()
            .flex()
            .flex_col()
            .gap_4()
            .items_center()
            .justify_center()
            .size_full()
            .bg(rgb(0x1a1a2e))
            .text_color(rgb(0xffffff))
            .child(div().text_3xl().child("{display_name}"))
            .child(div().text_lg().child("Hello from rgpui!"))
    }}
}}

/// 打开主窗口（桌面与移动端共用）。
pub fn open_main_window(cx: &mut App) {{
    cx.open_window(rgpui::WindowOptions::default(), |_, cx| {{
        cx.new(|_| AppView)
    }})
    .expect("打开主窗口失败");
    cx.activate(true);
}}

// ── Android 入口 ─────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {{
    use rgpui::Application;
    use rgpui_android::bridge::{{init_platform, install_panic_hook, shared_platform}};

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("{lib_name}"),
    );
    install_panic_hook();

    let _platform = init_platform(&app);
    let shared = match shared_platform() {{
        Some(shared) => shared,
        None => return,
    }};
    Application::with_platform(shared.into_rc()).run(|cx| {{
        open_main_window(cx);
    }});
}}

// ── iOS 入口（M4 占位） ──────────────────────────────────────────────────────

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn rgpui_ios_register_app() {{
    log::info!("{name}: iOS 入口占位");
}}
"#
    )
}

fn main_rs(name: &str) -> String {
    format!(
        r#"//! 桌面入口。

fn main() {{
    {name}::open_main_window(&mut rgpui::App::default());
}}
"#
    )
}

fn root_build_gradle() -> String {
    r#"plugins {
    id("com.android.application") version "8.5.2" apply false
}
"#
    .to_string()
}

fn settings_gradle(name: &str) -> String {
    format!(
        r#"pluginManagement {{
    repositories {{
        google()
        mavenCentral()
        gradlePluginPortal()
    }}
}}

dependencyResolutionManagement {{
    repositories {{
        google()
        mavenCentral()
    }}
}}

rootProject.name = "{name}"
include(":app")
"#
    )
}

fn app_build_gradle(package_id: &str) -> String {
    format!(
        r#"import java.util.Properties

plugins {{
    id("com.android.application")
}}

val keystorePropsFile = rootProject.file("keystore.properties")
val keystoreProps = Properties()
if (keystorePropsFile.exists()) {{
    keystorePropsFile.inputStream().use {{ keystoreProps.load(it) }}
}}
val hasReleaseKey = keystoreProps.containsKey("storeFile")

android {{
    namespace = "{package_id}"
    compileSdk = 34

    defaultConfig {{
        applicationId = "{package_id}"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        ndk {{
            abiFilters += listOf("arm64-v8a")
        }}
    }}

    signingConfigs {{
        create("release") {{
            if (hasReleaseKey) {{
                storeFile = rootProject.file(keystoreProps.getProperty("storeFile"))
                storePassword = keystoreProps.getProperty("storePassword")
                keyAlias = keystoreProps.getProperty("keyAlias")
                keyPassword = keystoreProps.getProperty("keyPassword")
            }}
        }}
    }}

    buildTypes {{
        debug {{
            applicationIdSuffix = ".debug"
        }}
        release {{
            signingConfig = signingConfigs.getByName(
                if (hasReleaseKey) "release" else "debug"
            )
            isMinifyEnabled = false
            isShrinkResources = false
        }}
    }}

    sourceSets {{
        getByName("main") {{
            jniLibs.srcDirs("src/main/jniLibs")
        }}
    }}
}}
"#
    )
}

fn android_manifest(lib_name: &str, display_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <application
        android:label="{display_name}"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:theme="@android:style/Theme.NoTitleBar.Fullscreen"
        android:allowBackup="false"
        android:supportsRtl="true"
        android:resizeableActivity="true">
        <activity
            android:name="android.app.NativeActivity"
            android:exported="true"
            android:launchMode="singleTask"
            android:configChanges="orientation|keyboardHidden|screenSize|screenLayout|density|uiMode"
            android:screenOrientation="unspecified">
            <meta-data
                android:name="android.app.lib_name"
                android:value="{lib_name}" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#
    )
}

const COLORS_XML: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="ic_launcher_background">#101418</color>
</resources>
"##;

const IC_LAUNCHER_FOREGROUND_XML: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path
        android:fillColor="#3B82F6"
        android:pathData="M30,54 L54,30 L78,54 L54,78 Z" />
</vector>
"##;

const IC_LAUNCHER_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@drawable/ic_launcher_foreground" />
</adaptive-icon>
"#;

const IC_LAUNCHER_ROUND_XML: &str = IC_LAUNCHER_XML;

const GRADLE_WRAPPER_PROPERTIES: &str = r#"distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=https\://services.gradle.org/distributions/gradle-8.7-bin.zip
networkTimeout=10000
validateDistributionUrl=true
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
"#;

const ANDROID_GITIGNORE: &str = r#"app/src/main/jniLibs/
*.keystore
keystore.properties
local.properties
.gradle/
build/
app/build/
"#;

const KEYSTORE_PROPERTIES_EXAMPLE: &str = r#"storeFile=your-keystore.jks
storePassword=your-store-password
keyAlias=your-key-alias
keyPassword=your-key-password
"#;
