use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::SaveId;

#[derive(Serialize, Deserialize)]
pub struct ConfigFile {
    pub db_url: String,
    pub db_schema: String,
    pub max_connections: u32,
    pub sqlx_logging: bool,
    pub worker_count: u32,
    pub worker_size: u32,
    pub start_id: SaveId,
    pub max_timeout: f32,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            db_url: "postgres://srdown:srdown@192.168.3.22:10001/srdown".to_string(),
            db_schema: "public".to_string(),
            max_connections: 10,
            sqlx_logging: false,
            worker_count: 10,
            worker_size: 10,
            start_id: 173860,
            max_timeout: 1.0,
        }
    }
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
        std::time::Duration::from_secs_f32(self.max_timeout)
    }
}
