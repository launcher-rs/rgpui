use std::sync::OnceLock;

use rustls::ClientConfig;
use rustls_platform_verifier::ConfigVerifierExt;

/// 全局 TLS 配置，仅初始化一次
static TLS_CONFIG: OnceLock<rustls::ClientConfig> = OnceLock::new();

/// 获取全局 TLS 客户端配置
pub fn tls_config() -> ClientConfig {
    TLS_CONFIG
        .get_or_init(|| {
            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .ok();

            ClientConfig::with_platform_verifier()
        })
        .clone()
}
