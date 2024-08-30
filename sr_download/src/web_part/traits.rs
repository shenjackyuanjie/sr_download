use sea_orm::{ActiveEnum, DatabaseConnection};

use super::{LastData, LastSave, LastShip};
use crate::db_part::{self, DbData};
use crate::db_part::utils::FromDb;

impl FromDb for LastData {
    async fn from_db(db: &DatabaseConnection) -> Option<Self> {
        let id = db_part::search::max_id(db).await;
        let data = DbData::from_db(id, db).await?;
        Some(Self {
            save_id: data.save_id,
            save_type: data.save_type.to_value().to_string(),
            len: data.len,
            blake_hash: data.blake_hash,
        })
    }
}

impl FromDb for LastSave {
    async fn from_db(db: &DatabaseConnection) -> Option<Self> {
        let data = db_part::search::max_save(db).await?;
        Some(Self {
            save_id: data.save_id,
            len: data.len,
            blake_hash: data.blake_hash,
        })
    }
}

impl FromDb for LastShip {
    async fn from_db(db: &DatabaseConnection) -> Option<Self> {
        let data = db_part::search::max_ship(db).await?;
        Some(Self {
            save_id: data.save_id,
            len: data.len,
            blake_hash: data.blake_hash,
        })
    }
}
