use std::io::Write;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::{
    SaveId,
    db_part::{DbData, SaveType, utils},
    net::DownloadFile,
    web_part::{api_request_counter, service_uptime, web_request_counter},
};

#[derive(Serialize, Deserialize)]
pub struct LastData {
    pub save_id: SaveId,
    pub save_type: String,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

impl LastData {
    pub async fn from_db_by_id(db: &PgPool, id: SaveId) -> Option<Self> {
        let data = DbData::from_db(id, db).await?;
        let xml_tested = data.verify_xml();
        Some(Self {
            save_id: data.save_id,
            save_type: data.save_type.to_string(),
            len: data.len,
            blake_hash: data.blake_hash,
            xml_tested,
        })
    }

    pub fn from_file(file: &DownloadFile, id: SaveId) -> Self {
        let xml_tested = utils::verify_xml(file.ref_data()).is_ok();
        let save_type = file.save_type().to_string();
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

#[derive(Serialize, Deserialize)]
pub struct LastSave {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

#[derive(Serialize, Deserialize)]
pub struct LastShip {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

#[derive(Serialize, Deserialize)]
pub struct RawData {
    pub info: LastData,
    pub raw_data: String,
}

impl RawData {
    pub async fn from_db_by_id(db: &PgPool, id: SaveId) -> Option<Self> {
        let data = DbData::from_db(id, db).await?;
        let xml_tested = data.verify_xml();
        Some(Self {
            info: LastData {
                save_id: data.save_id,
                save_type: data.save_type.to_string(),
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

#[derive(Serialize, Deserialize)]
pub struct ServiceStatus {
    pub version: String,
    pub web_request_count: u64,
    pub api_request_count: u64,
    pub uptime_human: String,
    pub uptime_seconds: u64,
    pub min_lookup_id: SaveId,
}

impl ServiceStatus {
    pub fn collect() -> Self {
        let uptime = service_uptime();
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            web_request_count: web_request_counter(),
            api_request_count: api_request_counter(),
            uptime_human: humantime::format_duration(uptime).to_string(),
            uptime_seconds: uptime.as_secs(),
            min_lookup_id: 76858,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct DashboardOverview {
    pub latest_data: Option<LastData>,
    pub latest_ship: Option<LastShip>,
    pub latest_save: Option<LastSave>,
    pub service: ServiceStatus,
}

#[derive(Debug, Deserialize)]
pub struct MarketQuery {
    pub limit: Option<u16>,
    #[serde(rename = "type")]
    pub save_type: Option<String>,
    pub before: Option<SaveId>,
}

#[derive(Debug, Serialize)]
pub struct MarketPage {
    pub records: Vec<MarketRecord>,
    pub has_more: bool,
    pub next_before: Option<SaveId>,
}

#[derive(Debug, Serialize)]
pub struct MarketRecord {
    pub save_id: SaveId,
    pub save_type: String,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
    pub recorded_at: String,
}

#[derive(Debug, FromRow)]
struct MarketRow {
    save_id: i32,
    save_type: SaveType,
    len: i64,
    blake_hash: String,
    xml_tested: Option<bool>,
    time: DateTime<Utc>,
}

impl From<MarketRow> for MarketRecord {
    fn from(row: MarketRow) -> Self {
        Self {
            save_id: row.save_id as SaveId,
            save_type: row.save_type.to_string(),
            len: row.len,
            blake_hash: row.blake_hash,
            xml_tested: row.xml_tested.unwrap_or(false),
            recorded_at: row.time.to_rfc3339(),
        }
    }
}

impl MarketPage {
    pub async fn from_db(
        db: &PgPool,
        limit: u16,
        save_type: Option<SaveType>,
        before: Option<SaveId>,
    ) -> Result<Self, sqlx::Error> {
        let limit = i64::from(limit);
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT save_id, save_type, len, blake_hash, xml_tested, time
             FROM main_data
             WHERE len > 0",
        );
        match save_type {
            Some(save_type) => query.push(" AND save_type = ").push_bind(save_type),
            None => query.push(" AND save_type != ").push_bind(SaveType::None),
        };
        if let Some(before) = before {
            query.push(" AND save_id < ").push_bind(before as i32);
        }
        query
            .push(" ORDER BY save_id DESC LIMIT ")
            .push_bind(limit + 1);

        let mut rows = query.build_query_as::<MarketRow>().fetch_all(db).await?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_before = has_more
            .then(|| rows.last().map(|row| row.save_id as SaveId))
            .flatten();
        Ok(Self {
            records: rows.into_iter().map(Into::into).collect(),
            has_more,
            next_before,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct RecordDetail {
    pub info: LastData,
    pub xml_status: String,
    pub raw_data: Option<String>,
}
