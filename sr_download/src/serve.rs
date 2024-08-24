use axum::{extract::State, routing::get, Json, Router};
use sea_orm::{ActiveEnum, DatabaseConnection};
use serde::{Deserialize, Serialize};

use crate::{config::ConfigFile, db_part};
use migration::SaveId;

/// 最后一个数据的信息
#[derive(Serialize, Deserialize)]
pub struct LastData {
    pub save_id: SaveId,
    pub save_type: String,
    pub len: i64,
    pub blake_hash: String,
}

impl LastData {
    pub async fn from_db(db: &DatabaseConnection) -> Option<Self> {
        let id = db_part::find_max_id(db).await;
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

impl LastSave {
    pub async fn from_db(db: &DatabaseConnection) -> Option<Self> {
        let data = db_part::find_max_save(db).await?;
        Some(Self {
            save_id: data.save_id,
            len: data.len,
            blake_hash: data.blake_hash,
        })
    }
}

/// 最后一个飞船的信息
#[derive(Serialize, Deserialize)]
pub struct LastShip {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
}

impl LastShip {
    pub async fn from_db(db: &DatabaseConnection) -> Option<Self> {
        let data = db_part::find_max_ship(db).await?;
        Some(Self {
            save_id: data.save_id,
            len: data.len,
            blake_hash: data.blake_hash,
        })
    }
}

async fn get_last_data(State(db): State<DatabaseConnection>) -> Json<Option<LastData>> {
    Json(LastData::from_db(&db).await)
}

async fn get_last_save(State(db): State<DatabaseConnection>) -> Json<Option<LastSave>> {
    Json(LastSave::from_db(&db).await)
}

async fn get_last_ship(State(db): State<DatabaseConnection>) -> Json<Option<LastShip>> {
    Json(LastShip::from_db(&db).await)
}

pub async fn web_main() -> anyhow::Result<()> {
    let conf = crate::config::ConfigFile::try_read()?;
    
    let listener = tokio::net::TcpListener::bind(conf.serve.host_with_port.clone()).await?;
    let db = db_part::connect_server(&conf).await?;
    let app = Router::new()
        // get /last_data
        .route("/last_data", get(get_last_data))
        // get /last_save
        .route("/last_save", get(get_last_save))
        // get /last_ship
        .route("/last_ship", get(get_last_ship))
        // db
        .with_state(db);

    axum::serve(listener, app).await?;
    Ok(())
}
