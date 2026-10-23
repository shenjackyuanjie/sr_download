use crate::SaveId;
use anyhow::{Context, ensure};
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
    #[serde(rename = "db", default)]
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
#[serde(rename = "fast-sync", default)]
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
#[serde(rename = "sync", default)]
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
    #[serde(rename = "serve", default)]
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
        let data = std::fs::read_to_string(&file_path)
            .with_context(|| format!("无法读取配置 {}", file_path.display()))?;
        let config: ConfigFile = toml::from_str(&data).context("配置 TOML 格式错误")?;
        config.validate()?;
        Ok(config)
    }

    /// 在创建运行时、连接数据库或开始下载前验证所有配置边界。
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.db.url.trim().is_empty(), "db.url 不能为空");
        ensure!(!self.db.schema.trim().is_empty(), "db.schema 不能为空");
        ensure!(self.db.max_connections > 0, "db.max_connections 必须大于 0");
        ensure!(
            self.db.acquire_timeout_secs > 0,
            "db.acquire_timeout_secs 必须大于 0"
        );
        ensure!(
            self.db.idle_timeout_secs > 0,
            "db.idle_timeout_secs 必须大于 0"
        );
        for (name, seconds) in [
            ("db.acquire_timeout_secs", self.db.acquire_timeout_secs),
            ("db.idle_timeout_secs", self.db.idle_timeout_secs),
        ] {
            ensure!(
                std::time::Instant::now()
                    .checked_add(std::time::Duration::from_secs(seconds))
                    .is_some(),
                "{name} 超出平台计时范围"
            );
        }
        ensure!(
            self.db.statement_timeout_secs <= i32::MAX as u64 / 1_000,
            "db.statement_timeout_secs 超出 PostgreSQL 毫秒整数范围"
        );
        for (name, value) in [
            ("sync.max_timeout", self.sync.max_timeout),
            ("sync.serve_wait_time", self.sync.serve_wait_time),
        ] {
            let duration = std::time::Duration::try_from_secs_f32(value)
                .with_context(|| format!("{name} 必须是有限的正数秒数"))?;
            ensure!(!duration.is_zero(), "{name} 必须大于 0");
            ensure!(
                std::time::Instant::now().checked_add(duration).is_some(),
                "{name} 超出平台计时范围"
            );
        }
        ensure!(
            self.sync.fast.worker_count > 0,
            "sync.fast.worker_count 必须大于 0"
        );
        ensure!(
            self.sync.fast.worker_size > 0,
            "sync.fast.worker_size 必须大于 0"
        );
        ensure!(
            self.sync.fast.start_id <= self.sync.fast.end_id,
            "sync.fast.start_id 不能大于 end_id（结束 ID 不包含在区间内）"
        );
        // PostgreSQL integer 为有符号 32 位；结束边界允许比最后一个 ID 大 1。
        ensure!(
            self.sync.fast.end_id <= i32::MAX as u32 + 1,
            "sync.fast.end_id 超出数据库 ID 范围"
        );
        if self.serve.enable {
            ensure!(
                !self.serve.host_with_port.trim().is_empty(),
                "serve.host_with_port 不能为空"
            );
            ensure!(
                self.serve.db_max_connect > 0,
                "serve.db_max_connect 必须大于 0"
            );
            ensure!(
                self.serve.refresh_interval > 0,
                "serve.refresh_interval 必须大于 0（毫秒）"
            );
            ensure!(
                !self.serve.resync_token.trim().is_empty()
                    && self.serve.resync_token != ServeConfig::default().resync_token,
                "开启 Web 服务前请设置非空且非占位值的 serve.resync_token"
            );
        }
        Ok(())
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
        GLOBAL_CFG
            .set(conf)
            .map_err(|_| anyhow::anyhow!("Global configuration is already initialized"))?;
        crate::web_part::RESYNC_TOKEN.get_or_init(|| resync_token);
        Ok(())
    }

    pub fn get_global() -> &'static Self {
        GLOBAL_CFG.get().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_and_defaults_are_valid() {
        ConfigFile::default().validate().unwrap();
        let template: ConfigFile =
            toml::from_str(include_str!("../../config_template.toml")).unwrap();
        template.validate().unwrap();
        assert_eq!(template.serve.refresh_interval, 10_000);
    }

    #[test]
    fn partial_sections_keep_defaults() {
        let config: ConfigFile =
            toml::from_str("[sync.fast]\nworker_count = 3\n[db]\nschema = 'archive'\n").unwrap();
        config.validate().unwrap();
        assert_eq!(config.sync.fast.worker_count, 3);
        assert_eq!(config.sync.fast.worker_size, 10);
        assert_eq!(config.db.schema, "archive");
        assert_eq!(config.db.acquire_timeout_secs, 10);
    }

    #[test]
    fn rejects_invalid_durations_without_panicking() {
        for value in [
            0.0,
            -1.0,
            f32::NAN,
            f32::INFINITY,
            f32::MAX,
            f32::MIN_POSITIVE,
        ] {
            let mut config = ConfigFile::default();
            config.sync.max_timeout = value;
            assert!(config.validate().is_err(), "max_timeout = {value}");
            config = ConfigFile::default();
            config.sync.serve_wait_time = value;
            assert!(config.validate().is_err(), "serve_wait_time = {value}");
        }
    }

    #[test]
    fn rejects_zero_workers_and_invalid_ranges() {
        let mut config = ConfigFile::default();
        config.sync.fast.worker_count = 0;
        assert!(config.validate().is_err());
        config = ConfigFile::default();
        config.sync.fast.worker_size = 0;
        assert!(config.validate().is_err());
        config = ConfigFile::default();
        config.sync.fast.start_id = config.sync.fast.end_id + 1;
        assert!(config.validate().is_err());
        config = ConfigFile::default();
        config.sync.fast.end_id = u32::MAX;
        assert!(config.validate().is_err());
        config.sync.fast.start_id = i32::MAX as u32;
        config.sync.fast.end_id = i32::MAX as u32 + 1;
        config.validate().unwrap();
    }

    #[test]
    fn enabled_server_requires_private_token_and_valid_limits() {
        let mut config = ConfigFile::default();
        config.serve.enable = true;
        assert!(config.validate().is_err());
        config.serve.resync_token = "test-token-not-used-for-a-real-server".into();
        config.validate().unwrap();
        config.serve.refresh_interval = 0;
        assert!(config.validate().is_err());
        config.serve.refresh_interval = 10_000;
        config.serve.db_max_connect = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_extreme_pool_timeouts() {
        let mut config = ConfigFile::default();
        config.db.acquire_timeout_secs = u64::MAX;
        assert!(config.validate().is_err());
        config = ConfigFile::default();
        config.db.idle_timeout_secs = u64::MAX;
        assert!(config.validate().is_err());
        config = ConfigFile::default();
        config.db.statement_timeout_secs = i32::MAX as u64 / 1_000 + 1;
        assert!(config.validate().is_err());
        config.db.statement_timeout_secs = 0;
        config.validate().unwrap();
    }

    #[test]
    fn rejects_zero_pool_limits() {
        let mut config = ConfigFile::default();
        config.db.max_connections = 0;
        assert!(config.validate().is_err());
        config = ConfigFile::default();
        config.db.acquire_timeout_secs = 0;
        assert!(config.validate().is_err());
    }
}
