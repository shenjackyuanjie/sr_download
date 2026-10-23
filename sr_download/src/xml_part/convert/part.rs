//! 部件的值转换；集中维护状态默认值与可选属性的写回策略。

use crate::xml_part::{model, raw};

use super::bool_to_i8;

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
