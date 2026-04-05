#[derive(Debug, Clone, PartialEq)]
pub enum XmlDocument {
    Ship(ShipDocument),
    Save(SaveDocument),
}

impl XmlDocument {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Ship(_) => "Ship",
            Self::Save(_) => "Save",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipDocument {
    pub ship: ShipData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveDocument {
    pub time: f64,
    pub first_stage_activated: bool,
    pub solar_system: String,
    pub ship_id: i64,
    pub pod_id: i64,
    pub nodes: Vec<SaveNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SaveNode {
    Planet(PlanetNode),
    Ship(ShipNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanetNode {
    pub name: String,
    pub true_anomaly: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipNode {
    pub id: i64,
    pub planet: String,
    pub planet_radius: f64,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub ship: ShipData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipData {
    pub version: i32,
    pub lifted_off: bool,
    pub touching_ground: bool,
    pub parts: Vec<Part>,
    pub connections: Vec<Connection>,
    pub disconnected: Vec<DisconnectedGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisconnectedGroup {
    pub parts: Vec<Part>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub part_type_id: String,
    pub id: i64,
    pub x: f64,
    pub y: f64,
    pub editor_angle: i32,
    pub angle: f64,
    pub angle_v: f64,
    pub flipped_x: bool,
    pub flipped_y: bool,
    pub activated: bool,
    pub exploded: bool,
    pub attrs: PartAttrs,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartAttrs {
    pub tank_fuel: Option<f64>,
    pub engine_fuel: Option<f64>,
    pub pod: Option<PodData>,
    pub chute_x: Option<f64>,
    pub chute_y: Option<f64>,
    pub chute_angle: Option<f64>,
    pub chute_height: Option<f64>,
    pub extension: Option<f64>,
    pub inflate: Option<bool>,
    pub inflation: Option<f64>,
    pub deployed: Option<bool>,
    pub rope: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PodData {
    pub name: String,
    pub throttle: f64,
    pub current_stage: i32,
    pub steps: Vec<StageStep>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StageStep {
    pub activates: Vec<Activation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Activation {
    pub id: i64,
    pub moved: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Connection {
    Normal {
        parent_attach_point: i32,
        child_attach_point: i32,
        parent_part: i64,
        child_part: i64,
    },
    Dock {
        dock_part: i64,
        parent_part: i64,
        child_part: i64,
    },
}
