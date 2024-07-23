use migration::SaveId;
use std::path::Path;

use colored::Colorize;
use serde::{Deserialize, Serialize};
use tracing::{event, Level};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "db")]
pub struct DbConfig {
    pub url: String,
    pub schema: String,
    pub max_connections: u32,
    pub sqlx_logging: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: "postgres://srdown:srdown@localhost:5432/srdown".to_string(),
            schema: "public".to_string(),
            max_connections: 10,
            sqlx_logging: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "fast-sync")]
pub struct FastSyncConfig {
    pub start_id: SaveId,
    pub end_id: SaveId,
    pub worker_count: u32,
    pub worker_size: u32,
}

impl Default for FastSyncConfig {
    fn default() -> Self {
        Self {
            start_id: 76859,
            end_id: 1321469,
            worker_count: 10,
            worker_size: 10,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "sync")]
pub struct SyncConfig {
    pub max_timeout: f32,
    pub fast: FastSyncConfig,
}
impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_timeout: 1.0,
            fast: FastSyncConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ConfigFile {
    pub db: DbConfig,
    pub sync: SyncConfig,
}

impl ConfigFile {
    pub fn read_from_file(file_path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(file_path)?;
        let config: ConfigFile = toml::from_str(&data)?;
        Ok(config)
    }

    pub fn write_default_to_file(file_path: &Path) -> anyhow::Result<()> {
        let config = ConfigFile::default();
        let toml = toml::to_string(&config)?;
        std::fs::write(file_path, toml)?;
        Ok(())
    }

    pub fn timeout_as_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f32(self.sync.max_timeout)
    }

    pub fn read_or_panic() -> Self {
        match Self::read_from_file(Path::new("config.toml")) {
            Ok(conf) => conf,
            Err(e) => {
                let _ = tracing_subscriber::fmt::try_init();
                event!(Level::ERROR, "{}", "Please Fix the config.toml file".red());
                event!(Level::ERROR, "Error: {:?}", e);
                if let Err(e) = Self::write_default_to_file(Path::new("config_template.toml")) {
                    event!(Level::ERROR, "Error while writing file: {:?}", e);
                    event!(
                        Level::ERROR,
                        "template file like this: {}",
                        toml::to_string(&Self::default()).unwrap()
                    );
                };
                panic!("Please Fix the config.toml file");
            }
        }
    }
}
