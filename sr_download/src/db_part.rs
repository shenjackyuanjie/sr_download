use blake3::Hasher;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbErr, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
};
use tracing::{event, Level};

use crate::model::sea_orm_active_enums::SaveType;
use crate::{config::ConfigFile, SaveId};
use crate::{model, TEXT_DATA_MAX_LEN};
use migration::{Migrator, MigratorTrait, FULL_DATA_VIEW};

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
    let data: Result<Option<i32>, DbErr> = model::main_data::Entity::find()
        .order_by_desc(model::main_data::Column::SaveId)
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

/// 直接从数据库中查询数据, 这里数据库已经准备好了根据长度区分过的数据
/// 可以从 full view 里直接选数据
pub async fn get_raw_data(save_id: SaveId, db: &DatabaseConnection) -> Option<String> {
    let sql = format!(
        "SELECT data FROM {} WHERE save_id = {}",
        FULL_DATA_VIEW, save_id
    );
    db.query_one(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        sql,
    ))
    .await
    .ok()??
    .try_get_by_index(0)
    .ok()
    .flatten()
}

pub async fn check_data_len(db: &DatabaseConnection, save_id: SaveId) -> Option<i64> {
    // SELECT save_id from main_data WHERE save_id = $1 AND len > 0
    model::main_data::Entity::find()
        .filter(model::main_data::Column::SaveId.eq(save_id as i32))
        .select_only()
        .column(model::main_data::Column::Len)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten()
}

#[allow(unused)]
#[derive(Debug, Clone, Copy, Default)]
pub enum CoverStrategy {
    #[default]
    /// 如果数据已经存在, 则覆盖, 并返回 Ok(true)
    Cover,
    /// 如果现有数据不同, 则覆盖, 并返回 Ok(true)
    CoverIfDifferent,
    /// 如果数据已经存在, 则跳过, 并返回 Ok(false)
    Skip,
    /// 如果数据已经存在, 则报错, 并返回 anyhow::anyhow!("Data already exists")
    Error,
}

/// 保存船数据到数据库
/// 如果失败, 会返回 Err
/// 如果成功, 会返回 Ok(true)
/// 如果数据已经存在, 会根据策略返回
pub async fn save_data_to_db<T, D>(
    save_id: SaveId,
    save_type: T,
    data: D,
    cover_strategy: Option<CoverStrategy>,
    db: &DatabaseConnection,
) -> anyhow::Result<bool>
where
    D: Into<String>,
    T: Into<SaveType>,
{
    // 干活之前, 先检查一下数据是否已经存在
    // 如果已经存在, 那就根据策略来处理
    let cover_strategy = cover_strategy.unwrap_or_default();
    let save_type: SaveType = save_type.into();
    let exitst_data: Option<model::main_data::Model> = {
        model::main_data::Entity::find()
            .filter(model::main_data::Column::SaveId.eq(save_id as i32))
            .limit(1)
            .one(db)
            .await
            .ok()
            .flatten()
    };
    if exitst_data.is_some() {
        match cover_strategy {
            CoverStrategy::Error => return Err(anyhow::anyhow!("Data already exists")),
            CoverStrategy::Skip => return Ok(false),
            _ => (),
        }
    }

    let data: String = data.into();
    // 检查长度
    let data_len = data.len();
    // 计算 blake3 hash
    let mut hasher = Hasher::new();
    hasher.update(data.as_bytes());
    let hash = hasher.finalize().to_hex().to_string();

    if db.ping().await.is_err() {
        return Err(anyhow::anyhow!("Database connection is broken"));
    }
    let stuf = db.begin().await?;

    // 开个事务
    // 然后检测一下是否需要覆盖

    if exitst_data.is_some()
        && matches!(
            cover_strategy,
            CoverStrategy::Cover | CoverStrategy::CoverIfDifferent
        )
    {
        // 如果数据已经存在, 那就检查一下是否需要覆盖
        let exitst_data = exitst_data.unwrap();
        if exitst_data.blake_hash == hash
            && matches!(cover_strategy, CoverStrategy::CoverIfDifferent)
        {
            // 数据一样, 不需要覆盖
            stuf.commit().await?;
            return Ok(false);
        }
        // 数据不一致, 需要覆盖
        // 先删除旧数据
        if exitst_data.len > TEXT_DATA_MAX_LEN as i64 {
            // 长数据, 先删 long data
            model::long_data::Entity::delete_by_id(save_id as i32)
                .exec(db)
                .await?;
        }
        exitst_data.delete(db).await?;
    }

    if data_len > TEXT_DATA_MAX_LEN {
        // 过长, 需要把数据放到 long_data 里
        let new_data = model::main_data::Model {
            save_id: save_id as i32,
            save_type,
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
            save_type,
            blake_hash: hash,
            len: data_len as i64,
            short_data: Some(data),
        };
        new_data.into_active_model().insert(db).await?;
    }
    stuf.commit().await?;

    Ok(true)
}
