//! 部件 wire 结构和可选燃料/状态字段；保持缺失字段与显式数值的区别。

use serde::{Deserialize, Serialize};

use super::staging::RawPod;

fn default_editor_angle() -> i32 {
    0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawParts {
    #[serde(rename = "Part", default)]
    pub parts: Vec<RawPart>,
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
