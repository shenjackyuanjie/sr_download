use colored::Colorize;
use quick_xml::{events::Event, Reader};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use tracing::{event, Level};

use crate::config::ConfigFile;
use migration::{m20240721_00003_create_indexs::UPDATE_XML_TESTED, Migrator, MigratorTrait};

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
pub async fn update_xml_tested(db: &DatabaseConnection) -> Option<()>{
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
        .await.ok()??;
    let count: i64 = data.try_get("", "count").ok()?;
    if count == 0 {
        event!(Level::INFO, "无须数据检查");
        return Some(());
    }
    event!(Level::INFO, "正在检查 {} 条数据", count);
    let sql = format!("SELECT {}()", UPDATE_XML_TESTED);
    let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
    event!(Level::INFO, "正在更新数据库内所有 xml_tested = null 的数据");
    let _ = db.execute(stmt).await;
    event!(Level::INFO, "已经更新数据库内所有 xml_tested = null 的数据");
    Some(())
}

pub trait FromDb {
    async fn from_db(db: &DatabaseConnection) -> Option<Self>
    where
        Self: Sized;
}

/// 校验一下是不是合法 xml
pub fn verify_xml(data: &str) -> bool {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => (),
            Err(_) => return false,
        }
    }
    true
}
