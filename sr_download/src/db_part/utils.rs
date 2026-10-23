use anyhow::Context;
use colored::Colorize;
use sqlx::{
    PgPool, Row,
    postgres::{PgPoolOptions, PgRow},
};
use std::time::Duration;
use tracing::{Level, event};

use crate::{
    config::ConfigFile,
    db_part::SaveType,
    db_part::{
        CoverStrategy,
        defines::{SaveId, db_names},
        save_data_to_db,
    },
};

/// 创建连接池，并在每条物理连接上设置 schema 与语句超时。
///
/// 连接池中的连接互不共享会话状态，所以 `search_path` 必须置于
/// `after_connect`；所有未限定 schema 的 SQL 才会持续落在配置指定的 schema。
fn pool_options(conf: &ConfigFile, max_connections: u32) -> PgPoolOptions {
    let search_path_sql = format!(
        "SET search_path TO {}",
        crate::db_part::defines::quote_ident(&conf.db.schema)
    );
    let after_connect_sql = search_path_sql.clone();
    let statement_timeout = format!("{}ms", conf.db.statement_timeout_secs.saturating_mul(1_000));

    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(conf.db.acquire_timeout_secs))
        .idle_timeout(Some(Duration::from_secs(conf.db.idle_timeout_secs)))
        .after_connect(move |conn, _meta| {
            let search_path_sql = after_connect_sql.clone();
            let statement_timeout = statement_timeout.clone();
            Box::pin(async move {
                sqlx::raw_sql(sqlx::AssertSqlSafe(search_path_sql))
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SELECT set_config('statement_timeout', $1, false)")
                    .bind(statement_timeout)
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
}

pub async fn connect(conf: &ConfigFile) -> anyhow::Result<PgPool> {
    event!(
        Level::INFO,
        max_connections = conf.db.max_connections,
        "正在连接数据库"
    );
    let db = pool_options(conf, conf.db.max_connections)
        .connect(&conf.db.url)
        .await
        .context("无法连接数据库")?;
    sqlx::query("SELECT 1").execute(&db).await?;
    event!(Level::INFO, "{}", "已经连接数据库".blue());
    Ok(db)
}

pub async fn connect_server(conf: &ConfigFile) -> anyhow::Result<PgPool> {
    event!(
        Level::INFO,
        max_connections = conf.serve.db_max_connect,
        "服务器正在连接数据库"
    );
    let db = pool_options(conf, conf.serve.db_max_connect)
        .connect(&conf.db.url)
        .await
        .context("服务器无法连接数据库")?;
    sqlx::query("SELECT 1")
        .execute(&db)
        .await
        .context("服务器数据库连接验证失败")?;
    event!(Level::INFO, "{}", "服务器已经连接数据库".blue());
    Ok(db)
}

// 维护按页顺序执行，避免放大上游下载请求和同一 save_id 的写锁竞争。
const MAINTENANCE_PAGE_SIZE: i64 = 100;

/// 有界维护任务在当前页完成后报告的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaintenanceBatchResult {
    Completed,
    Cancelled,
}

/// 仅在维护页之间检查停止信号，使当前已开始的页完整结束而不再开始下一页。
pub(crate) fn maintenance_stop_requested(
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> bool {
    stop_receiver.try_recv().is_ok()
}

/// 更新数据库内所有 xml_tested = null 的数据
pub async fn update_xml_tested(db: &PgPool) -> anyhow::Result<()> {
    let (_stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
    let _ = update_xml_tested_until_stopped(db, &mut stop_receiver).await?;
    Ok(())
}

pub(crate) async fn update_xml_tested_until_stopped(
    db: &PgPool,
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<MaintenanceBatchResult> {
    let mut last_save_id: Option<i32> = None;
    let mut updated_total = 0_i64;

    loop {
        if maintenance_stop_requested(stop_receiver) {
            event!(Level::INFO, updated_total, "已取消 xml_tested 维护");
            return Ok(MaintenanceBatchResult::Cancelled);
        }
        // 游标取自本页选中的最大 save_id；即使关联数据仍为空也会前进，
        // 因而不会把全量 ID 留在内存中或在无法更新的行上无限重试。
        let page: PgRow = sqlx::query(
            r#"
WITH page AS (
    SELECT save_id
    FROM main_data
    WHERE xml_tested IS NULL
      AND len != 0
      AND save_type != 'none'
      AND ($1::integer IS NULL OR save_id > $1)
    ORDER BY save_id
    LIMIT $2
),
updated AS (
    UPDATE main_data AS md
    SET xml_tested = xml_is_well_formed_document(fd.data)
    FROM full_data AS fd
    JOIN page AS p ON p.save_id = fd.save_id
    WHERE md.save_id = p.save_id
      AND fd.data IS NOT NULL
    RETURNING md.save_id
)
SELECT
    (SELECT count(*) FROM page)::bigint AS page_count,
    (SELECT max(save_id) FROM page) AS last_save_id,
    (SELECT count(*) FROM updated)::bigint AS updated_count
"#,
        )
        .bind(last_save_id)
        .bind(MAINTENANCE_PAGE_SIZE)
        .fetch_one(db)
        .await
        .context("无法读取 xml_tested 维护分页")?;

        let page_count: i64 = page
            .try_get("page_count")
            .context("无法读取 xml_tested 维护分页大小")?;
        if page_count == 0 {
            break;
        }
        last_save_id = Some(
            page.try_get("last_save_id")
                .context("无法读取 xml_tested 维护分页游标")?,
        );
        let updated_count: i64 = page
            .try_get("updated_count")
            .context("无法读取 xml_tested 更新数量")?;
        updated_total += updated_count;
        event!(
            Level::INFO,
            page_count,
            updated_count,
            last_save_id = ?last_save_id,
            "已处理 xml_tested 维护分页"
        );
    }

    event!(Level::INFO, updated_total, "xml_tested 维护完成");
    Ok(MaintenanceBatchResult::Completed)
}

/// 检查所有 data = null 的数据
/// 然后补全
pub async fn check_null_data(db: &PgPool) -> anyhow::Result<()> {
    let (_stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
    let _ = check_null_data_until_stopped(db, &mut stop_receiver).await?;
    Ok(())
}

pub(crate) async fn check_null_data_until_stopped(
    db: &PgPool,
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<MaintenanceBatchResult> {
    let mut last_save_id = None;
    let mut repaired = 0_u64;
    let mut failures = 0_u64;
    let downloader = crate::Downloader::new(None);

    loop {
        if maintenance_stop_requested(stop_receiver) {
            event!(Level::INFO, repaired, failures, "已取消缺失数据维护");
            return Ok(MaintenanceBatchResult::Cancelled);
        }
        // 使用 save_id 键集分页，内存中最多仅保留一个维护页的 ID。
        let save_ids = sqlx::query_scalar::<_, db_names::DbSaveId>(
            r#"
SELECT save_id
FROM full_data
WHERE data IS NULL
  AND ($1::integer IS NULL OR save_id > $1)
ORDER BY save_id
LIMIT $2
"#,
        )
        .bind(last_save_id)
        .bind(MAINTENANCE_PAGE_SIZE)
        .fetch_all(db)
        .await
        .context("无法读取缺失数据维护分页")?;
        if save_ids.is_empty() {
            break;
        }

        event!(
            Level::WARN,
            page_count = save_ids.len(),
            last_save_id = ?last_save_id,
            "正在补全缺失数据分页"
        );

        for db_save_id in save_ids {
            last_save_id = Some(db_save_id);
            let id = SaveId::try_from(db_save_id)
                .with_context(|| format!("数据库中存在无效的 save_id: {db_save_id}"))?;
            event!(Level::INFO, save_id = id, "正在补全缺失数据");
            // 只有成功下载到的内容才允许 Cover；下载或保存失败时保留原行，
            // 继续处理其余页面，最后返回汇总错误以暴露所有待人工处理的失败。
            match downloader.try_download_as_any(id).await {
                Some(file) => {
                    let save_type: SaveType = (&file).into();
                    event!(
                        Level::INFO,
                        save_id = id,
                        data_len = file.len(),
                        save_type = %save_type,
                        "成功下载缺失数据"
                    );
                    if let Err(error) = save_data_to_db(
                        id,
                        save_type,
                        file.take_data(),
                        Some(CoverStrategy::Cover),
                        db,
                    )
                    .await
                    {
                        failures += 1;
                        event!(
                            Level::ERROR,
                            save_id = id,
                            error = ?error,
                            "保存补全数据失败；保留原有数据"
                        );
                    } else {
                        repaired += 1;
                        event!(Level::INFO, save_id = id, "成功补全缺失数据");
                    }
                }
                None => {
                    failures += 1;
                    event!(Level::WARN, save_id = id, "无法下载补全数据；保留原有数据");
                }
            }
        }
    }

    if failures > 0 {
        anyhow::bail!("缺失数据维护未完成: repaired={repaired}, failures={failures}");
    }

    event!(Level::INFO, repaired, "缺失数据维护完成");
    Ok(MaintenanceBatchResult::Completed)
}

pub trait FromDb {
    fn from_db(db: &PgPool) -> impl std::future::Future<Output = Option<Self>> + Send
    where
        Self: Sized;
}

// 兼容旧路径；XML 领域逻辑由 xml_part 拥有。
pub use crate::xml_part::verify::{ShipVerifyState, verify_ship, verify_xml};

#[cfg(test)]
mod tests {
    use super::maintenance_stop_requested;

    #[test]
    fn detects_maintenance_stop_signal_between_pages() {
        let (stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
        assert!(!maintenance_stop_requested(&mut stop_receiver));
        stop_sender.send(()).unwrap();
        assert!(maintenance_stop_requested(&mut stop_receiver));
    }
}
