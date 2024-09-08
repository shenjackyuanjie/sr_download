use colored::Colorize;
use futures::future::select_all;
use std::{io::Write, ops::Range};
use tokio::sync::oneshot::Receiver;
use tracing::{event, Level};

pub mod config;
pub mod db_part;
#[allow(unused)]
pub mod model;
pub mod net;
pub mod web_part;

use crate::db_part::CoverStrategy;
use migration::SaveId;
use model::sea_orm_active_enums::SaveType;
pub use net::Downloader;

async fn big_worker(
    db: sea_orm::DatabaseConnection,
    client: Downloader,
    work_range: Range<SaveId>,
) {
    for work_id in work_range {
        let exist_len = db_part::check_data_len(&db, work_id).await;
        if exist_len.is_some() && exist_len.unwrap() > 0 {
            event!(
                Level::INFO,
                "{}",
                format!("Skip download {} with exist data", work_id).blue()
            );
            continue;
        }
        match match client.try_download_as_any(work_id).await {
            Some(file) => {
                event!(
                    Level::INFO,
                    "{}",
                    format!(
                        "Download {} with {} data len: {}",
                        work_id,
                        file.type_name(),
                        file.len()
                    )
                    .green()
                );
                let save_type = (&file).into();
                db_part::save_data_to_db(
                    work_id,
                    save_type,
                    file.take_data(),
                    Some(CoverStrategy::CoverIfDifferent),
                    &db,
                )
            }
            None => {
                if exist_len.is_some() {
                    event!(
                        Level::INFO,
                        "{}",
                        format!("Skip save {} with no data", work_id).cyan()
                    );
                    continue;
                }
                event!(
                    Level::INFO,
                    "{}",
                    format!("Download {} with no data", work_id).yellow()
                );
                db_part::save_data_to_db(work_id, SaveType::None, "".to_string(), None, &db)
            }
        }
        .await
        {
            Ok(_) => (),
            Err(e) => event!(Level::WARN, "Save data {} failed: {:?}", work_id, e),
        }
    }
}

async fn serve_mode(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
    let span = tracing::span!(Level::INFO, "serve_mode");
    let _enter = span.enter();

    let conf = config::ConfigFile::try_read()?;

    let db_connect = db_part::connect(&conf).await?;
    db_part::migrate(&db_connect).await?;
    db_part::utils::check_null_data(&db_connect).await;
    db_part::utils::update_xml_tested(&db_connect).await;
    let mut db_max_id = db_part::search::max_id(&db_connect).await;

    let mut web_waiter = None;
    if conf.serve.enable {
        web_waiter = Some(tokio::spawn(web_part::web_main()));
    }

    event!(
        Level::INFO,
        "{}",
        format!(
            "数据库中最大的现有数据 id 为: {} 将从这里开始下载",
            db_max_id
        )
        .green()
    );

    let serve_wait_time = conf.serve_duration();
    let client = Downloader::new(None);

    let mut waited = false;
    // 开始等待的时间
    let mut start_wait_time = tokio::time::Instant::now();

    loop {
        if stop_receiver.try_recv().is_ok() {
            event!(Level::INFO, "{}", "结束下载!".yellow());
            // 结束 db
            db_connect.close().await?;
            if conf.serve.enable {
                if let Some(web_waiter) = web_waiter {
                    web_waiter.abort();
                }
            }
            return Ok(());
        }

        tokio::select! {
            _ = tokio::time::sleep(serve_wait_time) => {
                let work_id = db_max_id + 1;
                match client.try_download_as_any(work_id).await {
                    Some(file) => {
                        if waited {
                            println!();
                            waited = false;
                        }
                        let wait_time = start_wait_time.elapsed();
                        start_wait_time = tokio::time::Instant::now();
                        event!(
                            Level::INFO,
                            "{}",
                            format!(
                                "下载到了新的 {}!(懒得做中文了) ID为: {} 长度: {}, 等了 {}",
                                file.type_name(),
                                work_id,
                                file.len(),
                                format!("{:?}", wait_time).blue()
                            )
                            .green()
                        );
                        let save_type: SaveType = (&file).into();
                        match db_part::save_data_to_db(
                            work_id,
                            save_type,
                            file.take_data(),
                            Some(CoverStrategy::CoverIfDifferent),
                            &db_connect,
                        )
                        .await
                        {
                            Ok(_) => {
                                {
                                    db_max_id = work_id;
                                    event!(
                                        Level::INFO,
                                        "{}",
                                        format!(
                                            "保存好啦! (下一排的每一个 . 代表一个 {:?})",
                                            serve_wait_time
                                        )
                                        .green()
                                    );
                                    continue; // 保存好之后立即尝试下一次, 保证连续上传的时候的效率
                                };
                            }
                            Err(e) => {
                                event!(Level::ERROR, "呜呜呜, 数据保存失败了: {:?}\n我不玩了!", e);
                                return Err(e);
                            }
                        }
                    }
                    None => {
                        print!(".");
                        waited = true;
                        let _ = std::io::stdout().flush();
                    }
                }
            }
            _ = &mut stop_receiver => {
                event!(Level::INFO, "{}", "结束下载!".yellow());
                // 结束 db
                db_connect.close().await?;
                return Ok(());
            }
        }
    }
}

async fn fast_mode(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
    let span = tracing::span!(Level::INFO, "fast_mode");
    let _enter = span.enter();

    let conf = config::ConfigFile::try_read()?;

    let db_connect = db_part::connect(&conf).await?;
    db_part::migrate(&db_connect).await?;
    db_part::utils::check_null_data(&db_connect).await;
    db_part::utils::update_xml_tested(&db_connect).await;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    if stop_receiver.try_recv().is_ok() {
        event!(Level::INFO, "{}", "Stop download".red());
        // 结束 db
        db_connect.close().await?;
        return Ok(());
    }

    let end_id: SaveId = conf.sync.fast.end_id;
    let worker_size = conf.sync.fast.worker_size;
    let mut current_id = conf.sync.fast.start_id;
    let mut works = Vec::with_capacity(conf.sync.fast.worker_count as usize);
    let max_works = conf.sync.fast.worker_count as usize;
    for _ in 0..works.len() {
        if stop_receiver.try_recv().is_ok() {
            event!(Level::INFO, "{}", "Stop download".red());
            // 结束 db
            db_connect.close().await?;
            return Ok(());
        }
        let client = net::Downloader::new(Some(conf.net_timeout()));
        let end = current_id + worker_size;
        works.push(tokio::spawn(big_worker(
            db_connect.clone(),
            client,
            current_id..end,
        )));
        current_id = end;
    }

    while current_id < end_id || !works.is_empty() {
        if stop_receiver.try_recv().is_ok() {
            event!(Level::INFO, "{}", "Stop download".red());
            // 结束 db
            db_connect.close().await?;
            return Ok(());
        }
        while current_id < end_id && works.len() < max_works {
            let client = net::Downloader::new(Some(conf.net_timeout()));
            let end = current_id + worker_size;
            works.push(tokio::spawn(big_worker(
                db_connect.clone(),
                client,
                current_id..end,
            )));
            current_id = end;
        }

        if !works.is_empty() {
            let (_result, _index, remain) = select_all(works).await;
            works = remain;
        }
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> anyhow::Result<()> {
    // 判断是否有 -f / -s 参数
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"-d".to_string()) {
        // debug 模式
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    }
    event!(Level::INFO, "Starting srdownload");

    // 判断是否有 -f / -s 参数
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel::<()>();
    let stop_waiter = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C event");
        event!(Level::INFO, "{}", "Ctrl-C received".red());
        stop_sender.send(()).unwrap();
    });
    let job_waiter;

    if args.contains(&"-s".to_string()) {
        job_waiter = tokio::spawn(serve_mode(stop_receiver));
    } else if args.contains(&"-f".to_string()) {
        job_waiter = tokio::spawn(fast_mode(stop_receiver));
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
    job_waiter.await??;
    let _ = stop_waiter.await;
    Ok(())
}
