use anyhow::Ok;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

pub async fn init_db() -> anyhow::Result<()> {
    let mut opt =
        ConnectOptions::new("postgres://srdown:srdown@localhost:5432/srdown?currentSchema=srdown");
    opt.max_connections(10).sqlx_logging(true);

    let db = Database::connect(opt).await?;

    Ok(())
}
