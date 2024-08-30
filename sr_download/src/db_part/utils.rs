use colored::Colorize;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tracing::{event, Level};

use crate::config::ConfigFile;
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
