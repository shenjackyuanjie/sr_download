//! 存档 Runtime 和有序混合节点的 wire 结构；复用船只 schema。

use serde::{Deserialize, Serialize};

use super::ship::RawShipDocument;

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
