use std::{
    sync::{OnceLock, atomic::AtomicU64},
    time::Duration,
};

use axum::{Router, routing::get};
use tracing::{Level, event};

use crate::db_part;

pub mod assets;
pub mod cache;
pub mod handlers;
pub mod models;
pub mod response;
pub mod ship_analysis;
pub mod traits;

use handlers::{
    api_market, api_overview, api_record_analysis, api_record_detail, api_record_raw,
    api_service_status, dashboard_page, empty_info, empty_resync, get_data_by_id,
    get_data_info_by_id, get_last_data, get_last_save, get_last_ship, jump_to_dashboard,
    jump_to_dashboard_from_root, resync_request,
};

pub static WEB_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);
pub static API_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn web_request_counter_pp() -> u64 {
    WEB_REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

pub fn web_request_counter() -> u64 {
    WEB_REQUEST_COUNTER.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn api_request_counter_pp() -> u64 {
    API_REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

pub fn api_request_counter() -> u64 {
    API_REQUEST_COUNTER.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn service_uptime() -> Duration {
    elapsed_since(
        crate::START_TIME.get().copied(),
        std::time::SystemTime::now(),
    )
}

fn elapsed_since(start: Option<std::time::SystemTime>, now: std::time::SystemTime) -> Duration {
    start
        .and_then(|start| now.duration_since(start).ok())
        .unwrap_or_default()
}

pub const INFO_PAGE: &str = include_str!("../assets/info.html");
pub static RESYNC_TOKEN: OnceLock<String> = OnceLock::new();

pub async fn web_main() -> anyhow::Result<()> {
    web_main_until_stopped(std::future::pending()).await
}

/// 停止接收新请求，等待在途请求结束，再关闭 Web 专用连接池。
pub async fn web_main_until_stopped(
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let conf = crate::config::ConfigFile::get_global();

    let listener = tokio::net::TcpListener::bind(conf.serve.host_with_port.clone()).await?;
    let db = db_part::connect_server(conf).await?;
    let _cache = cache::WebCache::new(
        &db,
        Duration::from_millis(conf.serve.refresh_interval as u64),
    )
    .await;
    let app = Router::new()
        .route("/last/data", get(get_last_data).post(get_last_data))
        .route("/last/save", get(get_last_save).post(get_last_save))
        .route("/last/ship", get(get_last_ship).post(get_last_ship))
        .route(
            "/info/{id}",
            get(get_data_info_by_id).post(get_data_info_by_id),
        )
        .route("/info", get(empty_info).post(empty_info))
        .route("/resync/{id}", get(resync_request))
        .route("/resync", get(empty_resync).post(empty_resync))
        .route("/download/{id}", get(get_data_by_id).post(get_data_by_id))
        .route("/api/overview", get(api_overview))
        .route("/api/service", get(api_service_status))
        .route("/api/market", get(api_market))
        .route("/api/records/{id}", get(api_record_detail))
        .route("/api/records/{id}/analysis", get(api_record_analysis))
        .route("/api/records/{id}/raw", get(api_record_raw))
        .route("/dashboard", get(dashboard_page).post(dashboard_page))
        .route("/dashboard.html", get(dashboard_page).post(dashboard_page))
        .route("/favicon.ico", get(assets::favicon).post(assets::favicon))
        .route(
            "/assets/info.css",
            get(assets::info_css).post(assets::info_css),
        )
        .route(
            "/assets/info.js",
            get(assets::info_js).post(assets::info_js),
        )
        .route(
            "/assets/dark.js",
            get(assets::dark_js).post(assets::dark_js),
        )
        .route("/ua_display", get(assets::ua_display))
        .route("/{*path}", get(jump_to_dashboard).post(jump_to_dashboard))
        .route(
            "/",
            get(jump_to_dashboard_from_root).post(jump_to_dashboard_from_root),
        )
        .with_state(db.clone());

    event!(
        Level::INFO,
        "Starting http server on http://{}",
        conf.serve.host_with_port
    );
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;
    db.close().await;
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::elapsed_since;
    use std::time::{Duration, SystemTime};

    #[test]
    fn uptime_handles_an_uninitialized_start_time() {
        assert_eq!(elapsed_since(None, SystemTime::UNIX_EPOCH), Duration::ZERO);
    }

    #[test]
    fn uptime_handles_clock_rollback() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let start = now + Duration::from_secs(1);
        assert_eq!(elapsed_since(Some(start), now), Duration::ZERO);
    }

    #[test]
    fn uptime_reports_elapsed_time() {
        let start = SystemTime::UNIX_EPOCH;
        let now = start + Duration::from_secs(42);
        assert_eq!(elapsed_since(Some(start), now), Duration::from_secs(42));
    }
}
