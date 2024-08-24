use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use sea_orm::{ActiveEnum, DatabaseConnection};
use serde::{Deserialize, Serialize};

use crate::db_part;
use migration::SaveId;

pub mod traits;

use traits::FromDb;

#[derive(Serialize, Deserialize)]
pub struct WebResponse<T> {
    pub code: u32,
    pub msg: String,
    pub data: Option<T>,
}

impl<T> WebResponse<T> {
    pub fn new(data: Option<T>) -> Self {
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

    pub fn new_missing(msg: String) -> Self {
        Self {
            code: StatusCode::NOT_FOUND.as_u16() as u32,
            msg,
            data: None,
        }
    }

    pub fn new_error(status: StatusCode, msg: String) -> Self {
        Self {
            code: status.as_u16() as u32,
            msg,
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
}

impl LastData {
    pub async fn from_db_by_id(db: &DatabaseConnection, id: SaveId) -> Option<Self> {
        let data = db_part::get_raw_data(id, db).await?;
        Some(Self {
            save_id: data.save_id,
            save_type: data.save_type.to_value().to_string(),
            len: data.len,
            blake_hash: data.blake_hash,
        })
    }
}

/// 最后一个存档的信息
#[derive(Serialize, Deserialize)]
pub struct LastSave {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
}

/// 最后一个飞船的信息
#[derive(Serialize, Deserialize)]
pub struct LastShip {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
}

/// 实际信息
#[derive(Serialize, Deserialize)]
pub struct RawData {
    pub info: LastData,
    pub raw_data: String,
}

impl RawData {
    pub async fn from_db_by_id(db: &DatabaseConnection, id: SaveId) -> Option<Self> {
        let data = db_part::get_raw_data(id, db).await?;
        Some(Self {
            info: LastData {
                save_id: data.save_id,
                save_type: data.save_type.to_value().to_string(),
                len: data.len,
                blake_hash: data.blake_hash,
            },
            raw_data: data.text?,
        })
    }
}

async fn get_last_data(State(db): State<DatabaseConnection>) -> Json<WebResponse<LastData>> {
    Json(WebResponse::new(LastData::from_db(&db).await))
}

async fn get_last_save(State(db): State<DatabaseConnection>) -> Json<WebResponse<LastSave>> {
    Json(WebResponse::new(LastSave::from_db(&db).await))
}

async fn get_last_ship(State(db): State<DatabaseConnection>) -> Json<WebResponse<LastShip>> {
    Json(WebResponse::new(LastShip::from_db(&db).await))
}

async fn get_data_info_by_id(
    State(db): State<DatabaseConnection>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<LastData>> {
    match raw_id.parse::<SaveId>() {
        Ok(id) => match LastData::from_db_by_id(&db, id).await {
            Some(data) => Json(WebResponse::new_normal(data)),
            None => Json(WebResponse::new_missing("data not found".to_string())),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {:?}", e),
        )),
    }
}

async fn get_data_by_id(
    State(db): State<DatabaseConnection>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RawData>> {
    match raw_id.parse::<SaveId>() {
        Ok(id) => match RawData::from_db_by_id(&db, id).await {
            Some(data) => Json(WebResponse::new_normal(data)),
            None => Json(WebResponse::new_missing("data not found".to_string())),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {:?}", e),
        )),
    }
}

async fn jump_to_info(Path(path): Path<String>) -> impl IntoResponse {
    // html jump
    (
        StatusCode::MOVED_PERMANENTLY,
        Html(format!(
            "<h1>Jumping from {} to /info</h1><script>location.href='/info'</script>",
            path
        )),
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
const INFO_PAGE: &str = include_str!("info.html");

async fn info_page(State(db): State<DatabaseConnection>) -> Html<String> {
    let max_id = db_part::find_max_id(&db).await;
    let max_id_data = db_part::get_raw_data(max_id, &db).await.unwrap();
    let max_ship = db_part::find_max_ship(&db).await;
    let max_save = db_part::find_max_save(&db).await;

    let mut page_content = INFO_PAGE
        .replace("|MAX_ID|", &max_id_data.save_id.to_string())
        .replace(
            "|MAX_SAVE_TYPE|",
            &max_id_data.save_type.to_value().to_string(),
        )
        .replace("|MAX_LEN|", &max_id_data.len.to_string())
        .replace("|MAX_HASH|", &max_id_data.blake_hash);
    if let Some(max_ship) = max_ship {
        page_content = page_content
            .replace("|MAX_SHIP_ID|", &max_ship.save_id.to_string())
            .replace("|MAX_SHIP_LEN|", &max_ship.len.to_string())
            .replace("|MAX_SHIP_HASH|", &max_ship.blake_hash);
    } else {
        page_content = page_content
            .replace("|MAX_SHIP_ID|", "not found")
            .replace("|MAX_SHIP_LEN|", "not found")
            .replace("|MAX_SHIP_HASH|", "not found");
    }
    if let Some(max_save) = max_save {
        page_content = page_content
            .replace("|MAX_SAVE_ID|", &max_save.save_id.to_string())
            .replace("|MAX_SAVE_LEN|", &max_save.len.to_string())
            .replace("|MAX_SAVE_HASH|", &max_save.blake_hash);
    } else {
        page_content = page_content
            .replace("|MAX_SAVE_ID|", "not found")
            .replace("|MAX_SAVE_LEN|", "not found")
            .replace("|MAX_SAVE_HASH|", "not found");
    }

    Html(page_content)
}

pub async fn web_main() -> anyhow::Result<()> {
    let conf = crate::config::ConfigFile::try_read()?;

    let listener = tokio::net::TcpListener::bind(conf.serve.host_with_port.clone()).await?;
    let db = db_part::connect_server(&conf).await?;
    let app = Router::new()
        // 获取最后一个数据
        .route("/last/data", get(get_last_data).post(get_last_data))
        // 获取最后一个存档
        .route("/last/save", get(get_last_save).post(get_last_save))
        // 获取最后一个飞船
        .route("/last/ship", get(get_last_ship).post(get_last_ship))
        // 获取指定 id 的数据(也有可能返回 not found)
        .route(
            "/info/:id",
            get(get_data_info_by_id).post(get_data_info_by_id),
        )
        // 获取下载指定 id 的数据
        .route("/download/:id", get(get_data_by_id).post(get_data_by_id))
        // info 页面
        .route("/info", get(info_page).post(info_page))
        // 其他所有路径, 直接跳转到 info 页面
        .route("/*path", get(jump_to_info).post(jump_to_info))
        // db
        .with_state(db);

    axum::serve(listener, app).await?;
    Ok(())
}
