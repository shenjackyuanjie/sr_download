use std::{sync::LazyLock, time::Duration};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use sqlx::PgPool;

use crate::{
    Downloader, SaveId,
    db_part::{self, DbData, SaveType, utils::FromDb},
};

use super::{
    INFO_PAGE, RESYNC_TOKEN, api_request_counter_pp,
    models::{
        DashboardOverview, LastData, LastSave, LastShip, MarketPage, MarketQuery, RawData,
        RecordDetail, ServiceStatus,
    },
    response::WebResponse,
    ship_analysis::{ShipAnalysis, analyze_ship},
    web_request_counter_pp,
};

pub static RESYNC_DOWNLOADER: LazyLock<Downloader> =
    LazyLock::new(|| Downloader::new(Some(Duration::from_secs(10))));

pub async fn get_last_data(State(db): State<PgPool>) -> Json<WebResponse<LastData>> {
    api_request_counter_pp();
    Json(WebResponse::new_with_data(LastData::from_db(&db).await))
}

pub async fn get_last_save(State(db): State<PgPool>) -> Json<WebResponse<LastSave>> {
    api_request_counter_pp();
    Json(WebResponse::new_with_data(LastSave::from_db(&db).await))
}

pub async fn get_last_ship(State(db): State<PgPool>) -> Json<WebResponse<LastShip>> {
    api_request_counter_pp();
    Json(WebResponse::new_with_data(LastShip::from_db(&db).await))
}

pub async fn get_data_info_by_id(
    State(db): State<PgPool>,
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

pub async fn empty_info() -> Json<WebResponse<()>> {
    Json(WebResponse::new_missing(
        "you need to use /info/:id to get info",
    ))
}

pub async fn get_data_by_id(
    State(db): State<PgPool>,
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
            format!("id parse error: {e:?}"),
        )),
    }
}

pub async fn api_overview(State(db): State<PgPool>) -> Json<WebResponse<DashboardOverview>> {
    api_request_counter_pp();
    Json(WebResponse::new_normal(DashboardOverview {
        latest_data: LastData::from_db(&db).await,
        latest_ship: LastShip::from_db(&db).await,
        latest_save: LastSave::from_db(&db).await,
        service: ServiceStatus::collect(),
    }))
}

pub async fn api_service_status() -> Json<WebResponse<ServiceStatus>> {
    api_request_counter_pp();
    Json(WebResponse::new_normal(ServiceStatus::collect()))
}

pub async fn api_market(
    State(db): State<PgPool>,
    Query(query): Query<MarketQuery>,
) -> Json<WebResponse<MarketPage>> {
    api_request_counter_pp();
    let limit = query.limit.unwrap_or(48).clamp(1, 100);
    let save_type = match query.save_type.as_deref().unwrap_or("all") {
        "" | "all" => None,
        "ship" => Some(SaveType::Ship),
        "save" => Some(SaveType::Save),
        "unknown" => Some(SaveType::Unknown),
        value => {
            return Json(WebResponse::new_error(
                StatusCode::BAD_REQUEST,
                format!("unsupported market type: {value}"),
            ));
        }
    };
    match MarketPage::from_db(&db, limit, save_type, query.before).await {
        Ok(page) => Json(WebResponse::new_normal(page)),
        Err(error) => Json(WebResponse::new_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to query market records: {error}"),
        )),
    }
}

pub async fn api_record_detail(
    State(db): State<PgPool>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RecordDetail>> {
    api_request_counter_pp();
    match raw_id.parse::<SaveId>() {
        Ok(id) => match DbData::from_db(id, &db).await {
            Some(data) => Json(WebResponse::new_normal(RecordDetail {
                info: LastData {
                    save_id: data.save_id,
                    save_type: data.save_type.to_string(),
                    len: data.len,
                    blake_hash: data.blake_hash.clone(),
                    xml_tested: data.xml_tested,
                },
                xml_status: data.xml_status(),
                raw_data: data.text,
            })),
            None => Json(WebResponse::new_missing("data not found")),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {e:?}"),
        )),
    }
}

pub async fn api_record_analysis(
    State(db): State<PgPool>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<ShipAnalysis>> {
    api_request_counter_pp();
    match raw_id.parse::<SaveId>() {
        Ok(id) => match DbData::from_db(id, &db).await {
            Some(data) => {
                if data.save_type != SaveType::Ship
                    || !data.xml_tested
                    || !matches!(
                        data.verify_ship(),
                        db_part::utils::ShipVerifyState::VerifiedShip
                    )
                {
                    return Json(WebResponse::new_error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "record is not a verified ship",
                    ));
                }
                match data.parse_ship_xml() {
                    Ok(document) => Json(WebResponse::new_normal(analyze_ship(&document))),
                    Err(error) => Json(WebResponse::new_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to analyze ship xml: {error}"),
                    )),
                }
            }
            None => Json(WebResponse::new_missing("data not found")),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {e:?}"),
        )),
    }
}

pub async fn api_record_raw(
    State(db): State<PgPool>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RawData>> {
    api_request_counter_pp();
    match raw_id.parse::<SaveId>() {
        Ok(id) => match RawData::from_db_by_id(&db, id).await {
            Some(data) => Json(WebResponse::new_normal(data)),
            None => Json(WebResponse::new_missing("data not found")),
        },
        Err(e) => Json(WebResponse::new_error(
            StatusCode::BAD_REQUEST,
            format!("id parse error: {e:?}"),
        )),
    }
}

pub async fn jump_to_dashboard(Path(path): Path<String>) -> impl IntoResponse {
    (
        StatusCode::MOVED_PERMANENTLY,
        Html(format!(
            "<h1>Jumping from {path} to /dashboard</h1><script>location.href='/dashboard'</script>"
        )),
    )
}

pub async fn jump_to_dashboard_from_root() -> impl IntoResponse {
    (
        StatusCode::MOVED_PERMANENTLY,
        Html(
            "<h1>Jumping from / to /dashboard</h1><script>location.href='/dashboard'</script>"
                .to_string(),
        ),
    )
}

pub async fn dashboard_page() -> Html<String> {
    web_request_counter_pp();
    Html(INFO_PAGE.to_string())
}

pub async fn resync_request(
    headers: HeaderMap,
    State(db): State<PgPool>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RawData>> {
    api_request_counter_pp();
    const RESYNC_HEADER: &str = "X-Resync-Token";

    let token = RESYNC_TOKEN.get().unwrap();
    if let Some(receive_token) = headers.get(RESYNC_HEADER) {
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

pub async fn empty_resync() -> Json<WebResponse<()>> {
    Json(WebResponse::new_missing(
        "you need to use /resync/:id to call resync",
    ))
}
