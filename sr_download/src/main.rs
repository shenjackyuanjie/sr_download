use std::{sync::OnceLock, time::SystemTime};

use clap::{ArgGroup, Parser};
use colored::Colorize;
use tracing::{Level, event};

pub mod config;
pub mod db_part;
/// 快速同步
pub mod fast_mode;
#[allow(unused)]
pub mod model;
pub mod net;
/// 服务模式
pub mod serve_mode;
pub mod web_part;

use migration::SaveId;
pub use net::Downloader;

enum RunMode {
    /// 服务模式
    Serve,
    /// 快速模式
    Fast,
}
/// 开始时间
pub static START_TIME: OnceLock<SystemTime> = OnceLock::new();

#[derive(Parser, Debug)]
#[command(
    name = "srdownload",
    about = "simple rocket下载器",
    version,
    author,
    group(
        ArgGroup::new("mode")
            .required(true)
            .args(&["serve", "fast"])
    )
)]
struct Cli {
    /// Debug模式
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    /// 运行线程数 (默认10)
    #[arg(short = 't', long = "threads", default_value_t = 10)]
    threads: usize,

    /// 服务模式
    #[arg(short = 's', long = "serve", group = "mode")]
    serve: bool,

    /// 快速同步模式(用于从零开始)
    #[arg(short = 'f', long = "fast", group = "mode")]
    fast: bool,
}

fn main() -> anyhow::Result<()> {
    START_TIME.get_or_init(SystemTime::now);

    let cli = Cli::parse();

    if cli.debug {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    }

    let mode = if cli.serve {
        RunMode::Serve
    } else if cli.fast {
        RunMode::Fast
    } else {
        event!(
            Level::ERROR,
            "{}",
            "Please use -s or -f to start the program".red()
        );
        event!(Level::ERROR, "{}", "Use -s to start serve mode".red());
        event!(Level::ERROR, "{}", "Use -f to start fast mode".red());
        return Ok(());
    };

    event!(Level::INFO, "Starting sr download");

    config::ConfigFile::init_global(None);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cli.threads)
        .enable_all()
        .build()?;
    rt.block_on(async_main(mode))
}

async fn async_main(run_mode: RunMode) -> anyhow::Result<()> {
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel::<()>();

    let stop_waiter = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C event");
        event!(Level::INFO, "{}", "Ctrl-C received".red());
        stop_sender.send(()).unwrap();
    });

    let job_waiter = match run_mode {
        RunMode::Serve => tokio::spawn(serve_mode::main(stop_receiver)),
        RunMode::Fast => tokio::spawn(fast_mode::main(stop_receiver)),
    };
    job_waiter.await??;
    let _ = stop_waiter.await;
    Ok(())
}
