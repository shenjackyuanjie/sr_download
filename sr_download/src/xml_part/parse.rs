//! XML 解码、首根分派与文件读取入口。
//!
//! typed 入口直接按既有 quick_xml Serde schema 解码，不强制根名，
//! 也不在解码前添加全篇 XML 门禁。`parse_any_xml` 只用首个 Start/Empty
//! 事件选择文档类型；后续内容仍由同一个 Serde 解码器处理。
//! `raw::Raw*::from_str` 只向这里转发，实际解码不再回调 raw 入口。

use quick_xml::{Reader, events::Event};

use super::{
    codec,
    error::{XmlError, XmlResult},
    model::{SaveDocument, ShipDocument, XmlDocument},
    raw::{RawSaveDocument, RawShipDocument},
};

fn detect_root(data: &str) -> XmlResult<String> {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                return Ok(String::from_utf8_lossy(event.name().as_ref()).to_string());
            }
            Ok(Event::Eof) => return Err(XmlError::UnsupportedRoot("<empty>".to_string())),
            Ok(_) => {}
            Err(err) => return Err(err.into()),
        }
    }
}

pub fn parse_raw_ship_xml(data: &str) -> XmlResult<RawShipDocument> {
    codec::decode(data)
}

pub fn parse_raw_save_xml(data: &str) -> XmlResult<RawSaveDocument> {
    codec::decode(data)
}

pub fn parse_ship_xml(data: &str) -> XmlResult<ShipDocument> {
    Ok(parse_raw_ship_xml(data)?.into())
}

pub fn parse_save_xml(data: &str) -> XmlResult<SaveDocument> {
    Ok(parse_raw_save_xml(data)?.into())
}

pub fn parse_any_xml(data: &str) -> XmlResult<XmlDocument> {
    match detect_root(data)?.as_str() {
        "Ship" => Ok(parse_raw_ship_xml(data)?.into()),
        "Runtime" => Ok(parse_raw_save_xml(data)?.into()),
        root => Err(XmlError::UnsupportedRoot(root.to_string())),
    }
}

pub fn parse_ship_file(path: impl AsRef<std::path::Path>) -> XmlResult<ShipDocument> {
    let data = std::fs::read_to_string(path)?;
    parse_ship_xml(&data)
}

pub fn parse_save_file(path: impl AsRef<std::path::Path>) -> XmlResult<SaveDocument> {
    let data = std::fs::read_to_string(path)?;
    parse_save_xml(&data)
}
