use std::sync::OnceLock;

use rustls::ClientConfig;
use rustls_platform_verifier::ConfigVerifierExt;

/// 全局 TLS 配置，仅初始化一次
static TLS_CONFIG: OnceLock<rustls::ClientConfig> = OnceLock::new();
/// crypto provider 只安装一次（重复安装会返回 Err，忽略即可）。
static PROVIDER_INSTALLED: OnceLock<()> = OnceLock::new();

/// 安装 rustls crypto provider。
///
/// reqwest 使用 `rustls-no-provider`，任何 `Client::builder().use_rustls_tls()`
/// 之前都必须先装好 provider，否则构建 Client 直接 panic。`ReqwestClient::builder`
/// 会调用本函数；这里做成独立函数以便与 TLS 配置初始化解耦。
pub fn install_crypto_provider() {
    PROVIDER_INSTALLED.get_or_init(|| {
        // 用 ring 而非 aws-lc-rs：aws-lc-sys 需要 CMake 编译大量 C/汇编，
        // 是三平台构建的大头；ring 是纯 Rust + 少量汇编，编译更快。
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// 获取全局 TLS 客户端配置
pub fn tls_config() -> ClientConfig {
    TLS_CONFIG
        .get_or_init(|| {
            install_crypto_provider();

            ClientConfig::with_platform_verifier().unwrap_or_else(|err| {
                // 平台证书校验器不可用（系统根证书缺失等）属环境问题：退回空配置，
                // 由 TLS 握手阶段报错，而不是让进程在初始化时崩掉。
                log::error!("[rgpui::http_client] 平台证书校验器不可用：{err}");
                ClientConfig::builder()
                    .with_root_certificates(rustls::RootCertStore::empty())
                    .with_no_client_auth()
            })
        })
        .clone()
}
