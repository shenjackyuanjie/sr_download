use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tracing::{event, Level};

use crate::model;
use crate::model::sea_orm_active_enums::SaveType;
use migration::SaveId;

use super::DbData;

/// 找到最大的数据的 id
pub async fn max_id(db: &DatabaseConnection) -> SaveId {
    // SELECT save_id from main_data ORDER BY save_id DESC LIMIT 1
    // 我丢你老母, 有这时间写这个, 我都写完 sql 语句了
    let data: Result<Option<i32>, DbErr> = model::main_data::Entity::find()
        .order_by_desc(model::main_data::Column::SaveId)
        .filter(model::main_data::Column::Len.gt(0))
        .filter(model::main_data::Column::SaveType.ne(SaveType::None))
        .select_only()
        .column(model::main_data::Column::SaveId)
        .limit(1)
        .into_tuple()
        .one(db)
        .await;
    match data {
        Ok(model) => match model {
            Some(model) => model as SaveId,
            None => 0,
        },
        Err(e) => {
            event!(Level::WARN, "Error when find_max_id: {:?}", e);
            0
        }
    }
}

/// 找到最大的存档
pub async fn max_save(db: &DatabaseConnection) -> Option<DbData> {
    // SELECT * from main_data ORDER BY save_id DESC WHERE save_type = save LIMIT 1
    let data = model::main_data::Entity::find()
        .order_by_desc(model::main_data::Column::SaveId)
        .filter(model::main_data::Column::SaveType.eq(SaveType::Save))
        .one(db)
        .await;
    match data {
        Ok(model) => model.map(|model| model.into()),
        Err(e) => {
            event!(Level::WARN, "Error when find_max_save: {:?}", e);
            None
        }
    }
}

/// 找到最大的飞船
pub async fn max_ship(db: &DatabaseConnection) -> Option<DbData> {
    // SELECT * from main_data ORDER BY save_id DESC WHERE save_type = ship LIMIT 1
    let data = model::main_data::Entity::find()
        .order_by_desc(model::main_data::Column::SaveId)
        .filter(model::main_data::Column::SaveType.eq(SaveType::Ship))
        .one(db)
        .await;
    match data {
        Ok(model) => model.map(|model| model.into()),
        Err(e) => {
            event!(Level::WARN, "Error when find_max_ship: {:?}", e);
            None
        }
    }
}
