use sr_download::xml_part::{self, XmlError, model::XmlDocument, parse, write};

fn error_kind(error: XmlError) -> &'static str {
    match error {
        XmlError::Io(_) => "Io",
        XmlError::Deserialize(_) => "Deserialize",
        XmlError::Serialize(_) => "Serialize",
        XmlError::UnsupportedRoot(_) => "UnsupportedRoot",
        XmlError::UnexpectedDocumentType { .. } => "UnexpectedDocumentType",
    }
}

/// 此快照由重构前 HEAD + quick-xml 0.41 生成，固定接受范围和序列化结果。
pub fn snapshot() -> String {
    let cases = [
        ("minimal-ship", "<Ship><Parts/><Connections/></Ship>"),
        ("typed-other-root", "<Other><Parts/><Connections/></Other>"),
        (
            "trailing-second-document",
            "<Ship><Parts/><Connections/></Ship><Extra/>",
        ),
        (
            "unknown-extension",
            "<Ship future='x'><Future value='ignored'/><Parts/><Connections/></Ship>",
        ),
        (
            "nonzero-flags-and-omitted-options",
            "<Ship version='2' liftedOff='-2' touchingGround='3'><Parts><Part partType='pod-1' id='1' x='-0.0' y='0.125' angle='1e-9' angleV='2.5' flippedX='-1' activated='2' inflate='-3' deployed='0' rope='1'/></Parts><Connections/></Ship>",
        ),
        (
            "legacy-root-staging",
            "<Ship currentStage='3' throttle='0.5' liftedOff='0'><Parts><Part partType='pod-1' id='1' x='0' y='0.75' angle='0' angleV='0'/></Parts><Connections/><Staging><Step><Activate Id='1' moved='1'/></Step></Staging></Ship>",
        ),
        (
            "escaping-and-stage-order",
            "<Ship><Parts><Part partType='pod&amp;&quot;飞船' id='9223372036854775807' x='1' y='2' angle='0' angleV='0'><Pod name='A &amp; B &lt;C&gt;' throttle='0.25'><Staging currentStage='2'><Step/><Step><Activate Id='1' moved='-1'/><Activate Id='2' moved='0'/></Step></Staging></Pod></Part></Parts><Connections/><DisconnectedParts><DisconnectedPart><Parts/><Connections/></DisconnectedPart></DisconnectedParts></Ship>",
        ),
        (
            "attribute-control-characters",
            "<Runtime time='0' firstStageActivated='0' solarSystem='A&#9;B&#10;C&#13;D &amp;#9;' shipId='1' podId='1'><Nodes/></Runtime>",
        ),
        (
            "unmodeled-lander-state",
            "<Ship><Parts><Part partType='lander-1' id='1' x='0' y='0' angle='0' angleV='0' lower='1' raise='0' length='3.125' legAngle='45'/></Parts><Connections/><DisconnectedParts/></Ship>",
        ),
        (
            "missing-save-pod-id",
            "<Runtime time='0' firstStageActivated='0' solarSystem='Solar System' shipId='1'><Nodes/></Runtime>",
        ),
        (
            "unmodeled-runtime-challenge-state",
            "<Runtime time='0' firstStageActivated='0' solarSystem='Solar System' shipId='1' podId='1' maxSpeed='1200.000000' FuelUsed='3.125' fuelUsed='2.5' MoonTouchDown='1' angleTraveled='0.25' lastAngle='0.5' lastAngleDelta='0.125' recordedAngle='1' goodJob='0'><Nodes/></Runtime>",
        ),
        (
            "minimal-save",
            "<Runtime time='0' firstStageActivated='0' solarSystem='Solar System' shipId='1' podId='1'><Nodes/></Runtime>",
        ),
        (
            "mixed-save-nodes",
            "<Runtime time='12.5' firstStageActivated='-1' solarSystem='A &amp; B' shipId='1' podId='2'><Nodes><PlanetNode name='Earth'/><ShipNode id='1' planet='Earth' planetRadius='100' x='1' y='2' vx='3' vy='4'><Ship><Parts/><Connections/></Ship></ShipNode><PlanetNode name='Mars' trueAnomaly='1.25'/></Nodes></Runtime>",
        ),
        ("malformed-first-root", "<Ship"),
        ("missing-root", "<!-- only a comment -->"),
        (
            "comment-prolog",
            "<?xml version='1.0'?><!-- before --><Ship><Parts/><Connections/></Ship><!-- after -->",
        ),
    ];
    let mut result = String::new();
    for (name, xml) in cases {
        result.push_str(&format!("## {name}\n"));
        result.push_str(&format!(
            "verify: {} {}\n",
            xml_part::verify::verify_xml(xml).is_ok(),
            xml_part::verify::verify_ship(xml)
        ));
        for (label, value) in [
            (
                "raw-ship",
                parse::parse_raw_ship_xml(xml).map(|raw| write::write_raw_ship_xml(&raw).unwrap()),
            ),
            (
                "model-ship",
                parse::parse_ship_xml(xml).map(|model| write::write_ship_xml(&model).unwrap()),
            ),
            (
                "raw-save",
                parse::parse_raw_save_xml(xml).map(|raw| write::write_raw_save_xml(&raw).unwrap()),
            ),
            (
                "model-save",
                parse::parse_save_xml(xml).map(|model| write::write_save_xml(&model).unwrap()),
            ),
            (
                "any",
                parse::parse_any_xml(xml).map(|model| {
                    let name = match &model {
                        XmlDocument::Ship(_) => "Ship",
                        XmlDocument::Save(_) => "Save",
                    };
                    format!("{name}:{}", write::write_xml_document(&model).unwrap())
                }),
            ),
        ] {
            match value {
                Ok(xml) => result.push_str(&format!("{label}: {xml}\n")),
                Err(error) => result.push_str(&format!("{label}: ERROR {}\n", error_kind(error))),
            }
        }
    }
    for xml in ["", "text", "<A/><B/>", "text<A/>", "<A>"] {
        result.push_str(&format!(
            "fragment {xml:?}: {} {}\n",
            xml_part::verify::verify_xml(xml).is_ok(),
            xml_part::verify::verify_ship(xml)
        ));
    }
    // 公共转换错误类别也是既有契约，不能借重构顺手更改。
    let error = xml_part::XmlError::from(quick_xml::Error::from(std::io::Error::other("fixture")));
    result.push_str(&format!(
        "quick-xml-error-conversion: {}\n",
        error_kind(error)
    ));
    result
}

#[test]
fn xml_read_write_matches_pre_refactor_snapshot() {
    assert_eq!(snapshot(), include_str!("fixtures/xml_compat.snap"));
}
