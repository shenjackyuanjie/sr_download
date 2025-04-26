use colored::Colorize;
use quick_xml::{Reader, events::Event};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use tracing::{Level, event};

use crate::{
    config::ConfigFile,
    db_part::{
        CoverStrategy,
        defines::{SaveId, db_names},
        save_data_to_db,
    },
    model::sea_orm_active_enums::SaveType,
};
use migration::{Migrator, MigratorTrait};

pub async fn connect(conf: &ConfigFile) -> anyhow::Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(conf.db.url.clone());
    opt.max_connections(conf.db.max_connections)
        .set_schema_search_path(conf.db.schema.clone())
        .sqlx_logging(conf.db.sqlx_logging);
    event!(Level::INFO, "正在连接数据库");
    let db: DatabaseConnection = Database::connect(opt).await?;
    db.ping().await?;
    event!(Level::INFO, "{}", "已经连接数据库".blue());
    Ok(db)
}

pub async fn connect_server(conf: &ConfigFile) -> anyhow::Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(conf.db.url.clone());
    opt.max_connections(conf.serve.db_max_connect)
        .set_schema_search_path(conf.db.schema.clone())
        .sqlx_logging(conf.db.sqlx_logging);
    event!(Level::INFO, "服务器正在连接数据库");
    let db: DatabaseConnection = Database::connect(opt).await?;
    db.ping().await?;
    event!(Level::INFO, "{}", "服务器已经连接数据库".blue());
    Ok(db)
}

pub async fn migrate(db: &DatabaseConnection) -> anyhow::Result<()> {
    event!(Level::INFO, "Starting migration");
    Migrator::up(db, None).await?;
    event!(Level::INFO, "Migration finished");
    Ok(())
}

/// 更新数据库内所有 xml_tested = null 的数据
pub async fn update_xml_tested(db: &DatabaseConnection) -> Option<()> {
    let sql = r#"SELECT count(1)
	from full_data
	where xml_tested is NULL
	and len != 0
	and "save_type" != 'none'"#;
    let data = db
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        ))
        .await
        .ok()??;
    let count: i64 = data.try_get("", "count").ok()?;
    if count == 0 {
        event!(Level::INFO, "所有的 xml_tested 都已经更新过了");
        return Some(());
    }
    event!(Level::INFO, "正在检查 {} 条数据的xml状态", count);
    let sql = format!("SELECT {}()", db_names::UPDATE_XML_TESTED);
    let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
    event!(Level::INFO, "正在更新数据库内所有 xml_tested = null 的数据");
    let _ = db.execute(stmt).await;
    event!(Level::INFO, "已经更新数据库内所有 xml_tested = null 的数据");
    Some(())
}

/// 检查所有 data = null 的数据
/// 然后补全
pub async fn check_null_data(db: &DatabaseConnection) -> Option<()> {
    let sql = format!(
        "SELECT count(1) from {} where data is NULL",
        db_names::FULL_DATA_TABLE
    );
    let data = db
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        ))
        .await
        .ok()??;
    let count: i64 = data.try_get("", "count").ok()?;
    if count == 0 {
        event!(Level::INFO, "数据库内数据都是完整的, 放心");
        return Some(());
    }
    event!(
        Level::WARN,
        "数据库内有 {} 条数据的 data 是空的, 正在更新",
        count
    );
    let sql = format!(
        "SELECT save_id from {} where data is NULL",
        db_names::FULL_DATA_TABLE
    );
    let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
    let quert_results = db.query_all(stmt).await.ok()?;
    let downloader = crate::Downloader::new(None);
    for result in quert_results {
        let id: db_names::DbSaveId = result.try_get("", "save_id").ok()?;
        let id = id as SaveId;
        event!(Level::INFO, "正在补全id: {} 的数据", id);
        match downloader.try_download_as_any(id).await {
            Some(file) => {
                let save_type: SaveType = (&file).into();
                event!(Level::INFO, "成功下载id: {} 的数据 {}", id, file.info());
                match save_data_to_db(
                    id,
                    save_type,
                    file.take_data(),
                    Some(CoverStrategy::Cover),
                    db,
                )
                .await
                {
                    Ok(_) => {
                        event!(Level::INFO, "成功补全id: {} 的数据", id);
                    }
                    Err(e) => {
                        event!(
                            Level::ERROR,
                            "补全id: {} 的时候出现错误: {}, 将使用 Unknown 覆盖",
                            id,
                            e
                        );
                        let _ = save_data_to_db(
                            id,
                            SaveType::Unknown,
                            "",
                            Some(CoverStrategy::Cover),
                            db,
                        )
                        .await;
                    }
                }
            }
            None => {
                event!(Level::WARN, "尝试补全id: {} 的时候没下载到东西", id);
            }
        }
    }
    Some(())
}

pub trait FromDb {
    async fn from_db(db: &DatabaseConnection) -> Option<Self>
    where
        Self: Sized;
}

/// 校验一下是不是合法 xml
pub fn verify_xml(data: &str) -> quick_xml::Result<()> {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
