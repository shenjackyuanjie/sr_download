use std::{sync::atomic::AtomicBool, time::Duration};

use sea_orm::DatabaseConnection;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CacheData {}

impl CacheData {
    pub async fn new_from_db(db: &DatabaseConnection) -> Self {
        Self {}
    }
}

#[derive(Debug)]
pub struct WebCache {
    updating: AtomicBool,
    refresh_interval: Duration,
    cache: RwLock<CacheData>,
}

impl WebCache {
    pub async fn new(db: &DatabaseConnection, refresh_interval: Duration) -> Self {
        let cache = CacheData::new_from_db(db).await;
        Self {
            updating: AtomicBool::new(false),
            refresh_interval,
            cache: RwLock::new(cache),
        }
    }
}
