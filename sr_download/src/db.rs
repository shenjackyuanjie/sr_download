use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait, IntoActiveModel, QueryOrder, QuerySelect, TransactionTrait
};
use blake3::Hasher;
use tracing::{event, Level};

use crate::{model, TEXT_DATA_MAX_LEN};
use crate::{config::ConfigFile, SaveId};
use migration::{Migrator, MigratorTrait};
use crate::model::sea_orm_active_enums::SaveType;

pub async fn connect(conf: &ConfigFile) -> anyhow::Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(conf.db_url.clone());
    opt.max_connections(conf.max_connections)
        .set_schema_search_path(conf.db_schema.clone())
        .sqlx_logging(conf.sqlx_logging);
    event!(Level::INFO, "Connecting to database");
    let db: DatabaseConnection = Database::connect(opt).await?;
    db.ping().await?;
    event!(Level::INFO, "Connected to database, starting migration");
    Migrator::up(&db, None).await?;
    event!(Level::INFO, "Migration finished");
    Ok(db)
}

pub async fn find_max_id(db: &DatabaseConnection) -> SaveId {
    // SELECT save_id from main_data ORDER BY save_id DESC LIMIT 1
    // 我丢你老母, 有这时间写这个, 我都写完 sql 语句了
    match model::main_data::Entity::find()
        .order_by_desc(model::main_data::Column::SaveId)
        .select_only()
        .column(model::main_data::Column::SaveId)
        .one(db)
        .await
    {
        Ok(model) => match model {
            Some(model) => model.save_id as SaveId,
            None => 0,
        },
        Err(_) => 0,
    }
}

pub async fn save_ship_to_db<D>(
    save_id: SaveId,
    data: D,
    db: &DatabaseConnection,
) -> anyhow::Result<()>
where
    D: Into<String>,
{
    let data: String = data.into();
    // 检查长度
    let data_len = data.len();
    // 计算 blake3 hash
    let mut hasher = Hasher::new();
    hasher.update(data.as_bytes());
    let hash = hasher.finalize().to_hex().to_string();

    // 开个事务
    let stuf = db.begin().await?;
    if data_len > TEXT_DATA_MAX_LEN {
        // 过长, 需要把数据放到 long_data 里
        let new_data = model::main_data::Model {
            save_id: save_id as i32,
            save_type: SaveType::Ship,
            blake_hash: hash,
            len: data_len as i64,
            short_data: None,
        };
        let long_data = model::long_data::Model {
            save_id: save_id as i32,
            len: data_len as i64,
            text: data,
        };
        // 先插入 new_data
        // 然后插入 long_data
        new_data.into_active_model().insert(db).await?;
        long_data.into_active_model().insert(db).await?;
    } else {
        // 直接放到 main_data 里即可
        let new_data = model::main_data::Model {
            save_id: save_id as i32,
            save_type: SaveType::Ship,
            blake_hash: hash,
            len: data_len as i64,
            short_data: Some(data),
        };
        new_data.into_active_model().insert(db).await?;
    }
    stuf.commit().await?;

    Ok(())
}
