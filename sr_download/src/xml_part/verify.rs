//! XML 事件扫描与船只语义校验，无数据库或网络依赖。
//!
//! 读取、事件扫描与语义校验是三份不同契约：能解析不代表语义合法；
//! 历史 verify_xml 接受 XML 片段，调用者不要将其当作严格文档校验器。

use super::raw::{
    RawConnection as VerifyConnection, RawConnections, RawDisconnectedParts, RawPart as VerifyPart,
    RawParts,
};
use quick_xml::{Reader, de::from_str, errors::IllFormedError, events::Event, name::QName};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipVerifyState {
    NotXml,
    NotShip,
    FakeShip,
    BrokenShip,
    VerifiedShip,
}

impl ShipVerifyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotXml => "not xml",
            Self::NotShip => "not ship",
            Self::FakeShip => "fake ship",
            Self::BrokenShip => "broken ship",
            Self::VerifiedShip => "verified ship",
        }
    }
}

impl std::fmt::Display for ShipVerifyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 保留历史事件扫描契约：检查标签闭合，但不要求单根或根外空白。
pub fn verify_xml(data: &str) -> quick_xml::Result<()> {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_comments = true;
    let mut open_elements = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                open_elements.push(element.name().as_ref().to_vec());
            }
            Ok(Event::End(_)) => {
                let _ = open_elements.pop();
            }
            Ok(Event::Eof) => {
                if let Some(element) = open_elements.last() {
                    return Err(quick_xml::Error::IllFormed(IllFormedError::MissingEndTag(
                        String::from_utf8_lossy(element).into_owned(),
                    )));
                }
                break;
            }
            Ok(_) => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn verify_ship(data: &str) -> ShipVerifyState {
    if verify_xml(data).is_err() {
        return ShipVerifyState::NotXml;
    }
    if !matches_basic_ship_shape(data) {
        return ShipVerifyState::NotShip;
    }
    match from_str::<RawShipForVerify>(data) {
        Ok(ship) => {
            if ship.is_semantically_valid() {
                ShipVerifyState::VerifiedShip
            } else {
                ShipVerifyState::BrokenShip
            }
        }
        Err(_) => ShipVerifyState::FakeShip,
    }
}

fn matches_basic_ship_shape(data: &str) -> bool {
    let mut reader = Reader::from_str(data);
    reader.config_mut().trim_text(true);

    let mut depth = 0usize;
    let mut root_checked = false;
    let mut has_parts = false;
    let mut has_connections = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                if !root_checked {
                    root_checked = true;
                    if e.name() != QName(b"Ship") {
                        return false;
                    }
                } else if depth == 1 {
                    match e.name() {
                        QName(b"Parts") => has_parts = true,
                        QName(b"Connections") => has_connections = true,
                        _ => {}
                    }
                }
                depth += 1;
            }
            Ok(Event::Empty(ref e)) => {
                if !root_checked {
                    return false;
                }
                if depth == 1 {
                    match e.name() {
                        QName(b"Parts") => has_parts = true,
                        QName(b"Connections") => has_connections = true,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return false,
        }
    }

    root_checked && has_parts && has_connections
}

fn default_ship_version() -> i32 {
    1
}
fn default_lifted_off() -> i8 {
    0
}
fn default_touching_ground() -> i8 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(rename = "Ship")]
struct RawShipForVerify {
    #[serde(rename = "Parts")]
    #[allow(dead_code)]
    parts: RawParts,
    #[serde(rename = "Connections")]
    #[allow(dead_code)]
    connects: RawConnections,
    #[serde(rename = "@version", default = "default_ship_version")]
    #[allow(dead_code)]
    version: i32,
    #[serde(rename = "@liftedOff", default = "default_lifted_off")]
    #[allow(dead_code)]
    lift_off: i8,
    #[serde(rename = "@touchingGround", default = "default_touching_ground")]
    #[allow(dead_code)]
    touch_ground: i8,
    #[serde(rename = "DisconnectedParts")]
    #[allow(dead_code)]
    disconnected: RawDisconnectedParts,
}

impl RawShipForVerify {
    fn is_semantically_valid(&self) -> bool {
        let mut part_ids = HashSet::new();
        if !collect_part_ids(&self.parts.parts, &mut part_ids) {
            return false;
        }
        for disconnected in &self.disconnected.parts {
            if !collect_part_ids(&disconnected.parts.parts, &mut part_ids) {
                return false;
            }
        }

        connections_valid(&self.connects.connections, &part_ids)
            && normal_connections_acyclic(&self.connects.connections)
            && self.disconnected.parts.iter().all(|part| {
                connections_valid(&part.connections.connections, &part_ids)
                    && normal_connections_acyclic(&part.connections.connections)
            })
            && part_activations_valid(&self.parts.parts, &part_ids)
            && self
                .disconnected
                .parts
                .iter()
                .all(|part| part_activations_valid(&part.parts.parts, &part_ids))
    }
}

fn collect_part_ids(parts: &[VerifyPart], all_ids: &mut HashSet<i64>) -> bool {
    for part in parts {
        if !all_ids.insert(part.id) {
            return false;
        }
    }
    true
}

fn connections_valid(connections: &[VerifyConnection], part_ids: &HashSet<i64>) -> bool {
    let mut seen_normal = HashSet::new();
    let mut seen_dock = HashSet::new();

    connections.iter().all(|connection| match connection {
        VerifyConnection::Normal {
            parent_attach_point,
            child_attach_point,
            parent_part,
            child_part,
        } => {
            if !part_ids.contains(parent_part) || !part_ids.contains(child_part) {
                return false;
            }
            if parent_part == child_part {
                return false;
            }
            seen_normal.insert((
                *parent_attach_point,
                *child_attach_point,
                *parent_part,
                *child_part,
            ))
        }
        VerifyConnection::Dock {
            dock_part,
            parent_part,
            child_part,
        } => {
            if !part_ids.contains(dock_part)
                || !part_ids.contains(parent_part)
                || !part_ids.contains(child_part)
            {
                return false;
            }
            if parent_part == child_part || *dock_part == *parent_part || *dock_part == *child_part
            {
                return false;
            }
            seen_dock.insert((*dock_part, *parent_part, *child_part))
        }
    })
}

fn part_activations_valid(parts: &[VerifyPart], part_ids: &HashSet<i64>) -> bool {
    parts.iter().all(|part| {
        let Some(pod) = &part.pod else {
            return true;
        };
        pod.staging
            .steps
            .iter()
            .flat_map(|step| step.activates.iter())
            .all(|activate| part_ids.contains(&activate.id))
    })
}

fn normal_connections_acyclic(connections: &[VerifyConnection]) -> bool {
    // 使用拓扑排序，避免深链递归耗尽线程栈，且每条边只处理一次。
    let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut incoming: HashMap<i64, usize> = HashMap::new();
    for connection in connections {
        if let VerifyConnection::Normal {
            parent_part,
            child_part,
            ..
        } = connection
        {
            children.entry(*parent_part).or_default().push(*child_part);
            incoming.entry(*parent_part).or_default();
            *incoming.entry(*child_part).or_default() += 1;
        }
    }

    let mut ready: Vec<i64> = incoming
        .iter()
        .filter_map(|(&node, &count)| (count == 0).then_some(node))
        .collect();
    let mut processed = 0;
    while let Some(node) = ready.pop() {
        processed += 1;
        if let Some(children) = children.get(&node) {
            for child in children {
                if let Some(count) = incoming.get_mut(child) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push(*child);
                    }
                }
            }
        }
    }
    processed == incoming.len()
}

#[cfg(test)]
mod tests {
    use super::{
        ShipVerifyState, VerifyConnection, normal_connections_acyclic, verify_ship, verify_xml,
    };

    #[test]
    fn legacy_scanner_keeps_fragment_and_root_external_content_behavior() {
        // 此处刻画历史契约，不是宣称这些片段满足完整 XML 文档标准。
        for xml in [
            "",
            " ",
            "text",
            "<!-- comment -->",
            "<A/><B/>",
            "text<A/>",
            "<A/>text",
            "<![CDATA[ ]]><A/>",
        ] {
            assert!(verify_xml(xml).is_ok(), "历史扫描结果应保持: {xml:?}");
        }
    }

    #[test]
    fn xml_accepts_a_root_with_legal_surrounding_markup() {
        for xml in [
            "<A/>",
            "<A></A>",
            "<A><B/>text&amp;<![CDATA[data]]></A>",
            "<?xml version='1.0'?><A/>",
            "<!-- before --><A/><!-- after -->",
            "<!DOCTYPE A><A/>",
        ] {
            assert!(verify_xml(xml).is_ok(), "应接受单根文档: {xml:?}");
        }
    }

    #[test]
    fn xml_rejects_mismatched_and_unclosed_elements() {
        for xml in ["<A>", "<A><B/></C>", "<A></A></A>"] {
            assert!(verify_xml(xml).is_err());
        }
    }

    fn connection(parent_part: i64, child_part: i64) -> VerifyConnection {
        VerifyConnection::Normal {
            parent_attach_point: 0,
            child_attach_point: 0,
            parent_part,
            child_part,
        }
    }

    #[test]
    fn verifies_deep_connection_chains_without_recursion() {
        let mut connections: Vec<_> = (0..20_000)
            .map(|index| connection(index, index + 1))
            .collect();
        assert!(normal_connections_acyclic(&connections));
        connections.push(connection(20_000, 0));
        assert!(!normal_connections_acyclic(&connections));
    }

    #[test]
    fn accepts_converging_dag_and_ignores_docking_edges_for_cycles() {
        let mut connections = vec![
            connection(1, 2),
            connection(1, 3),
            connection(2, 4),
            connection(3, 4),
        ];
        assert!(normal_connections_acyclic(&connections));
        connections.push(VerifyConnection::Dock {
            dock_part: 5,
            parent_part: 4,
            child_part: 1,
        });
        assert!(normal_connections_acyclic(&connections));
    }

    #[test]
    fn verify_ship_reports_not_xml() {
        assert_eq!(verify_ship("<Ship"), ShipVerifyState::NotXml);
    }

    #[test]
    fn verify_ship_reports_not_ship() {
        let xml = r#"<Runtime><Nodes /></Runtime>"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::NotShip);
    }

    #[test]
    fn verify_ship_reports_fake_ship() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part />
  </Parts>
  <Connections />
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::FakeShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0">
          <Step>
            <Activate Id="999" moved="0" />
          </Step>
        </Staging>
      </Pod>
    </Part>
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="0" parentPart="1" childPart="2" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship_for_self_connection() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0" />
      </Pod>
    </Part>
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="1" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship_for_duplicate_connection() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0" />
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="2" x="1" y="0" editorAngle="0" angle="0" angleV="0" />
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_broken_ship_for_connection_cycle() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="0" />
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="2" x="1" y="0" editorAngle="0" angle="0" angleV="0" />
    <Part partType="fueltank-1" id="3" x="2" y="0" editorAngle="0" angle="0" angleV="0" />
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="2" childPart="3" />
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="3" childPart="1" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::BrokenShip);
    }

    #[test]
    fn verify_ship_reports_verified_ship() {
        let xml = r#"
<Ship version="1" liftedOff="0" touchingGround="1">
  <Parts>
    <Part partType="pod-1" id="1" x="0" y="0" editorAngle="0" angle="0" angleV="0">
      <Pod name="Test" throttle="0">
        <Staging currentStage="1">
          <Step>
            <Activate Id="2" moved="0" />
          </Step>
        </Staging>
      </Pod>
    </Part>
    <Part partType="fueltank-1" id="2" x="1" y="0" editorAngle="0" angle="0" angleV="0" />
  </Parts>
  <Connections>
    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2" />
  </Connections>
  <DisconnectedParts />
</Ship>
"#;
        assert_eq!(verify_ship(xml), ShipVerifyState::VerifiedShip);
    }
    #[test]
    fn semantic_verifier_keeps_its_required_root_fields() {
        let xml = "<Ship><Parts/><Connections/></Ship>";
        assert!(super::super::parse::parse_ship_xml(xml).is_ok());
        assert_eq!(verify_ship(xml), ShipVerifyState::FakeShip);
        assert_eq!(
            verify_ship("<Ship><Parts/><Connections/><DisconnectedParts/></Ship>"),
            ShipVerifyState::VerifiedShip
        );
    }

    #[test]
    fn topological_cycle_check_matches_legacy_dfs_for_all_small_graphs() {
        fn has_cycle(
            node: i64,
            edges: &[(i64, i64)],
            visiting: &mut std::collections::HashSet<i64>,
            visited: &mut std::collections::HashSet<i64>,
        ) -> bool {
            if visited.contains(&node) {
                return false;
            }
            if !visiting.insert(node) {
                return true;
            }
            for &(_, child) in edges.iter().filter(|(parent, _)| *parent == node) {
                if has_cycle(child, edges, visiting, visited) {
                    return true;
                }
            }
            visiting.remove(&node);
            visited.insert(node);
            false
        }
        let possible: Vec<_> = (0..3)
            .flat_map(|parent| (0..3).map(move |child| (parent, child)))
            .collect();
        for mask in 0..(1 << possible.len()) {
            let edges: Vec<_> = possible
                .iter()
                .enumerate()
                .filter_map(|(index, &edge)| (mask & (1 << index) != 0).then_some(edge))
                .collect();
            let mut visiting = std::collections::HashSet::new();
            let mut visited = std::collections::HashSet::new();
            let legacy_acyclic =
                (0..3).all(|node| !has_cycle(node, &edges, &mut visiting, &mut visited));
            let connections: Vec<_> = edges
                .iter()
                .map(|&(parent, child)| connection(parent, child))
                .collect();
            assert_eq!(
                normal_connections_acyclic(&connections),
                legacy_acyclic,
                "graph = {edges:?}"
            );
        }
    }
}
