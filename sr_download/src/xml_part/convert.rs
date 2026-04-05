use crate::xml_part::{model, raw};

fn i8_to_bool(value: i8) -> bool {
    value != 0
}
fn bool_to_i8(value: bool) -> i8 {
    if value { 1 } else { 0 }
}

impl From<raw::RawActivate> for model::Activation {
    fn from(value: raw::RawActivate) -> Self {
        Self {
            id: value.id,
            moved: i8_to_bool(value.moved),
        }
    }
}

impl From<model::Activation> for raw::RawActivate {
    fn from(value: model::Activation) -> Self {
        Self {
            id: value.id,
            moved: bool_to_i8(value.moved),
        }
    }
}

impl From<raw::RawStep> for model::StageStep {
    fn from(value: raw::RawStep) -> Self {
        Self {
            activates: value.activates.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<model::StageStep> for raw::RawStep {
    fn from(value: model::StageStep) -> Self {
        Self {
            activates: value.activates.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<raw::RawPod> for model::PodData {
    fn from(value: raw::RawPod) -> Self {
        Self {
            name: value.name,
            throttle: value.throttle,
            current_stage: value.staging.current_stage,
            steps: value.staging.steps.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<model::PodData> for raw::RawPod {
    fn from(value: model::PodData) -> Self {
        Self {
            staging: raw::RawStaging {
                current_stage: value.current_stage,
                steps: value.steps.into_iter().map(Into::into).collect(),
            },
            name: value.name,
            throttle: value.throttle,
        }
    }
}

impl From<raw::RawPart> for model::Part {
    fn from(value: raw::RawPart) -> Self {
        Self {
            part_type_id: value.part_type_id,
            id: value.id,
            x: value.x,
            y: value.y,
            editor_angle: value.editor_angle,
            angle: value.angle,
            angle_v: value.angle_v,
            flipped_x: value.flipped_x.unwrap_or(0) != 0,
            flipped_y: value.flipped_y.unwrap_or(0) != 0,
            activated: value.activated.unwrap_or(0) != 0,
            exploded: value.exploded.unwrap_or(0) != 0,
            attrs: model::PartAttrs {
                tank_fuel: value.tank.map(|tank| tank.fuel),
                engine_fuel: value.engine.map(|engine| engine.fuel),
                pod: value.pod.map(Into::into),
                chute_x: value.chute_x,
                chute_y: value.chute_y,
                chute_angle: value.chute_angle,
                chute_height: value.chute_height,
                extension: value.extension,
                inflate: value.inflate.map(|v| v != 0),
                inflation: value.inflation,
                deployed: value.deployed.map(|v| v != 0),
                rope: value.rope.map(|v| v != 0),
            },
        }
    }
}

impl From<model::Part> for raw::RawPart {
    fn from(value: model::Part) -> Self {
        Self {
            tank: value.attrs.tank_fuel.map(|fuel| raw::RawFuelTank { fuel }),
            engine: value
                .attrs
                .engine_fuel
                .map(|fuel| raw::RawFuelEngine { fuel }),
            pod: value.attrs.pod.map(Into::into),
            part_type_id: value.part_type_id,
            id: value.id,
            x: value.x,
            y: value.y,
            editor_angle: value.editor_angle,
            angle: value.angle,
            angle_v: value.angle_v,
            flipped_x: Some(bool_to_i8(value.flipped_x)),
            flipped_y: Some(bool_to_i8(value.flipped_y)),
            activated: Some(bool_to_i8(value.activated)),
            exploded: Some(bool_to_i8(value.exploded)),
            chute_x: value.attrs.chute_x,
            chute_y: value.attrs.chute_y,
            chute_angle: value.attrs.chute_angle,
            chute_height: value.attrs.chute_height,
            extension: value.attrs.extension,
            inflate: value.attrs.inflate.map(bool_to_i8),
            inflation: value.attrs.inflation,
            deployed: value.attrs.deployed.map(bool_to_i8),
            rope: value.attrs.rope.map(bool_to_i8),
        }
    }
}

impl From<raw::RawConnection> for model::Connection {
    fn from(value: raw::RawConnection) -> Self {
        match value {
            raw::RawConnection::Normal {
                parent_attach_point,
                child_attach_point,
                parent_part,
                child_part,
            } => Self::Normal {
                parent_attach_point,
                child_attach_point,
                parent_part,
                child_part,
            },
            raw::RawConnection::Dock {
                dock_part,
                parent_part,
                child_part,
            } => Self::Dock {
                dock_part,
                parent_part,
                child_part,
            },
        }
    }
}

impl From<model::Connection> for raw::RawConnection {
    fn from(value: model::Connection) -> Self {
        match value {
            model::Connection::Normal {
                parent_attach_point,
                child_attach_point,
                parent_part,
                child_part,
            } => Self::Normal {
                parent_attach_point,
                child_attach_point,
                parent_part,
                child_part,
            },
            model::Connection::Dock {
                dock_part,
                parent_part,
                child_part,
            } => Self::Dock {
                dock_part,
                parent_part,
                child_part,
            },
        }
    }
}

impl From<raw::RawDisconnectedPart> for model::DisconnectedGroup {
    fn from(value: raw::RawDisconnectedPart) -> Self {
        Self {
            parts: value.parts.parts.into_iter().map(Into::into).collect(),
            connections: value
                .connections
                .connections
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<model::DisconnectedGroup> for raw::RawDisconnectedPart {
    fn from(value: model::DisconnectedGroup) -> Self {
        Self {
            parts: raw::RawParts {
                parts: value.parts.into_iter().map(Into::into).collect(),
            },
            connections: raw::RawConnections {
                connections: value.connections.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<raw::RawShipDocument> for model::ShipData {
    fn from(value: raw::RawShipDocument) -> Self {
        Self {
            version: value.version,
            lifted_off: value.lifted_off != 0,
            touching_ground: value.touching_ground != 0,
            parts: value.parts.parts.into_iter().map(Into::into).collect(),
            connections: value
                .connections
                .connections
                .into_iter()
                .map(Into::into)
                .collect(),
            disconnected: value
                .disconnected
                .parts
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<model::ShipData> for raw::RawShipDocument {
    fn from(value: model::ShipData) -> Self {
        Self {
            parts: raw::RawParts {
                parts: value.parts.into_iter().map(Into::into).collect(),
            },
            connections: raw::RawConnections {
                connections: value.connections.into_iter().map(Into::into).collect(),
            },
            version: value.version,
            lifted_off: bool_to_i8(value.lifted_off),
            touching_ground: bool_to_i8(value.touching_ground),
            disconnected: raw::RawDisconnectedParts {
                parts: value.disconnected.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<raw::RawShipDocument> for model::ShipDocument {
    fn from(value: raw::RawShipDocument) -> Self {
        Self { ship: value.into() }
    }
}

impl From<model::ShipDocument> for raw::RawShipDocument {
    fn from(value: model::ShipDocument) -> Self {
        value.ship.into()
    }
}

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

impl From<raw::RawShipDocument> for model::XmlDocument {
    fn from(value: raw::RawShipDocument) -> Self {
        Self::Ship(value.into())
    }
}
