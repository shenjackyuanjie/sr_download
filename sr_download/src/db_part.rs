use blake3::Hasher;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, QueryFilter, QuerySelect, Statement, TransactionTrait,
};
use tracing::{Level, event};

use crate::config::ConfigFile;
use crate::model;
pub use crate::model::sea_orm_active_enums::SaveType;
use migration::{FULL_DATA_VIEW, SaveId, TEXT_DATA_MAX_LEN};

pub mod defines;
pub mod search;
pub mod updates;
pub mod utils;

pub use utils::{connect, connect_server, migrate};

pub async fn full_update(db: &DatabaseConnection, conf: &ConfigFile) {
    // sea_orm 的迁移
    if let Err(e) = migrate(db).await {
        event!(Level::ERROR, "sea_orm 迁移失败: {:?}", e);
    };

    // 自己的迁移
    updates::update_db(db, conf).await;

    // 数据更新
    utils::check_null_data(db).await;
    utils::update_xml_tested(db).await;
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct DbData {
    pub text: Option<String>,
    pub save_id: SaveId,
    pub save_type: SaveType,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

impl From<model::main_data::Model> for DbData {
    fn from(data: model::main_data::Model) -> Self {
        Self {
            text: data.short_data,
            save_id: data.save_id as SaveId,
            save_type: data.save_type,
            len: data.len,
            blake_hash: data.blake_hash,
            xml_tested: data.xml_tested.unwrap_or(false),
        }
    }
}

impl From<(model::main_data::Model, model::long_data::Model)> for DbData {
    fn from(data: (model::main_data::Model, model::long_data::Model)) -> Self {
        Self {
            text: Some(data.1.text),
            save_id: data.0.save_id as SaveId,
            save_type: data.0.save_type,
            len: data.0.len,
            blake_hash: data.0.blake_hash,
            xml_tested: data.0.xml_tested.unwrap_or(false),
        }
    }
}

#[allow(unused)]
/// 我承认, 这玩意大部分没用上(捂脸)
impl DbData {
    pub fn new(save_id: SaveId, data: String, save_type: SaveType) -> Self {
        let len = data.len() as i64;
        let mut hasher = Hasher::new();
        hasher.update(data.as_bytes());
        let hash = hasher.finalize().to_hex().to_string();
        let xml_tested = utils::verify_xml(&data).is_ok();
        Self {
            text: Some(data),
            save_id,
            save_type,
            len,
            blake_hash: hash,
            xml_tested,
        }
    }

    pub fn need_long_data(&self) -> bool {
        self.len > TEXT_DATA_MAX_LEN as i64 && self.text.is_none()
    }

    pub fn verify_hash(&self) -> bool {
        if self.text.is_none() {
            return false;
        }
        let text = self.text.as_ref().unwrap();
        let mut hasher = Hasher::new();
        hasher.update(text.as_bytes());
        let hash = hasher.finalize().to_hex().to_string();
        hash == self.blake_hash
    }

    pub fn verify_xml(&self) -> bool {
        if self.text.is_none() {
            return false;
        }
        utils::verify_xml(self.text.as_ref().unwrap()).is_ok()
    }

    pub fn xml_status(&self) -> String {
        if self.xml_tested {
            return "ok".to_string();
        }
        if self.text.is_none() {
            return "no data".to_string();
        }
        if let Err(e) = utils::verify_xml(self.text.as_ref().unwrap()) {
            return format!("error: {}", e);
        }
        "ok".to_string()
    }

    /// 直接从 full_data 里选即可
    pub async fn from_db(save_id: SaveId, db: &DatabaseConnection) -> Option<Self> {
        let sql = format!(
            r#"SELECT "data","save_id","save_type"::"varchar","len","blake_hash" FROM {} WHERE save_id = {}"#,
            FULL_DATA_VIEW, save_id
        );

        let datas = db
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                sql,
            ))
            .await
            .ok()??;
        let text = datas.try_get("", "data").ok()?;
        let save_id: i32 = datas.try_get("", "save_id").ok()?;
        let save_type: SaveType = datas.try_get("", "save_type").ok()?;
        let len: i64 = datas.try_get("", "len").ok()?;
        let blake_hash: String = datas.try_get("", "blake_hash").ok()?;
        let xml_tested: Option<bool> = datas.try_get("", "xml_tested").ok()?;
        Some(Self {
            text,
            save_id: save_id as SaveId,
            save_type,
            len,
            blake_hash,
            xml_tested: xml_tested.unwrap_or(false),
        })
    }
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
    let time = chrono::Utc::now();
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

    let xml_tested = Some(utils::verify_xml(&data).is_ok());

    if data_len > TEXT_DATA_MAX_LEN {
        // 过长, 需要把数据放到 long_data 里
        let new_data = model::main_data::Model {
            save_id: save_id as i32,
            save_type,
            blake_hash: hash,
            len: data_len as i64,
            short_data: None,
            xml_tested,
            time: time.into(),
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
            xml_tested,
            time: time.into(),
        };
        new_data.into_active_model().insert(db).await?;
    }
    stuf.commit().await?;

    Ok(true)
}
