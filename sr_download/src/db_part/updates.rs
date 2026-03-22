use sqlx::{Executor, PgPool};
use tracing::{Level, event};

use crate::config::ConfigFile;
use crate::db_part::defines::{
    self, CREATE_DB_VERSION_SQL, CREATE_FULL_DATA_VIEW_SQL, CREATE_LONG_DATA_SQL,
    CREATE_LONG_SAVE_ID_INDEX_SQL, CREATE_MAIN_DATA_SQL, CREATE_MAIN_HASH_COVERING_INDEX_SQL,
    CREATE_MAIN_SAVE_TYPE_SAVE_ID_INDEX_SQL, CREATE_SAVE_TYPE_SQL, CREATE_UPDATE_XML_TESTED_SQL,
    CURRENT_DB_VERSION, UPSERT_DB_VERSION_SQL,
};

pub mod pre_local {
    use super::*;
    use crate::db_part::defines::check_table_exists;

    pub async fn try_merge(db: &PgPool, conf: &ConfigFile) {
        event!(Level::INFO, "尝试从 sea_orm 表迁移数据");
        if !check_table_exists(db, defines::db_names::SEA_ORM_TABLE, &conf.db.schema).await {
            event!(Level::DEBUG, "sea_orm 表不存在, 不需要迁移");
            return;
        }
        event!(
            Level::DEBUG,
            "当前仅保留 seaql_migrations 兼容检测, 不再执行 SeaORM 迁移"
        );
    }
}

async fn ensure_schema(db: &PgPool, conf: &ConfigFile) -> anyhow::Result<()> {
    if !defines::check_type_exists(db, "save_type", &conf.db.schema).await {
        db.execute(CREATE_SAVE_TYPE_SQL).await?;
    }
    if !defines::check_table_exists(db, defines::db_names::MAIN_DATA_TABLE, &conf.db.schema).await {
        db.execute(CREATE_MAIN_DATA_SQL).await?;
    }
    if !defines::check_table_exists(db, defines::db_names::LONG_DATA_TABLE, &conf.db.schema).await {
        db.execute(CREATE_LONG_DATA_SQL).await?;
    }

    db.execute(CREATE_FULL_DATA_VIEW_SQL).await?;
    db.execute(CREATE_UPDATE_XML_TESTED_SQL).await?;
    db.execute(CREATE_DB_VERSION_SQL).await?;

    if !defines::check_index_exists(db, "maindata_savetype_saveid_idx", &conf.db.schema).await {
        db.execute(CREATE_MAIN_SAVE_TYPE_SAVE_ID_INDEX_SQL).await?;
    }
    if !defines::check_index_exists(db, "longdata_saveid_idx", &conf.db.schema).await {
        db.execute(CREATE_LONG_SAVE_ID_INDEX_SQL).await?;
    }
    if !defines::check_index_exists(db, "idx_main_data_hash_covering", &conf.db.schema).await {
        db.execute(CREATE_MAIN_HASH_COVERING_INDEX_SQL).await?;
    }

    sqlx::query(UPSERT_DB_VERSION_SQL)
        .bind(CURRENT_DB_VERSION)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn update_db(db: &PgPool, conf: &ConfigFile) {
    event!(Level::INFO, "开始更新数据库");
    pre_local::try_merge(db, conf).await;
    if let Err(e) = ensure_schema(db, conf).await {
        event!(Level::ERROR, "无法更新数据库结构: {:?}", e);
        return;
    }
    event!(Level::INFO, "更新完成");
}
