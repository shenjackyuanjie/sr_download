use colored::Colorize;
use tracing::{event, Level};

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

const HELP_MSG: &str = r#"Usage: srdownload [options] -s/f
Options:
    -d    Debug mode
    -t=xx 运行线程数 (默认 10)
    -s    服务模式
    -f    快速同步模式(用于从零开始)"#;

fn main() -> anyhow::Result<()> {
    // 检查 CLI 参数

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"-h".to_string()) {
        println!("{}", HELP_MSG);
        return Ok(());
    }

    let mut thread_count = 10;
    if args.iter().any(|x| x.starts_with("-t=")) {
        thread_count = args
            .iter()
            .find(|x| x.starts_with("-t="))
            .unwrap()
            .split('=')
            .last()
            .unwrap()
            .parse::<usize>()?;
    }
    if args.contains(&"-d".to_string()) {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    }
    let mode = {
        if args.contains(&"-s".to_string()) {
            RunMode::Serve
        } else if args.contains(&"-f".to_string()) {
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
        }
    };

    event!(Level::INFO, "Starting sr download");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(thread_count)
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
