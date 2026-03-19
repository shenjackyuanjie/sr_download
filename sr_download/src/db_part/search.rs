use sqlx::{PgPool, Row};
use tracing::{Level, event};

use crate::db_part::SaveType;
use crate::db_part::defines::SaveId;

use super::DbData;

/// 找到最大的数据的 id
pub async fn max_id(db: &PgPool) -> SaveId {
    let data = sqlx::query(
        "SELECT save_id
         FROM main_data
         WHERE len > 0 AND save_type != $1
         ORDER BY save_id DESC
         LIMIT 1",
    )
    .bind(SaveType::None)
    .fetch_optional(db)
    .await;
    match data {
        Ok(Some(row)) => row.try_get::<i32, _>("save_id").map(|v| v as SaveId).unwrap_or(0),
        Ok(None) => 0,
        Err(e) => {
            event!(Level::WARN, "Error when find_max_id: {:?}", e);
            0
        }
    }
}

/// 找到最大的存档
pub async fn max_save(db: &PgPool) -> Option<DbData> {
    let data = sqlx::query(
        "SELECT save_id, save_type, blake_hash, len, short_data, xml_tested
         FROM main_data
         WHERE save_type = $1
         ORDER BY save_id DESC
         LIMIT 1",
    )
    .bind(SaveType::Save)
    .fetch_optional(db)
    .await;
    match data {
        Ok(Some(row)) => Some(DbData {
            text: row.try_get("short_data").ok()?,
            save_id: row.try_get::<i32, _>("save_id").ok()? as SaveId,
            save_type: row.try_get("save_type").ok()?,
            len: row.try_get("len").ok()?,
            blake_hash: row.try_get("blake_hash").ok()?,
            xml_tested: row.try_get::<Option<bool>, _>("xml_tested").ok()?.unwrap_or(false),
        }),
        Ok(None) => None,
        Err(e) => {
            event!(Level::WARN, "Error when find_max_save: {:?}", e);
            None
        }
    }
}

/// 找到最大的飞船
pub async fn max_ship(db: &PgPool) -> Option<DbData> {
    let data = sqlx::query(
        "SELECT save_id, save_type, blake_hash, len, short_data, xml_tested
         FROM main_data
         WHERE save_type = $1
         ORDER BY save_id DESC
         LIMIT 1",
    )
    .bind(SaveType::Ship)
    .fetch_optional(db)
    .await;
    match data {
        Ok(Some(row)) => Some(DbData {
            text: row.try_get("short_data").ok()?,
            save_id: row.try_get::<i32, _>("save_id").ok()? as SaveId,
            save_type: row.try_get("save_type").ok()?,
            len: row.try_get("len").ok()?,
            blake_hash: row.try_get("blake_hash").ok()?,
            xml_tested: row.try_get::<Option<bool>, _>("xml_tested").ok()?.unwrap_or(false),
        }),
        Ok(None) => None,
        Err(e) => {
            event!(Level::WARN, "Error when find_max_ship: {:?}", e);
            None
        }
    }
}
