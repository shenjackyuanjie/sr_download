use anyhow::Ok;
use sea_orm::{ConnectOptions, Database};

use migration::{Migrator, MigratorTrait};

pub async fn init_db() -> anyhow::Result<()> {
    let mut opt =
        ConnectOptions::new("postgres://srdown:srdown@localhost:5432/srdown?currentSchema=srdown");
    opt.max_connections(10)
        .sqlx_logging(true)
        .set_schema_search_path("srdown");

    let db = Database::connect(opt).await?;

    Migrator::up(&db, None).await?;

    Ok(())
}
