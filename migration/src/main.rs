use sea_orm_migration::prelude::*;

mod m20240719_000001_create_main_data_table;

#[tokio::main]
async fn main() {
    cli::run_cli(migration::Migrator).await;
}
