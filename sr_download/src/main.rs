use colored::Colorize;
use futures::future::select_all;
use std::{ops::Range, path::Path};
use tracing::{event, Level};

mod config;
mod db_part;
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
        if db_part::check_have_none_empty_data(&db, work_id).await {
            event!(Level::INFO, "{}", format!("Skip {}", work_id).blue());
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

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    event!(Level::INFO, "Starting srdownload");

    let conf = match config::ConfigFile::read_from_file(Path::new("config.toml")) {
        Ok(conf) => conf,
        Err(_) => {
            config::ConfigFile::write_default_to_file(Path::new("config.toml"))?;
            config::ConfigFile::default()
        }
    };

    let db_connect = db_part::connect(&conf).await?;
    let db_max_id = db_part::find_max_id(&db_connect).await;

    event!(Level::INFO, "db max downloaded save_id: {}", db_max_id);

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
    }

    Ok(())
}
