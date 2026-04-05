use blake3::Hasher;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::config::ConfigFile;
use crate::xml_part::{XmlResult, model::SaveDocument, model::ShipDocument, model::XmlDocument};
pub use defines::{SaveId, TEXT_DATA_MAX_LEN};

pub mod defines;
pub mod search;
pub mod updates;
pub mod utils;

pub use utils::{connect, connect_server};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "save_type", rename_all = "lowercase")]
pub enum SaveType {
    None,
    Save,
    Ship,
    Unknown,
}

impl SaveType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Save => "save",
            Self::Ship => "ship",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for SaveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub async fn full_update(db: &PgPool, conf: &ConfigFile) {
    updates::update_db(db, conf).await;
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

#[derive(Debug, FromRow)]
struct FullDataRow {
    data: Option<String>,
    save_id: i32,
    save_type: SaveType,
    len: i64,
    blake_hash: String,
    xml_tested: Option<bool>,
}

#[derive(Debug, FromRow)]
struct ExistingMainDataRow {
    save_id: i32,
    save_type: SaveType,
    blake_hash: String,
    len: i64,
    short_data: Option<String>,
    xml_tested: Option<bool>,
}

impl From<FullDataRow> for DbData {
    fn from(data: FullDataRow) -> Self {
        Self {
            text: data.data,
            save_id: data.save_id as SaveId,
            save_type: data.save_type,
            len: data.len,
            blake_hash: data.blake_hash,
            xml_tested: data.xml_tested.unwrap_or(false),
        }
    }
}

impl From<ExistingMainDataRow> for DbData {
    fn from(data: ExistingMainDataRow) -> Self {
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

    pub fn verify_ship(&self) -> utils::ShipVerifyState {
        let Some(text) = self.text.as_ref() else {
            return utils::ShipVerifyState::NotShip;
        };
        utils::verify_ship(text)
    }

    pub fn parse_xml(&self) -> XmlResult<XmlDocument> {
        let Some(text) = self.text.as_ref() else {
            return Err(crate::xml_part::XmlError::UnsupportedRoot(
                "<missing data>".to_string(),
            ));
        };
        crate::xml_part::parse::parse_any_xml(text)
    }

    pub fn parse_ship_xml(&self) -> XmlResult<ShipDocument> {
        match self.parse_xml()? {
            XmlDocument::Ship(doc) => Ok(doc),
            XmlDocument::Save(_) => Err(crate::xml_part::XmlError::UnexpectedDocumentType {
                expected: "Ship",
                found: "Save",
            }),
        }
    }

    pub fn parse_save_xml(&self) -> XmlResult<SaveDocument> {
        match self.parse_xml()? {
            XmlDocument::Save(doc) => Ok(doc),
            XmlDocument::Ship(_) => Err(crate::xml_part::XmlError::UnexpectedDocumentType {
                expected: "Save",
                found: "Ship",
            }),
        }
    }

    pub fn xml_status(&self) -> String {
        self.verify_ship().to_string()
    }

    /// 直接从 full_data 里选即可
    pub async fn from_db(save_id: SaveId, db: &PgPool) -> Option<Self> {
        sqlx::query_as::<_, FullDataRow>(
            "SELECT data, save_id, save_type, len, blake_hash, xml_tested
             FROM full_data
             WHERE save_id = $1",
        )
        .bind(save_id as i32)
        .fetch_optional(db)
        .await
        .ok()?
        .map(Into::into)
    }
}

pub async fn check_data_len(db: &PgPool, save_id: SaveId) -> Option<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT len
         FROM main_data
         WHERE save_id = $1
         LIMIT 1",
    )
    .bind(save_id as i32)
    .fetch_optional(db)
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
    db: &PgPool,
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
    let exitst_data = sqlx::query_as::<_, ExistingMainDataRow>(
        "SELECT save_id, save_type, blake_hash, len, short_data, xml_tested
         FROM main_data
         WHERE save_id = $1
         LIMIT 1",
    )
    .bind(save_id as i32)
    .fetch_optional(db)
    .await?;
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

    sqlx::query("SELECT 1").execute(db).await?;
    let mut tx = db.begin().await?;

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
            tx.commit().await?;
            return Ok(false);
        }
        if exitst_data.len > TEXT_DATA_MAX_LEN as i64 {
            sqlx::query("DELETE FROM long_data WHERE save_id = $1")
                .bind(save_id as i32)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM main_data WHERE save_id = $1")
            .bind(save_id as i32)
            .execute(&mut *tx)
            .await?;
    }

    let xml_tested = Some(utils::verify_xml(&data).is_ok());

    if data_len > TEXT_DATA_MAX_LEN {
        sqlx::query(
            "INSERT INTO main_data
             (save_id, save_type, blake_hash, len, short_data, xml_tested, time)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(save_id as i32)
        .bind(save_type)
        .bind(&hash)
        .bind(data_len as i64)
        .bind(Option::<String>::None)
        .bind(xml_tested)
        .bind(time)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO long_data (save_id, len, text) VALUES ($1, $2, $3)")
            .bind(save_id as i32)
            .bind(data_len as i64)
            .bind(data)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "INSERT INTO main_data
             (save_id, save_type, blake_hash, len, short_data, xml_tested, time)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(save_id as i32)
        .bind(save_type)
        .bind(hash)
        .bind(data_len as i64)
        .bind(Some(data))
        .bind(xml_tested)
        .bind(time)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(true)
}
