use std::path::Path;
use tracing::{event, Level};

mod config;
mod db;
mod model;
mod net;

pub type SaveId = u32;
pub const TEXT_DATA_MAX_LEN: usize = 1024;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    event!(Level::INFO, "Starting srdownload");

    let conf = match config::ConfigFile::read_from_file(Path::new("config.toml")) {
        Ok(conf) => conf,
        Err(_) => {
            config::ConfigFile::write_default_to_file(Path::new("config.toml"))?;
            config::ConfigFile::default()
        }
    };

    let db_connect = db::connect(&conf).await?;
    let start_id = db::find_max_id(&db_connect).await;

    event!(Level::INFO, "Starting download from save_id: {}", start_id);

    Ok(())
}
