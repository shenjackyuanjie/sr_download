//! 控制舱与分级 wire 结构；保留 Step、Activate 的顺序及原数值标记。

use serde::{Deserialize, Serialize};

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
