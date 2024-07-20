use colored::Colorize;
use futures::future::select_all;
use std::{ops::Range, path::Path};
use tokio::sync::oneshot::Receiver;
use tracing::{event, Level};

mod config;
mod db_part;
#[allow(unused)]
mod model;
mod net;

pub type SaveId = u32;
pub const TEXT_DATA_MAX_LEN: usize = 1024;

use model::sea_orm_active_enums::SaveType;

use crate::db_part::CoverStrategy;

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

async fn main_works(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
    let conf = match config::ConfigFile::read_from_file(Path::new("config.toml")) {
        Ok(conf) => conf,
        Err(_) => {
            config::ConfigFile::write_default_to_file(Path::new("config.toml"))?;
            config::ConfigFile::default()
        }
    };

    let db_connect = db_part::connect(&conf).await?;
    let db_max_id = db_part::find_max_id(&db_connect).await;

    event!(
        Level::INFO,
        "{}",
        format!("db max downloaded save_id: {}", db_max_id).green()
    );

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 1321469 end
    let end_id: SaveId = 1321469;

    let mut current_id = conf.start_id;

    let batch_size = conf.worker_size;
    // 10 works
    let mut works = Vec::with_capacity(conf.worker_count as usize);
    let max_works = conf.worker_count as usize;
    for _ in 0..works.len() {
        let client = net::Downloader::new(conf.timeout_as_duration());
        let end = current_id + batch_size;
        works.push(tokio::spawn(big_worker(
            db_connect.clone(),
            client,
            current_id..end,
        )));
        current_id = end;
    }

    while current_id < end_id || !works.is_empty() {
        while current_id < end_id && works.len() < max_works {
            let client = net::Downloader::new(conf.timeout_as_duration());
            let end = current_id + batch_size;
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
        if stop_receiver.try_recv().is_ok() {
            event!(Level::INFO, "{}", "Stop download".red());
            // 结束 db
            db_connect.close().await?;
            break;
        }
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    event!(Level::INFO, "Starting srdownload");

    // 初始化一个 ctrl-c 的监听器
    let (ctrl_c_sender, ctrl_c_receiver) = tokio::sync::oneshot::channel::<()>();

    // 把 main_works spawn 出去, 这样就可以在主线程检测 ctrl-c 了
    let main_works = tokio::spawn(main_works(ctrl_c_receiver));

    // ctrl-c 信号处理
    let ctrl_c_waiter = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C event");
        event!(Level::INFO, "{}", "Ctrl-C received".red());
        ctrl_c_sender.send(()).unwrap();
    });

    main_works.await??;
    if !ctrl_c_waiter.is_finished() {
        ctrl_c_waiter.abort()
    }
    Ok(())
}
