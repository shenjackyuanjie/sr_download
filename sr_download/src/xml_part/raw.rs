//! XML wire 模型及历史兼容入口。
//!
//! 各领域子模块只描述既有 Serde schema；解码和文件读取由 [`super::parse`]
//! 负责，序列化由 [`super::write`] 负责。这里的 `from_str` 只转发，
//! 不做根名或全篇校验。所有旧的 `raw::Raw*` 路径通过 re-export 保留。
//!
//! raw 往返保留已建模字段的值及可选字段的缺失状态，不保证原 XML 字节保真：
//! 空白、实体写法、属性顺序和未知结构体字段不会原样保留，默认值也会被写出。
//! 不依据游戏参考补全字段或调整现有 schema；`$value` 枚举仍按已有变体解码。

mod part;
mod save;
mod ship;
mod staging;

pub use part::{RawFuelEngine, RawFuelTank, RawPart, RawParts};
pub use save::{RawNode, RawNodes, RawPlanetNode, RawSaveDocument, RawShipNode};
pub use ship::{
    RawConnection, RawConnections, RawDisconnectedPart, RawDisconnectedParts, RawShipDocument,
};
pub use staging::{RawActivate, RawPod, RawStaging, RawStep};

use super::error::XmlResult;

impl RawShipDocument {
    #[expect(clippy::should_implement_trait, reason = "保留已有固有方法 API 与行为")]
    pub fn from_str(data: &str) -> XmlResult<Self> {
        super::parse::parse_raw_ship_xml(data)
    }
}

impl RawSaveDocument {
    #[expect(clippy::should_implement_trait, reason = "保留已有固有方法 API 与行为")]
    pub fn from_str(data: &str) -> XmlResult<Self> {
        super::parse::parse_raw_save_xml(data)
    }
}
