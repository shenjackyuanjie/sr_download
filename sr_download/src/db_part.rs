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

/// 初始化正常下载和服务模式所需的数据库对象。
///
/// 为兼容外部调用方保留此旧名称；数据修复必须显式调用
/// [`run_maintenance`]，不会随正常启动执行。
pub async fn full_update(db: &PgPool, conf: &ConfigFile) -> anyhow::Result<()> {
    initialize_db(db, conf).await
}

pub async fn initialize_db(db: &PgPool, conf: &ConfigFile) -> anyhow::Result<()> {
    updates::update_db(db, conf).await
}

/// 运行显式、可能耗时较长的数据修复任务。
pub async fn run_maintenance(db: &PgPool, conf: &ConfigFile) -> anyhow::Result<()> {
    let (_stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
    match run_maintenance_until_stopped(db, conf, &mut stop_receiver).await? {
        MaintenanceOutcome::Completed => Ok(()),
        MaintenanceOutcome::Cancelled => unreachable!("未发送停止信号的维护任务不应被取消"),
    }
}

/// 可由调用方停止的维护任务的明确结束状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceOutcome {
    Completed,
    Cancelled,
}

/// 在维护页之间检查停止信号，取消时不再开始下一页或下一项维护任务。
pub async fn run_maintenance_until_stopped(
    db: &PgPool,
    conf: &ConfigFile,
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<MaintenanceOutcome> {
    initialize_db(db, conf).await?;
    if utils::maintenance_stop_requested(stop_receiver) {
        return Ok(MaintenanceOutcome::Cancelled);
    }
    if utils::check_null_data_until_stopped(db, stop_receiver).await?
        == utils::MaintenanceBatchResult::Cancelled
    {
        return Ok(MaintenanceOutcome::Cancelled);
    }
    if utils::update_xml_tested_until_stopped(db, stop_receiver).await?
        == utils::MaintenanceBatchResult::Cancelled
    {
        return Ok(MaintenanceOutcome::Cancelled);
    }
    Ok(MaintenanceOutcome::Completed)
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
        match self.save_type {
            SaveType::Ship => self.verify_ship().to_string(),
            SaveType::Save if self.xml_tested => "verified save".to_string(),
            SaveType::Save => "not xml".to_string(),
            SaveType::Unknown if self.xml_tested => "valid xml".to_string(),
            SaveType::Unknown => "not xml".to_string(),
            SaveType::None => "empty data".to_string(),
        }
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

#[cfg(test)]
mod xml_status_tests {
    use super::{DbData, SaveType};

    fn data(save_type: SaveType, text: &str) -> DbData {
        DbData::new(1, text.to_string(), save_type)
    }

    #[test]
    fn reports_valid_save_as_save_instead_of_not_ship() {
        assert_eq!(
            data(SaveType::Save, "<SaveState />").xml_status(),
            "verified save"
        );
    }

    #[test]
    fn reports_unknown_well_formed_document_as_valid_xml() {
        assert_eq!(
            data(SaveType::Unknown, "<Document />").xml_status(),
            "valid xml"
        );
    }

    #[test]
    fn reports_malformed_non_ship_document() {
        assert_eq!(data(SaveType::Save, "<SaveState>").xml_status(), "not xml");
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveAction {
    Write,
    Skip,
}

fn select_save_action(
    existing_hash: Option<&str>,
    new_hash: &str,
    cover_strategy: CoverStrategy,
) -> anyhow::Result<SaveAction> {
    let Some(existing_hash) = existing_hash else {
        return Ok(SaveAction::Write);
    };

    match cover_strategy {
        CoverStrategy::Cover => Ok(SaveAction::Write),
        CoverStrategy::CoverIfDifferent if existing_hash == new_hash => Ok(SaveAction::Skip),
        CoverStrategy::CoverIfDifferent => Ok(SaveAction::Write),
        CoverStrategy::Skip => Ok(SaveAction::Skip),
        CoverStrategy::Error => Err(anyhow::anyhow!("Data already exists")),
    }
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
    let cover_strategy = cover_strategy.unwrap_or_default();
    let time = chrono::Utc::now();
    let save_type: SaveType = save_type.into();
    let data: String = data.into();
    let data_len = i64::try_from(data.len()).map_err(|_| anyhow::anyhow!("Data is too large"))?;
    let mut hasher = Hasher::new();
    hasher.update(data.as_bytes());
    let hash = hasher.finalize().to_hex().to_string();
    let xml_tested = Some(utils::verify_xml(&data).is_ok());

    let mut tx = db.begin().await?;
    // 事务级锁只按 save_id 分组，因此连主记录尚不存在的竞争也会串行化；
    // 随后的已存记录查询、策略判断和写入必须留在同一事务中。
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(i64::from(save_id))
        .execute(&mut *tx)
        .await?;

    let existing_hash = sqlx::query_scalar::<_, String>(
        "SELECT blake_hash
         FROM main_data
         WHERE save_id = $1",
    )
    .bind(save_id as i32)
    .fetch_optional(&mut *tx)
    .await?;

    if select_save_action(existing_hash.as_deref(), &hash, cover_strategy)? == SaveAction::Skip {
        tx.commit().await?;
        return Ok(false);
    }

    // 主表始终先 UPSERT，既不删除主键也不触发外键级联；随后在同一事务内
    // 写入或清理从表，使提交前外部连接永远看不到短/长数据切换的中间状态。
    if data_len > TEXT_DATA_MAX_LEN as i64 {
        sqlx::query(
            "INSERT INTO main_data
             (save_id, save_type, blake_hash, len, short_data, xml_tested, time)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (save_id) DO UPDATE
             SET save_type = EXCLUDED.save_type,
                 blake_hash = EXCLUDED.blake_hash,
                 len = EXCLUDED.len,
                 short_data = EXCLUDED.short_data,
                 xml_tested = EXCLUDED.xml_tested,
                 time = EXCLUDED.time",
        )
        .bind(save_id as i32)
        .bind(save_type)
        .bind(&hash)
        .bind(data_len)
        .bind(Option::<String>::None)
        .bind(xml_tested)
        .bind(time)
        .execute(&mut *tx)
        .await?;
        // 长数据以同一 save_id UPSERT，避免并发覆盖时产生从表唯一键冲突。
        sqlx::query(
            "INSERT INTO long_data (save_id, len, text)
             VALUES ($1, $2, $3)
             ON CONFLICT (save_id) DO UPDATE
             SET len = EXCLUDED.len,
                 text = EXCLUDED.text",
        )
        .bind(save_id as i32)
        .bind(data_len)
        .bind(data)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO main_data
             (save_id, save_type, blake_hash, len, short_data, xml_tested, time)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (save_id) DO UPDATE
             SET save_type = EXCLUDED.save_type,
                 blake_hash = EXCLUDED.blake_hash,
                 len = EXCLUDED.len,
                 short_data = EXCLUDED.short_data,
                 xml_tested = EXCLUDED.xml_tested,
                 time = EXCLUDED.time",
        )
        .bind(save_id as i32)
        .bind(save_type)
        .bind(hash)
        .bind(data_len)
        .bind(Some(data))
        .bind(xml_tested)
        .bind(time)
        .execute(&mut *tx)
        .await?;
        // 从长数据切回短数据时只删除已不适用的从表行，保留 main_data 主键及其外键语义。
        sqlx::query("DELETE FROM long_data WHERE save_id = $1")
            .bind(save_id as i32)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(true)
}

#[cfg(test)]
mod save_strategy_tests {
    use super::{CoverStrategy, SaveAction, select_save_action};

    const OLD_HASH: &str = "old";
    const NEW_HASH: &str = "new";

    #[test]
    fn writes_when_no_data_exists_regardless_of_strategy() {
        for strategy in [
            CoverStrategy::Cover,
            CoverStrategy::CoverIfDifferent,
            CoverStrategy::Skip,
            CoverStrategy::Error,
        ] {
            assert_eq!(
                select_save_action(None, NEW_HASH, strategy).unwrap(),
                SaveAction::Write
            );
        }
    }

    #[test]
    fn applies_each_existing_data_strategy() {
        assert_eq!(
            select_save_action(Some(OLD_HASH), NEW_HASH, CoverStrategy::Cover).unwrap(),
            SaveAction::Write
        );
        assert_eq!(
            select_save_action(Some(OLD_HASH), NEW_HASH, CoverStrategy::CoverIfDifferent).unwrap(),
            SaveAction::Write
        );
        assert_eq!(
            select_save_action(Some(OLD_HASH), NEW_HASH, CoverStrategy::Skip).unwrap(),
            SaveAction::Skip
        );
        assert!(select_save_action(Some(OLD_HASH), NEW_HASH, CoverStrategy::Error).is_err());
    }

    #[test]
    fn cover_if_different_skips_identical_data() {
        assert_eq!(
            select_save_action(Some(OLD_HASH), OLD_HASH, CoverStrategy::CoverIfDifferent).unwrap(),
            SaveAction::Skip
        );
    }
}
