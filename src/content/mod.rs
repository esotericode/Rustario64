pub mod animation;
pub mod visual;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseId(pub u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelId(pub u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreaId(pub u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Act(u8);
impl Act {
    pub fn new(value: u8) -> Option<Self> {
        (1..=6).contains(&value).then_some(Self(value))
    }
    pub fn number(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActMask(pub u8);
impl ActMask {
    /// Original OBJECT uses 0x1F as an all-acts sentinel, including act six.
    pub fn includes(self, act: Act) -> bool {
        self.0 == 0x1F || self.0 & (1 << (act.0 - 1)) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BehaviorId(pub u16);

/// Register only implemented behaviors; unresolved scripts stay in import diagnostics.
#[derive(Default)]
pub struct BehaviorRegistry {
    by_script: BTreeMap<u32, BehaviorId>,
}
impl BehaviorRegistry {
    pub fn register(&mut self, script: u32, id: BehaviorId) -> Option<BehaviorId> {
        self.by_script.insert(script, id)
    }
    pub fn resolve(&self, script: u32) -> Option<BehaviorId> {
        self.by_script.get(&script).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Triangle {
    pub indices: [u16; 3],
    pub surface: i16,
    pub force: Option<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialPlacement {
    pub preset: u8,
    pub position: [i16; 3],
    /// Original shorts, including rotation/params; interpretation is preset-specific.
    pub extra: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollisionMesh {
    pub vertices: Vec<[i16; 3]>,
    /// Source order retained for future reference collision loading and tie breaking.
    pub triangles: Vec<Triangle>,
    pub specials: Vec<SpecialPlacement>,
    pub environment: Vec<[i16; 6]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedSpawn {
    pub source_address: u32,
    pub acts: ActMask,
    pub model: u8,
    pub position: [i16; 3],
    pub angles_degrees: [i16; 3],
    pub behavior_params: u32,
    /// Import-only unresolved reference. Must resolve to BehaviorId before simulation.
    pub behavior_script: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroPlacement {
    pub source_address: u32,
    /// Preserve the packed word as well as its decoded fields.
    pub packed_preset_and_yaw: u16,
    /// Import-only index into the revision's macro preset table, not a BehaviorId.
    pub preset_id: u16,
    pub position: [i16; 3],
    /// Original signed angle units, after the packed rotation conversion.
    pub yaw: i16,
    /// Unmodified placement word: preset defaults and respawn rules are not applied.
    pub raw_params: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarpNode {
    pub id: u8,
    pub destination_level: LevelId,
    pub destination_area: AreaId,
    pub destination_node: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportIssue {
    pub address: u32,
    pub feature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedArea {
    pub id: AreaId,
    pub geometry_layout: u32,
    pub terrain: Option<u32>,
    pub macro_objects: Option<u32>,
    pub macro_spawns: Vec<MacroPlacement>,
    pub spawns: Vec<ImportedSpawn>,
    pub warps: Vec<WarpNode>,
    /// Original area defaults and TERRAIN_TYPE bitwise accumulation.
    pub terrain_type: u16,
    /// SHOW_DIALOG's two slots; 0xFF is the original DIALOG_NONE sentinel.
    pub dialog_ids: [u8; 2],
    pub background_music: AreaMusic,
}

/// Original signed music words, retained as data; no audio is executed at import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreaMusic {
    pub settings_preset: i16,
    pub sequence: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedLevel {
    pub course: CourseId,
    pub level: LevelId,
    pub areas: Vec<ImportedArea>,
    /// Raw MARIO_POS fields: area, yaw in signed degrees, and position.
    /// Runtime angle units use `(i32::from(yaw) * 0x8000 / 180) as i16`.
    pub mario_start: Option<(AreaId, i16, [i16; 3])>,
    pub segment_loads: Vec<SegmentLoad>,
    pub models: Vec<ModelDefinition>,
    pub commands: BTreeMap<u8, usize>,
    pub issues: Vec<ImportIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentLoad {
    pub address: u32,
    pub segment: u8,
    pub rom_start: u32,
    pub rom_end: u32,
    pub mio0: bool,
    pub texture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub model: u16,
    pub pointer: u32,
    pub geometry_layout: bool,
    pub layer: Option<u8>,
}

/// Runtime transitions will invalidate presentation snapshots through an epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    Area { level: LevelId, area: AreaId },
    Death,
    Teleport,
}

#[derive(Default)]
pub struct WorldCollision {
    pub static_meshes: Vec<CollisionMesh>,
    pub dynamic_meshes: BTreeMap<u32, CollisionMesh>,
}
