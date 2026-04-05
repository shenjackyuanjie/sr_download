pub mod convert;
pub mod error;
pub mod model;
pub mod parse;
pub mod raw;
pub mod verify;
pub mod write;

pub use error::{XmlError, XmlResult};

#[cfg(test)]
mod tests {
    use super::{model::XmlDocument, parse, write};

    const EMPTY_SHIP: &str = crate::net::EMPTY_SHIP;
    const SAMPLE_SAVE: &str = include_str!("../save_1294489.xml");

    #[test]
    fn parses_ship_document() {
        let doc = parse::parse_ship_xml(EMPTY_SHIP).unwrap();
        assert_eq!(doc.ship.parts.len(), 1);
        assert!(doc.ship.connections.is_empty());
    }

    #[test]
    fn parses_save_document() {
        let doc = parse::parse_save_xml(SAMPLE_SAVE).unwrap();
        assert!(!doc.nodes.is_empty());
        assert!(
            doc.nodes
                .iter()
                .any(|node| matches!(node, super::model::SaveNode::Ship(_)))
        );
    }

    #[test]
    fn ship_round_trip_model() {
        let doc = parse::parse_ship_xml(EMPTY_SHIP).unwrap();
        let xml = write::write_ship_xml(&doc).unwrap();
        let reparsed = parse::parse_ship_xml(&xml).unwrap();
        assert_eq!(doc, reparsed);
    }

    #[test]
    fn save_round_trip_model() {
        let doc = parse::parse_save_xml(SAMPLE_SAVE).unwrap();
        let xml = write::write_save_xml(&doc).unwrap();
        let reparsed = parse::parse_save_xml(&xml).unwrap();
        assert_eq!(doc, reparsed);
    }

    #[test]
    fn detects_document_type() {
        assert!(matches!(
            parse::parse_any_xml(EMPTY_SHIP).unwrap(),
            XmlDocument::Ship(_)
        ));
        assert!(matches!(
            parse::parse_any_xml(SAMPLE_SAVE).unwrap(),
            XmlDocument::Save(_)
        ));
    }
}
