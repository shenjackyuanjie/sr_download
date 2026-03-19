use std::io::Write;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    SaveId,
    db_part::{DbData, utils},
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

#[derive(Serialize, Deserialize)]
pub struct RecordDetail {
    pub info: LastData,
    pub xml_status: String,
    pub raw_data: Option<String>,
}
