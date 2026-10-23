use sqlx::{Executor, Postgres};

pub mod db_names {
    /// 主数据表
    pub const MAIN_DATA_TABLE: &str = "main_data";
    /// 过长数据的表
    pub const LONG_DATA_TABLE: &str = "long_data";
    /// 拼接后的完整数据表
    pub const FULL_DATA_TABLE: &str = "full_data";
    /// 用于存储 db 版本号的表
    pub const DB_VERSION_TABLE: &str = "db_version";

    /// 更新 xml_tested 的函数
    pub const UPDATE_XML_TESTED: &str = "update_xml_tested";

    pub type DbSaveId = i32;
}

/// 当前数据库版本 (用于检查是否需要更新)
///
/// ## 版本历史
/// 1. 原始版本, 基于 SeaORM 的版本
/// 2. 初始版本, 开始自己写定义了
///    各个表的信息可以在对应的文件中查看
///    - `main_data` 表
///    - `long_data` 表
///    - `full_data` 视图
///    - `ships` 表
/// 3. 清理 SeaORM 遗留表并重建干净数据表
/// 4. 将运行时 XML 维护改为分页执行，并按版本刷新 full_data 视图
pub const CURRENT_DB_VERSION: i32 = 4;

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
pub const FETCH_DB_VERSION_SQL: &str = r#"
SELECT MAX(version)
FROM db_version
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

pub async fn check_table_exists<'e, E>(
    executor: E,
    table_name: &str,
    schema: &str,
) -> anyhow::Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_tables
            WHERE tablename = $1 AND schemaname = $2
        )",
    )
    .bind(table_name)
    .bind(schema)
    .fetch_one(executor)
    .await
    .map_err(Into::into)
}

pub async fn check_type_exists<'e, E>(
    executor: E,
    type_name: &str,
    schema: &str,
) -> anyhow::Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
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
    .fetch_one(executor)
    .await
    .map_err(Into::into)
}

pub async fn check_index_exists<'e, E>(
    executor: E,
    index_name: &str,
    schema: &str,
) -> anyhow::Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_indexes
            WHERE indexname = $1 AND schemaname = $2
        )",
    )
    .bind(index_name)
    .bind(schema)
    .fetch_one(executor)
    .await
    .map_err(Into::into)
}

pub async fn check_view_exists<'e, E>(
    executor: E,
    view_name: &str,
    schema: &str,
) -> anyhow::Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM pg_views
            WHERE viewname = $1 AND schemaname = $2
        )",
    )
    .bind(view_name)
    .bind(schema)
    .fetch_one(executor)
    .await
    .map_err(Into::into)
}

pub async fn fetch_db_version<'e, E>(executor: E) -> anyhow::Result<Option<i32>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar::<_, Option<i32>>(FETCH_DB_VERSION_SQL)
        .fetch_one(executor)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::quote_ident;

    #[test]
    fn quotes_schema_as_one_identifier() {
        assert_eq!(quote_ident("public"), r#""public""#);
        assert_eq!(quote_ident("schema.with.dots"), r#""schema.with.dots""#);
        assert_eq!(quote_ident("中文 schema"), r#""中文 schema""#);
    }

    #[test]
    fn escapes_quotes_inside_identifiers() {
        assert_eq!(
            quote_ident(r#"a"; DROP SCHEMA public; --"#),
            r#""a""; DROP SCHEMA public; --""#
        );
    }
}
