//! 离线兼容行为刻画：值往返、默认/省略策略、首文档接受范围和错误接口。

use super::{
    XmlError,
    model::XmlDocument,
    parse,
    raw::{RawSaveDocument, RawShipDocument},
    write,
};

const EMPTY_SHIP: &str = crate::net::EMPTY_SHIP;
const SAMPLE_SAVE: &str = include_str!("../save_1294489.xml");

const MINIMAL_SHIP: &str = "<Ship><Parts/><Connections/></Ship>";
const MINIMAL_SAVE: &str = r#"<Runtime time="0" firstStageActivated="0" solarSystem="Solar System" shipId="1" podId="1"><Nodes/></Runtime>"#;
const COMPLEX_SHIP: &str = r#"
<Ship version="2" liftedOff="1" touchingGround="0">
  <Parts>
    <Part partType="pod&amp;&quot;&lt;&gt;&apos;-飞船" id="1" x="-1.25" y="9.5" editorAngle="90" angle="0.25" angleV="-0.5" flippedX="1" flippedY="0" activated="1" exploded="0" chuteX="1.5" chuteY="-2.5" chuteAngle="0.75" chuteHeight="3.5" extension="0.125" inflate="1" inflation="0.5" deployed="0" rope="1">
      <Tank fuel="10.5"/>
      <Engine fuel="0.125"/>
      <Pod name="A &amp; B &lt;C&gt; &quot;中&quot;" throttle="0.5">
        <Staging currentStage="2">
          <Step/>
          <Step><Activate Id="9223372036854775807" moved="1"/><Activate Id="1" moved="0"/></Step>
        </Staging>
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="9223372036854775807" x="1" y="2" angle="0" angleV="0"/>
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="9223372036854775807"/>
    <DockConnection dockPart="3" parentPart="1" childPart="9223372036854775807"/>
    <Connection parentAttachPoint="2" childAttachPoint="3" parentPart="9223372036854775807" childPart="1"/>
  </Connections>
  <DisconnectedParts>
    <DisconnectedPart>
  <Parts><Part partType="engine-1" id="3" x="3" y="4" angle="1" angleV="2"><Engine fuel="1.5"/></Part></Parts>
      <Connections/>
    </DisconnectedPart>
    <DisconnectedPart><Parts/><Connections/></DisconnectedPart>
  </DisconnectedParts>
</Ship>
"#;

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
fn attribute_control_characters_round_trip_as_references() {
    const RAW: &str = r#"<Runtime time="0" firstStageActivated="0" solarSystem="A&#9;B&#10;C&#13;D" shipId="1" podId="1"><Nodes/></Runtime>"#;
    let doc = parse::parse_save_xml(RAW).unwrap();
    assert_eq!(doc.solar_system, "A\tB\nC\rD");

    let xml = write::write_save_xml(&doc).unwrap();
    assert!(xml.contains(r#"solarSystem="A&#9;B&#10;C&#13;D""#));
    assert!(!xml.contains("A\tB"));
    assert_eq!(parse::parse_save_xml(&xml).unwrap(), doc);
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

#[test]
fn parsers_preserve_prolog_comments_and_processing_instruction_compatibility() {
    let xml = format!(
        "\u{feff}<?xml version=\"1.0\"?>\n<!-- before -->\n<?editor test?>\n<!DOCTYPE Ship>\n{MINIMAL_SHIP}\n<?done?>\n<!-- after -->\n"
    );
    let expected = parse::parse_ship_xml(MINIMAL_SHIP).unwrap();
    assert_eq!(parse::parse_ship_xml(&xml).unwrap(), expected);
    assert_eq!(
        parse::parse_any_xml(&xml).unwrap(),
        XmlDocument::Ship(expected)
    );
}

#[test]
fn parsers_keep_defaults_and_ignore_unknown_extensions() {
    let xml = r#"<Ship future="1"><Future valid="yes"/><Parts/><Connections/></Ship>"#;
    let ship = parse::parse_ship_xml(xml).unwrap();
    assert_eq!(ship.ship.version, 1);
    assert!(!ship.ship.lifted_off);
    assert!(ship.ship.touching_ground);
    assert!(ship.ship.disconnected.is_empty());
    assert_eq!(ship, parse::parse_ship_xml(MINIMAL_SHIP).unwrap());
}

#[test]
fn complex_ship_round_trips_raw_and_model_data() {
    let raw = parse::parse_raw_ship_xml(COMPLEX_SHIP).unwrap();
    let raw_xml = write::write_raw_ship_xml(&raw).unwrap();
    assert_eq!(raw, parse::parse_raw_ship_xml(&raw_xml).unwrap());
    assert!(raw_xml.contains("<Activate Id="));
    assert_eq!(raw.parts.parts[0].part_type_id, r#"pod&"<>'-飞船"#);
    assert_eq!(
        raw.parts.parts[0].pod.as_ref().unwrap().name,
        r#"A & B <C> "中""#
    );

    let ship = parse::parse_ship_xml(COMPLEX_SHIP).unwrap();
    let xml = write::write_ship_xml(&ship).unwrap();
    assert_eq!(ship, parse::parse_ship_xml(&xml).unwrap());
    assert_eq!(ship.ship.connections.len(), 3);
    assert_eq!(ship.ship.disconnected.len(), 2);
    assert_eq!(ship.ship.parts[1].id, i64::MAX);
}

#[test]
fn mixed_save_nodes_round_trip_in_order() {
    let xml = format!(
        r#"<Runtime time="12.5" firstStageActivated="1" solarSystem="A &amp; B" shipId="9223372036854775807" podId="1"><Nodes><PlanetNode name="Earth &amp; Moon"/><ShipNode id="9223372036854775807" planet="Earth" planetRadius="100" x="1" y="2" vx="3" vy="4">{COMPLEX_SHIP}</ShipNode><PlanetNode name="Mars" trueAnomaly="1.25"/></Nodes></Runtime>"#
    );
    let raw = parse::parse_raw_save_xml(&xml).unwrap();
    let raw_xml = write::write_raw_save_xml(&raw).unwrap();
    assert_eq!(raw, parse::parse_raw_save_xml(&raw_xml).unwrap());

    let document = parse::parse_any_xml(&xml).unwrap();
    let rewritten = write::write_xml_document(&document).unwrap();
    assert_eq!(document, parse::parse_any_xml(&rewritten).unwrap());
    let XmlDocument::Save(save) = document else {
        panic!("应解析为存档")
    };
    assert!(matches!(save.nodes[0], super::model::SaveNode::Planet(_)));
    assert!(matches!(save.nodes[1], super::model::SaveNode::Ship(_)));
    assert!(matches!(save.nodes[2], super::model::SaveNode::Planet(_)));
    assert_eq!(save.solar_system, "A & B");
    let owned_raw = RawSaveDocument::from(save.clone());
    assert_eq!(rewritten, write::write_raw_save_xml(&owned_raw).unwrap());
}

#[test]
fn typed_entry_points_keep_serde_root_name_leniency() {
    let ship_xml = MINIMAL_SHIP.replace("Ship", "Other");
    let expected_raw_ship = parse::parse_raw_ship_xml(MINIMAL_SHIP).unwrap();
    let expected_ship = parse::parse_ship_xml(MINIMAL_SHIP).unwrap();
    assert_eq!(
        parse::parse_raw_ship_xml(&ship_xml).unwrap(),
        expected_raw_ship
    );
    assert_eq!(
        RawShipDocument::from_str(&ship_xml).unwrap(),
        expected_raw_ship
    );
    assert_eq!(parse::parse_ship_xml(&ship_xml).unwrap(), expected_ship);

    let save_xml = MINIMAL_SAVE.replace("Runtime", "Other");
    let expected_raw_save = parse::parse_raw_save_xml(MINIMAL_SAVE).unwrap();
    let expected_save = parse::parse_save_xml(MINIMAL_SAVE).unwrap();
    assert_eq!(
        parse::parse_raw_save_xml(&save_xml).unwrap(),
        expected_raw_save
    );
    assert_eq!(
        RawSaveDocument::from_str(&save_xml).unwrap(),
        expected_raw_save
    );
    assert_eq!(parse::parse_save_xml(&save_xml).unwrap(), expected_save);

    for xml in [&ship_xml, &save_xml] {
        assert!(matches!(
            parse::parse_any_xml(xml),
            Err(XmlError::UnsupportedRoot(root)) if root == "Other"
        ));
    }
}

#[test]
fn parsers_keep_first_document_acceptance_without_a_full_document_gate() {
    let raw_ship = RawShipDocument::from_str(MINIMAL_SHIP).unwrap();
    let ship = parse::parse_ship_xml(MINIMAL_SHIP).unwrap();
    for suffix in ["<Extra/>", MINIMAL_SHIP, "<unfinished"] {
        let xml = format!("{MINIMAL_SHIP}{suffix}");
        assert_eq!(RawShipDocument::from_str(&xml).unwrap(), raw_ship);
        assert_eq!(parse::parse_raw_ship_xml(&xml).unwrap(), raw_ship);
        assert_eq!(parse::parse_ship_xml(&xml).unwrap(), ship);
        assert_eq!(
            parse::parse_any_xml(&xml).unwrap(),
            XmlDocument::Ship(ship.clone())
        );
    }

    let raw_save = RawSaveDocument::from_str(MINIMAL_SAVE).unwrap();
    let save = parse::parse_save_xml(MINIMAL_SAVE).unwrap();
    for suffix in ["<Extra/>", MINIMAL_SAVE, "<unfinished"] {
        let xml = format!("{MINIMAL_SAVE}{suffix}");
        assert_eq!(RawSaveDocument::from_str(&xml).unwrap(), raw_save);
        assert_eq!(parse::parse_raw_save_xml(&xml).unwrap(), raw_save);
        assert_eq!(parse::parse_save_xml(&xml).unwrap(), save);
        assert_eq!(
            parse::parse_any_xml(&xml).unwrap(),
            XmlDocument::Save(save.clone())
        );
    }
}

#[test]
fn document_dispatch_uses_only_the_first_root() {
    for xml in [
        format!("<Other/>{MINIMAL_SHIP}"),
        "<Other><unfinished".to_string(),
    ] {
        assert!(matches!(
            parse::parse_any_xml(&xml),
            Err(XmlError::UnsupportedRoot(root)) if root == "Other"
        ));
    }
    for xml in [
        "",
        " \n\t",
        "<!-- 注释 -->",
        r#"<?xml version="1.0"?>"#,
        "text",
    ] {
        assert!(matches!(
            parse::parse_any_xml(xml),
            Err(XmlError::UnsupportedRoot(root)) if root == "<empty>"
        ));
    }
}

#[test]
fn parser_error_categories_keep_reader_and_serde_distinct() {
    assert!(matches!(
        parse::parse_any_xml("<Ship"),
        Err(XmlError::Serialize(_))
    ));
    assert!(matches!(
        parse::parse_raw_ship_xml("<Ship"),
        Err(XmlError::Deserialize(_))
    ));
    assert!(matches!(
        RawShipDocument::from_str("<Ship/>"),
        Err(XmlError::Deserialize(_))
    ));
    assert!(matches!(
        RawSaveDocument::from_str("<Runtime/>"),
        Err(XmlError::Deserialize(_))
    ));
    for xml in [
        "<Ship><Parts></Connections></Ship>",
        r#"<Ship version="bad"><Parts/><Connections/></Ship>"#,
    ] {
        assert!(matches!(
            parse::parse_ship_xml(xml),
            Err(XmlError::Deserialize(_))
        ));
    }
}

#[test]
fn xml_error_from_display_and_source_contract_stays_unchanged() {
    let check = |error: XmlError, expected: String| {
        assert_eq!(error.to_string(), expected);
        assert!(std::error::Error::source(&error).is_none());
    };
    let io_error = std::io::Error::other("离线 IO 错误");
    let io_text = io_error.to_string();
    let io_error = XmlError::from(io_error);
    assert!(matches!(&io_error, XmlError::Io(_)));
    check(io_error, format!("io error: {io_text}"));

    let de_error = quick_xml::DeError::Custom("解码错误".to_string());
    let de_text = de_error.to_string();
    let de_error = XmlError::from(de_error);
    assert!(matches!(&de_error, XmlError::Deserialize(_)));
    check(de_error, format!("xml deserialize error: {de_text}"));

    let mut reader = quick_xml::Reader::from_str("<Ship");
    let reader_error = reader.read_event().unwrap_err();
    let reader_text = reader_error.to_string();
    let reader_error = XmlError::from(reader_error);
    assert!(matches!(&reader_error, XmlError::Serialize(text) if text == &reader_text));
    check(reader_error, format!("xml serialize error: {reader_text}"));
    check(
        XmlError::Serialize("编码错误".to_string()),
        "xml serialize error: 编码错误".to_string(),
    );
    check(
        XmlError::UnsupportedRoot("Other".to_string()),
        "unsupported xml root: Other".to_string(),
    );
    check(
        XmlError::UnexpectedDocumentType {
            expected: "Ship",
            found: "Save",
        },
        "unexpected xml document type: expected Ship, found Save".to_string(),
    );
}

#[test]
fn unknown_struct_fields_are_discarded_during_raw_and_model_round_trips() {
    let xml = r#"<Ship future="1"><Future><Nested/></Future><Parts><Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" future="yes"><Future/></Part></Parts><Connections/></Ship>"#;
    let expected_xml = r#"<Ship><Parts><Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0"/></Parts><Connections/></Ship>"#;
    let raw = parse::parse_raw_ship_xml(xml).unwrap();
    assert_eq!(raw, parse::parse_raw_ship_xml(expected_xml).unwrap());
    let raw_xml = write::write_raw_ship_xml(&raw).unwrap();
    assert!(!raw_xml.contains("future"));
    assert!(!raw_xml.contains("Future"));
    let model = parse::parse_ship_xml(xml).unwrap();
    assert_eq!(model, parse::parse_ship_xml(expected_xml).unwrap());
    let model_xml = write::write_ship_xml(&model).unwrap();
    assert!(!model_xml.contains("future"));
    assert!(!model_xml.contains("Future"));
}

#[test]
fn unknown_mixed_node_variants_still_follow_the_existing_enum_schema() {
    let ship = MINIMAL_SHIP.replace("<Connections/>", "<Connections><Future/></Connections>");
    let save = MINIMAL_SAVE.replace("<Nodes/>", "<Nodes><Future/></Nodes>");
    assert!(matches!(
        parse::parse_raw_ship_xml(&ship),
        Err(XmlError::Deserialize(_))
    ));
    assert!(matches!(
        parse::parse_raw_save_xml(&save),
        Err(XmlError::Deserialize(_))
    ));
}

#[test]
fn default_and_missing_optional_fields_keep_their_existing_write_policy() {
    let xml = r#"<Ship><Parts><Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0"/></Parts><Connections/></Ship>"#;
    let raw = parse::parse_raw_ship_xml(xml).unwrap();
    assert_eq!(
        (raw.version, raw.lifted_off, raw.touching_ground),
        (1, 0, 1)
    );
    assert_eq!(raw.parts.parts[0].editor_angle, 0);
    let raw_xml = write::write_raw_ship_xml(&raw).unwrap();
    assert_eq!(raw, parse::parse_raw_ship_xml(&raw_xml).unwrap());
    for field in [
        r#"version="1""#,
        r#"liftedOff="0""#,
        r#"touchingGround="1""#,
        r#"editorAngle="0""#,
    ] {
        assert!(raw_xml.contains(field), "{field}");
    }
    for field in [
        "flippedX",
        "flippedY",
        "activated",
        "exploded",
        "chuteX",
        "inflate",
        "deployed",
        "rope",
    ] {
        assert!(!raw_xml.contains(field), "{field}");
    }
    assert!(raw_xml.contains("DisconnectedParts"));

    let model = parse::parse_ship_xml(xml).unwrap();
    let model_xml = write::write_ship_xml(&model).unwrap();
    let normalized = parse::parse_raw_ship_xml(&model_xml).unwrap();
    let part = &normalized.parts.parts[0];
    assert_eq!(
        (
            part.flipped_x,
            part.flipped_y,
            part.activated,
            part.exploded
        ),
        (Some(0), Some(0), Some(0), Some(0))
    );
    assert!(part.tank.is_none() && part.engine.is_none() && part.pod.is_none());
    assert!(
        part.chute_x.is_none()
            && part.chute_y.is_none()
            && part.chute_angle.is_none()
            && part.chute_height.is_none()
    );
    assert!(
        part.extension.is_none()
            && part.inflate.is_none()
            && part.inflation.is_none()
            && part.deployed.is_none()
            && part.rope.is_none()
    );
    assert_eq!(model, parse::parse_ship_xml(&model_xml).unwrap());
}

#[test]
fn numeric_booleans_normalize_only_when_converted_through_model() {
    let xml = r#"<Ship liftedOff="-128" touchingGround="2"><Parts><Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" flippedX="127" flippedY="-128" activated="-2" exploded="0" inflate="-1" deployed="2" rope="0"><Pod name="控制舱" throttle="0"><Staging currentStage="0"><Step><Activate Id="1" moved="-3"/></Step></Staging></Pod></Part></Parts><Connections/></Ship>"#;
    let raw = parse::parse_raw_ship_xml(xml).unwrap();
    let raw_xml = write::write_raw_ship_xml(&raw).unwrap();
    assert_eq!(raw, parse::parse_raw_ship_xml(&raw_xml).unwrap());
    assert_eq!((raw.lifted_off, raw.touching_ground), (-128, 2));
    assert_eq!(raw.parts.parts[0].activated, Some(-2));

    let model = parse::parse_ship_xml(xml).unwrap();
    assert!(model.ship.lifted_off && model.ship.touching_ground);
    let part = &model.ship.parts[0];
    assert!(part.flipped_x && part.flipped_y && part.activated && !part.exploded);
    assert_eq!(
        (part.attrs.inflate, part.attrs.deployed, part.attrs.rope),
        (Some(true), Some(true), Some(false))
    );
    assert!(part.attrs.pod.as_ref().unwrap().steps[0].activates[0].moved);
    let normalized = parse::parse_raw_ship_xml(&write::write_ship_xml(&model).unwrap()).unwrap();
    assert_eq!((normalized.lifted_off, normalized.touching_ground), (1, 1));
    let part = &normalized.parts.parts[0];
    assert_eq!(
        (
            part.flipped_x,
            part.flipped_y,
            part.activated,
            part.exploded
        ),
        (Some(1), Some(1), Some(1), Some(0))
    );
    assert_eq!(
        (part.inflate, part.deployed, part.rope),
        (Some(1), Some(1), Some(0))
    );
    assert_eq!(
        part.pod.as_ref().unwrap().staging.steps[0].activates[0].moved,
        1
    );

    let save_xml =
        MINIMAL_SAVE.replace(r#"firstStageActivated="0""#, r#"firstStageActivated="-7""#);
    let raw_save = parse::parse_raw_save_xml(&save_xml).unwrap();
    assert_eq!(raw_save.first_stage_activated, -7);
    assert_eq!(
        raw_save,
        parse::parse_raw_save_xml(&write::write_raw_save_xml(&raw_save).unwrap()).unwrap()
    );
    let save = parse::parse_save_xml(&save_xml).unwrap();
    assert!(save.first_stage_activated);
    assert_eq!(
        parse::parse_raw_save_xml(&write::write_save_xml(&save).unwrap())
            .unwrap()
            .first_stage_activated,
        1
    );
}

#[test]
fn borrowed_writers_match_the_existing_owned_conversion_output() {
    for xml in [MINIMAL_SHIP, EMPTY_SHIP, COMPLEX_SHIP] {
        let document = parse::parse_ship_xml(xml).unwrap();
        let owned = RawShipDocument::from(document.clone());
        assert_eq!(RawShipDocument::from(&document), owned);
        assert_eq!(
            write::write_ship_xml(&document).unwrap(),
            write::write_raw_ship_xml(&owned).unwrap()
        );
    }
    for xml in [MINIMAL_SAVE, SAMPLE_SAVE] {
        let document = parse::parse_save_xml(xml).unwrap();
        let owned = RawSaveDocument::from(document.clone());
        assert_eq!(RawSaveDocument::from(&document), owned);
        assert_eq!(
            write::write_save_xml(&document).unwrap(),
            write::write_raw_save_xml(&owned).unwrap()
        );
    }
}
