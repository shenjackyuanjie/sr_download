use std::{collections::BTreeMap, sync::OnceLock};

use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

use crate::xml_part::model::{Connection, Part, ShipDocument};

const MASS_SCALE: f64 = 500.0;

#[derive(Debug, Clone, Serialize)]
pub struct ShipAnalysis {
    pub state: ShipState,
    pub totals: ShipTotals,
    pub mass: MassSummary,
    pub fuel: FuelSummary,
    pub propulsion: PropulsionSummary,
    pub geometry: Option<GeometrySummary>,
    pub inventory: Vec<PartInventory>,
    pub parts: Vec<PartAnalysis>,
    pub staging: Vec<PodAnalysis>,
    pub connections: ConnectionSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShipState {
    pub version: i32,
    pub lifted_off: bool,
    pub touching_ground: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ShipTotals {
    pub parts: usize,
    pub connected_parts: usize,
    pub disconnected_groups: usize,
    pub disconnected_parts: usize,
    pub active_parts: usize,
    pub exploded_parts: usize,
    pub unknown_part_types: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MassSummary {
    pub catalog_units: f64,
    pub scaled_units: f64,
    pub scale: f64,
    pub known_parts: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FuelSummary {
    pub current: f64,
    pub capacity: f64,
    pub by_type: Vec<FuelBucket>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FuelBucket {
    pub fuel_type: String,
    pub current: f64,
    pub capacity: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PropulsionSummary {
    pub engines: PropulsionUnitSummary,
    pub rcs: PropulsionUnitSummary,
    pub solar_count: usize,
    pub solar_charge: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PropulsionUnitSummary {
    pub count: usize,
    pub power: f64,
    pub consumption: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeometrySummary {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub width: f64,
    pub height: f64,
    pub known_parts: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartInventory {
    pub part_type_id: String,
    pub name: String,
    pub category: String,
    pub count: usize,
    pub catalog_mass: Option<f64>,
    pub current_fuel: f64,
    pub fuel_capacity: f64,
    pub engine_power: f64,
    pub engine_consumption: f64,
    pub rcs_power: f64,
    pub rcs_consumption: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartAnalysis {
    pub id: i64,
    pub part_type_id: String,
    pub name: String,
    pub category: String,
    pub group: Option<usize>,
    pub x: f64,
    pub y: f64,
    pub angle_degrees: f64,
    pub active: bool,
    pub exploded: bool,
    pub fuel: Option<f64>,
    pub fuel_capacity: Option<f64>,
    pub catalog_mass: Option<f64>,
    pub engine_power: Option<f64>,
    pub engine_consumption: Option<f64>,
    pub pod_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PodAnalysis {
    pub part_id: i64,
    pub name: String,
    pub throttle: f64,
    pub current_stage: i32,
    pub steps: Vec<StageStepAnalysis>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageStepAnalysis {
    pub index: usize,
    pub activations: Vec<ActivationAnalysis>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivationAnalysis {
    pub part_id: i64,
    pub moved: bool,
    pub part_type_id: Option<String>,
    pub part_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ConnectionSummary {
    pub total: usize,
    pub normal: usize,
    pub dock: usize,
    pub disconnected_groups: Vec<DisconnectedGroupSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisconnectedGroupSummary {
    pub index: usize,
    pub parts: usize,
    pub connections: usize,
    pub dock_connections: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct PartCatalogFile {
    #[serde(rename = "PartType", default)]
    part_types: Vec<PartCatalogEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct PartCatalogEntry {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@type")]
    part_type: String,
    #[serde(rename = "@mass")]
    mass: f64,
    #[serde(rename = "@category", default)]
    category: Option<String>,
    #[serde(rename = "Tank")]
    tank: Option<CatalogTank>,
    #[serde(rename = "Engine")]
    engine: Option<CatalogEngine>,
    #[serde(rename = "Rcs")]
    rcs: Option<CatalogRcs>,
    #[serde(rename = "Solar")]
    solar: Option<CatalogSolar>,
    #[serde(rename = "@width")]
    width: f64,
    #[serde(rename = "@height")]
    height: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogTank {
    #[serde(rename = "@fuel")]
    fuel: f64,
    #[serde(rename = "@fuelType", default)]
    fuel_type: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogEngine {
    #[serde(rename = "@power")]
    power: f64,
    #[serde(rename = "@consumption")]
    consumption: f64,
    #[serde(rename = "@fuelType", default)]
    fuel_type: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogRcs {
    #[serde(rename = "@power")]
    power: f64,
    #[serde(rename = "@consumption")]
    consumption: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogSolar {
    #[serde(rename = "@chargeRate")]
    charge_rate: f64,
}

impl PartCatalogEntry {
    fn category(&self) -> String {
        self.category
            .clone()
            .unwrap_or_else(|| self.part_type.clone())
    }

    fn fuel_type(&self) -> Option<u8> {
        self.tank
            .as_ref()
            .and_then(|tank| tank.fuel_type)
            .or_else(|| self.engine.as_ref().and_then(|engine| engine.fuel_type))
    }

    fn fuel_capacity(&self) -> Option<f64> {
        self.tank.as_ref().map(|tank| tank.fuel)
    }
}

fn catalog() -> &'static BTreeMap<String, PartCatalogEntry> {
    static PART_CATALOG: OnceLock<BTreeMap<String, PartCatalogEntry>> = OnceLock::new();
    PART_CATALOG.get_or_init(|| {
        let source = include_str!("../../sql/PartList.xml");
        from_str::<PartCatalogFile>(source)
            .map(|file| {
                file.part_types
                    .into_iter()
                    .map(|entry| (entry.id.clone(), entry))
                    .collect()
            })
            .unwrap_or_default()
    })
}

pub fn analyze_ship(document: &ShipDocument) -> ShipAnalysis {
    let catalog = catalog();
    let ship = &document.ship;
    let mut totals = ShipTotals {
        connected_parts: ship.parts.len(),
        disconnected_groups: ship.disconnected.len(),
        ..ShipTotals::default()
    };
    let mut mass = MassSummary {
        scale: MASS_SCALE,
        ..MassSummary::default()
    };
    let mut fuel = FuelSummary::default();
    let mut propulsion = PropulsionSummary::default();
    let mut inventory: BTreeMap<String, InventoryAccumulator> = BTreeMap::new();
    let mut parts = Vec::new();
    let mut all_part_ids = BTreeMap::new();
    let mut bounds = BoundsAccumulator::default();

    let mut add_parts = |items: &[Part], group: Option<usize>| {
        for part in items {
            totals.parts += 1;
            if group.is_some() {
                totals.disconnected_parts += 1;
            }
            if part.activated {
                totals.active_parts += 1;
            }
            if part.exploded {
                totals.exploded_parts += 1;
            }

            let catalog_entry = catalog.get(&part.part_type_id);
            if catalog_entry.is_none() {
                totals.unknown_part_types += 1;
            }
            if let Some(entry) = catalog_entry {
                mass.catalog_units += entry.mass;
                mass.known_parts += 1;
                bounds.add(part, entry);
            }

            let current_fuel = part.attrs.tank_fuel.or(part.attrs.engine_fuel);
            let fuel_capacity = catalog_entry.and_then(PartCatalogEntry::fuel_capacity);
            if let Some(value) = current_fuel {
                fuel.current += value;
            }
            if let Some(value) = fuel_capacity {
                fuel.capacity += value;
            }
            if let Some(entry) = catalog_entry {
                if let Some(value) = current_fuel {
                    let bucket = fuel_bucket(&mut fuel.by_type, entry.fuel_type());
                    bucket.current += value;
                }
                if let Some(value) = fuel_capacity {
                    let bucket = fuel_bucket(&mut fuel.by_type, entry.fuel_type());
                    bucket.capacity += value;
                }
                if let Some(engine) = &entry.engine {
                    propulsion.engines.count += 1;
                    propulsion.engines.power += engine.power;
                    propulsion.engines.consumption += engine.consumption;
                }
                if let Some(rcs) = &entry.rcs {
                    propulsion.rcs.count += 1;
                    propulsion.rcs.power += rcs.power;
                    propulsion.rcs.consumption += rcs.consumption;
                }
                if let Some(solar) = &entry.solar {
                    propulsion.solar_count += 1;
                    propulsion.solar_charge += solar.charge_rate;
                }
            }

            let (name, category, catalog_mass) = catalog_entry
                .map(|entry| (entry.name.clone(), entry.category(), Some(entry.mass)))
                .unwrap_or_else(|| ("未知部件".to_string(), "unknown".to_string(), None));
            let inventory_entry = inventory
                .entry(part.part_type_id.clone())
                .or_insert_with(|| InventoryAccumulator::new(&part.part_type_id, &name, &category));
            inventory_entry.count += 1;
            inventory_entry.catalog_mass = catalog_mass;
            inventory_entry.current_fuel += current_fuel.unwrap_or(0.0);
            inventory_entry.fuel_capacity += fuel_capacity.unwrap_or(0.0);
            if let Some(entry) = catalog_entry {
                if let Some(engine) = &entry.engine {
                    inventory_entry.engine_power += engine.power;
                    inventory_entry.engine_consumption += engine.consumption;
                }
                if let Some(rcs) = &entry.rcs {
                    inventory_entry.rcs_power += rcs.power;
                    inventory_entry.rcs_consumption += rcs.consumption;
                }
            }

            all_part_ids.insert(part.id, (part.part_type_id.clone(), name.clone()));
            parts.push(PartAnalysis {
                id: part.id,
                part_type_id: part.part_type_id.clone(),
                name,
                category,
                group,
                x: part.x,
                y: part.y,
                angle_degrees: part.angle.to_degrees(),
                active: part.activated,
                exploded: part.exploded,
                fuel: current_fuel,
                fuel_capacity,
                catalog_mass,
                engine_power: catalog_entry
                    .and_then(|entry| entry.engine.as_ref().map(|e| e.power)),
                engine_consumption: catalog_entry
                    .and_then(|entry| entry.engine.as_ref().map(|e| e.consumption)),
                pod_name: part.attrs.pod.as_ref().map(|pod| pod.name.clone()),
            });
        }
    };

    add_parts(&ship.parts, None);
    for (index, group) in ship.disconnected.iter().enumerate() {
        add_parts(&group.parts, Some(index));
    }

    let staging = collect_staging(ship, &all_part_ids);
    let connections = collect_connections(ship);
    totals.disconnected_parts = totals.parts.saturating_sub(totals.connected_parts);
    mass.scaled_units = mass.catalog_units * MASS_SCALE;

    ShipAnalysis {
        state: ShipState {
            version: ship.version,
            lifted_off: ship.lifted_off,
            touching_ground: ship.touching_ground,
        },
        totals,
        mass,
        fuel,
        propulsion,
        geometry: bounds.finish(),
        inventory: inventory.into_values().map(Into::into).collect(),
        parts,
        staging,
        connections,
    }
}

#[derive(Debug, Default)]
struct InventoryAccumulator {
    part_type_id: String,
    name: String,
    category: String,
    count: usize,
    catalog_mass: Option<f64>,
    current_fuel: f64,
    fuel_capacity: f64,
    engine_power: f64,
    engine_consumption: f64,
    rcs_power: f64,
    rcs_consumption: f64,
}

impl InventoryAccumulator {
    fn new(id: &str, name: &str, category: &str) -> Self {
        Self {
            part_type_id: id.to_string(),
            name: name.to_string(),
            category: category.to_string(),
            ..Self::default()
        }
    }
}

impl From<InventoryAccumulator> for PartInventory {
    fn from(value: InventoryAccumulator) -> Self {
        Self {
            part_type_id: value.part_type_id,
            name: value.name,
            category: value.category,
            count: value.count,
            catalog_mass: value.catalog_mass,
            current_fuel: value.current_fuel,
            fuel_capacity: value.fuel_capacity,
            engine_power: value.engine_power,
            engine_consumption: value.engine_consumption,
            rcs_power: value.rcs_power,
            rcs_consumption: value.rcs_consumption,
        }
    }
}

fn fuel_bucket(buckets: &mut Vec<FuelBucket>, fuel_type: Option<u8>) -> &mut FuelBucket {
    let key = fuel_type_label(fuel_type).to_string();
    if let Some(index) = buckets.iter().position(|bucket| bucket.fuel_type == key) {
        return &mut buckets[index];
    }
    buckets.push(FuelBucket {
        fuel_type: key,
        current: 0.0,
        capacity: 0.0,
    });
    buckets.last_mut().expect("fuel bucket was inserted")
}

fn fuel_type_label(fuel_type: Option<u8>) -> &'static str {
    match fuel_type.unwrap_or(0) {
        1 => "RCS",
        2 => "电量",
        3 => "固体推进剂",
        _ => "普通燃料",
    }
}

fn collect_staging(
    ship: &crate::xml_part::model::ShipData,
    part_ids: &BTreeMap<i64, (String, String)>,
) -> Vec<PodAnalysis> {
    ship.parts
        .iter()
        .chain(
            ship.disconnected
                .iter()
                .flat_map(|group| group.parts.iter()),
        )
        .filter_map(|part| {
            let pod = part.attrs.pod.as_ref()?;
            Some(PodAnalysis {
                part_id: part.id,
                name: pod.name.clone(),
                throttle: pod.throttle,
                current_stage: pod.current_stage,
                steps: pod
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(index, step)| StageStepAnalysis {
                        index,
                        activations: step
                            .activates
                            .iter()
                            .map(|activation| {
                                let (part_type_id, part_name) = part_ids
                                    .get(&activation.id)
                                    .map(|(id, name)| (Some(id.clone()), Some(name.clone())))
                                    .unwrap_or((None, None));
                                ActivationAnalysis {
                                    part_id: activation.id,
                                    moved: activation.moved,
                                    part_type_id,
                                    part_name,
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            })
        })
        .collect()
}

fn collect_connections(ship: &crate::xml_part::model::ShipData) -> ConnectionSummary {
    let mut summary = ConnectionSummary::default();
    let mut add = |connections: &[Connection]| {
        summary.total += connections.len();
        for connection in connections {
            match connection {
                Connection::Normal { .. } => summary.normal += 1,
                Connection::Dock { .. } => summary.dock += 1,
            }
        }
    };
    add(&ship.connections);
    for (index, group) in ship.disconnected.iter().enumerate() {
        add(&group.connections);
        summary.disconnected_groups.push(DisconnectedGroupSummary {
            index,
            parts: group.parts.len(),
            connections: group.connections.len(),
            dock_connections: group
                .connections
                .iter()
                .filter(|connection| matches!(connection, Connection::Dock { .. }))
                .count(),
        });
    }
    summary
}

#[derive(Debug, Default)]
struct BoundsAccumulator {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    known_parts: usize,
}

impl BoundsAccumulator {
    fn add(&mut self, part: &Part, entry: &PartCatalogEntry) {
        let half_width = entry.width / 2.0;
        let half_height = entry.height / 2.0;
        let (sin, cos) = part.angle.sin_cos();
        for (x, y) in [
            (-half_width, -half_height),
            (half_width, -half_height),
            (half_width, half_height),
            (-half_width, half_height),
        ] {
            let rotated_x = x * cos - y * sin + part.x * 2.0;
            let rotated_y = x * sin + y * cos + part.y * 2.0;
            if self.known_parts == 0 {
                self.min_x = rotated_x;
                self.max_x = rotated_x;
                self.min_y = rotated_y;
                self.max_y = rotated_y;
            } else {
                self.min_x = self.min_x.min(rotated_x);
                self.max_x = self.max_x.max(rotated_x);
                self.min_y = self.min_y.min(rotated_y);
                self.max_y = self.max_y.max(rotated_y);
            }
        }
        self.known_parts += 1;
    }

    fn finish(self) -> Option<GeometrySummary> {
        (self.known_parts > 0).then_some(GeometrySummary {
            min_x: self.min_x,
            min_y: self.min_y,
            max_x: self.max_x,
            max_y: self.max_y,
            width: self.max_x - self.min_x,
            height: self.max_y - self.min_y,
            known_parts: self.known_parts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::analyze_ship;
    use crate::{net::EMPTY_SHIP, xml_part::parse::parse_ship_xml};

    #[test]
    fn analyzes_empty_ship_with_catalog_data() {
        let document = parse_ship_xml(EMPTY_SHIP).expect("empty ship should parse");
        let analysis = analyze_ship(&document);
        assert_eq!(analysis.totals.parts, 1);
        assert_eq!(analysis.mass.known_parts, 1);
        assert_eq!(analysis.mass.scaled_units, 500.0);
        assert_eq!(analysis.inventory[0].part_type_id, "pod-1");
        assert_eq!(analysis.connections.total, 0);
    }

    #[test]
    fn analyzes_propulsion_and_fuel_data() {
        let xml = r#"
            <Ship version="1" liftedOff="1" touchingGround="0">
                <Parts>
                    <Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" editorAngle="0">
                        <Pod throttle="0.5" name="Test"><Staging currentStage="1"><Step><Activate Id="3" moved="1"/></Step></Staging></Pod>
                    </Part>
                    <Part partType="fueltank-1" id="2" x="0" y="2" angle="0" angleV="0" editorAngle="0">
                        <Tank fuel="1000"/>
                    </Part>
                    <Part partType="engine-1" id="3" x="0" y="4" angle="0" angleV="0" editorAngle="0">
                        <Engine fuel="20"/>
                    </Part>
                </Parts>
                <Connections>
                    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="1" childPart="2"/>
                    <Connection parentAttachPoint="0" childAttachPoint="1" parentPart="2" childPart="3"/>
                </Connections>
                <DisconnectedParts/>
            </Ship>
        "#;
        let document = parse_ship_xml(xml).expect("ship should parse");
        let analysis = analyze_ship(&document);
        assert_eq!(analysis.totals.parts, 3);
        assert_eq!(analysis.propulsion.engines.count, 1);
        assert_eq!(analysis.propulsion.engines.power, 1.0);
        assert_eq!(analysis.fuel.current, 1020.0);
        assert_eq!(analysis.fuel.by_type.len(), 1);
        assert_eq!(analysis.staging[0].steps[0].activations[0].part_id, 3);
        assert_eq!(analysis.connections.normal, 2);
    }
}
