use quick_xml::se::to_string;

use crate::xml_part::{
    error::{XmlError, XmlResult},
    model::{SaveDocument, ShipDocument, XmlDocument},
    raw::{RawSaveDocument, RawShipDocument},
};

pub fn write_raw_ship_xml(raw: &RawShipDocument) -> XmlResult<String> {
    to_string(raw).map_err(|err| XmlError::Serialize(err.to_string()))
}

pub fn write_raw_save_xml(raw: &RawSaveDocument) -> XmlResult<String> {
    to_string(raw).map_err(|err| XmlError::Serialize(err.to_string()))
}

pub fn write_ship_xml(document: &ShipDocument) -> XmlResult<String> {
    write_raw_ship_xml(&RawShipDocument::from(document.clone()))
}

pub fn write_save_xml(document: &SaveDocument) -> XmlResult<String> {
    write_raw_save_xml(&RawSaveDocument::from(document.clone()))
}

pub fn write_xml_document(document: &XmlDocument) -> XmlResult<String> {
    match document {
        XmlDocument::Ship(doc) => write_ship_xml(doc),
        XmlDocument::Save(doc) => write_save_xml(doc),
    }
}
