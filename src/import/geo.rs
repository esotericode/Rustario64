//! Bounded geo-layout decoder. Command widths, control flow and node attachment
//! follow pinned CC0 sm64 include/geo_commands.h, src/engine/geo_layout.c and
//! src/engine/graph_node_manager.c. See PROVENANCE.md.
//!
//! The result is an import-stage scene graph: it still holds segmented pointers
//! and native callback addresses. Callbacks are recorded, never executed; the
//! visual-model builder decides how to treat each node kind and reports gaps.
use super::{ImportError, Result, reader::Reader, segments::Segments};
use serde::Serialize;

/// The original stack and node list have 16 and 32 entries.
const MAX_STACK: usize = 16;
const MAX_DEPTH: usize = 32;
const MAX_COMMANDS: usize = 8192;

pub const GRAPH_RENDER_ACTIVE: u16 = 1 << 0;
pub const GRAPH_RENDER_Z_BUFFER: u16 = 1 << 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GeoNodeKind {
    /// GEO_NODE_SCREEN_AREA: view count, then centre and half-extent in screen units.
    Root {
        views: i16,
        x: i16,
        y: i16,
        width: i16,
        height: i16,
    },
    Ortho {
        scale: i16,
    },
    Perspective {
        fov: i16,
        near: i16,
        far: i16,
        callback: Option<u32>,
    },
    Start,
    /// GEO_ZBUFFER: holds per-layer display-list heads in the original renderer.
    MasterList {
        z_buffer: bool,
    },
    LevelOfDetail {
        min_distance: i16,
        max_distance: i16,
    },
    /// The command's parameter is stored as `numCases`, which callbacks read
    /// as a parameter (Mario's select a body state or a hand); the selected
    /// case starts at 0 and only a callback changes it.
    SwitchCase {
        num_cases: i16,
        callback: u32,
    },
    Camera {
        mode: i16,
        position: [i16; 3],
        focus: [i16; 3],
        callback: u32,
    },
    /// Rotation is in original signed angle units after the degree conversion.
    TranslationRotation {
        layer: u8,
        translation: [i16; 3],
        rotation: [i16; 3],
        display_list: Option<u32>,
    },
    Translation {
        layer: u8,
        translation: [i16; 3],
        display_list: Option<u32>,
    },
    Rotation {
        layer: u8,
        rotation: [i16; 3],
        display_list: Option<u32>,
    },
    AnimatedPart {
        layer: u8,
        translation: [i16; 3],
        display_list: Option<u32>,
    },
    Billboard {
        layer: u8,
        translation: [i16; 3],
        display_list: Option<u32>,
    },
    DisplayList {
        layer: u8,
        display_list: u32,
    },
    Shadow {
        shadow_type: u8,
        solidity: u8,
        scale: i16,
    },
    /// GEO_RENDER_OBJ: objects are attached here at runtime.
    ObjectParent,
    /// GEO_ASM: a native callback that may generate display lists.
    Generated {
        param: i16,
        callback: u32,
    },
    Background {
        /// Background ID with a callback, otherwise an RGBA5551 clear colour.
        background: i16,
        callback: Option<u32>,
    },
    HeldObject {
        param: u8,
        offset: [i16; 3],
        callback: u32,
    },
    /// Scale is the original 16.16 fixed-point word.
    Scale {
        layer: u8,
        scale: u32,
        display_list: Option<u32>,
    },
    CullingRadius {
        radius: i16,
    },
}

impl GeoNodeKind {
    /// Drawing layer for nodes that carry a display list, otherwise zero.
    pub fn layer(&self) -> u8 {
        match *self {
            GeoNodeKind::TranslationRotation { layer, .. }
            | GeoNodeKind::Translation { layer, .. }
            | GeoNodeKind::Rotation { layer, .. }
            | GeoNodeKind::AnimatedPart { layer, .. }
            | GeoNodeKind::Billboard { layer, .. }
            | GeoNodeKind::DisplayList { layer, .. }
            | GeoNodeKind::Scale { layer, .. } => layer,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeoNode {
    pub source_address: u32,
    pub flags: u16,
    pub kind: GeoNodeKind,
    pub children: Vec<usize>,
    /// GEO_ASSIGN_AS_VIEW indices registered on this node, in command order.
    pub views: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeoLayout {
    pub entry: u32,
    pub root: Option<usize>,
    /// Arena in registration order. Children preserve original sibling order.
    pub nodes: Vec<GeoNode>,
    /// Nodes registered at depth zero after the root; the original discards them.
    pub detached: Vec<usize>,
}

impl GeoLayout {
    /// Pre-order traversal from the root, matching original sibling order.
    pub fn walk(&self, mut visit: impl FnMut(usize, &GeoNode, usize)) {
        let mut stack: Vec<(usize, usize)> = self.root.map(|r| (r, 0)).into_iter().collect();
        while let Some((index, depth)) = stack.pop() {
            let node = &self.nodes[index];
            visit(index, node, depth);
            for &child in node.children.iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }
}

#[derive(Clone, Copy)]
enum StackEntry {
    Address(Option<u32>),
    Return { depth: usize, return_index: usize },
}

fn angle(degrees: i16) -> i16 {
    // read_vec3s_angle: (s16 << 15) / 180 with C int division, stored as s16.
    ((i32::from(degrees) << 15) / 180) as i16
}

fn vec3(r: Reader<'_>, at: usize) -> Result<[i16; 3]> {
    Ok([r.i16(at)?, r.i16(at + 2)?, r.i16(at + 4)?])
}

fn vec3_angle(r: Reader<'_>, at: usize) -> Result<[i16; 3]> {
    Ok(vec3(r, at)?.map(angle))
}

fn register_node(
    layout: &mut GeoLayout,
    list: &mut [Option<usize>; MAX_DEPTH],
    depth: usize,
    address: u32,
    kind: GeoNodeKind,
) -> Result<()> {
    // init_graph_node_*: drawing layers live in the upper flag byte.
    let flags = match kind {
        GeoNodeKind::MasterList { z_buffer: true } => GRAPH_RENDER_ACTIVE | GRAPH_RENDER_Z_BUFFER,
        _ => (u16::from(kind.layer()) << 8) | GRAPH_RENDER_ACTIVE,
    };
    let index = layout.nodes.len();
    layout.nodes.push(GeoNode {
        source_address: address,
        flags,
        kind,
        children: vec![],
        views: vec![],
    });
    list[depth] = Some(index);
    if depth == 0 {
        if layout.root.is_none() {
            layout.root = Some(index);
        } else {
            layout.detached.push(index);
        }
        return Ok(());
    }
    let parent = list[depth - 1].ok_or_else(|| {
        ImportError::new(
            "geo layout",
            address as usize,
            "node opened without a parent",
        )
    })?;
    // An object parent's child becomes its shared child; it is the same edge here.
    layout.nodes[parent].children.push(index);
    Ok(())
}

/// Decode one geo layout. Unknown commands fail with their segmented address.
pub fn decode(segments: &Segments, entry: u32) -> Result<GeoLayout> {
    let mut layout = GeoLayout {
        entry,
        root: None,
        nodes: vec![],
        detached: vec![],
    };
    let mut list: [Option<usize>; MAX_DEPTH] = [None; MAX_DEPTH];
    let mut depth = 0usize;
    let mut stack = vec![
        StackEntry::Address(None),
        StackEntry::Return {
            depth: 0,
            return_index: 0,
        },
    ];
    let mut return_index = 2usize;
    let mut command = Some(entry);
    let err =
        |address: u32, detail: String| ImportError::new("geo layout", address as usize, detail);
    for _ in 0..MAX_COMMANDS {
        let Some(address) = command else {
            return Ok(layout);
        };
        let opcode = segments.read(address, 1)?[0];
        // Every command is at least four bytes; longer reads are checked below.
        let r = Reader::new(segments.tail(address)?, "geo command");
        let push = |stack: &mut Vec<StackEntry>, entry: StackEntry| {
            if stack.len() >= MAX_STACK {
                return Err(err(address, "geo stack overflow".into()));
            }
            stack.push(entry);
            Ok(())
        };
        let mut register = |layout: &mut GeoLayout, kind: GeoNodeKind, length: usize| {
            r.slice(0, length)?;
            register_node(layout, &mut list, depth, address, kind)?;
            Ok::<_, ImportError>(address + length as u32)
        };
        let layer = |byte: u8| byte & 0x0F;
        let next = match opcode {
            0x00 => {
                r.slice(0, 8)?;
                push(&mut stack, StackEntry::Address(Some(address + 8)))?;
                push(
                    &mut stack,
                    StackEntry::Return {
                        depth,
                        return_index,
                    },
                )?;
                return_index = stack.len();
                Some(r.u32(4)?)
            }
            0x01 => {
                stack.truncate(return_index);
                let Some(StackEntry::Return {
                    depth: saved_depth,
                    return_index: saved_return,
                }) = stack.pop()
                else {
                    return Err(err(address, "GEO_END without return state".into()));
                };
                let Some(StackEntry::Address(target)) = stack.pop() else {
                    return Err(err(address, "GEO_END without return address".into()));
                };
                depth = saved_depth;
                return_index = saved_return;
                target
            }
            0x02 => {
                r.slice(0, 8)?;
                // The original pushes a return address only for type 1.
                if r.u8(1)? == 1 {
                    push(&mut stack, StackEntry::Address(Some(address + 8)))?;
                }
                Some(r.u32(4)?)
            }
            0x03 => match stack.pop() {
                Some(StackEntry::Address(target)) => target,
                _ => return Err(err(address, "GEO_RETURN without branch".into())),
            },
            0x04 => {
                if depth + 1 >= MAX_DEPTH {
                    return Err(err(address, "geo node depth exceeded".into()));
                }
                list[depth + 1] = list[depth];
                depth += 1;
                Some(address + 4)
            }
            0x05 => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| err(address, "GEO_CLOSE_NODE at depth zero".into()))?;
                Some(address + 4)
            }
            0x06 => {
                let current = list[depth]
                    .ok_or_else(|| err(address, "GEO_ASSIGN_AS_VIEW without node".into()))?;
                layout.nodes[current].views.push(r.i16(2)?);
                Some(address + 4)
            }
            0x07 => {
                let current = list[depth]
                    .ok_or_else(|| err(address, "GEO_UPDATE_NODE_FLAGS without node".into()))?;
                let bits = r.u16(2)?;
                let flags = &mut layout.nodes[current].flags;
                match r.u8(1)? {
                    0 => *flags = bits,
                    1 => *flags |= bits,
                    2 => *flags &= !bits,
                    _ => {}
                }
                Some(address + 4)
            }
            0x08 => {
                let kind = GeoNodeKind::Root {
                    views: r.i16(2)?,
                    x: r.i16(4)?,
                    y: r.i16(6)?,
                    width: r.i16(8)?,
                    height: r.i16(10)?,
                };
                Some(register(&mut layout, kind, 12)?)
            }
            0x09 => Some(register(
                &mut layout,
                GeoNodeKind::Ortho { scale: r.i16(2)? },
                4,
            )?),
            0x0A => {
                let with_func = r.u8(1)? != 0;
                let kind = GeoNodeKind::Perspective {
                    fov: r.i16(2)?,
                    near: r.i16(4)?,
                    far: r.i16(6)?,
                    callback: if with_func { Some(r.u32(8)?) } else { None },
                };
                Some(register(&mut layout, kind, if with_func { 12 } else { 8 })?)
            }
            0x0B => Some(register(&mut layout, GeoNodeKind::Start, 4)?),
            0x0C => Some(register(
                &mut layout,
                GeoNodeKind::MasterList {
                    z_buffer: r.u8(1)? != 0,
                },
                4,
            )?),
            0x0D => {
                let kind = GeoNodeKind::LevelOfDetail {
                    min_distance: r.i16(4)?,
                    max_distance: r.i16(6)?,
                };
                Some(register(&mut layout, kind, 8)?)
            }
            0x0E => {
                let kind = GeoNodeKind::SwitchCase {
                    num_cases: r.i16(2)?,
                    callback: r.u32(4)?,
                };
                Some(register(&mut layout, kind, 8)?)
            }
            0x0F => {
                let kind = GeoNodeKind::Camera {
                    mode: r.i16(2)?,
                    position: vec3(r, 4)?,
                    focus: vec3(r, 10)?,
                    callback: r.u32(16)?,
                };
                Some(register(&mut layout, kind, 20)?)
            }
            0x10 => {
                let params = r.u8(1)?;
                let (translation, rotation, mut length) = match (params & 0x70) >> 4 {
                    0 => (vec3(r, 4)?, vec3_angle(r, 10)?, 16),
                    1 => (vec3(r, 2)?, [0; 3], 8),
                    2 => ([0; 3], vec3_angle(r, 2)?, 8),
                    3 => ([0; 3], [0, angle(r.i16(2)?), 0], 4),
                    _ => {
                        return Err(err(
                            address,
                            format!("translate/rotate form 0x{params:02X} is not defined"),
                        ));
                    }
                };
                let display_list = if params & 0x80 != 0 {
                    let pointer = r.u32(length)?;
                    length += 4;
                    Some(pointer)
                } else {
                    None
                };
                let kind = GeoNodeKind::TranslationRotation {
                    layer: if display_list.is_some() {
                        layer(params)
                    } else {
                        0
                    },
                    translation,
                    rotation,
                    display_list,
                };
                Some(register(&mut layout, kind, length)?)
            }
            0x11 | 0x12 | 0x14 => {
                let params = r.u8(1)?;
                let display_list = if params & 0x80 != 0 {
                    Some(r.u32(8)?)
                } else {
                    None
                };
                let layer = if display_list.is_some() {
                    layer(params)
                } else {
                    0
                };
                let kind = match opcode {
                    0x11 => GeoNodeKind::Translation {
                        layer,
                        translation: vec3(r, 2)?,
                        display_list,
                    },
                    0x12 => GeoNodeKind::Rotation {
                        layer,
                        rotation: vec3_angle(r, 2)?,
                        display_list,
                    },
                    _ => GeoNodeKind::Billboard {
                        layer,
                        translation: vec3(r, 2)?,
                        display_list,
                    },
                };
                let length = if display_list.is_some() { 12 } else { 8 };
                Some(register(&mut layout, kind, length)?)
            }
            0x13 => {
                let pointer = r.u32(8)?;
                let kind = GeoNodeKind::AnimatedPart {
                    layer: r.u8(1)?,
                    translation: vec3(r, 2)?,
                    display_list: (pointer != 0).then_some(pointer),
                };
                Some(register(&mut layout, kind, 12)?)
            }
            0x15 => {
                let kind = GeoNodeKind::DisplayList {
                    layer: r.u8(1)?,
                    display_list: r.u32(4)?,
                };
                Some(register(&mut layout, kind, 8)?)
            }
            0x16 => {
                let kind = GeoNodeKind::Shadow {
                    shadow_type: r.i16(2)? as u8,
                    solidity: r.i16(4)? as u8,
                    scale: r.i16(6)?,
                };
                Some(register(&mut layout, kind, 8)?)
            }
            0x17 => Some(register(&mut layout, GeoNodeKind::ObjectParent, 4)?),
            0x18 => {
                let kind = GeoNodeKind::Generated {
                    param: r.i16(2)?,
                    callback: r.u32(4)?,
                };
                Some(register(&mut layout, kind, 8)?)
            }
            0x19 => {
                let callback = r.u32(4)?;
                let kind = GeoNodeKind::Background {
                    background: r.i16(2)?,
                    callback: (callback != 0).then_some(callback),
                };
                Some(register(&mut layout, kind, 8)?)
            }
            0x1A | 0x1E => {
                r.slice(0, 8)?;
                Some(address + 8)
            }
            0x1F => {
                r.slice(0, 16)?;
                Some(address + 16)
            }
            0x1B => {
                return Err(err(
                    address,
                    "GEO_COPY_VIEW is not implemented by this importer".into(),
                ));
            }
            0x1C => {
                let kind = GeoNodeKind::HeldObject {
                    param: r.u8(1)?,
                    offset: vec3(r, 2)?,
                    callback: r.u32(8)?,
                };
                Some(register(&mut layout, kind, 12)?)
            }
            0x1D => {
                let params = r.u8(1)?;
                let display_list = if params & 0x80 != 0 {
                    Some(r.u32(8)?)
                } else {
                    None
                };
                let kind = GeoNodeKind::Scale {
                    layer: if display_list.is_some() {
                        layer(params)
                    } else {
                        0
                    },
                    scale: r.u32(4)?,
                    display_list,
                };
                let length = if display_list.is_some() { 12 } else { 8 };
                Some(register(&mut layout, kind, length)?)
            }
            0x20 => Some(register(
                &mut layout,
                GeoNodeKind::CullingRadius { radius: r.i16(2)? },
                4,
            )?),
            _ => {
                return Err(err(
                    address,
                    format!("unsupported geo opcode 0x{opcode:02X}"),
                ));
            }
        };
        if let Some(target) = next {
            // Validate before following so a bad pointer reports its source command.
            segments.read(target, 4).map_err(|e| {
                err(
                    address,
                    format!("geo control target 0x{target:08X} is invalid: {e}"),
                )
            })?;
        }
        command = next;
    }
    Err(err(
        command.unwrap_or(entry),
        "geo command budget exceeded (possible cycle)".into(),
    ))
}
