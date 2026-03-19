use sqlx::{PgPool, Row};

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

    /// 更新 xml_tested 的函数
    pub const UPDATE_XML_TESTED: &str = "update_xml_tested";

    pub type DbSaveId = i32;
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

pub const CREATE_SAVE_TYPE_SQL: &str =
    "CREATE TYPE save_type AS ENUM ('ship', 'save', 'unknown', 'none')";
pub const CREATE_MAIN_DATA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS main_data (
    save_id integer PRIMARY KEY,
    save_type save_type NOT NULL,
    blake_hash character(64) NOT NULL,
    len bigint NOT NULL,
    short_data character varying(1024),
    xml_tested boolean,
    time timestamp with time zone NOT NULL
)
"#;
pub const CREATE_LONG_DATA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS long_data (
    save_id integer PRIMARY KEY,
    len bigint NOT NULL,
    text character varying NOT NULL,
    CONSTRAINT save_id FOREIGN KEY (save_id)
        REFERENCES main_data(save_id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
)
"#;
pub const CREATE_FULL_DATA_VIEW_SQL: &str = r#"
CREATE OR REPLACE VIEW full_data AS
SELECT
    md.save_id,
    md.save_type,
    md.blake_hash,
    md.xml_tested,
    md.len,
    CASE
        WHEN md.len > 1024 THEN ld.text
        ELSE md.short_data
    END AS data
FROM main_data md
LEFT JOIN long_data ld ON md.save_id = ld.save_id
"#;
pub const CREATE_UPDATE_XML_TESTED_SQL: &str = r#"
CREATE OR REPLACE FUNCTION update_xml_tested()
RETURNS VOID AS $$
BEGIN
    UPDATE main_data
    SET xml_tested = xml_is_well_formed_document(fd.data)
    FROM full_data fd
    WHERE main_data.save_id = fd.save_id
      AND main_data.xml_tested IS NULL
      AND fd.data IS NOT NULL;
END;
$$ LANGUAGE plpgsql
"#;
pub const CREATE_DB_VERSION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS db_version (
    version integer PRIMARY KEY,
    updated_at timestamp with time zone NOT NULL DEFAULT now()
)
"#;
pub const UPSERT_DB_VERSION_SQL: &str = r#"
INSERT INTO db_version (version, updated_at)
VALUES ($1, now())
ON CONFLICT (version) DO UPDATE SET updated_at = EXCLUDED.updated_at
"#;
pub const CREATE_MAIN_SAVE_TYPE_SAVE_ID_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS maindata_savetype_saveid_idx
ON main_data (save_type, save_id, len, xml_tested)
"#;
pub const CREATE_LONG_SAVE_ID_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS longdata_saveid_idx
ON long_data (save_id)
"#;
pub const CREATE_MAIN_HASH_COVERING_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_main_data_hash_covering
ON main_data (blake_hash) INCLUDE (save_id, save_type, len)
"#;

pub fn quote_ident(input: &str) -> String {
    format!("\"{}\"", input.replace('"', "\"\""))
}

pub async fn check_table_exists(db: &PgPool, table_name: &str, schema: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_tables
            WHERE tablename = $1 AND schemaname = $2
        )",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

pub async fn check_type_exists(db: &PgPool, type_name: &str, schema: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_type t
            JOIN pg_namespace n ON n.oid = t.typnamespace
            WHERE t.typname = $1 AND n.nspname = $2
        )",
    )
    .bind(type_name)
    .bind(schema)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

pub async fn check_index_exists(db: &PgPool, index_name: &str, schema: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_indexes
            WHERE indexname = $1 AND schemaname = $2
        )",
    )
    .bind(index_name)
    .bind(schema)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

pub async fn fetch_db_version(db: &PgPool) -> Option<i32> {
    let row = sqlx::query(
        "SELECT version
         FROM db_version
         ORDER BY updated_at DESC, version DESC
         LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .ok()??;

    row.try_get("version").ok()
}
