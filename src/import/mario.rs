//! Mario's model from the identified ROM. The main level scripts'
//! `level_main_scripts_entry` loads group0 (Mario's display lists and textures
//! as segment 4, his geo layouts as segment 0x17) and registers MODEL_MARIO
//! from `mario_geo`; the scan reads those commands rather than assuming
//! addresses, and the ranges must agree with the version adapter. The generic
//! geo decoder decodes the layout. Mario's native callbacks resolve by address
//! through the version adapter to `MarioCallback` roles, which presentation
//! implements. Display lists are built per switch configuration on demand from
//! the segments, which stay in memory and are never exported.
use super::{
    ImportError, Result,
    bob::load_segment,
    geo::{self, GeoLayout, GeoNodeKind},
    gfx::{Builder, IDENTITY},
    reader::Reader,
    rom::Rom,
    segments::Segments,
    version,
};
use crate::content::{
    ImportIssue, SegmentLoad,
    visual::{LAYER_COUNT, SkinnedModel},
};
use serde::Serialize;
use std::collections::BTreeMap;

/// One item a render traversal appends to a layer's master list, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Draw {
    /// A display list drawn with `bone`'s matrix (a geo node index, or
    /// `MarioModelSource::object_bone` for the object's own matrix).
    List { layer: u8, address: u32, bone: u16 },
    /// A generated display list's G_SETENVCOLOR (geo_mirror_mario_set_alpha).
    EnvColor { layer: u8, color: [u8; 4] },
}

impl Draw {
    pub fn layer(&self) -> u8 {
        match *self {
            Draw::List { layer, .. } | Draw::EnvColor { layer, .. } => layer,
        }
    }
}

/// Mario's native geo callbacks (pinned src/game/mario_misc.c), as roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MarioCallback {
    /// geo_mirror_mario_backface_culling: only acts for the castle mirror's Mario.
    MirrorBackfaceCulling,
    /// geo_mirror_mario_set_alpha: the vanish cap's alpha and layer.
    MirrorSetAlpha,
    /// geo_switch_mario_stand_run: full detail while stationary, LOD otherwise.
    SwitchStandRun,
    /// geo_switch_mario_cap_effect: normal, vanish, metal or metal-vanish body.
    SwitchCapEffect,
    /// geo_switch_mario_cap_on_off: cap on or off, and the wings' visibility.
    SwitchCapOnOff,
    /// geo_switch_mario_eyes: blinking or a fixed eye state.
    SwitchEyes,
    /// geo_switch_mario_hand: fists, open, peace sign, holding a cap.
    SwitchHand,
    /// geo_mario_head_rotation: the head turn while reading or in water.
    HeadRotation,
    /// geo_mario_tilt_torso: the torso tilt while walking and sliding.
    TiltTorso,
    /// geo_mario_rotate_wing_cap_wings: the wings' flap.
    RotateWingCapWings,
    /// geo_mario_hand_foot_scaler: the punch and kick scale.
    HandFootScaler,
    /// geo_move_mario_part_from_parent: positions a held object.
    MovePartFromParent,
    /// geo_switch_mario_hand_grab_pos: where a held object sits.
    HandGrabPos,
}

impl MarioCallback {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "geo_mirror_mario_backface_culling" => Self::MirrorBackfaceCulling,
            "geo_mirror_mario_set_alpha" => Self::MirrorSetAlpha,
            "geo_switch_mario_stand_run" => Self::SwitchStandRun,
            "geo_switch_mario_cap_effect" => Self::SwitchCapEffect,
            "geo_switch_mario_cap_on_off" => Self::SwitchCapOnOff,
            "geo_switch_mario_eyes" => Self::SwitchEyes,
            "geo_switch_mario_hand" => Self::SwitchHand,
            "geo_mario_head_rotation" => Self::HeadRotation,
            "geo_mario_tilt_torso" => Self::TiltTorso,
            "geo_mario_rotate_wing_cap_wings" => Self::RotateWingCapWings,
            "geo_mario_hand_foot_scaler" => Self::HandFootScaler,
            "geo_move_mario_part_from_parent" => Self::MovePartFromParent,
            "geo_switch_mario_hand_grab_pos" => Self::HandGrabPos,
            _ => return None,
        })
    }
}

/// Mario's decoded geo layout with its segments and resolved callbacks.
pub struct MarioModelSource {
    pub geo: GeoLayout,
    pub(crate) segments: Segments,
    pub callbacks: BTreeMap<u32, MarioCallback>,
    pub loads: Vec<SegmentLoad>,
}

impl MarioModelSource {
    /// Decode `layout` from loaded segments (the ROM's, or an authored
    /// fixture's) with these callback roles; every native callback in the
    /// layout must have one.
    pub fn from_segments(
        segments: Segments,
        layout: u32,
        callbacks: BTreeMap<u32, MarioCallback>,
        loads: Vec<SegmentLoad>,
    ) -> Result<Self> {
        let geo = geo::decode(&segments, layout)?;
        for node in &geo.nodes {
            let address = match node.kind {
                GeoNodeKind::SwitchCase { callback, .. }
                | GeoNodeKind::Generated { callback, .. }
                | GeoNodeKind::HeldObject { callback, .. } => callback,
                _ => continue,
            };
            if !callbacks.contains_key(&address) {
                return Err(ImportError::new(
                    "Mario geo",
                    node.source_address as usize,
                    format!("unresolved native callback 0x{address:08X}"),
                ));
            }
        }
        Ok(Self {
            geo,
            segments,
            callbacks,
            loads,
        })
    }

    /// The bone of display lists drawn with the object's own matrix.
    pub fn object_bone(&self) -> u16 {
        self.geo.nodes.len() as u16
    }

    /// Run a traversal's draws as geo_process_master_list_sub does: layer by
    /// layer, in order within a layer, Z-buffered, RSP/RDP state carried from
    /// one list to the next. Each vertex keeps the bone of its list.
    pub fn build(&self, draws: &[Draw]) -> Result<(SkinnedModel, Vec<ImportIssue>)> {
        let mut builder = Builder::new(&self.segments);
        for layer in 0..LAYER_COUNT as u8 {
            for draw in draws.iter().filter(|d| d.layer() == layer) {
                match *draw {
                    Draw::List { address, bone, .. } => {
                        builder.set_bone(bone);
                        builder.run(address, layer, &IDENTITY, true)?;
                    }
                    Draw::EnvColor { color, .. } => builder.set_env_color(color),
                }
            }
        }
        Ok(builder.finish_skinned())
    }

    /// The callback role of a SwitchCase, Generated or HeldObject node.
    pub fn callback(&self, node: usize) -> Option<MarioCallback> {
        let address = match self.geo.nodes[node].kind {
            GeoNodeKind::SwitchCase { callback, .. }
            | GeoNodeKind::Generated { callback, .. }
            | GeoNodeKind::HeldObject { callback, .. } => callback,
            _ => return None,
        };
        self.callbacks.get(&address).copied()
    }
}

/// What `level_main_scripts_entry` loads before Mario's model, and the
/// layout it registers for MODEL_MARIO.
fn scan_main_scripts(scripts: &[u8]) -> Result<(Vec<SegmentLoad>, u32)> {
    let base = u32::from(version::MAIN_SCRIPTS_SEGMENT) << 24;
    let mut loads = vec![];
    let mut offset = 0;
    // Loads, ALLOC_LEVEL_POOL, then LOAD_MODEL_FROM_GEO commands; Mario's is
    // the first model.
    for _ in 0..64 {
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
            (0x22, 8) => {
                let model = r.u16(2)? & 0xFFF;
                if model != version::MODEL_MARIO {
                    return Err(ImportError::new(
                        "main scripts",
                        offset,
                        format!("first model is {model}, not MODEL_MARIO"),
                    ));
                }
                return Ok((loads, r.u32(4)?));
            }
            _ => {
                return Err(ImportError::new(
                    "main scripts",
                    offset,
                    format!(
                        "unexpected command 0x{opcode:02X} (length {length}) before Mario's model"
                    ),
                ));
            }
        }
        offset += length;
    }
    Err(ImportError::new(
        "main scripts",
        offset,
        "no MODEL_MARIO registration",
    ))
}

/// Load Mario's segments and decode his geo layout.
pub fn import(rom: &Rom) -> Result<MarioModelSource> {
    let range = version::MAIN_LEVEL_SCRIPTS;
    let scripts = rom.reader().slice(range.start, range.len())?;
    let (loads, layout) = scan_main_scripts(scripts)?;
    let mut segments = Segments::default();
    for (segment, expected, mio0) in [
        (version::GROUP0_SEGMENT, version::GROUP0_MIO0, true),
        (version::GROUP0_GEO_SEGMENT, version::GROUP0_GEO, false),
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
        segments.insert(segment, load_segment(rom, load)?)?;
    }
    if layout >> 24 != u32::from(version::GROUP0_GEO_SEGMENT) {
        return Err(ImportError::new(
            "main scripts",
            layout as usize,
            "MODEL_MARIO's layout is outside group0's geo segment",
        ));
    }
    let mut callbacks = BTreeMap::new();
    for (name, address) in version::MARIO_GEO_CALLBACKS {
        let role = MarioCallback::from_name(name).ok_or_else(|| {
            ImportError::new(
                "Mario geo",
                address as usize,
                format!("unknown callback {name}"),
            )
        })?;
        callbacks.insert(address, role);
    }
    MarioModelSource::from_segments(segments, layout, callbacks, loads)
}
