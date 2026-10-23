use std::{sync::LazyLock, time::Duration};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
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

fn parse_record_id(raw_id: &str) -> Result<SaveId, String> {
    let id = raw_id
        .parse::<SaveId>()
        .map_err(|error| error.to_string())?;
    i32::try_from(id).map_err(|_| format!("id must be at most {}", i32::MAX))?;
    Ok(id)
}

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
    match parse_record_id(&raw_id) {
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
    match parse_record_id(&raw_id) {
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
    match parse_record_id(&raw_id) {
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
    match parse_record_id(&raw_id) {
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
    match parse_record_id(&raw_id) {
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

pub async fn jump_to_dashboard(Path(_path): Path<String>) -> impl IntoResponse {
    jump_to_dashboard_from_root().await
}

pub async fn jump_to_dashboard_from_root() -> impl IntoResponse {
    (
        StatusCode::MOVED_PERMANENTLY,
        [(header::LOCATION, "/dashboard")],
        Html(r#"<a href="/dashboard">Dashboard</a>"#),
    )
}

pub async fn dashboard_page() -> Html<String> {
    web_request_counter_pp();
    Html(INFO_PAGE.to_string())
}

fn authorize_resync(
    headers: &HeaderMap,
    token: Option<&str>,
) -> Result<(), (StatusCode, &'static str)> {
    let token = token
        .filter(|token| !token.is_empty())
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Resync is not configured"))?;
    let received = headers.get("X-Resync-Token").ok_or((
        StatusCode::UNAUTHORIZED,
        "Missing resync token header, use X-Resync-Token please",
    ))?;
    if received != token {
        return Err((StatusCode::UNAUTHORIZED, "Invalid token"));
    }
    Ok(())
}

pub async fn resync_request(
    headers: HeaderMap,
    State(db): State<PgPool>,
    Path(raw_id): Path<String>,
) -> Json<WebResponse<RawData>> {
    api_request_counter_pp();
    if let Err((status, message)) =
        authorize_resync(&headers, RESYNC_TOKEN.get().map(String::as_str))
    {
        return Json(WebResponse::new_error(status, message));
    }
    let id = match parse_record_id(&raw_id) {
        Ok(id) => id,
        Err(error) => {
            return Json(WebResponse::new_error(
                StatusCode::BAD_REQUEST,
                format!("failed to parse id: {error}"),
            ));
        }
    };
    let Some(data) = RESYNC_DOWNLOADER.try_download_as_any(id).await else {
        return Json(WebResponse::new_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Download failed",
        ));
    };
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
        Ok(true) => Json(WebResponse::new_normal(RawData::from_file(data, id))),
        Ok(false) => Json(WebResponse::new(
            StatusCode::OK,
            "Data unchanged, no update needed",
            Some(RawData::from_file(data, id)),
        )),
        Err(error) => Json(WebResponse::new_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save data to db: {error}"),
        )),
    }
}

pub async fn empty_resync() -> Json<WebResponse<()>> {
    Json(WebResponse::new_missing(
        "you need to use /resync/:id to call resync",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::HeaderValue};
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

    #[test]
    fn parses_only_ids_supported_by_postgres_integer() {
        for (raw, expected) in [
            ("0", 0),
            ("76858", 76858),
            ("2147483647", i32::MAX as SaveId),
        ] {
            assert_eq!(parse_record_id(raw).unwrap(), expected);
        }
        for raw in ["", "-1", "abc", "2147483648", "4294967295", "4294967296"] {
            assert!(parse_record_id(raw).is_err(), "应拒绝 ID: {raw}");
        }
    }

    #[tokio::test]
    async fn record_handlers_report_bad_input_without_database_access() {
        let db = PgPoolOptions::new().connect_lazy_with(PgConnectOptions::new());
        db.close().await;
        for raw in ["-1", "abc", "2147483648", "4294967295"] {
            let response = get_data_info_by_id(State(db.clone()), Path(raw.to_string())).await;
            assert_eq!(response.0.code, 400);
            let response = get_data_by_id(State(db.clone()), Path(raw.to_string())).await;
            assert_eq!(response.0.code, 400);
            let response = api_record_detail(State(db.clone()), Path(raw.to_string())).await;
            assert_eq!(response.0.code, 400);
            let response = api_record_raw(State(db.clone()), Path(raw.to_string())).await;
            assert_eq!(response.0.code, 400);
            let response = api_record_analysis(State(db.clone()), Path(raw.to_string())).await;
            assert_eq!(response.0.code, 400);
        }
    }

    #[tokio::test]
    async fn market_rejects_unknown_type_before_database_access() {
        let db = PgPoolOptions::new().connect_lazy_with(PgConnectOptions::new());
        db.close().await;
        let response = api_market(
            State(db),
            Query(MarketQuery {
                limit: None,
                save_type: Some("invalid".to_string()),
                before: None,
            }),
        )
        .await;
        assert_eq!(response.0.code, 400);
    }

    #[test]
    fn resync_requires_a_configured_nonempty_token() {
        let headers = HeaderMap::new();
        for token in [None, Some("")] {
            assert_eq!(
                authorize_resync(&headers, token).unwrap_err().0,
                StatusCode::SERVICE_UNAVAILABLE
            );
        }
    }

    #[test]
    fn resync_validates_missing_incorrect_and_correct_tokens() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            authorize_resync(&headers, Some("test-token"))
                .unwrap_err()
                .0,
            StatusCode::UNAUTHORIZED
        );
        headers.insert("X-Resync-Token", HeaderValue::from_static("incorrect"));
        assert_eq!(
            authorize_resync(&headers, Some("test-token"))
                .unwrap_err()
                .0,
            StatusCode::UNAUTHORIZED
        );
        headers.insert("X-Resync-Token", HeaderValue::from_static("test-token"));
        assert_eq!(authorize_resync(&headers, Some("test-token")), Ok(()));
    }

    #[tokio::test]
    async fn dashboard_redirect_uses_location_and_does_not_echo_the_path() {
        let response = jump_to_dashboard(Path("<script>alert(1)</script>".to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(response.headers()[header::LOCATION], "/dashboard");
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body.contains("<script>"));
        assert!(!body.contains("alert(1)"));
    }
}
