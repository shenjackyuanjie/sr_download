use sea_orm::sea_query::{ColumnDef, ForeignKey, ForeignKeyAction, Table};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use crate::config::ConfigFile;

pub mod db_names {
    /// 主数据表
    pub const MAIN_DATA_TABLE: &str = "main_data";
    /// 过长数据的表
    pub const LONG_DATA_TABLE: &str = "long_data";
    /// 拼接后的完整数据表
    pub const FULL_DATA_TABLE: &str = "full_data";
    /// 用于存储 db 版本号的表
    pub const DB_VERSION_TABLE: &str = "db_version";
    /// 老的 sea_orm 的标记表
    pub const SEA_ORM_TABLE: &str = "seaql_migrations";
}

pub async fn check_table_exists(
    db: &DatabaseConnection,
    table_name: &str,
    schema: &str,
) -> Option<bool> {
    let sql = format!(
        "SELECT EXISTS (SELECT FROM pg_tables WHERE tablename = '{}' AND schemaname = '{}');",
        table_name, schema
    );
    let exists = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await.ok()?;
    exists?.try_get("", "exists").ok()
}

pub async fn check_main_data_exists(
    db: &DatabaseConnection,
    conf: &ConfigFile,
) -> Option<bool> {
    check_table_exists(db, db_names::MAIN_DATA_TABLE, &conf.db.schema).await
}

pub async fn check_long_data_exists(
    db: &DatabaseConnection,
    conf: &ConfigFile,
) -> Option<bool> {
    check_table_exists(db, db_names::LONG_DATA_TABLE, &conf.db.schema).await
}

pub async fn check_full_data_exists(
    db: &DatabaseConnection,
    conf: &ConfigFile,
) -> Option<bool> {
    check_table_exists(db, db_names::FULL_DATA_TABLE, &conf.db.schema).await
}

pub async fn check_db_version_exists(
    db: &DatabaseConnection,
    conf: &ConfigFile,
) -> Option<bool> {
    check_table_exists(db, db_names::DB_VERSION_TABLE, &conf.db.schema).await
}

pub async fn check_sea_orm_exists(
    db: &DatabaseConnection,
    conf: &ConfigFile,
) -> Option<bool> {
    check_table_exists(db, db_names::SEA_ORM_TABLE, &conf.db.schema).await
}
