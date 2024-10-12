use std::ops::Range;

use migration::SaveId;

use colored::Colorize;
use futures::future::select_all;
use tokio::sync::oneshot::Receiver;
use tracing::{event, Level};

use crate::db_part::{CoverStrategy, SaveType};
use crate::{config, db_part, Downloader};

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

pub async fn main(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
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
        let client = Downloader::new(Some(conf.net_timeout()));
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
            let client = Downloader::new(Some(conf.net_timeout()));
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
