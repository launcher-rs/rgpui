mod build;
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
    },
    /// 构建项目（cargo ndk → strip → gradlew）
    Build {
        /// 构建 release 版本
        #[arg(long)]
        release: bool,
    },
    /// 构建并安装到设备
    Install {
        /// 构建 release 版本
        #[arg(long)]
        release: bool,
    },
    /// 构建 + 安装 + 启动 + 实时日志
    Run {
        /// 构建 release 版本
        #[arg(long)]
        release: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor => doctor::run()?,
        Commands::New { name } => new::run(&name)?,
        Commands::Build { release } => build::run(release)?,
        Commands::Install { release } => {
            build::run(release)?;
            install::run()?;
        }
        Commands::Run { release } => {
            build::run(release)?;
            run::run()?;
        }
    }

    Ok(())
}
