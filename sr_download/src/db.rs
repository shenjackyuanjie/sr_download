use anyhow::Ok;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use migration::{Migrator, MigratorTrait};

pub struct DbConfig {
    pub db_url: String,
    pub db_schema: String,
    pub max_connections: u32,
    pub sqlx_logging: bool,
}

pub async fn init_db<C>(conf: C) -> anyhow::Result<DatabaseConnection>
where
    C: Into<DbConfig>,
{
    let conf: DbConfig = conf.into();
    let mut opt = ConnectOptions::new(conf.db_url.clone());
    opt.max_connections(conf.max_connections)
        .sqlx_logging(conf.sqlx_logging)
        .set_schema_search_path(conf.db_schema.clone());

    let db = Database::connect(opt).await?;
    Migrator::up(&db, None).await?;
    Ok(db)
}
