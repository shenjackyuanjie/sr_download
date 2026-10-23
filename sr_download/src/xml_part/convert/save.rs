//! Runtime 与混合节点的值转换；保持节点顺序并复用船只转换。

use crate::xml_part::{model, raw};

use super::bool_to_i8;

impl From<raw::RawPlanetNode> for model::PlanetNode {
    fn from(value: raw::RawPlanetNode) -> Self {
        Self {
            name: value.name,
            true_anomaly: value.true_anomaly,
        }
    }
}

impl From<model::PlanetNode> for raw::RawPlanetNode {
    fn from(value: model::PlanetNode) -> Self {
        Self {
            name: value.name,
            true_anomaly: value.true_anomaly,
        }
    }
}

impl From<raw::RawShipNode> for model::ShipNode {
    fn from(value: raw::RawShipNode) -> Self {
        Self {
            id: value.id,
            planet: value.planet,
            planet_radius: value.planet_radius,
            x: value.x,
            y: value.y,
            vx: value.vx,
            vy: value.vy,
            ship: value.ship.into(),
        }
    }
}

impl From<model::ShipNode> for raw::RawShipNode {
    fn from(value: model::ShipNode) -> Self {
        Self {
            id: value.id,
            planet: value.planet,
            planet_radius: value.planet_radius,
            x: value.x,
            y: value.y,
            vx: value.vx,
            vy: value.vy,
            ship: value.ship.into(),
        }
    }
}

impl From<raw::RawNode> for model::SaveNode {
    fn from(value: raw::RawNode) -> Self {
        match value {
            raw::RawNode::Planet(node) => Self::Planet(node.into()),
            raw::RawNode::Ship(node) => Self::Ship(node.into()),
        }
    }
}

impl From<model::SaveNode> for raw::RawNode {
    fn from(value: model::SaveNode) -> Self {
        match value {
            model::SaveNode::Planet(node) => Self::Planet(node.into()),
            model::SaveNode::Ship(node) => Self::Ship(node.into()),
        }
    }
}

impl From<raw::RawSaveDocument> for model::SaveDocument {
    fn from(value: raw::RawSaveDocument) -> Self {
        Self {
            time: value.time,
            first_stage_activated: value.first_stage_activated != 0,
            solar_system: value.solar_system,
            ship_id: value.ship_id,
            pod_id: value.pod_id,
            nodes: value.nodes.nodes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<model::SaveDocument> for raw::RawSaveDocument {
    fn from(value: model::SaveDocument) -> Self {
        Self {
            time: value.time,
            first_stage_activated: bool_to_i8(value.first_stage_activated),
            solar_system: value.solar_system,
            ship_id: value.ship_id,
            pod_id: value.pod_id,
            nodes: raw::RawNodes {
                nodes: value.nodes.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<raw::RawSaveDocument> for model::XmlDocument {
    fn from(value: raw::RawSaveDocument) -> Self {
        Self::Save(value.into())
    }
}

// 存档借用式写入逐个遍历节点，嵌套船只复用船只容器的借用转换。
impl From<&model::ShipNode> for raw::RawShipNode {
    fn from(value: &model::ShipNode) -> Self {
        Self {
            id: value.id,
            planet: value.planet.clone(),
            planet_radius: value.planet_radius,
            x: value.x,
            y: value.y,
            vx: value.vx,
            vy: value.vy,
            ship: (&value.ship).into(),
        }
    }
}

impl From<&model::SaveNode> for raw::RawNode {
    fn from(value: &model::SaveNode) -> Self {
        match value {
            model::SaveNode::Planet(node) => Self::Planet(node.clone().into()),
            model::SaveNode::Ship(node) => Self::Ship(node.into()),
        }
    }
}

impl From<&model::SaveDocument> for raw::RawSaveDocument {
    fn from(value: &model::SaveDocument) -> Self {
        Self {
            time: value.time,
            first_stage_activated: bool_to_i8(value.first_stage_activated),
            solar_system: value.solar_system.clone(),
            ship_id: value.ship_id,
            pod_id: value.pod_id,
            nodes: raw::RawNodes {
                nodes: value.nodes.iter().map(Into::into).collect(),
            },
        }
    }
}
