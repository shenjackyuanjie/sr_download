use std::io::Write;

use colored::Colorize;
use tokio::sync::oneshot::Receiver;
use tracing::{event, Level};

use crate::db_part::{CoverStrategy, SaveType};
use crate::{config, db_part, web_part, Downloader};

pub async fn main(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
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
