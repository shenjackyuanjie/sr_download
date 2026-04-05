use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

use crate::xml_part::error::XmlResult;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Ship")]
pub struct RawShipDocument {
    #[serde(rename = "Parts")]
    pub parts: RawParts,
    #[serde(rename = "Connections")]
    pub connections: RawConnections,
    #[serde(rename = "@version", default = "default_ship_version")]
    pub version: i32,
    #[serde(rename = "@liftedOff", default = "default_lifted_off")]
    pub lifted_off: i8,
    #[serde(rename = "@touchingGround", default = "default_touching_ground")]
    pub touching_ground: i8,
    #[serde(rename = "DisconnectedParts", default)]
    pub disconnected: RawDisconnectedParts,
}

impl RawShipDocument {
    pub fn from_str(data: &str) -> XmlResult<Self> {
        Ok(from_str(data)?)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawParts {
    #[serde(rename = "Part", default)]
    pub parts: Vec<RawPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawConnections {
    #[serde(rename = "$value", default)]
    pub connections: Vec<RawConnection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawDisconnectedParts {
    #[serde(rename = "DisconnectedPart", default)]
    pub parts: Vec<RawDisconnectedPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawDisconnectedPart {
    #[serde(rename = "Parts")]
    pub parts: RawParts,
    #[serde(rename = "Connections")]
    pub connections: RawConnections,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPart {
    #[serde(rename = "Tank", skip_serializing_if = "Option::is_none")]
    pub tank: Option<RawFuelTank>,
    #[serde(rename = "Engine", skip_serializing_if = "Option::is_none")]
    pub engine: Option<RawFuelEngine>,
    #[serde(rename = "Pod", skip_serializing_if = "Option::is_none")]
    pub pod: Option<RawPod>,
    #[serde(rename = "@partType")]
    pub part_type_id: String,
    #[serde(rename = "@id")]
    pub id: i64,
    #[serde(rename = "@x")]
    pub x: f64,
    #[serde(rename = "@y")]
    pub y: f64,
    #[serde(rename = "@editorAngle", default = "default_editor_angle")]
    pub editor_angle: i32,
    #[serde(rename = "@angle")]
    pub angle: f64,
    #[serde(rename = "@angleV")]
    pub angle_v: f64,
    #[serde(rename = "@flippedX", skip_serializing_if = "Option::is_none")]
    pub flipped_x: Option<i8>,
    #[serde(rename = "@flippedY", skip_serializing_if = "Option::is_none")]
    pub flipped_y: Option<i8>,
    #[serde(rename = "@activated", skip_serializing_if = "Option::is_none")]
    pub activated: Option<i8>,
    #[serde(rename = "@exploded", skip_serializing_if = "Option::is_none")]
    pub exploded: Option<i8>,
    #[serde(rename = "@chuteX", skip_serializing_if = "Option::is_none")]
    pub chute_x: Option<f64>,
    #[serde(rename = "@chuteY", skip_serializing_if = "Option::is_none")]
    pub chute_y: Option<f64>,
    #[serde(rename = "@chuteAngle", skip_serializing_if = "Option::is_none")]
    pub chute_angle: Option<f64>,
    #[serde(rename = "@chuteHeight", skip_serializing_if = "Option::is_none")]
    pub chute_height: Option<f64>,
    #[serde(rename = "@extension", skip_serializing_if = "Option::is_none")]
    pub extension: Option<f64>,
    #[serde(rename = "@inflate", skip_serializing_if = "Option::is_none")]
    pub inflate: Option<i8>,
    #[serde(rename = "@inflation", skip_serializing_if = "Option::is_none")]
    pub inflation: Option<f64>,
    #[serde(rename = "@deployed", skip_serializing_if = "Option::is_none")]
    pub deployed: Option<i8>,
    #[serde(rename = "@rope", skip_serializing_if = "Option::is_none")]
    pub rope: Option<i8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawFuelTank {
    #[serde(rename = "@fuel")]
    pub fuel: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawFuelEngine {
    #[serde(rename = "@fuel")]
    pub fuel: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPod {
    #[serde(rename = "Staging")]
    pub staging: RawStaging,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@throttle")]
    pub throttle: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawStaging {
    #[serde(rename = "@currentStage")]
    pub current_stage: i32,
    #[serde(rename = "Step", default)]
    pub steps: Vec<RawStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawStep {
    #[serde(rename = "Activate", default)]
    pub activates: Vec<RawActivate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Activate")]
pub struct RawActivate {
    #[serde(rename = "@Id")]
    pub id: i64,
    #[serde(rename = "@moved")]
    pub moved: i8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawConnection {
    #[serde(rename = "Connection")]
    Normal {
        #[serde(rename = "@parentAttachPoint")]
        parent_attach_point: i32,
        #[serde(rename = "@childAttachPoint")]
        child_attach_point: i32,
        #[serde(rename = "@parentPart")]
        parent_part: i64,
        #[serde(rename = "@childPart")]
        child_part: i64,
    },
    #[serde(rename = "DockConnection")]
    Dock {
        #[serde(rename = "@dockPart")]
        dock_part: i64,
        #[serde(rename = "@parentPart")]
        parent_part: i64,
        #[serde(rename = "@childPart")]
        child_part: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Runtime")]
pub struct RawSaveDocument {
    #[serde(rename = "@time")]
    pub time: f64,
    #[serde(rename = "@firstStageActivated")]
    pub first_stage_activated: i8,
    #[serde(rename = "@solarSystem")]
    pub solar_system: String,
    #[serde(rename = "@shipId")]
    pub ship_id: i64,
    #[serde(rename = "@podId")]
    pub pod_id: i64,
    #[serde(rename = "Nodes")]
    pub nodes: RawNodes,
}

impl RawSaveDocument {
    pub fn from_str(data: &str) -> XmlResult<Self> {
        Ok(from_str(data)?)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawNodes {
    #[serde(rename = "$value", default)]
    pub nodes: Vec<RawNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawNode {
    #[serde(rename = "PlanetNode")]
    Planet(RawPlanetNode),
    #[serde(rename = "ShipNode")]
    Ship(RawShipNode),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPlanetNode {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@trueAnomaly", skip_serializing_if = "Option::is_none")]
    pub true_anomaly: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawShipNode {
    #[serde(rename = "@id")]
    pub id: i64,
    #[serde(rename = "@planet")]
    pub planet: String,
    #[serde(rename = "@planetRadius")]
    pub planet_radius: f64,
    #[serde(rename = "@x")]
    pub x: f64,
    #[serde(rename = "@y")]
    pub y: f64,
    #[serde(rename = "@vx")]
    pub vx: f64,
    #[serde(rename = "@vy")]
    pub vy: f64,
    #[serde(rename = "Ship")]
    pub ship: RawShipDocument,
}
