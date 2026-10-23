use anyhow::{Context, ensure};
use sqlx::PgPool;
use tokio::sync::oneshot::Receiver;
use tracing::{Level, event};

use crate::db_part::{CoverStrategy, SaveType};
use crate::{Downloader, SaveId, config, db_part, web_part};

fn next_id(current: SaveId) -> anyhow::Result<SaveId> {
    let next = current.checked_add(1).context("下载 ID 已到达上限")?;
    ensure!(next <= i32::MAX as u32, "下载 ID 超出数据库 integer 范围");
    Ok(next)
}

async fn download_loop(db: &PgPool, conf: &config::ConfigFile) -> anyhow::Result<()> {
    let client = Downloader::try_new(Some(conf.net_timeout()))?;
    let mut current = db_part::search::max_id(db).await;
    event!(Level::INFO, current, "从数据库最大 ID 继续下载");
    loop {
        let id = next_id(current)?;
        match client.download_any(id).await {
            Ok(Some(file)) => {
                let save_type: SaveType = (&file).into();
                db_part::save_data_to_db(
                    id,
                    save_type,
                    file.take_data(),
                    Some(CoverStrategy::CoverIfDifferent),
                    db,
                )
                .await
                .with_context(|| format!("保存 ID {id} 失败"))?;
                current = id;
                event!(Level::INFO, id, "下载并保存成功");
                // 连续上传时不等待，立即尝试下一条。
                continue;
            }
            Ok(None) => event!(Level::DEBUG, id, "等待新数据"),
            Err(error) => {
                event!(Level::WARN, id, error = %error, "下载暂时失败，等待后重试同一 ID")
            }
        }
        tokio::time::sleep(conf.serve_duration()).await;
    }
}

#[tracing::instrument(skip_all)]
pub async fn main(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
    let conf = config::ConfigFile::get_global();
    conf.validate()?;
    let db = db_part::connect(conf).await?;
    let result = async {
        db_part::initialize_db(&db, conf).await?;
        let (web_stop, web_stopped) = tokio::sync::oneshot::channel();
        let mut web_task = conf.serve.enable.then(|| {
            tokio::spawn(web_part::web_main_until_stopped(async move {
                let _ = web_stopped.await;
            }))
        });
        let mut web_finished = false;
        let result = tokio::select! {
            biased;
            _ = &mut stop_receiver => {
                event!(Level::INFO, "收到停止请求，结束下载");
                Ok(())
            }
            result = async { web_task.as_mut().unwrap().await }, if web_task.is_some() => {
                web_finished = true;
                match result {
                    Err(error) => Err(anyhow::Error::new(error).context("Web 工作任务异常退出")),
                    Ok(Err(error)) => Err(error.context("Web 服务运行失败")),
                    Ok(Ok(())) => Err(anyhow::anyhow!("Web 服务意外结束")),
                }
            }
            result = download_loop(&db, conf) => result,
        };
        if !web_finished && let Some(mut task) = web_task {
            let _ = web_stop.send(());
            if tokio::time::timeout(std::time::Duration::from_secs(10), &mut task)
                .await
                .is_err()
            {
                event!(Level::WARN, "Web 优雅停止超过 10 秒，取消剩余请求");
                task.abort();
                let _ = task.await;
            }
        }
        result
    }
    .await;
    db.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_id_stays_in_database_range() {
        assert_eq!(next_id(0).unwrap(), 1);
        assert_eq!(next_id(i32::MAX as u32 - 1).unwrap(), i32::MAX as u32);
        assert!(next_id(i32::MAX as u32).is_err());
        assert!(next_id(u32::MAX).is_err());
    }
}
