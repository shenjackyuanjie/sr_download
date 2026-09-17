use anyhow::Context;
use colored::Colorize;
use quick_xml::{Reader, de::from_str, errors::IllFormedError, events::Event, name::QName};
use serde::Deserialize;
use sqlx::{
    PgPool, Row,
    postgres::{PgPoolOptions, PgRow},
};
use std::{collections::HashSet, time::Duration};
use tracing::{Level, event};

use crate::{
    config::ConfigFile,
    db_part::SaveType,
    db_part::{
        CoverStrategy,
        defines::{SaveId, db_names},
        save_data_to_db,
    },
};

/// 创建连接池，并在每条物理连接上设置 schema 与语句超时。
///
/// 连接池中的连接互不共享会话状态，所以 `search_path` 必须置于
/// `after_connect`；所有未限定 schema 的 SQL 才会持续落在配置指定的 schema。
fn pool_options(conf: &ConfigFile, max_connections: u32) -> PgPoolOptions {
    let search_path_sql = format!(
        "SET search_path TO {}",
        crate::db_part::defines::quote_ident(&conf.db.schema)
    );
    let after_connect_sql = search_path_sql.clone();
    let statement_timeout = format!("{}ms", conf.db.statement_timeout_secs.saturating_mul(1_000));

    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(conf.db.acquire_timeout_secs))
        .idle_timeout(Some(Duration::from_secs(conf.db.idle_timeout_secs)))
        .after_connect(move |conn, _meta| {
            let search_path_sql = after_connect_sql.clone();
            let statement_timeout = statement_timeout.clone();
            Box::pin(async move {
                sqlx::raw_sql(sqlx::AssertSqlSafe(search_path_sql))
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SELECT set_config('statement_timeout', $1, false)")
                    .bind(statement_timeout)
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
}

pub async fn connect(conf: &ConfigFile) -> anyhow::Result<PgPool> {
    event!(
        Level::INFO,
        max_connections = conf.db.max_connections,
        "正在连接数据库"
    );
    let db = pool_options(conf, conf.db.max_connections)
        .connect(&conf.db.url)
        .await
        .context("无法连接数据库")?;
    sqlx::query("SELECT 1").execute(&db).await?;
    event!(Level::INFO, "{}", "已经连接数据库".blue());
    Ok(db)
}

pub async fn connect_server(conf: &ConfigFile) -> anyhow::Result<PgPool> {
    event!(
        Level::INFO,
        max_connections = conf.serve.db_max_connect,
        "服务器正在连接数据库"
    );
    let db = pool_options(conf, conf.serve.db_max_connect)
        .connect(&conf.db.url)
        .await
        .context("服务器无法连接数据库")?;
    sqlx::query("SELECT 1")
        .execute(&db)
        .await
        .context("服务器数据库连接验证失败")?;
    event!(Level::INFO, "{}", "服务器已经连接数据库".blue());
    Ok(db)
}

// 维护按页顺序执行，避免放大上游下载请求和同一 save_id 的写锁竞争。
const MAINTENANCE_PAGE_SIZE: i64 = 100;

/// 有界维护任务在当前页完成后报告的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaintenanceBatchResult {
    Completed,
    Cancelled,
}

/// 仅在维护页之间检查停止信号，使当前已开始的页完整结束而不再开始下一页。
pub(crate) fn maintenance_stop_requested(
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> bool {
    stop_receiver.try_recv().is_ok()
}

/// 更新数据库内所有 xml_tested = null 的数据
pub async fn update_xml_tested(db: &PgPool) -> anyhow::Result<()> {
    let (_stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
    let _ = update_xml_tested_until_stopped(db, &mut stop_receiver).await?;
    Ok(())
}

pub(crate) async fn update_xml_tested_until_stopped(
    db: &PgPool,
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<MaintenanceBatchResult> {
    let mut last_save_id: Option<i32> = None;
    let mut updated_total = 0_i64;

    loop {
        if maintenance_stop_requested(stop_receiver) {
            event!(Level::INFO, updated_total, "已取消 xml_tested 维护");
            return Ok(MaintenanceBatchResult::Cancelled);
        }
        // 游标取自本页选中的最大 save_id；即使关联数据仍为空也会前进，
        // 因而不会把全量 ID 留在内存中或在无法更新的行上无限重试。
        let page: PgRow = sqlx::query(
            r#"
WITH page AS (
    SELECT save_id
    FROM main_data
    WHERE xml_tested IS NULL
      AND len != 0
      AND save_type != 'none'
      AND ($1::integer IS NULL OR save_id > $1)
    ORDER BY save_id
    LIMIT $2
),
updated AS (
    UPDATE main_data AS md
    SET xml_tested = xml_is_well_formed_document(fd.data)
    FROM full_data AS fd
    JOIN page AS p ON p.save_id = fd.save_id
    WHERE md.save_id = p.save_id
      AND fd.data IS NOT NULL
    RETURNING md.save_id
)
SELECT
    (SELECT count(*) FROM page)::bigint AS page_count,
    (SELECT max(save_id) FROM page) AS last_save_id,
    (SELECT count(*) FROM updated)::bigint AS updated_count
"#,
        )
        .bind(last_save_id)
        .bind(MAINTENANCE_PAGE_SIZE)
        .fetch_one(db)
        .await
        .context("无法读取 xml_tested 维护分页")?;

        let page_count: i64 = page
            .try_get("page_count")
            .context("无法读取 xml_tested 维护分页大小")?;
        if page_count == 0 {
            break;
        }
        last_save_id = Some(
            page.try_get("last_save_id")
                .context("无法读取 xml_tested 维护分页游标")?,
        );
        let updated_count: i64 = page
            .try_get("updated_count")
            .context("无法读取 xml_tested 更新数量")?;
        updated_total += updated_count;
        event!(
            Level::INFO,
            page_count,
            updated_count,
            last_save_id = ?last_save_id,
            "已处理 xml_tested 维护分页"
        );
    }

    event!(Level::INFO, updated_total, "xml_tested 维护完成");
    Ok(MaintenanceBatchResult::Completed)
}

/// 检查所有 data = null 的数据
/// 然后补全
pub async fn check_null_data(db: &PgPool) -> anyhow::Result<()> {
    let (_stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
    let _ = check_null_data_until_stopped(db, &mut stop_receiver).await?;
    Ok(())
}

pub(crate) async fn check_null_data_until_stopped(
    db: &PgPool,
    stop_receiver: &mut tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<MaintenanceBatchResult> {
    let mut last_save_id = None;
    let mut repaired = 0_u64;
    let mut failures = 0_u64;
    let downloader = crate::Downloader::new(None);

    loop {
        if maintenance_stop_requested(stop_receiver) {
            event!(Level::INFO, repaired, failures, "已取消缺失数据维护");
            return Ok(MaintenanceBatchResult::Cancelled);
        }
        // 使用 save_id 键集分页，内存中最多仅保留一个维护页的 ID。
        let save_ids = sqlx::query_scalar::<_, db_names::DbSaveId>(
            r#"
SELECT save_id
FROM full_data
WHERE data IS NULL
  AND ($1::integer IS NULL OR save_id > $1)
ORDER BY save_id
LIMIT $2
"#,
        )
        .bind(last_save_id)
        .bind(MAINTENANCE_PAGE_SIZE)
        .fetch_all(db)
        .await
        .context("无法读取缺失数据维护分页")?;
        if save_ids.is_empty() {
            break;
        }

        event!(
            Level::WARN,
            page_count = save_ids.len(),
            last_save_id = ?last_save_id,
            "正在补全缺失数据分页"
        );

        for db_save_id in save_ids {
            last_save_id = Some(db_save_id);
            let id = SaveId::try_from(db_save_id)
                .with_context(|| format!("数据库中存在无效的 save_id: {db_save_id}"))?;
            event!(Level::INFO, save_id = id, "正在补全缺失数据");
            // 只有成功下载到的内容才允许 Cover；下载或保存失败时保留原行，
            // 继续处理其余页面，最后返回汇总错误以暴露所有待人工处理的失败。
            match downloader.try_download_as_any(id).await {
                Some(file) => {
                    let save_type: SaveType = (&file).into();
                    event!(
                        Level::INFO,
                        save_id = id,
                        data_len = file.len(),
                        save_type = %save_type,
                        "成功下载缺失数据"
                    );
                    if let Err(error) = save_data_to_db(
                        id,
                        save_type,
                        file.take_data(),
                        Some(CoverStrategy::Cover),
                        db,
                    )
                    .await
                    {
                        failures += 1;
                        event!(
                            Level::ERROR,
                            save_id = id,
                            error = ?error,
                            "保存补全数据失败；保留原有数据"
                        );
                    } else {
                        repaired += 1;
                        event!(Level::INFO, save_id = id, "成功补全缺失数据");
                    }
                }
                None => {
                    failures += 1;
                    event!(Level::WARN, save_id = id, "无法下载补全数据；保留原有数据");
                }
            }
        }
    }

    if failures > 0 {
        anyhow::bail!("缺失数据维护未完成: repaired={repaired}, failures={failures}");
    }

    event!(Level::INFO, repaired, "缺失数据维护完成");
    Ok(MaintenanceBatchResult::Completed)
}

pub trait FromDb {
    fn from_db(db: &PgPool) -> impl std::future::Future<Output = Option<Self>> + Send
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipVerifyState {
    NotXml,
    NotShip,
    FakeShip,
    BrokenShip,
    VerifiedShip,
}

impl ShipVerifyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotXml => "not xml",
            Self::NotShip => "not ship",
            Self::FakeShip => "fake ship",
            Self::BrokenShip => "broken ship",
            Self::VerifiedShip => "verified ship",
        }
    }
}

impl std::fmt::Display for ShipVerifyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 校验一下是不是合法 xml
pub fn verify_xml(data: &str) -> quick_xml::Result<()> {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_comments = true;
    let mut open_elements = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                open_elements.push(element.name().as_ref().to_vec());
            }
            Ok(Event::End(_)) => {
                let _ = open_elements.pop();
            }
            Ok(Event::Eof) => {
                if let Some(element) = open_elements.last() {
                    return Err(quick_xml::Error::IllFormed(IllFormedError::MissingEndTag(
                        String::from_utf8_lossy(element).into_owned(),
                    )));
                }
                break;
            }
            Ok(_) => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn verify_ship(data: &str) -> ShipVerifyState {
    if verify_xml(data).is_err() {
        return ShipVerifyState::NotXml;
    }
    if !matches_basic_ship_shape(data) {
        return ShipVerifyState::NotShip;
    }
    match from_str::<RawShipForVerify>(data) {
        Ok(ship) => {
            if ship.is_semantically_valid() {
                ShipVerifyState::VerifiedShip
            } else {
                ShipVerifyState::BrokenShip
            }
        }
        Err(_) => ShipVerifyState::FakeShip,
    }
}

fn matches_basic_ship_shape(data: &str) -> bool {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);

    let mut depth = 0usize;
    let mut root_checked = false;
    let mut has_parts = false;
    let mut has_connections = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                if !root_checked {
                    root_checked = true;
                    if e.name() != QName(b"Ship") {
                        return false;
                    }
                } else if depth == 1 {
                    match e.name() {
                        QName(b"Parts") => has_parts = true,
                        QName(b"Connections") => has_connections = true,
                        _ => {}
                    }
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                if !root_checked {
                    return false;
                }
                if depth == 1 {
                    match e.name() {
                        QName(b"Parts") => has_parts = true,
                        QName(b"Connections") => has_connections = true,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return false,
        }
    }

    root_checked && has_parts && has_connections
}

fn default_ship_version() -> i32 {
    1
}
fn default_lifted_off() -> i8 {
    0
}
fn default_touching_ground() -> i8 {
    1
}
fn default_editor_angle() -> i32 {
    0
}

#[derive(Debug, Deserialize)]
#[serde(rename = "Ship")]
struct RawShipForVerify {
    #[serde(rename = "Parts")]
    #[allow(dead_code)]
    parts: VerifyParts,
    #[serde(rename = "Connections")]
    #[allow(dead_code)]
    connects: VerifyConnections,
    #[serde(rename = "@version", default = "default_ship_version")]
    #[allow(dead_code)]
    version: i32,
    #[serde(rename = "@liftedOff", default = "default_lifted_off")]
    #[allow(dead_code)]
    lift_off: i8,
    #[serde(rename = "@touchingGround", default = "default_touching_ground")]
    #[allow(dead_code)]
    touch_ground: i8,
    #[serde(rename = "DisconnectedParts")]
    #[allow(dead_code)]
    disconnected: VerifyDisconnectedParts,
}

impl RawShipForVerify {
    fn is_semantically_valid(&self) -> bool {
        let mut part_ids = HashSet::new();
        if !collect_part_ids(&self.parts.parts, &mut part_ids) {
            return false;
        }
        for disconnected in &self.disconnected.parts {
            if !collect_part_ids(&disconnected.parts.parts, &mut part_ids) {
                return false;
            }
        }

        connections_valid(&self.connects.connections, &part_ids)
            && normal_connections_acyclic(&self.connects.connections)
            && self.disconnected.parts.iter().all(|part| {
                connections_valid(&part.connects.connections, &part_ids)
                    && normal_connections_acyclic(&part.connects.connections)
            })
            && part_activations_valid(&self.parts.parts, &part_ids)
            && self
                .disconnected
                .parts
                .iter()
                .all(|part| part_activations_valid(&part.parts.parts, &part_ids))
    }
}

fn collect_part_ids(parts: &[VerifyPart], all_ids: &mut HashSet<i64>) -> bool {
    for part in parts {
        if !all_ids.insert(part.id) {
            return false;
        }
    }
    true
}

fn connections_valid(connections: &[VerifyConnection], part_ids: &HashSet<i64>) -> bool {
    let mut seen_normal = HashSet::new();
    let mut seen_dock = HashSet::new();

    connections.iter().all(|connection| match connection {
        VerifyConnection::Normal {
            parent_attach_point,
            child_attach_point,
            parent_part,
            child_part,
        } => {
            if !part_ids.contains(parent_part) || !part_ids.contains(child_part) {
                return false;
            }
            if parent_part == child_part {
                return false;
            }
            seen_normal.insert((
                *parent_attach_point,
                *child_attach_point,
                *parent_part,
                *child_part,
            ))
        }
        VerifyConnection::Dock {
            dock_part,
            parent_part,
            child_part,
        } => {
            if !part_ids.contains(dock_part)
                || !part_ids.contains(parent_part)
                || !part_ids.contains(child_part)
            {
                return false;
            }
            if parent_part == child_part || *dock_part == *parent_part || *dock_part == *child_part
            {
                return false;
            }
            seen_dock.insert((*dock_part, *parent_part, *child_part))
        }
    })
}

fn part_activations_valid(parts: &[VerifyPart], part_ids: &HashSet<i64>) -> bool {
    parts.iter().all(|part| {
        let Some(pod) = &part.pod else {
            return true;
        };
        pod.stages
            .steps
            .iter()
            .flat_map(|step| step.activates.iter())
            .all(|activate| part_ids.contains(&activate.id))
    })
}

fn normal_connections_acyclic(connections: &[VerifyConnection]) -> bool {
    let mut nodes = HashSet::new();
    let mut edges = Vec::new();

    for connection in connections {
        if let VerifyConnection::Normal {
            parent_part,
            child_part,
            ..
        } = connection
        {
            nodes.insert(*parent_part);
            nodes.insert(*child_part);
            edges.push((*parent_part, *child_part));
        }
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();

    nodes
        .into_iter()
        .all(|node| !has_cycle(node, &edges, &mut visiting, &mut visited))
}

fn has_cycle(
    node: i64,
    edges: &[(i64, i64)],
    visiting: &mut HashSet<i64>,
    visited: &mut HashSet<i64>,
) -> bool {
    if visited.contains(&node) {
        return false;
    }
    if !visiting.insert(node) {
        return true;
    }

    for (_, child) in edges.iter().filter(|(parent, _)| *parent == node) {
        if has_cycle(*child, edges, visiting, visited) {
            return true;
        }
    }

    visiting.remove(&node);
    visited.insert(node);
    false
}

#[derive(Debug, Deserialize)]
struct VerifyParts {
    #[serde(rename = "Part", default)]
    #[allow(dead_code)]
    parts: Vec<VerifyPart>,
}

#[derive(Debug, Deserialize)]
struct VerifyConnections {
    #[serde(rename = "$value", default)]
    #[allow(dead_code)]
    connections: Vec<VerifyConnection>,
}

#[derive(Debug, Deserialize)]
struct VerifyDisconnectedParts {
    #[serde(rename = "DisconnectedPart", default)]
    #[allow(dead_code)]
    parts: Vec<VerifyDisconnectedPart>,
}

#[derive(Debug, Deserialize)]
struct VerifyDisconnectedPart {
    #[serde(rename = "Parts")]
    #[allow(dead_code)]
    parts: VerifyParts,
    #[serde(rename = "Connections")]
    #[allow(dead_code)]
    connects: VerifyConnections,
}

#[derive(Debug, Deserialize)]
struct VerifyPart {
    #[serde(rename = "Tank")]
    #[allow(dead_code)]
    tank: Option<VerifyFuel>,
    #[serde(rename = "Engine")]
    #[allow(dead_code)]
    engine: Option<VerifyFuel>,
    #[serde(rename = "Pod")]
    #[allow(dead_code)]
    pod: Option<VerifyPod>,
    #[serde(rename = "@partType")]
    #[allow(dead_code)]
    part_type_id: String,
    #[serde(rename = "@id")]
    #[allow(dead_code)]
    id: i64,
    #[serde(rename = "@x")]
    #[allow(dead_code)]
    x: f64,
    #[serde(rename = "@y")]
    #[allow(dead_code)]
    y: f64,
    #[serde(rename = "@editorAngle", default = "default_editor_angle")]
    #[allow(dead_code)]
    editor_angle: i32,
    #[serde(rename = "@angle")]
    #[allow(dead_code)]
    angle: f64,
    #[serde(rename = "@angleV")]
    #[allow(dead_code)]
    angle_v: f64,
    #[serde(rename = "@flippedX")]
    #[allow(dead_code)]
    flip_x: Option<i8>,
    #[serde(rename = "@flippedY")]
    #[allow(dead_code)]
    flip_y: Option<i8>,
    #[serde(rename = "@chuteX")]
    #[allow(dead_code)]
    chute_x: Option<f64>,
    #[serde(rename = "@chuteY")]
    #[allow(dead_code)]
    chute_y: Option<f64>,
    #[serde(rename = "@chuteAngle")]
    #[allow(dead_code)]
    chute_angle: Option<f64>,
    #[serde(rename = "@chuteHeight")]
    #[allow(dead_code)]
    chute_height: Option<f64>,
    #[serde(rename = "@extension")]
    #[allow(dead_code)]
    extension: Option<f64>,
    #[serde(rename = "@inflate")]
    #[allow(dead_code)]
    inflate: Option<i8>,
    #[serde(rename = "@inflation")]
    #[allow(dead_code)]
    inflation: Option<f64>,
    #[serde(rename = "@exploded")]
    #[allow(dead_code)]
    exploded: Option<i8>,
    #[serde(rename = "@rope")]
    #[allow(dead_code)]
    rope: Option<i8>,
    #[serde(rename = "@activated")]
    #[allow(dead_code)]
    activated: Option<i8>,
    #[serde(rename = "@deployed")]
    #[allow(dead_code)]
    deployed: Option<i8>,
}

#[derive(Debug, Deserialize)]
struct VerifyFuel {
    #[serde(rename = "@fuel")]
    #[allow(dead_code)]
    fuel: f64,
}

#[derive(Debug, Deserialize)]
struct VerifyPod {
    #[serde(rename = "Staging")]
    #[allow(dead_code)]
    stages: VerifyStaging,
    #[serde(rename = "@name")]
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "@throttle")]
    #[allow(dead_code)]
    throttle: f64,
}

#[derive(Debug, Deserialize)]
struct VerifyStaging {
    #[serde(rename = "@currentStage")]
    #[allow(dead_code)]
    current_stage: i32,
    #[serde(rename = "Step", default)]
    #[allow(dead_code)]
    steps: Vec<VerifyStep>,
}

#[derive(Debug, Deserialize)]
struct VerifyStep {
    #[serde(rename = "Activate", default)]
    #[allow(dead_code)]
    activates: Vec<VerifyActivate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "Activate")]
struct VerifyActivate {
    #[serde(rename = "@Id")]
    #[allow(dead_code)]
    id: i64,
    #[serde(rename = "@moved")]
    #[allow(dead_code)]
    moved: i8,
}

#[derive(Debug, Deserialize)]
enum VerifyConnection {
    #[serde(rename = "Connection")]
    Normal {
        #[serde(rename = "@parentAttachPoint")]
        #[allow(dead_code)]
        parent_attach_point: i32,
        #[serde(rename = "@childAttachPoint")]
        #[allow(dead_code)]
        child_attach_point: i32,
        #[serde(rename = "@parentPart")]
        #[allow(dead_code)]
        parent_part: i64,
        #[serde(rename = "@childPart")]
        #[allow(dead_code)]
        child_part: i64,
    },
    #[serde(rename = "DockConnection")]
    Dock {
        #[serde(rename = "@dockPart")]
        #[allow(dead_code)]
        dock_part: i64,
        #[serde(rename = "@parentPart")]
        #[allow(dead_code)]
        parent_part: i64,
        #[serde(rename = "@childPart")]
        #[allow(dead_code)]
        child_part: i64,
    },
}

#[cfg(test)]
mod tests {
    use super::{ShipVerifyState, maintenance_stop_requested, verify_ship};

    #[test]
    fn detects_maintenance_stop_signal_between_pages() {
        let (stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel();
        assert!(!maintenance_stop_requested(&mut stop_receiver));
        stop_sender.send(()).unwrap();
        assert!(maintenance_stop_requested(&mut stop_receiver));
    }

    #[test]
    fn verify_ship_reports_not_xml() {
        assert_eq!(verify_ship("<Ship"), ShipVerifyState::NotXml);
    }

    #[test]
    fn verify_ship_reports_not_ship() {
        let xml = r#"<Runtime><Nodes /></Runtime>"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::NotShip);
    }

    #[test]
    fn verify_ship_reports_fake_ship() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part />
  </Parts>
  <Connections />
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::FakeShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0">
          <Step>
            <Activate Id="999" moved="0" />
          </Step>
        </Staging>
      </Pod>
    </Part>
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="0" parentPart="1" childPart="2" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship_for_self_connection() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0" />
      </Pod>
    </Part>
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="1" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship_for_duplicate_connection() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0" />
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="2" x="1" y="0" editorAngle="0" angle="0" angleV="0" />
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship_for_connection_cycle() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0" />
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="2" x="1" y="0" editorAngle="0" angle="0" angleV="0" />
    <Part partType="fueltank-1" id="3" x="2" y="0" editorAngle="0" angle="0" angleV="0" />
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="2" childPart="3" />
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="3" childPart="1" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_verified_ship() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="1">
          <Step>
            <Activate Id="2" moved="0" />
          </Step>
        </Staging>
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="2" x="1" y="0" editorAngle="0" angle="0" angleV="0" />
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::VerifiedShip);
    }
}
