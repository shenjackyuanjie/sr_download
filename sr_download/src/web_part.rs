use std::{
    io::Write,
    sync::{LazyLock, OnceLock, atomic::AtomicU64},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
};
use sea_orm::{ActiveEnum, DatabaseConnection};
use serde::{Deserialize, Serialize};
use tracing::{Level, event};

use crate::{
    Downloader,
    db_part::{
        self, DbData, SaveType,
        utils::{self, FromDb},
    },
    net::DownloadFile,
};
use migration::SaveId;

pub mod assets;
pub mod traits;

/// 网页请求总计数器
///
/// 就是闲得没事
pub static WEB_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// API 请求总计数器
///
/// 就是闲得没事
pub static API_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 获取网页请求计数器, 顺便 +1
pub fn web_request_counter_pp() -> u64 {
    WEB_REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

/// 获取网页请求计数器
pub fn web_request_counter() -> u64 {
    WEB_REQUEST_COUNTER.load(std::sync::atomic::Ordering::Relaxed)
}

/// 获取API请求计数器, 顺便 +1
pub fn api_request_counter_pp() -> u64 {
    API_REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

/// 获取API请求计数器
pub fn api_request_counter() -> u64 {
    API_REQUEST_COUNTER.load(std::sync::atomic::Ordering::Relaxed)
}

/// 获取服务开了多久
pub fn service_uptime() -> std::time::Duration {
    crate::START_TIME.get().unwrap().elapsed().unwrap()
}

#[derive(Serialize, Deserialize)]
pub struct WebResponse<T> {
    pub code: u32,
    pub msg: String,
    pub data: Option<T>,
}

impl<T> WebResponse<T> {
    pub fn new(status: StatusCode, msg: impl ToString, data: Option<T>) -> Self {
        Self {
            code: status.as_u16() as u32,
            msg: msg.to_string(),
            data,
        }
    }

    pub fn new_with_data(data: Option<T>) -> Self {
        match data {
            Some(data) => Self::new_normal(data),
            None => Self::new_missing("internal error?".to_string()),
        }
    }

    pub fn new_normal(data: T) -> Self {
        Self {
            code: StatusCode::OK.as_u16() as u32,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }

    pub fn new_missing(msg: impl ToString) -> Self {
        Self {
            code: StatusCode::NOT_FOUND.as_u16() as u32,
            msg: msg.to_string(),
            data: None,
        }
    }

    pub fn new_error(status: StatusCode, msg: impl ToString) -> Self {
        Self {
            code: status.as_u16() as u32,
            msg: msg.to_string(),
            data: None,
        }
    }
}

/// 最后一个数据的信息
#[derive(Serialize, Deserialize)]
pub struct LastData {
    pub save_id: SaveId,
    pub save_type: String,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

impl LastData {
    pub async fn from_db_by_id(db: &DatabaseConnection, id: SaveId) -> Option<Self> {
        let data = DbData::from_db(id, db).await?;
        let xml_tested = data.verify_xml();
        Some(Self {
            save_id: data.save_id,
            save_type: data.save_type.to_value().to_string(),
            len: data.len,
            blake_hash: data.blake_hash,
            xml_tested,
        })
    }

    pub fn from_file(file: &DownloadFile, id: SaveId) -> Self {
        let xml_tested = utils::verify_xml(file.ref_data()).is_ok();
        let save_type = file.save_type().to_value().to_string();
        let len = file.len();
        let blake_hash = {
            let mut hasher = blake3::Hasher::new();
            let _ = hasher.write(file.ref_data().as_bytes());
            hasher.finalize().to_hex().to_string()
        };
        Self {
            save_id: id,
            save_type,
            len: len as i64,
            blake_hash,
            xml_tested,
        }
    }
}

/// 最后一个存档的信息
#[derive(Serialize, Deserialize)]
pub struct LastSave {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

/// 最后一个飞船的信息
#[derive(Serialize, Deserialize)]
pub struct LastShip {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

/// 实际信息
#[derive(Serialize, Deserialize)]
pub struct RawData {
    pub info: LastData,
    pub raw_data: String,
}

impl RawData {
    pub async fn from_db_by_id(db: &DatabaseConnection, id: SaveId) -> Option<Self> {
        let data = DbData::from_db(id, db).await?;
        let xml_tested = data.verify_xml();
        Some(Self {
            info: LastData {
                save_id: data.save_id,
                save_type: data.save_type.to_value().to_string(),
                len: data.len,
                blake_hash: data.blake_hash,
                xml_tested,
            },
            raw_data: data.text?,
        })
    }

    pub fn from_file(file: DownloadFile, id: SaveId) -> Self {
        let info = LastData::from_file(&file, id);
        Self {
            info,
            raw_data: file.take_data(),
        }
    }
}

async fn get_last_data(State(db): State<DatabaseConnection>) -> Json<WebResponse<LastData>> {
    api_request_counter_pp();
    Json(WebResponse::new_with_data(LastData::from_db(&db).await))
}

async fn get_last_save(State(db): State<DatabaseConnection>) -> Json<WebResponse<LastSave>> {
    api_request_counter_pp();
    Json(WebResponse::new_with_data(LastSave::from_db(&db).await))
}

async fn get_last_ship(State(db): State<DatabaseConnection>) -> Json<WebResponse<LastShip>> {
    api_request_counter_pp();
    Json(WebResponse::new_with_data(LastShip::from_db(&db).await))
}

async fn get_data_info_by_id(
    State(db): State<DatabaseConnection>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<LastData>> {
    api_request_counter_pp();
    match raw_id.parse::<SaveId>() {
        Ok(id) => match LastData::from_db_by_id(&db, id).await {
            Some(data) => Json(WebResponse::new_normal(data)),
            None => Json(WebResponse::new_missing("data not found".to_string())),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {e:?}"),
        )),
    }
}

async fn empty_info() -> Json<WebResponse<()>> {
    Json(WebResponse::new_missing(
        "you need to use /info/:id to get info",
    ))
}

async fn get_data_by_id(
    State(db): State<DatabaseConnection>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RawData>> {
    api_request_counter_pp();
    match raw_id.parse::<SaveId>() {
        Ok(id) => match RawData::from_db_by_id(&db, id).await {
            Some(data) => Json(WebResponse::new_normal(data)),
            None => Json(WebResponse::new_missing("data not found".to_string())),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {e:?}",),
        )),
    }
}

async fn jump_to_dashboard(Path(path): Path<String>) -> impl IntoResponse {
    // html jump
    (
        StatusCode::MOVED_PERMANENTLY,
        Html(format!(
            "<h1>Jumping from {path} to /dashboard</h1><script>location.href='/dashboard'</script>"
        )),
    )
}

async fn jump_to_dashboard_from_root() -> impl IntoResponse {
    (
        StatusCode::MOVED_PERMANENTLY,
        Html(
            "<h1>Jumping from / to /dashboard</h1><script>location.href='/dashboard'</script>"
                .to_string(),
        ),
    )
}

/// 下面这段话是用于喂给 GitHub Copilot 让他帮我生成一个好用的 info 页面的 prompt
/// 页面背景 F5F5F5FF
/// 页面标题为 "sr-download 信息页面"
/// 页面内容为三个白色框, 横向排列
/// 里面分别是最大 id, 最大飞船 id, 最大存档 id 的信息展示
/// 框内文字居中，字体大小 24px, 字体为浏览器默认给的
/// 最大 id 部分的文字为 "最大 id: |MAX_ID|\n存档类型: |MAX_SAVE_TYPE|\n长度: |MAX_LEN|\nblake hash: |MAX_HASH|"
/// 最大飞船 id 部分展示相关信息, 存档 id 部分同理,  用 相关 |xxx| 作为占位符
/// 同时展示 长度, blake hash
/// 两个部分分别会展示三行字
/// 三个框之间的间距为 20px, 宽度为 80%, 高度为 100%
/// 框上面分别是 "最新数据" "最新飞船" "最新存档" 的标题
const INFO_PAGE: &str = include_str!("../assets/info.html");

async fn dashboard_page(State(db): State<DatabaseConnection>) -> Html<String> {
    let start_time = std::time::Instant::now();
    let max_id = db_part::search::max_id(&db).await;
    let max_id_data = DbData::from_db(max_id, &db).await;
    let max_ship = db_part::search::max_ship(&db).await;
    let max_save = db_part::search::max_save(&db).await;

    let mut page_content = INFO_PAGE.replace("|MAX_ID|", &max_id.to_string());

    if let Some(max_id_data) = max_id_data {
        page_content = page_content
            .replace("|MAX_ID|", &max_id_data.save_id.to_string())
            .replace(
                "|MAX_SAVE_TYPE|",
                &max_id_data.save_type.to_value().to_string(),
            )
            .replace("|MAX_LEN|", &max_id_data.len.to_string())
            .replace("|MAX_HASH|", &max_id_data.blake_hash)
            .replace("|MAX_XML|", &max_id_data.xml_status());
    } else {
        page_content = page_content
            .replace("|MAX_ID|", "not found")
            .replace("|MAX_SAVE_TYPE|", "not found")
            .replace("|MAX_LEN|", "not found")
            .replace("|MAX_HASH|", "not found")
            .replace("|MAX_XML|", "not found");
    }
    if let Some(max_ship) = max_ship {
        page_content = page_content
            .replace("|MAX_SHIP_ID|", &max_ship.save_id.to_string())
            .replace("|MAX_SHIP_LEN|", &max_ship.len.to_string())
            .replace("|MAX_SHIP_HASH|", &max_ship.blake_hash)
            .replace("|MAX_SHIP_XML|", &max_ship.xml_status());
    } else {
        page_content = page_content
            .replace("|MAX_SHIP_ID|", "not found")
            .replace("|MAX_SHIP_LEN|", "not found")
            .replace("|MAX_SHIP_HASH|", "not found")
            .replace("|MAX_SHIP_XML|", "not found");
    }
    if let Some(max_save) = max_save {
        page_content = page_content
            .replace("|MAX_SAVE_ID|", &max_save.save_id.to_string())
            .replace("|MAX_SAVE_LEN|", &max_save.len.to_string())
            .replace("|MAX_SAVE_HASH|", &max_save.blake_hash)
            .replace("|MAX_SAVE_XML|", &max_save.xml_status());
    } else {
        page_content = page_content
            .replace("|MAX_SAVE_ID|", "not found")
            .replace("|MAX_SAVE_LEN|", "not found")
            .replace("|MAX_SAVE_HASH|", "not found")
            .replace("|MAX_SAVE_XML|", "not found");
    }

    let elapsed = start_time.elapsed();

    let web_request_count = web_request_counter_pp();
    let api_request_count = api_request_counter();
    let service_uptime = service_uptime();

    page_content = page_content
        .replace("|COST_TIME|", &format!("{elapsed:?}"))
        .replace("|VERSION|", env!("CARGO_PKG_VERSION"))
        .replace("|WEB_REQUEST_COUNT|", &web_request_count.to_string())
        .replace("|API_REQUEST_COUNT|", &api_request_count.to_string())
        .replace(
            "|SERVICE_UPTIME|",
            &humantime::format_duration(service_uptime).to_string(),
        );

    Html(page_content)
}

pub static RESYNC_TOKEN: OnceLock<String> = OnceLock::new();
/// 用于 resync 的下载器
static RESYNC_DOWNLOADER: LazyLock<Downloader> =
    LazyLock::new(|| Downloader::new(Some(Duration::from_secs(10))));

async fn resync_request(
    headers: HeaderMap,
    State(db): State<DatabaseConnection>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RawData>> {
    api_request_counter_pp();
    const RESYNC_HEADER: &str = "X-Resync-Token"; // 自定义头部名称

    let token = RESYNC_TOKEN.get().unwrap();
    if let Some(receive_token) = headers.get(RESYNC_HEADER) {
        // 比较令牌值（注意HeaderValue需要转换）
        if receive_token != token {
            return Json(WebResponse::new_error(
                StatusCode::UNAUTHORIZED,
                "Invalid token",
            ));
        }
        match raw_id.parse::<SaveId>() {
            Ok(id) => match RESYNC_DOWNLOADER.try_download_as_any(id).await {
                Some(data) => {
                    let save_type: SaveType = (&data).into();
                    match db_part::save_data_to_db(
                        id,
                        save_type,
                        data.ref_data(),
                        Some(db_part::CoverStrategy::CoverIfDifferent),
                        &db,
                    )
                    .await
                    {
                        Ok(true) => {
                            let data = RawData::from_file(data, id);
                            Json(WebResponse::new_normal(data))
                        }
                        Ok(false) => {
                            let data = RawData::from_file(data, id);
                            Json(WebResponse::new(
                                StatusCode::OK,
                                "Data unchanged, no update needed",
                                Some(data),
                            ))
                        }
                        Err(e) => Json(WebResponse::new_error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Faild to save data to db e:{e}"),
                        )),
                    }
                }
                None => Json(WebResponse::new_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Download failed",
                )),
            },
            Err(e) => Json(WebResponse::new_error(
                StatusCode::BAD_REQUEST,
                format!("failed to parse id:{e}"),
            )),
        }
    } else {
        Json(WebResponse::new_error(
            StatusCode::UNAUTHORIZED,
            "Missing resync token header, use X-Resync-Token please",
        ))
    }
}

async fn empty_resync() -> Json<WebResponse<()>> {
    Json(WebResponse::new_missing(
        "you need to use /resync/:id to call resync",
    ))
}

pub async fn web_main() -> anyhow::Result<()> {
    let conf = crate::config::ConfigFile::get_global();

    let listener = tokio::net::TcpListener::bind(conf.serve.host_with_port.clone()).await?;
    let db = db_part::connect_server(conf).await?;
    let app = Router::new()
        // 获取最后一个数据
        .route("/last/data", get(get_last_data).post(get_last_data))
        // 获取最后一个存档
        .route("/last/save", get(get_last_save).post(get_last_save))
        // 获取最后一个飞船
        .route("/last/ship", get(get_last_ship).post(get_last_ship))
        // 获取指定 id 的数据(也有可能返回 not found)
        .route(
            "/info/{id}",
            get(get_data_info_by_id).post(get_data_info_by_id),
        )
        .route("/info", get(empty_info).post(empty_info))
        // 重新同步指定 id 的数据 (需要 token)
        .route("/resync/{id}", get(resync_request))
        .route("/resync", get(empty_resync).post(empty_resync))
        // 获取下载指定 id 的数据
        .route("/download/{id}", get(get_data_by_id).post(get_data_by_id))
        // info 页面
        .route("/dashboard", get(dashboard_page).post(dashboard_page))
        .route("/dashboard.html", get(dashboard_page).post(dashboard_page))
        // favicon
        .route("/favicon.ico", get(assets::favicon).post(assets::favicon))
        // assets
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
        // 其他所有路径, 直接跳转到 info 页面
        .route("/{*path}", get(jump_to_dashboard).post(jump_to_dashboard))
        // 包括根路径
        .route(
            "/",
            get(jump_to_dashboard_from_root).post(jump_to_dashboard_from_root),
        )
        // db
        .with_state(db);

    event!(
        Level::INFO,
        "Starting http server on http://{}",
        conf.serve.host_with_port
    );
    axum::serve(listener, app).await?;
    Ok(())
}
