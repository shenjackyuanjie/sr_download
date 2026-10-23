//! 船只、连接和分离部件组的值转换；保持集合顺序与船只默认语义。

use crate::xml_part::{model, raw};

use super::bool_to_i8;

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

impl From<raw::RawShipDocument> for model::XmlDocument {
    fn from(value: raw::RawShipDocument) -> Self {
        Self::Ship(value.into())
    }
}

// 借用式写入只克隆当前部件/连接的值，不预先复制整份模型或集合。
// 部件继续走既有 owned 转换，共享可选字段与数值布尔的唯一写回规则。
impl From<&model::DisconnectedGroup> for raw::RawDisconnectedPart {
    fn from(value: &model::DisconnectedGroup) -> Self {
        Self {
            parts: raw::RawParts {
                parts: value.parts.iter().cloned().map(Into::into).collect(),
            },
            connections: raw::RawConnections {
                connections: value.connections.iter().cloned().map(Into::into).collect(),
            },
        }
    }
}

impl From<&model::ShipData> for raw::RawShipDocument {
    fn from(value: &model::ShipData) -> Self {
        Self {
            parts: raw::RawParts {
                parts: value.parts.iter().cloned().map(Into::into).collect(),
            },
            connections: raw::RawConnections {
                connections: value.connections.iter().cloned().map(Into::into).collect(),
            },
            version: value.version,
            lifted_off: bool_to_i8(value.lifted_off),
            touching_ground: bool_to_i8(value.touching_ground),
            disconnected: raw::RawDisconnectedParts {
                parts: value.disconnected.iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<&model::ShipDocument> for raw::RawShipDocument {
    fn from(value: &model::ShipDocument) -> Self {
        (&value.ship).into()
    }
}
