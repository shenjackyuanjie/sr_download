//! 船只 wire 结构、连接和分离部件组；只声明既有 XML schema 与默认值。

use serde::{Deserialize, Serialize};

use super::part::RawParts;

fn default_ship_version() -> i32 {
    1
}
fn default_lifted_off() -> i8 {
    0
}
fn default_touching_ground() -> i8 {
    1
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
