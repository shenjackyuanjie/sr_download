//! XML 序列化入口；复用 wire schema，不单独维护 writer schema。
//!
//! raw 写入保留已建模数值与可选值，不保留原 XML 的字节表示。model 写入先
//! 按 `convert` 的既有规则规范化，再编码相同 wire 类型；不补充新 schema 字段。
//! 借用式容器转换逐项构造 raw，避免先克隆整份 model，仍只使用一份 wire schema。

use super::{
    codec,
    error::XmlResult,
    model::{SaveDocument, ShipDocument, XmlDocument},
    raw::{RawSaveDocument, RawShipDocument},
};

pub fn write_raw_ship_xml(raw: &RawShipDocument) -> XmlResult<String> {
    codec::encode(raw)
}

pub fn write_raw_save_xml(raw: &RawSaveDocument) -> XmlResult<String> {
    codec::encode(raw)
}

pub fn write_ship_xml(document: &ShipDocument) -> XmlResult<String> {
    write_raw_ship_xml(&RawShipDocument::from(document))
}

pub fn write_save_xml(document: &SaveDocument) -> XmlResult<String> {
    write_raw_save_xml(&RawSaveDocument::from(document))
}

pub fn write_xml_document(document: &XmlDocument) -> XmlResult<String> {
    match document {
        XmlDocument::Ship(doc) => write_ship_xml(doc),
        XmlDocument::Save(doc) => write_save_xml(doc),
    }
}
