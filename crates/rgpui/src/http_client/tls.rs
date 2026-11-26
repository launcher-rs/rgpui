use std::sync::OnceLock;

use rustls::ClientConfig;
use rustls_platform_verifier::ConfigVerifierExt;

/// 全局 TLS 配置，仅初始化一次
static TLS_CONFIG: OnceLock<rustls::ClientConfig> = OnceLock::new();

/// 获取全局 TLS 客户端配置
pub fn tls_config() -> ClientConfig {
    TLS_CONFIG
        .get_or_init(|| {
            // 用 ring 而非 aws-lc-rs：aws-lc-sys 需要 CMake 编译大量 C/汇编，
            // 是三平台构建的大头；ring 是纯 Rust + 少量汇编，编译更快。
            rustls::crypto::ring::default_provider()
                .install_default()
                .ok();

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
