use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use crate::config::ConfigFile;

pub mod full_data_view;
pub mod long_data_table;
pub mod main_data_table;
pub mod ships_table;

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

/// 当前数据库版本 (用于检查是否需要更新)
///
/// ## 版本历史
/// 1. 原始版本, 基于 sea_orm 的版本
/// 2. 初始版本, 开始自己写定义了
///    各个表的信息可以在对应的文件中查看
///    - `main_data` 表
///    - `long_data` 表
///    - `full_data` 视图
///    - `ships` 表
pub const CURRENT_DB_VERSION: i32 = 2;

pub const TEXT_DATA_MAX_LEN: usize = 1024;
pub type SaveId = u32;

pub async fn check_table_exists(db: &DatabaseConnection, table_name: &str, schema: &str) -> bool {
    let sql = format!(
        "SELECT EXISTS (SELECT FROM pg_tables WHERE tablename = '{}' AND schemaname = '{}');",
        table_name, schema
    );
    if let Ok(Some(exists)) = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
    {
        if let Ok(exists) = exists.try_get("", "exists") {
            return exists;
        }
    }
    false
}

pub async fn check_main_data_exists(db: &DatabaseConnection, conf: &ConfigFile) -> bool {
    check_table_exists(db, db_names::MAIN_DATA_TABLE, &conf.db.schema).await
}

pub async fn check_long_data_exists(db: &DatabaseConnection, conf: &ConfigFile) -> bool {
    check_table_exists(db, db_names::LONG_DATA_TABLE, &conf.db.schema).await
}

pub async fn check_full_data_exists(db: &DatabaseConnection, conf: &ConfigFile) -> bool {
    check_table_exists(db, db_names::FULL_DATA_TABLE, &conf.db.schema).await
}

pub async fn check_db_version_exists(db: &DatabaseConnection, conf: &ConfigFile) -> bool {
    check_table_exists(db, db_names::DB_VERSION_TABLE, &conf.db.schema).await
}

pub async fn check_sea_orm_exists(db: &DatabaseConnection, conf: &ConfigFile) -> bool {
    check_table_exists(db, db_names::SEA_ORM_TABLE, &conf.db.schema).await
}
