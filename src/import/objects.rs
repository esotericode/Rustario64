//! Object content from the identified ROM: the behavior segment the main
//! level scripts load (segment 0x13), sMacroObjectPresets, the models the
//! main scripts and a level register (gLoadedGraphNodes), the render
//! traversals of the main scripts' geo models, and an area's placements
//! resolved into the simulation's spawn records.
//!
//! Addresses come from the version adapter, which tools/check_behavior_reference.py
//! and tools/check_object_model_reference.py verify against the pinned
//! decomp; the importer additionally requires the main scripts to load
//! exactly the adapter's segment ranges.
use super::{
    ImportError, Result, animation,
    bob::load_segment,
    geo::{self, GeoNodeKind},
    level,
    reader::Reader,
    rom::Rom,
    segments::Segments,
    version,
};
use crate::{
    content::{Act, ImportedArea, SegmentLoad, animation::ObjectAnimations},
    simulation::{
        mario::{
            render::{MarioCallback, MarioModel},
            tick::LevelObjects,
        },
        object::{
            render::{ObjectModel, ObjectModels, RenderNode, RenderNodeKind},
            script::{Behavior, BehaviorScripts, Native},
            spawn::{AreaObjects, MacroEntry, SpawnInfo, convert_rotation},
        },
    },
};
use std::collections::BTreeMap;

/// One sMacroObjectPresets entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroPreset {
    pub behavior: u32,
    pub model: i16,
    pub param: i16,
}

/// A LOAD_MODEL_FROM_GEO or LOAD_MODEL_FROM_DL registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRegistration {
    pub address: u32,
    pub pointer: u32,
    pub geometry_layout: bool,
    /// LOAD_MODEL_FROM_DL's layer.
    pub layer: Option<u8>,
}

/// What the main level scripts provide for objects.
pub struct ObjectContent {
    pub scripts: BehaviorScripts,
    pub presets: Vec<MacroPreset>,
    /// level_main_scripts_entry's registrations, by model ID (last wins).
    pub main_models: BTreeMap<u16, ModelRegistration>,
    /// The main scripts' segments that hold object models (3, 4, 0x16, 0x17).
    pub segments: Segments,
    pub loads: Vec<SegmentLoad>,
}

/// Segment loads and model registrations, in script order.
type MainEntry = (Vec<SegmentLoad>, Vec<(u16, ModelRegistration)>);

/// level_main_scripts_entry up to its FREE_LEVEL_POOL: segment loads and
/// model registrations, in order.
fn scan_main_entry(scripts: &[u8]) -> Result<MainEntry> {
    let base = u32::from(version::MAIN_SCRIPTS_SEGMENT) << 24;
    let (mut loads, mut models) = (vec![], vec![]);
    let mut offset = 0;
    for _ in 0..512 {
        let r = Reader::new(
            scripts
                .get(offset..)
                .ok_or_else(|| ImportError::new("main scripts", offset, "truncated"))?,
            "main scripts",
        );
        let (opcode, length) = (r.u8(0)?, usize::from(r.u8(1)?));
        let address = base | offset as u32;
        match (opcode, length) {
            (0x17 | 0x18, 12) => loads.push(SegmentLoad {
                address,
                segment: u8::try_from(r.u16(2)?).map_err(|_| {
                    ImportError::new("main scripts", offset, "segment ID out of range")
                })?,
                rom_start: r.u32(4)?,
                rom_end: r.u32(8)?,
                mio0: opcode == 0x18,
                texture: false,
            }),
            (0x1D, 4) => {}
            (0x21 | 0x22, 8) => {
                let field = r.u16(2)?;
                models.push((
                    field & 0xFFF,
                    ModelRegistration {
                        address,
                        pointer: r.u32(4)?,
                        geometry_layout: opcode == 0x22,
                        layer: (opcode == 0x21).then_some((field >> 12) as u8),
                    },
                ));
            }
            (0x1E, 4) => return Ok((loads, models)),
            _ => {
                return Err(ImportError::new(
                    "main scripts",
                    offset,
                    format!("unexpected command 0x{opcode:02X} (length {length})"),
                ));
            }
        }
        offset += length;
    }
    Err(ImportError::new(
        "main scripts",
        offset,
        "no FREE_LEVEL_POOL",
    ))
}

/// Load the main scripts' object content.
pub fn import(rom: &Rom) -> Result<ObjectContent> {
    let range = version::MAIN_LEVEL_SCRIPTS;
    let scripts = rom.reader().slice(range.start, range.len())?;
    let (loads, registrations) = scan_main_entry(scripts)?;
    let mut segments = Segments::default();
    let mut behavior_bytes = None;
    for (segment, expected, mio0) in [
        (version::GROUP0_SEGMENT, version::GROUP0_MIO0, true),
        (version::GROUP0_GEO_SEGMENT, version::GROUP0_GEO, false),
        (version::COMMON1_SEGMENT, version::COMMON1_MIO0, true),
        (version::COMMON1_GEO_SEGMENT, version::COMMON1_GEO, false),
        (version::BEHAVIOR_SEGMENT, version::BEHAVIOR_DATA, false),
    ] {
        let load = loads.iter().find(|l| l.segment == segment).ok_or_else(|| {
            ImportError::new(
                "main scripts",
                0,
                format!("segment 0x{segment:02X} is not loaded"),
            )
        })?;
        if (load.rom_start as usize, load.rom_end as usize) != (expected.start, expected.end)
            || load.mio0 != mio0
        {
            return Err(ImportError::new(
                "main scripts",
                load.address as usize,
                format!("segment 0x{segment:02X} load disagrees with the pinned manifest"),
            ));
        }
        let bytes = load_segment(rom, load)?;
        if segment == version::BEHAVIOR_SEGMENT {
            behavior_bytes = Some(bytes);
        } else {
            segments.insert(segment, bytes)?;
        }
    }
    let natives = version::BEHAVIOR_NATIVES
        .iter()
        .map(|(name, address)| {
            Native::from_name(name)
                .map(|n| (n, *address))
                .ok_or_else(|| ImportError::new("behavior natives", *address as usize, *name))
        })
        .collect::<Result<Vec<_>>>()?;
    let behaviors = version::BEHAVIOR_SCRIPTS
        .iter()
        .map(|(name, address)| {
            Behavior::from_name(name)
                .map(|b| (b, *address))
                .ok_or_else(|| ImportError::new("behavior scripts", *address as usize, *name))
        })
        .collect::<Result<Vec<_>>>()?;
    let scripts = BehaviorScripts::new(&behavior_bytes.unwrap(), natives, behaviors)
        .map_err(|e| ImportError::new("behavior segment", 0, e))?;
    let table = version::MACRO_PRESET_TABLE;
    let r = Reader::new(
        rom.reader().slice(table.start, table.len())?,
        "macro presets",
    );
    let presets = (0..usize::from(version::MACRO_PRESET_COUNT))
        .map(|i| {
            Ok(MacroPreset {
                behavior: r.u32(8 * i)?,
                model: r.i16(8 * i + 4)?,
                param: r.i16(8 * i + 6)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ObjectContent {
        scripts,
        presets,
        main_models: registrations.into_iter().collect(),
        segments,
        loads,
    })
}

/// The render traversal of a geo layout: switch callbacks resolve through
/// the version adapter; anything else with a callback, and level-of-detail
/// nodes, are unaudited.
pub fn traversal(segments: &Segments, layout: u32) -> Result<ObjectModel> {
    let decoded = geo::decode(segments, layout)?;
    let root = decoded
        .root
        .ok_or_else(|| ImportError::new("object geo", layout as usize, "empty layout"))?;
    let callback = |address: u32| {
        version::OBJECT_GEO_CALLBACKS
            .iter()
            .find(|(_, a)| *a == address)
            .map(|(name, _)| *name)
    };
    let nodes = decoded
        .nodes
        .iter()
        .map(|node| {
            let kind = match node.kind {
                GeoNodeKind::SwitchCase {
                    num_cases,
                    callback: function,
                } => match callback(function) {
                    Some("geo_switch_anim_state") => RenderNodeKind::AnimStateSwitch { num_cases },
                    _ => RenderNodeKind::Unaudited {
                        what: format!("switch callback 0x{function:08X}"),
                    },
                },
                GeoNodeKind::CullingRadius { radius } => RenderNodeKind::CullingRadius { radius },
                GeoNodeKind::Generated { callback: f, .. }
                | GeoNodeKind::HeldObject { callback: f, .. } => RenderNodeKind::Unaudited {
                    what: format!("geo callback 0x{f:08X}"),
                },
                GeoNodeKind::Perspective {
                    callback: Some(f), ..
                }
                | GeoNodeKind::Background {
                    callback: Some(f), ..
                }
                | GeoNodeKind::Camera { callback: f, .. } => RenderNodeKind::Unaudited {
                    what: format!("geo callback 0x{f:08X}"),
                },
                GeoNodeKind::LevelOfDetail { .. } => RenderNodeKind::Unaudited {
                    what: "a level-of-detail node".into(),
                },
                _ => RenderNodeKind::Plain,
            };
            RenderNode {
                kind,
                children: node.children.clone(),
            }
        })
        .collect();
    Ok(ObjectModel { nodes, root })
}

/// The model registrations a level script makes between its pool allocation
/// and FREE_LEVEL_POOL, following its JUMP_LINKs into the main scripts'
/// global model functions (script_func_global_N) and its own segment.
pub fn level_models(
    level_segment: &[u8],
    entry: u32,
    main_scripts: &[u8],
) -> Result<Vec<(u16, ModelRegistration)>> {
    let mut out = vec![];
    let mut stack = vec![];
    let mut address = entry;
    for _ in 0..4096 {
        let (bytes, base) = match (address >> 24) as u8 {
            s if s == version::SCRIPT_SEGMENT => (level_segment, 0),
            s if s == version::MAIN_SCRIPTS_SEGMENT => (main_scripts, 0),
            other => {
                return Err(ImportError::new(
                    "level models",
                    address as usize,
                    format!("script in unmapped segment 0x{other:02X}"),
                ));
            }
        };
        let offset = (address & 0xFF_FFFF) as usize + base;
        let r = Reader::new(
            bytes
                .get(offset..)
                .ok_or_else(|| ImportError::new("level models", address as usize, "truncated"))?,
            "level models",
        );
        let (opcode, length) = (r.u8(0)?, usize::from(r.u8(1)?));
        if level::expected_length(opcode) != Some(length) {
            return Err(ImportError::new(
                "level models",
                address as usize,
                format!("opcode 0x{opcode:02X} length {length}"),
            ));
        }
        let next = address + length as u32;
        match opcode {
            0x06 => {
                stack.push(next);
                address = r.u32(4)?;
                continue;
            }
            0x07 => {
                address = stack.pop().ok_or_else(|| {
                    ImportError::new("level models", address as usize, "RETURN without caller")
                })?;
                continue;
            }
            0x21 | 0x22 => {
                let field = r.u16(2)?;
                out.push((
                    field & 0xFFF,
                    ModelRegistration {
                        address,
                        pointer: r.u32(4)?,
                        geometry_layout: opcode == 0x22,
                        layer: (opcode == 0x21).then_some((field >> 12) as u8),
                    },
                ));
            }
            0x1E if stack.is_empty() => return Ok(out),
            0x02 | 0x04 | 0x05 => {
                return Err(ImportError::new(
                    "level models",
                    address as usize,
                    "script ended before FREE_LEVEL_POOL",
                ));
            }
            _ => {}
        }
        address = next;
    }
    Err(ImportError::new(
        "level models",
        address as usize,
        "command limit",
    ))
}

impl ObjectContent {
    /// gLoadedGraphNodes after the main scripts and a level's registrations,
    /// with traversals for the main scripts' geo models and, given the
    /// level's own loaded segments, the level's geo models; a model whose
    /// layout cannot be decoded from those segments is loaded without one.
    pub fn models(
        &self,
        level: &[(u16, ModelRegistration)],
        level_segments: Option<&Segments>,
    ) -> ObjectModels {
        let mut models = ObjectModels::default();
        for (&model, registration) in &self.main_models {
            let traversal = if registration.geometry_layout {
                traversal(&self.segments, registration.pointer).ok()
            } else {
                Some(ObjectModel {
                    nodes: vec![RenderNode {
                        kind: RenderNodeKind::Plain,
                        children: vec![],
                    }],
                    root: 0,
                })
            };
            models.insert(model, traversal);
        }
        for (model, registration) in level {
            let traversal = level_segments
                .filter(|_| registration.geometry_layout)
                .and_then(|segments| traversal(segments, registration.pointer).ok());
            models.insert(*model, traversal);
        }
        if let Some(model) = self.mario_model() {
            models.set_mario_model(model);
        }
        models
    }

    /// MODEL_MARIO's graph from its registration (`mario_geo`), with the
    /// version adapter's callback roles; None if a callback is unknown or
    /// the layout does not decode.
    pub fn mario_model(&self) -> Option<MarioModel> {
        let registration = self.main_models.get(&version::MODEL_MARIO)?;
        if !registration.geometry_layout {
            return None;
        }
        let layout = geo::decode(&self.segments, registration.pointer).ok()?;
        let callbacks: BTreeMap<u32, MarioCallback> = version::MARIO_GEO_CALLBACKS
            .iter()
            .map(|&(name, address)| MarioCallback::from_name(name).map(|role| (address, role)))
            .collect::<Option<_>>()?;
        for node in &layout.nodes {
            if let GeoNodeKind::SwitchCase { callback, .. }
            | GeoNodeKind::Generated { callback, .. }
            | GeoNodeKind::HeldObject { callback, .. } = node.kind
                && !callbacks.contains_key(&callback)
            {
                return None;
            }
        }
        Some(MarioModel { layout, callbacks })
    }

    /// The animation tables of every LOAD_ANIMATIONS the port's runnable
    /// behaviors reach, decoded from whichever of the main scripts' or the
    /// level's segments holds them. A table in a segment this level does not
    /// load is left out; an object that animates from it cannot spawn here
    /// (its model is not loaded either) and would stop with a clear panic.
    pub fn animations(&self, level_segments: Option<&Segments>) -> Result<ObjectAnimations> {
        let mut out = ObjectAnimations::default();
        for behavior in Behavior::ALL {
            let script = self.scripts.address(behavior);
            let Ok(tables) = self.scripts.animation_tables(script) else {
                continue;
            };
            for table in tables {
                let segment = (table >> 24) as u8;
                let segments = if self.segments.is_mapped(segment) {
                    &self.segments
                } else if let Some(level) = level_segments.filter(|l| l.is_mapped(segment)) {
                    level
                } else {
                    continue;
                };
                animation::decode_object_table(segments, table, &mut out)?;
            }
        }
        Ok(out)
    }

    /// An area's placements for an act: the macro list with presets
    /// resolved, and the OBJECT commands the act includes, in the order the
    /// level script links them (each prepends to the area's list).
    pub fn area_objects(&self, area: &ImportedArea, act: Act) -> Result<AreaObjects> {
        let macros = area
            .macro_spawns
            .iter()
            .map(|placement| {
                let preset = self
                    .presets
                    .get(usize::from(placement.preset_id))
                    .ok_or_else(|| {
                        ImportError::new(
                            "macro objects",
                            placement.source_address as usize,
                            "preset outside the table",
                        )
                    })?;
                let packed = placement.packed_preset_and_yaw as i16;
                Ok(MacroEntry {
                    source: placement.source_address,
                    packed: placement.packed_preset_and_yaw,
                    preset: placement.preset_id,
                    behavior: preset.behavior,
                    model: preset.model,
                    preset_param: preset.param,
                    pos: placement.position,
                    yaw: convert_rotation(((packed >> 9) & 0x7F) << 1),
                    params: placement.raw_params,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let spawn_infos = area
            .spawns
            .iter()
            .rev()
            .filter(|spawn| spawn.acts.includes(act))
            .map(|spawn| SpawnInfo {
                source: spawn.source_address,
                start_pos: spawn.position,
                start_angle: spawn
                    .angles_degrees
                    .map(|degrees| (i32::from(degrees) * 0x8000 / 180) as i16),
                area_index: area.id.0 as i8,
                active_area_index: area.id.0 as i8,
                behavior_arg: spawn.behavior_params,
                behavior_script: spawn.behavior_script,
                model: spawn.model,
            })
            .collect();
        Ok(AreaObjects {
            area_index: area.id.0 as i8,
            macros,
            spawn_infos,
            skipped: vec![],
        })
    }
}

/// A level's object content: the main scripts' content, gLoadedGraphNodes
/// after the level's registrations, the object animations its runnable
/// behaviors use, the level's own loaded segments (for drawing its models)
/// and the entered area's placements.
pub struct LevelObjectContent {
    pub content: ObjectContent,
    pub models: ObjectModels,
    pub animations: ObjectAnimations,
    pub level_segments: Segments,
    /// The level's model registrations (gLoadedGraphNodes entries it adds).
    pub level_models: Vec<(u16, ModelRegistration)>,
    pub area: AreaObjects,
}

impl LevelObjectContent {
    /// The level's objects as the simulation enters them.
    pub fn level_objects(&self) -> LevelObjects<'_> {
        LevelObjects {
            scripts: &self.content.scripts,
            models: &self.models,
            animations: &self.animations,
            area: self.area.clone(),
        }
    }

    /// The same scripts and models with no placements: Mario alone.
    pub fn mario_only(&self) -> LevelObjects<'_> {
        LevelObjects::mario_only(&self.content.scripts, &self.models, &self.animations)
    }
}

/// Bob-omb Battlefield's object content for an act, from its import.
pub fn bob(
    rom: &Rom,
    level: &crate::content::ImportedLevel,
    act: Act,
) -> Result<LevelObjectContent> {
    let content = import(rom)?;
    let range = version::BOB_LEVEL;
    let script = rom.reader().slice(range.start, range.len())?;
    let entry = level::bob_entry(script)?;
    let main = version::MAIN_LEVEL_SCRIPTS;
    let main = rom.reader().slice(main.start, main.len())?;
    let registrations = level_models(script, entry, main)?;
    // The level's script segment also holds its own geo layouts.
    let mut level_segments = Segments::default();
    level_segments.insert(version::SCRIPT_SEGMENT, script.to_vec())?;
    for load in &level.segment_loads {
        let bytes = load_segment(rom, load)?;
        level_segments
            .insert(load.segment, bytes)
            .map_err(|e| ImportError::new("segment load", load.address as usize, e.to_string()))?;
    }
    let models = content.models(&registrations, Some(&level_segments));
    let animations = content.animations(Some(&level_segments))?;
    let (area_id, ..) = level
        .mario_start
        .ok_or_else(|| ImportError::new("BOB", 0, "no Mario start"))?;
    let area = level
        .areas
        .iter()
        .find(|a| a.id == area_id)
        .ok_or_else(|| ImportError::new("BOB", 0, "Mario's start area was not imported"))?;
    let area = content.area_objects(area, act)?;
    Ok(LevelObjectContent {
        content,
        models,
        animations,
        level_segments,
        level_models: registrations,
        area,
    })
}
