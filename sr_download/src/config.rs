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
            end_id: 1321698,
            worker_count: 10,
            worker_size: 10,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "sync")]
pub struct SyncConfig {
    pub max_timeout: f32,
    pub serve_wait_time: f32,
    pub fast: FastSyncConfig,
}
impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_timeout: 1.0,
            serve_wait_time: 10.0,
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

    pub fn serve_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f32(self.sync.serve_wait_time)
    }

    pub fn net_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f32(self.sync.max_timeout)
    }

    /// 自动帮你骂用户了
    /// 你直接 ? 就行
    pub fn try_read() -> anyhow::Result<Self> {
        match Self::read_from_file(Path::new("config.toml")) {
            Ok(conf) => Ok(conf),
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
                }
                Err(e)
            }
        }
    }

    /// 同理, 也帮你骂好了
    /// 甚至不需要你 ?
    #[allow(unused)]
    pub fn read_or_panic() -> Self {
        Self::try_read().expect("Please Fix the config.toml file")
    }
}
