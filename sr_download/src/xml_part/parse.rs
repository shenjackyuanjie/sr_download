use quick_xml::{Reader, events::Event};

use crate::xml_part::{
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
    RawShipDocument::from_str(data)
}

pub fn parse_raw_save_xml(data: &str) -> XmlResult<RawSaveDocument> {
    RawSaveDocument::from_str(data)
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
