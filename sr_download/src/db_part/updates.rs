use anyhow::Context;
use sqlx::{Executor, PgPool, postgres::PgConnection};
use tracing::{Level, event};

use crate::config::ConfigFile;
use crate::db_part::defines::{
    self, CREATE_DB_VERSION_SQL, CREATE_FULL_DATA_VIEW_SQL, CREATE_LONG_DATA_SQL,
    CREATE_LONG_SAVE_ID_INDEX_SQL, CREATE_MAIN_DATA_SQL, CREATE_MAIN_HASH_COVERING_INDEX_SQL,
    CREATE_MAIN_SAVE_TYPE_SAVE_ID_INDEX_SQL, CREATE_SAVE_TYPE_SQL, CURRENT_DB_VERSION,
    UPSERT_DB_VERSION_SQL,
};

const SCHEMA_MIGRATION_LOCK_KEY: i64 = 0x5352_444F_574E_0001;

/// 判断是否允许并需要前进 schema 版本。
///
/// 没有版本记录表示首次纳管；发现更高版本则显式失败，避免旧程序反向覆盖数据库对象。
fn requires_version_update(existing_version: Option<i32>) -> anyhow::Result<bool> {
    match existing_version {
        Some(version) if version > CURRENT_DB_VERSION => {
            anyhow::bail!("数据库版本 {version} 高于当前程序支持的版本 {CURRENT_DB_VERSION}")
        }
        Some(version) => Ok(version < CURRENT_DB_VERSION),
        None => Ok(true),
    }
}

fn requires_full_data_view_refresh(
    version_update_required: bool,
    full_data_view_missing: bool,
) -> bool {
    version_update_required || full_data_view_missing
}

async fn ensure_schema(db: &PgPool, conf: &ConfigFile) -> anyhow::Result<()> {
    let mut tx = db.begin().await.context("无法开始数据库结构更新事务")?;
    // 此全局事务锁须先于任何 schema 检查取得，并持续到版本 marker 写入后提交；
    // 它同时串行化不同版本进程的“读版本→DDL→写版本”临界区。
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(SCHEMA_MIGRATION_LOCK_KEY)
        .execute(&mut *tx)
        .await
        .context("无法取得数据库结构更新锁")?;
    ensure_schema_locked(&mut tx, conf).await?;
    tx.commit().await.context("无法提交数据库结构更新事务")?;
    Ok(())
}

async fn ensure_schema_locked(conn: &mut PgConnection, conf: &ConfigFile) -> anyhow::Result<()> {
    // 存在性检查使用配置 schema；DDL 刻意保持未限定 schema，依赖每条连接已设置的
    // search_path，避免把对象错误创建到固定 schema。
    // 此处的 DDL 不使用 CONCURRENTLY，因为 PostgreSQL 不允许其置于上述事务块中。
    if !defines::check_type_exists(&mut *conn, "save_type", &conf.db.schema)
        .await
        .context("无法检查 save_type 是否存在")?
    {
        conn.execute(CREATE_SAVE_TYPE_SQL)
            .await
            .context("无法创建 save_type")?;
    }
    if !defines::check_table_exists(
        &mut *conn,
        defines::db_names::MAIN_DATA_TABLE,
        &conf.db.schema,
    )
    .await
    .context("无法检查 main_data 是否存在")?
    {
        conn.execute(CREATE_MAIN_DATA_SQL)
            .await
            .context("无法创建 main_data")?;
    }
    if !defines::check_table_exists(
        &mut *conn,
        defines::db_names::LONG_DATA_TABLE,
        &conf.db.schema,
    )
    .await
    .context("无法检查 long_data 是否存在")?
    {
        conn.execute(CREATE_LONG_DATA_SQL)
            .await
            .context("无法创建 long_data")?;
    }
    let version_table_missing = !defines::check_table_exists(
        &mut *conn,
        defines::db_names::DB_VERSION_TABLE,
        &conf.db.schema,
    )
    .await
    .context("无法检查 db_version 是否存在")?;
    if version_table_missing {
        conn.execute(CREATE_DB_VERSION_SQL)
            .await
            .context("无法创建 db_version")?;
    }

    let existing_version = defines::fetch_db_version(&mut *conn)
        .await
        .context("无法读取数据库版本")?;
    let version_update_required = requires_version_update(existing_version)?;
    let full_data_view_missing = !defines::check_view_exists(
        &mut *conn,
        defines::db_names::FULL_DATA_TABLE,
        &conf.db.schema,
    )
    .await
    .context("无法检查 full_data 是否存在")?;
    // CREATE OR REPLACE 只在版本迁移或对象缺失时执行，正常启动不再重建视图/函数。
    if requires_full_data_view_refresh(version_update_required, full_data_view_missing) {
        conn.execute(CREATE_FULL_DATA_VIEW_SQL)
            .await
            .context("无法创建或更新 full_data 视图")?;
    }

    if !defines::check_index_exists(&mut *conn, "maindata_savetype_saveid_idx", &conf.db.schema)
        .await
        .context("无法检查 maindata_savetype_saveid_idx 是否存在")?
    {
        conn.execute(CREATE_MAIN_SAVE_TYPE_SAVE_ID_INDEX_SQL)
            .await
            .context("无法创建 maindata_savetype_saveid_idx")?;
    }
    if !defines::check_index_exists(&mut *conn, "longdata_saveid_idx", &conf.db.schema)
        .await
        .context("无法检查 longdata_saveid_idx 是否存在")?
    {
        conn.execute(CREATE_LONG_SAVE_ID_INDEX_SQL)
            .await
            .context("无法创建 longdata_saveid_idx")?;
    }
    if !defines::check_index_exists(&mut *conn, "idx_main_data_hash_covering", &conf.db.schema)
        .await
        .context("无法检查 idx_main_data_hash_covering 是否存在")?
    {
        conn.execute(CREATE_MAIN_HASH_COVERING_INDEX_SQL)
            .await
            .context("无法创建 idx_main_data_hash_covering")?;
    }

    if version_update_required {
        sqlx::query(UPSERT_DB_VERSION_SQL)
            .bind(CURRENT_DB_VERSION)
            .execute(&mut *conn)
            .await
            .context("无法更新数据库版本")?;
    }
    Ok(())
}

pub async fn update_db(db: &PgPool, conf: &ConfigFile) -> anyhow::Result<()> {
    event!(Level::INFO, schema = %conf.db.schema, "开始检查数据库结构");
    ensure_schema(db, conf).await?;
    event!(Level::INFO, "更新完成");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CURRENT_DB_VERSION, requires_full_data_view_refresh, requires_version_update};
    use crate::db_part::defines::FETCH_DB_VERSION_SQL;

    #[test]
    fn requires_update_for_missing_and_older_versions() {
        assert!(requires_version_update(None).unwrap());
        assert!(requires_version_update(Some(CURRENT_DB_VERSION - 1)).unwrap());
    }

    #[test]
    fn skips_update_for_current_version() {
        assert!(!requires_version_update(Some(CURRENT_DB_VERSION)).unwrap());
    }

    #[test]
    fn rejects_newer_database_versions() {
        assert!(requires_version_update(Some(CURRENT_DB_VERSION + 1)).is_err());
    }

    #[test]
    fn selects_and_rejects_the_highest_recorded_version() {
        let highest_recorded_version = [
            CURRENT_DB_VERSION - 1,
            CURRENT_DB_VERSION + 1,
            CURRENT_DB_VERSION,
        ]
        .into_iter()
        .max();
        assert_eq!(highest_recorded_version, Some(CURRENT_DB_VERSION + 1));
        assert!(requires_version_update(highest_recorded_version).is_err());
        assert!(FETCH_DB_VERSION_SQL.contains("MAX(version)"));
        assert!(!FETCH_DB_VERSION_SQL.contains("updated_at"));
    }

    #[test]
    fn refreshes_view_only_for_a_schema_change_or_missing_object() {
        assert!(requires_full_data_view_refresh(true, false));
        assert!(requires_full_data_view_refresh(false, true));
        assert!(!requires_full_data_view_refresh(false, false));
    }
}
