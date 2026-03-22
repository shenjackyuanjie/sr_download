use colored::Colorize;
use quick_xml::{Reader, de::from_str, events::Event, name::QName};
use serde::Deserialize;
use sqlx::{
    Executor, PgPool, Row,
    postgres::{PgPoolOptions, PgRow},
};
use std::collections::HashSet;
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

pub async fn connect(conf: &ConfigFile) -> anyhow::Result<PgPool> {
    let search_path_sql = format!(
        "SET search_path TO {}",
        crate::db_part::defines::quote_ident(&conf.db.schema)
    );
    let after_connect_sql = search_path_sql.clone();
    event!(Level::INFO, "正在连接数据库");
    let db = PgPoolOptions::new()
        .max_connections(conf.db.max_connections)
        .after_connect(move |conn, _meta| {
            let sql = after_connect_sql.clone();
            Box::pin(async move {
                conn.execute(sql.as_str()).await?;
                Ok(())
            })
        })
        .connect(&conf.db.url)
        .await?;
    db.execute(search_path_sql.as_str()).await?;
    sqlx::query("SELECT 1").execute(&db).await?;
    event!(Level::INFO, "{}", "已经连接数据库".blue());
    Ok(db)
}

pub async fn connect_server(conf: &ConfigFile) -> anyhow::Result<PgPool> {
    let search_path_sql = format!(
        "SET search_path TO {}",
        crate::db_part::defines::quote_ident(&conf.db.schema)
    );
    let after_connect_sql = search_path_sql.clone();
    event!(Level::INFO, "服务器正在连接数据库");
    let db = PgPoolOptions::new()
        .max_connections(conf.serve.db_max_connect)
        .after_connect(move |conn, _meta| {
            let sql = after_connect_sql.clone();
            Box::pin(async move {
                conn.execute(sql.as_str()).await?;
                Ok(())
            })
        })
        .connect(&conf.db.url)
        .await?;
    db.execute(search_path_sql.as_str()).await?;
    sqlx::query("SELECT 1").execute(&db).await?;
    event!(Level::INFO, "{}", "服务器已经连接数据库".blue());
    Ok(db)
}

/// 更新数据库内所有 xml_tested = null 的数据
pub async fn update_xml_tested(db: &PgPool) -> Option<()> {
    let sql = r#"SELECT count(1)
	from full_data
	where xml_tested is NULL
	and len != 0
	and "save_type" != 'none'"#;
    let data: PgRow = sqlx::query(sql).fetch_one(db).await.ok()?;
    let count: i64 = data.try_get("count").ok()?;
    if count == 0 {
        event!(Level::INFO, "所有的 xml_tested 都已经更新过了");
        return Some(());
    }
    event!(Level::INFO, "正在检查 {} 条数据的xml状态", count);
    let sql = format!("SELECT {}()", db_names::UPDATE_XML_TESTED);
    event!(Level::INFO, "正在更新数据库内所有 xml_tested = null 的数据");
    let _ = db.execute(sql.as_str()).await;
    event!(Level::INFO, "已经更新数据库内所有 xml_tested = null 的数据");
    Some(())
}

/// 检查所有 data = null 的数据
/// 然后补全
pub async fn check_null_data(db: &PgPool) -> Option<()> {
    let sql = format!(
        "SELECT count(1) from {} where data is NULL",
        db_names::FULL_DATA_TABLE
    );
    let data: PgRow = sqlx::query(&sql).fetch_one(db).await.ok()?;
    let count: i64 = data.try_get("count").ok()?;
    if count == 0 {
        event!(Level::INFO, "数据库内数据都是完整的, 放心");
        return Some(());
    }
    event!(
        Level::WARN,
        "数据库内有 {} 条数据的 data 是空的, 正在更新",
        count
    );
    let sql = format!(
        "SELECT save_id from {} where data is NULL",
        db_names::FULL_DATA_TABLE
    );
    let quert_results = sqlx::query(&sql).fetch_all(db).await.ok()?;
    let downloader = crate::Downloader::new(None);
    for result in quert_results {
        let id: db_names::DbSaveId = result.try_get("save_id").ok()?;
        let id = id as SaveId;
        event!(Level::INFO, "正在补全id: {} 的数据", id);
        match downloader.try_download_as_any(id).await {
            Some(file) => {
                let save_type: SaveType = (&file).into();
                event!(Level::INFO, "成功下载id: {} 的数据 {}", id, file.info());
                match save_data_to_db(
                    id,
                    save_type,
                    file.take_data(),
                    Some(CoverStrategy::Cover),
                    db,
                )
                .await
                {
                    Ok(_) => {
                        event!(Level::INFO, "成功补全id: {} 的数据", id);
                    }
                    Err(e) => {
                        event!(
                            Level::ERROR,
                            "补全id: {} 的时候出现错误: {}, 将使用 Unknown 覆盖",
                            id,
                            e
                        );
                        let _ = save_data_to_db(
                            id,
                            SaveType::Unknown,
                            "",
                            Some(CoverStrategy::Cover),
                            db,
                        )
                        .await;
                    }
                }
            }
            None => {
                event!(Level::WARN, "尝试补全id: {} 的时候没下载到东西", id);
            }
        }
    }
    Some(())
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
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
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
    use super::{ShipVerifyState, verify_ship};

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
