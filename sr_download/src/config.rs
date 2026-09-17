use crate::SaveId;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};
use tracing::{Level, event};

pub mod db_config {
    use serde::{Deserialize, Serialize};

    fn default_acquire_timeout_secs() -> u64 {
        10
    }

    fn default_idle_timeout_secs() -> u64 {
        300
    }

    fn default_statement_timeout_secs() -> u64 {
        30
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename = "db")]
    pub struct DbConfig {
        pub url: String,
        pub schema: String,
        pub max_connections: u32,
        pub sqlx_logging: bool,
        /// 从池中取得连接的最长等待时间，单位为秒。
        #[serde(default = "default_acquire_timeout_secs")]
        pub acquire_timeout_secs: u64,
        /// 空闲物理连接在池中保留的最长时间，单位为秒。
        #[serde(default = "default_idle_timeout_secs")]
        pub idle_timeout_secs: u64,
        /// PostgreSQL 对每条连接执行语句的最长时间，单位为秒；0 表示数据库不设限。
        #[serde(default = "default_statement_timeout_secs")]
        pub statement_timeout_secs: u64,
    }

    impl Default for DbConfig {
        fn default() -> Self {
            Self {
                url: "postgres://username:password@localhost:5432/srdown".to_string(),
                schema: "public".to_string(),
                max_connections: 10,
                sqlx_logging: false,
                acquire_timeout_secs: default_acquire_timeout_secs(),
                idle_timeout_secs: default_idle_timeout_secs(),
                statement_timeout_secs: default_statement_timeout_secs(),
            }
        }
    }
}

pub use db_config::DbConfig;

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
            end_id: 1322267,
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

pub mod serve_config {
    use serde::{Deserialize, Serialize};

    fn default_serve() -> String {
        "0.0.0.0:10002".to_string()
    }

    fn default_serve_connect() -> u32 {
        10
    }

    fn just_false() -> bool {
        false
    }

    const LONG_TOKEN: &str = "change-me-before-enabling-server";

    pub fn loong_token() -> String {
        LONG_TOKEN.to_string()
    }

    pub fn 十秒() -> u32 {
        10_000
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename = "serve")]
    pub struct ServeConfig {
        #[serde(default = "default_serve")]
        pub host_with_port: String,
        #[serde(default = "default_serve_connect")]
        pub db_max_connect: u32,
        #[serde(default = "just_false")]
        pub enable: bool,
        #[serde(default = "loong_token")]
        pub resync_token: String,
        #[serde(default = "十秒")]
        pub refresh_interval: u32,
    }

    impl Default for ServeConfig {
        fn default() -> Self {
            Self {
                host_with_port: default_serve(),
                db_max_connect: default_serve_connect(),
                enable: just_false(),
                resync_token: loong_token(),
                refresh_interval: 十秒(),
            }
        }
    }
}

pub use serve_config::ServeConfig;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub db: DbConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub serve: ServeConfig,
}

pub static GLOBAL_CFG: OnceLock<ConfigFile> = OnceLock::new();

impl ConfigFile {
    pub fn read_from_file(file_path: PathBuf) -> anyhow::Result<Self> {
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

    pub fn init_global(path: Option<PathBuf>) -> anyhow::Result<()> {
        let path = path.unwrap_or(PathBuf::from("config.toml"));
        let conf = Self::read_from_file(path.clone()).map_err(|e| {
            event!(Level::ERROR, path = %path.display(), error = ?e, "无法读取配置文件");
            e
        })?;
        let resync_token = conf.serve.resync_token.clone();
        crate::web_part::RESYNC_TOKEN.get_or_init(|| resync_token);
        GLOBAL_CFG
            .set(conf)
            .map_err(|_| anyhow::anyhow!("Global configuration is already initialized"))?;
        Ok(())
    }

    pub fn get_global() -> &'static Self {
        GLOBAL_CFG.get().unwrap()
    }
}
