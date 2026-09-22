mod adb;
mod build;
mod config;
mod doctor;
mod install;
mod new;
mod run;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rgpui",
    about = "rgpui 移动端开发 CLI",
    long_about = "一站式工具：检查环境 / 创建项目 / 构建部署到 Android/iOS 真机"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 检查移动端开发环境（NDK / SDK / Rust target / 设备连接）
    Doctor,
    /// 创建新的 rgpui 移动端项目
    New {
        /// 项目名（kebab-case，如 my_app）
        name: String,
        /// 包名前缀（覆盖配置），如 com.mycompany.
        #[arg(long)]
        package_prefix: Option<String>,
        /// rgpui git 分支（覆盖配置）
        #[arg(long)]
        branch: Option<String>,
    },
    /// 构建项目（cargo ndk → strip → gradlew）
    Build {
        /// 构建 release 版本
        #[arg(long)]
        release: bool,
        /// 主 ABI（覆盖配置），如 arm64-v8a / x86_64
        #[arg(long)]
        abi: Option<String>,
        /// 跳过 strip（覆盖配置）
        #[arg(long)]
        no_strip: bool,
    },
    /// 构建并安装到设备
    Install {
        /// 构建 release 版本
        #[arg(long)]
        release: bool,
        /// 主 ABI（覆盖配置）
        #[arg(long)]
        abi: Option<String>,
        /// 跳过 strip
        #[arg(long)]
        no_strip: bool,
    },
    /// 构建 + 安装 + 启动 + 实时日志
    Run {
        /// 构建 release 版本
        #[arg(long)]
        release: bool,
        /// 主 ABI（覆盖配置）
        #[arg(long)]
        abi: Option<String>,
        /// 跳过 strip
        #[arg(long)]
        no_strip: bool,
    },
    /// 配置管理（查看 / 生成 rgpui.toml）
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// 显示生效配置（全局 + 项目合并后）
    Show,
    /// 在当前目录生成默认 rgpui.toml
    Init,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor => doctor::run()?,
        Commands::New {
            name,
            package_prefix,
            branch,
        } => {
            let (mut cfg, _) = config::load(std::path::Path::new("."))?;
            if let Some(p) = package_prefix {
                cfg.project.package_prefix = Some(p);
            }
            if let Some(b) = branch {
                cfg.project.branch = Some(b);
            }
            new::run(&name, &cfg)?;
        }
        Commands::Build {
            release,
            abi,
            no_strip,
        } => {
            let (mut cfg, _) = config::load(std::path::Path::new("."))?;
            if let Some(a) = abi {
                cfg.android.abis = Some(vec![a]);
            }
            if no_strip {
                cfg.android.strip = Some(false);
            }
            build::run(release, &cfg)?;
        }
        Commands::Install {
            release,
            abi,
            no_strip,
        } => {
            let (mut cfg, _) = config::load(std::path::Path::new("."))?;
            if let Some(a) = abi {
                cfg.android.abis = Some(vec![a]);
            }
            if no_strip {
                cfg.android.strip = Some(false);
            }
            build::run(release, &cfg)?;
            install::run(release)?;
        }
        Commands::Run {
            release,
            abi,
            no_strip,
        } => {
            let (mut cfg, _) = config::load(std::path::Path::new("."))?;
            if let Some(a) = abi {
                cfg.android.abis = Some(vec![a]);
            }
            if no_strip {
                cfg.android.strip = Some(false);
            }
            build::run(release, &cfg)?;
            run::run(release, &cfg)?;
        }
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                let (cfg, sources) = config::load(std::path::Path::new("."))?;
                cfg.print_effective(&sources);
            }
            ConfigAction::Init => {
                let path = std::path::Path::new(config::PROJECT_CONFIG_FILE);
                if path.exists() {
                    anyhow::bail!("{} 已存在，不覆盖", config::PROJECT_CONFIG_FILE);
                }
                std::fs::write(path, config::default_config_toml())?;
                println!("已生成 {}", config::PROJECT_CONFIG_FILE);
            }
        },
    }

    Ok(())
}
