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

fn market_query(
    limit: u16,
    save_type: Option<SaveType>,
    before: Option<SaveId>,
) -> QueryBuilder<Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT save_id, save_type, len, blake_hash, xml_tested, time
         FROM main_data
         WHERE len > 0 AND save_id >= 0",
    );
    match save_type {
        Some(save_type) => query.push(" AND save_type = ").push_bind(save_type),
        None => query.push(" AND save_type != ").push_bind(SaveType::None),
    };
    if let Some(before) = before {
        // 游标是排他的上界，可大于数据库最大 ID；用 bigint 保持完整的 u32 范围。
        query
            .push(" AND save_id < ")
            .push_bind(i64::from(before))
            .push("::bigint");
    }
    query
        .push(" ORDER BY save_id DESC LIMIT ")
        .push_bind(i64::from(limit) + 1);
    query
}

impl MarketPage {
    pub async fn from_db(
        db: &PgPool,
        limit: u16,
        save_type: Option<SaveType>,
        before: Option<SaveId>,
    ) -> Result<Self, sqlx::Error> {
        let limit = limit.clamp(1, 100);
        let mut query = market_query(limit, save_type, before);
        let rows = query.build_query_as::<MarketRow>().fetch_all(db).await?;
        Ok(Self::from_rows(rows, usize::from(limit)))
    }

    fn from_rows(mut rows: Vec<MarketRow>, limit: usize) -> Self {
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let next_before = has_more
            .then(|| rows.last().map(|row| row.save_id as SaveId))
            .flatten();
        Self {
            records: rows.into_iter().map(Into::into).collect(),
            has_more,
            next_before,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RecordDetail {
    pub info: LastData,
    pub xml_status: String,
    pub raw_data: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{MarketPage, MarketRow, SaveType, market_query};
    use chrono::DateTime;

    fn row(save_id: i32) -> MarketRow {
        MarketRow {
            save_id,
            save_type: SaveType::Ship,
            len: 1,
            blake_hash: "hash".to_string(),
            xml_tested: Some(true),
            time: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    #[test]
    fn market_query_uses_exclusive_bigint_cursor_and_descending_ids() {
        let query = market_query(2, None, Some(u32::MAX));
        assert!(query.sql().as_str().contains("save_id >= 0"));
        assert!(query.sql().as_str().contains("save_type != $1"));
        assert!(query.sql().as_str().contains("save_id < $2::bigint"));
        assert!(
            query
                .sql()
                .as_str()
                .ends_with("ORDER BY save_id DESC LIMIT $3")
        );
    }

    #[test]
    fn market_query_binds_the_type_filter_without_a_cursor() {
        let query = market_query(48, Some(SaveType::Save), None);
        assert!(query.sql().as_str().contains("save_type = $1"));
        assert!(!query.sql().as_str().contains("save_id <"));
        assert!(
            query
                .sql()
                .as_str()
                .ends_with("ORDER BY save_id DESC LIMIT $2")
        );
    }

    #[test]
    fn next_page_cursor_uses_the_last_returned_row_not_the_lookahead_row() {
        let page = MarketPage::from_rows(vec![row(9), row(8), row(7)], 2);
        assert_eq!(
            page.records
                .iter()
                .map(|record| record.save_id)
                .collect::<Vec<_>>(),
            vec![9, 8]
        );
        assert!(page.has_more);
        assert_eq!(page.next_before, Some(8));
    }

    #[test]
    fn last_page_has_no_continuation_even_when_it_fills_the_limit() {
        let page = MarketPage::from_rows(vec![row(9), row(8)], 2);
        assert_eq!(page.records.len(), 2);
        assert!(!page.has_more);
        assert_eq!(page.next_before, None);
    }

    #[test]
    fn empty_page_has_no_continuation() {
        let page = MarketPage::from_rows(Vec::new(), 2);
        assert!(page.records.is_empty());
        assert!(!page.has_more);
        assert_eq!(page.next_before, None);
    }
}
