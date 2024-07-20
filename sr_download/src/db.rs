use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::config::ConfigFile;
use migration::{Migrator, MigratorTrait};

pub async fn connect(conf: &ConfigFile) -> anyhow::Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(conf.db_url.clone());
    opt.max_connections(conf.max_connections)
        .set_schema_search_path(conf.db_schema.clone())
        .sqlx_logging(conf.sqlx_logging);

    let db: DatabaseConnection = Database::connect(opt).await?;
    db.ping().await?;

    Migrator::up(&db, None).await?;

    Ok(db)
}
