//! 分级、激活项和控制舱的值转换；数值标记转为非零即真的 bool。

use crate::xml_part::{model, raw};

use super::{bool_to_i8, i8_to_bool};

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
