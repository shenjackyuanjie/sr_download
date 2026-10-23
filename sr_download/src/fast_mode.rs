use std::{future::Future, ops::Range};

use anyhow::{Context, ensure};
use sqlx::PgPool;
use tokio::{sync::oneshot::Receiver, task::JoinSet};
use tracing::{Level, event};

use crate::db_part::{CoverStrategy, SaveType};
use crate::{Downloader, SaveId, config, db_part};

async fn big_worker(
    db: PgPool,
    client: Downloader,
    work_range: Range<SaveId>,
) -> anyhow::Result<()> {
    let mut failures = 0usize;
    for work_id in work_range {
        if db_part::check_data_len(&db, work_id)
            .await
            .is_some_and(|len| len > 0)
        {
            event!(Level::DEBUG, work_id, "跳过已有非空数据");
            continue;
        }
        let file = match client.download_any(work_id).await {
            Ok(Some(file)) => file,
            Ok(None) => {
                event!(Level::DEBUG, work_id, "上游没有该 ID，数据库保持不变");
                continue;
            }
            Err(error) => {
                failures += 1;
                event!(Level::WARN, work_id, error = %error, "下载失败，数据库保持不变");
                continue;
            }
        };
        let save_type: SaveType = (&file).into();
        event!(
            Level::INFO,
            work_id,
            file_type = file.type_name(),
            bytes = file.len(),
            "下载成功"
        );
        if let Err(error) = db_part::save_data_to_db(
            work_id,
            save_type,
            file.take_data(),
            Some(CoverStrategy::CoverIfDifferent),
            &db,
        )
        .await
        {
            failures += 1;
            event!(Level::WARN, work_id, error = %error, "保存失败");
        }
    }
    ensure!(
        failures == 0,
        "该批次有 {failures} 条下载或保存失败，请重新运行以补齐"
    );
    Ok(())
}

/// 只生成 [start_id, end_id) 内的批次，饱和加法避免大批次溢出。
fn next_batch(current: &mut SaveId, end: SaveId, size: u32) -> Option<Range<SaveId>> {
    if *current >= end || size == 0 {
        return None;
    }
    let start = *current;
    *current = current.saturating_add(size).min(end);
    Some(start..*current)
}

async fn run_batches<F, Fut>(
    config: &config::FastSyncConfig,
    stop: &mut Receiver<()>,
    mut worker: F,
) -> anyhow::Result<()>
where
    F: FnMut(Range<SaveId>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    ensure!(
        config.worker_count > 0 && config.worker_size > 0,
        "工作并发数和批次大小必须大于 0"
    );
    ensure!(config.start_id <= config.end_id, "开始 ID 不能大于结束 ID");
    let mut current = config.start_id;
    let mut tasks = JoinSet::new();
    let mut first_error = None;
    loop {
        // 已收到的停止请求优先于派发新任务。
        if !matches!(
            stop.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ) {
            tasks.shutdown().await;
            return first_error.map_or(Ok(()), Err);
        }
        while tasks.len() < config.worker_count as usize {
            let Some(range) = next_batch(&mut current, config.end_id, config.worker_size) else {
                break;
            };
            tasks.spawn(worker(range));
        }
        if tasks.is_empty() {
            break;
        }
        tokio::select! {
            biased;
            _ = &mut *stop => {
                // 先取消并回收工作任务，再关闭连接池；被取消事务由 SQLx 回滚。
                tasks.shutdown().await;
                return first_error.map_or(Ok(()), Err);
            }
            result = tasks.join_next() => {
                if let Some(result) = result {
                    let result = result.context("下载工作任务异常退出").and_then(|result| result);
                    if let Err(error) = result {
                        event!(Level::ERROR, error = %error, "批次失败，继续处理剩余批次");
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[tracing::instrument(skip_all)]
pub async fn main(mut stop_receiver: Receiver<()>) -> anyhow::Result<()> {
    let conf = config::ConfigFile::get_global();
    conf.validate()?;
    let client = Downloader::try_new(Some(conf.net_timeout()))?;
    let db = db_part::connect(conf).await?;
    let result = async {
        db_part::initialize_db(&db, conf).await?;
        run_batches(&conf.sync.fast, &mut stop_receiver, |range| {
            big_worker(db.clone(), client.clone(), range)
        })
        .await
    }
    .await;
    db.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    fn test_config() -> config::FastSyncConfig {
        config::FastSyncConfig {
            start_id: 10,
            end_id: 21,
            worker_count: 2,
            worker_size: 4,
        }
    }

    #[test]
    fn batches_never_cross_the_exclusive_end() {
        let mut current = 10;
        assert_eq!(next_batch(&mut current, 21, 4), Some(10..14));
        assert_eq!(next_batch(&mut current, 21, 4), Some(14..18));
        assert_eq!(next_batch(&mut current, 21, 4), Some(18..21));
        assert_eq!(next_batch(&mut current, 21, 4), None);
    }

    #[test]
    fn batches_handle_overflow_empty_and_zero_size() {
        let mut current = u32::MAX - 2;
        assert_eq!(
            next_batch(&mut current, u32::MAX, 10),
            Some(u32::MAX - 2..u32::MAX)
        );
        assert_eq!(next_batch(&mut current, u32::MAX, 10), None);
        let mut current = 10;
        assert_eq!(next_batch(&mut current, 10, 4), None);
        assert_eq!(next_batch(&mut current, 20, 0), None);
    }

    #[tokio::test]
    async fn scheduler_visits_each_id_once_with_bounded_concurrency() {
        let (_sender, mut stop) = tokio::sync::oneshot::channel();
        let visited = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        run_batches(&test_config(), &mut stop, |range| {
            let (visited, running, maximum) = (visited.clone(), running.clone(), maximum.clone());
            async move {
                let count = running.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(count, Ordering::SeqCst);
                tokio::task::yield_now().await;
                visited.lock().unwrap().extend(range);
                running.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .await
        .unwrap();
        let mut visited = visited.lock().unwrap().clone();
        visited.sort_unstable();
        assert_eq!(visited, (10..21).collect::<Vec<_>>());
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn scheduler_reports_errors_after_processing_other_batches() {
        let (_sender, mut stop) = tokio::sync::oneshot::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let result = run_batches(&test_config(), &mut stop, |range| {
            let count = count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                ensure!(range.start != 10, "模拟下载失败");
                Ok(())
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn scheduler_reports_worker_panics() {
        let (_sender, mut stop) = tokio::sync::oneshot::channel();
        let result = run_batches(&test_config(), &mut stop, |_| async {
            panic!("模拟工作任务 panic");
            #[allow(unreachable_code)]
            Ok(())
        })
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn stop_before_start_dispatches_nothing() {
        let (sender, mut stop) = tokio::sync::oneshot::channel();
        sender.send(()).unwrap();
        run_batches(&test_config(), &mut stop, |_| async {
            panic!("已停止时不应派发任务");
            #[allow(unreachable_code)]
            Ok(())
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn stop_cancels_and_joins_running_workers() {
        struct Guard(Arc<AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let (sender, mut stop) = tokio::sync::oneshot::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let running = Arc::new(AtomicUsize::new(0));
        let sender_task = tokio::spawn(async move {
            ready.await.unwrap();
            sender.send(()).unwrap();
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            run_batches(&test_config(), &mut stop, |_| {
                let running = running.clone();
                let started = started.take();
                async move {
                    running.fetch_add(1, Ordering::SeqCst);
                    let _guard = Guard(running);
                    if let Some(started) = started {
                        let _ = started.send(());
                    }
                    std::future::pending::<anyhow::Result<()>>().await
                }
            }),
        )
        .await
        .unwrap()
        .unwrap();
        sender_task.await.unwrap();
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }
}
