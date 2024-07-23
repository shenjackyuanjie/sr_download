use colored::Colorize;
use futures::future::select_all;
use std::{io::Write, ops::Range};
use tokio::sync::oneshot::Receiver;
use tracing::{event, Level};

mod config;
mod db_part;
#[allow(unused)]
mod model;
mod net;

use crate::db_part::CoverStrategy;
use migration::SaveId;
use model::sea_orm_active_enums::SaveType;

async fn big_worker(
    db: sea_orm::DatabaseConnection,
    client: net::Downloader,
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
    let conf = config::ConfigFile::read_or_panic();

    let db_connect = db_part::connect(&conf).await?;
    db_part::migrate(&db_connect).await?;
    let mut db_max_id = db_part::find_max_id(&db_connect).await;

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
    let client = net::Downloader::new(conf.net_timeout());

    let mut waited = false;
    loop {
        if stop_receiver.try_recv().is_ok() {
            event!(Level::INFO, "{}", "结束下载!".yellow());
            // 结束 db
            db_connect.close().await?;
            return Ok(());
        }

        let work_id = db_max_id + 1;
        match client.try_download_as_any(work_id).await {
            Some(file) => {
                if waited {
                    println!();
                    waited = false;
                }
                event!(
                    Level::INFO,
                    "{}",
                    format!(
                        "下载到了新的 {}!(懒得做中文了) ID为: {} 长度: {}",
                        file.type_name(),
                        work_id,
                        file.len()
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

        tokio::time::sleep(serve_wait_time).await;
    }
}

async fn fast_mode(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
    let conf = config::ConfigFile::read_or_panic();

    let db_connect = db_part::connect(&conf).await?;
    db_part::migrate(&db_connect).await?;

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
        let client = net::Downloader::new(conf.net_timeout());
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
            let client = net::Downloader::new(conf.net_timeout());
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
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    event!(Level::INFO, "Starting srdownload");

    // 判断是否有 -f / -s 参数
    let args: Vec<String> = std::env::args().collect();
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel::<()>();

    if args.contains(&"-s".to_string()) {
        let job_waiter = tokio::spawn(serve_mode(stop_receiver));
        // serve 模式的任务不会结束, 所以需要等待 ctrl-c
        tokio::signal::ctrl_c().await?;
        let _ = stop_sender.send(()); // 反正不需要管, 发过去了就行
        job_waiter.await??;
        event!(Level::INFO, "{}", "ctrl-c 收到啦! 停止下载".green());
        return Ok(());
    } else if args.contains(&"-f".to_string()) {
        let stop_waiter = tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for Ctrl+C event");
            event!(Level::INFO, "{}", "Ctrl-C received".red());
            stop_sender.send(()).unwrap();
        });
        let job_waiter = tokio::spawn(fast_mode(stop_receiver));
        // fast 模式的任务会结束, 所以需要等待任务结束
        job_waiter.await??;
        let _ = stop_waiter.await;
        return Ok(());
    }

    event!(
        Level::ERROR,
        "{}",
        "Please use -s or -f to start the program".red()
    );
    event!(Level::ERROR, "{}", "Use -s to start serve mode".red());
    event!(Level::ERROR, "{}", "Use -f to start fast mode".red());
    Ok(())
}
