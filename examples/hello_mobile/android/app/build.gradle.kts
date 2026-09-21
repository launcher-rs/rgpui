import java.util.Properties

plugins {
    id("com.android.application")
}

val keystorePropsFile = rootProject.file("keystore.properties")
val keystoreProps = Properties()
if (keystorePropsFile.exists()) {
    keystorePropsFile.inputStream().use { keystoreProps.load(it) }
}
val hasReleaseKey = keystoreProps.containsKey("storeFile")

android {
    namespace = "com.example.hellomobile"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.example.hellomobile"
        // 与 rgpui-android 最低要求对齐：API 26+（Android 8.0）。
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "1.4.0"

        // 只打 arm64-v8a（真机）；模拟器调试时临时加回 x86_64。
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    signingConfigs {
        // release 签名读 android/keystore.properties（见同名 .example 文件），
        // 不存在则回退 debug 签名，保证 assembleRelease 在 CI 也能出包。
        create("release") {
            if (hasReleaseKey) {
                storeFile = rootProject.file(keystoreProps.getProperty("storeFile"))
                storePassword = keystoreProps.getProperty("storePassword")
                keyAlias = keystoreProps.getProperty("keyAlias")
                keyPassword = keystoreProps.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
        }
        release {
            signingConfig = signingConfigs.getByName(
                if (hasReleaseKey) "release" else "debug"
            )
            isMinifyEnabled = false
            isShrinkResources = false
        }
    }

    // cargo-ndk 把 .so 写到 app/src/main/jniLibs/<abi>/，AGP 自动打包。
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}
