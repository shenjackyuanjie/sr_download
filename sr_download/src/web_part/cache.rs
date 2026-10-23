//! 兼容保留的缓存入口；目前没有实际缓存内容，不启动无效刷新任务。

use sqlx::PgPool;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct CacheData {}

impl CacheData {
    pub async fn new_from_db(_db: &PgPool) -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub struct WebCache {}

impl WebCache {
    pub async fn new(_db: &PgPool, _refresh_interval: Duration) -> Self {
        Self::default()
    }
}
