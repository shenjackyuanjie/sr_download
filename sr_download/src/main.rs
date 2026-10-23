use std::{num::NonZeroUsize, path::PathBuf, time::SystemTime};

use clap::{ArgGroup, Parser};
use colored::Colorize;
use sr_download::{START_TIME, config, fast_mode, serve_mode};
use tracing::{Level, event};

enum RunMode {
    /// 服务模式
    Serve,
    /// 快速模式
    Fast,
    /// 数据维护模式
    Maintenance,
}
#[derive(Parser, Debug)]
#[command(
    name = "srdownload",
    about = "simple rocket下载器",
    version,
    author,
    group(
        ArgGroup::new("mode")
            .required(true)
            .args(&["serve", "fast", "maintenance"])
    )
)]
struct Cli {
    /// Debug模式
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    /// 运行线程数 (默认10)
    #[arg(short = 't', long = "threads", default_value = "10")]
    threads: NonZeroUsize,

    /// 配置文件路径 (默认 "./config.toml")
    #[arg(short = 'c', long = "config", default_value = "./config.toml")]
    config: String,

    /// 服务模式
    #[arg(short = 's', long = "serve", group = "mode")]
    serve: bool,

    /// 快速同步模式(用于从零开始)
    #[arg(short = 'f', long = "fast", group = "mode")]
    fast: bool,

    /// 执行数据库修复任务，不启动下载或 Web 服务
    #[arg(long = "maintenance", group = "mode")]
    maintenance: bool,
}

fn main() -> anyhow::Result<()> {
    START_TIME.get_or_init(SystemTime::now);

    let cli = Cli::parse();

    if cli.debug {
        tracing_subscriber::fmt()
            .with_ansi(true)
            .with_ansi_sanitization(false)
            .with_max_level(Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_ansi(true)
            .with_ansi_sanitization(false)
            .with_max_level(Level::INFO)
            .init();
    }

    let mode = if cli.serve {
        RunMode::Serve
    } else if cli.fast {
        RunMode::Fast
    } else if cli.maintenance {
        RunMode::Maintenance
    } else {
        event!(
            Level::ERROR,
            "{}",
            "Please use -s, -f, or --maintenance to start the program".red()
        );
        event!(Level::ERROR, "{}", "Use -s to start serve mode".red());
        event!(Level::ERROR, "{}", "Use -f to start fast mode".red());
        event!(
            Level::ERROR,
            "{}",
            "Use --maintenance to run database repairs".red()
        );
        return Ok(());
    };

    event!(Level::INFO, "Starting sr download");

    let cfg_path = cli.config.clone();

    config::ConfigFile::init_global(Some(PathBuf::from(cfg_path)))?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cli.threads.get())
        .enable_all()
        .build()?;
    rt.block_on(async_main(mode))
}

async fn async_main(run_mode: RunMode) -> anyhow::Result<()> {
    use anyhow::Context;
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel::<()>();
    let run = async move {
        match run_mode {
            RunMode::Serve => serve_mode::main(stop_receiver).await,
            RunMode::Fast => fast_mode::main(stop_receiver).await,
            RunMode::Maintenance => maintenance_main(stop_receiver).await,
        }
    };
    tokio::pin!(run);
    tokio::select! {
        result = &mut run => result,
        signal = tokio::signal::ctrl_c() => {
            signal.context("无法监听 Ctrl-C 信号")?;
            event!(Level::INFO, "{}", "收到 Ctrl-C，正在停止".red());
            let _ = stop_sender.send(());
            run.await
        }
    }
}

async fn maintenance_main(
    mut stop_receiver: tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let conf = config::ConfigFile::get_global();
    let db = sr_download::db_part::connect(conf).await?;
    // 先保存维护结果再关闭连接池，确保维护失败不会因清理资源而被吞掉。
    let result =
        sr_download::db_part::run_maintenance_until_stopped(&db, conf, &mut stop_receiver).await;
    db.close().await;
    match result? {
        sr_download::db_part::MaintenanceOutcome::Completed => Ok(()),
        sr_download::db_part::MaintenanceOutcome::Cancelled => {
            event!(Level::INFO, "维护任务已按 Ctrl-C 请求取消");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_requires_exactly_one_mode() {
        assert!(Cli::try_parse_from(["sr_download"]).is_err());
        assert!(Cli::try_parse_from(["sr_download", "--serve", "--fast"]).is_err());
        assert!(Cli::try_parse_from(["sr_download", "--maintenance"]).is_ok());
    }

    #[test]
    fn cli_rejects_zero_threads() {
        assert!(Cli::try_parse_from(["sr_download", "--fast", "--threads", "0"]).is_err());
        let cli = Cli::try_parse_from(["sr_download", "--fast", "--threads", "2"]).unwrap();
        assert_eq!(cli.threads.get(), 2);
    }
}
