//! Fast3D (F3D, the US/JP "f3d_old" microcode) display-list interpreter that
//! converts original display lists into engine-owned `VisualModel` batches.
//!
//! Command encodings come from the pinned CC0 sm64 include/PR/gbi.h built with
//! F3D_OLD, cross-checked by compiling its macros (see docs/ROM_VALIDATION.md).
//! RSP/RDP state is tracked only as far as presentation needs it. Unsupported
//! commands are reported with their segmented address, never silently accepted.
//! Nothing here affects gameplay; collision stays in the separate collision mesh.
use super::{ImportError, Result, reader::Reader, segments::Segments, texture};
use crate::content::{ImportIssue, visual::*};
use std::collections::{BTreeSet, HashMap};

const VERTEX_SLOTS: usize = 16;
const MAX_DL_DEPTH: usize = 18;
const MAX_COMMANDS: usize = 1 << 20;
const MAX_LIGHT_SLOTS: usize = 8;

// Geometry-mode bits (F3D_OLD values from the pinned gbi.h).
pub const G_ZBUFFER: u32 = 0x0000_0001;
pub const G_SHADE: u32 = 0x0000_0004;
pub const G_SHADING_SMOOTH: u32 = 0x0000_0200;
pub const G_CULL_FRONT: u32 = 0x0000_1000;
pub const G_CULL_BACK: u32 = 0x0000_2000;
pub const G_FOG: u32 = 0x0001_0000;
pub const G_LIGHTING: u32 = 0x0002_0000;
pub const G_TEXTURE_GEN: u32 = 0x0004_0000;
pub const G_TEXTURE_GEN_LINEAR: u32 = 0x0008_0000;

// Render-mode bits in othermode low.
const Z_CMP: u32 = 0x10;
const Z_UPD: u32 = 0x20;
const ZMODE_MASK: u32 = 0xC00;
const ZMODE_DEC: u32 = 0xC00;
const CVG_X_ALPHA: u32 = 0x1000;
const FORCE_BL: u32 = 0x4000;
const G_BL_CLR_FOG: u32 = 3;
const G_BL_CLR_MEM: u32 = 1;
const G_BL_1MA: u32 = 0;

/// Z-buffered render modes by layer, cycle-one and cycle-two words combined
/// (renderModeTable_1Cycle/2Cycle[1] in pinned src/game/rendering_graph_node.c,
/// values computed from the pinned gbi.h macros).
pub const LAYER_RENDER_MODES_ZB: [u32; LAYER_COUNT] = [
    0x0055_2230, // G_RM_ZB_OPA_SURF
    0x0055_2078, // G_RM_AA_ZB_OPA_SURF
    0x0055_2D58, // G_RM_AA_ZB_OPA_DECAL
    0x0055_2478, // G_RM_AA_ZB_OPA_INTER
    0x0055_3078, // G_RM_AA_ZB_TEX_EDGE
    0x0050_49D8, // G_RM_AA_ZB_XLU_SURF
    0x0050_4DD8, // G_RM_AA_ZB_XLU_DECAL
    0x0050_45D8, // G_RM_AA_ZB_XLU_INTER
];

/// renderModeTable_1Cycle/2Cycle[0], used by master lists without a Z buffer.
pub const LAYER_RENDER_MODES: [u32; LAYER_COUNT] = [
    0x0F0A_4000, // G_RM_OPA_SURF
    0x0055_2048, // G_RM_AA_OPA_SURF
    0x0055_2048, // G_RM_AA_OPA_SURF
    0x0055_2048, // G_RM_AA_OPA_SURF
    0x0055_3048, // G_RM_AA_TEX_EDGE
    0x0050_41C8, // G_RM_AA_XLU_SURF
    0x0050_41C8, // G_RM_AA_XLU_SURF
    0x0050_41C8, // G_RM_AA_XLU_SURF
];

/// Row-vector 4x4 transform with SM64's Mat4 layout: v' = [x, y, z, 1] * M.
pub type Mat4 = [[f32; 4]; 4];

pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// a * b with the same entry order as the original mtxf_mul (affine only).
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = IDENTITY;
    for row in 0..4 {
        for col in 0..3 {
            out[row][col] = a[row][0] * b[0][col] + a[row][1] * b[1][col] + a[row][2] * b[2][col];
        }
        if row == 3 {
            for col in 0..3 {
                out[3][col] += b[3][col];
            }
        }
    }
    out
}

fn transform(m: &Mat4, v: [i16; 3]) -> [f32; 3] {
    let [x, y, z] = v.map(f32::from);
    [
        x * m[0][0] + y * m[1][0] + z * m[2][0] + m[3][0],
        x * m[0][1] + y * m[1][1] + z * m[2][1] + m[3][1],
        x * m[0][2] + y * m[1][2] + z * m[2][2] + m[3][2],
    ]
}

#[derive(Debug, Clone, Copy, Default)]
struct Tile {
    format: u8,
    size: u8,
    line: u16,
    tmem: u16,
    palette: u8,
    cmt: u8,
    maskt: u8,
    shiftt: u8,
    cms: u8,
    masks: u8,
    shifts: u8,
    uls: u16,
    ult: u16,
    lrs: u16,
    lrt: u16,
}

#[derive(Debug, Clone, Copy)]
struct TmemLoad {
    tmem: u16,
    address: u32,
    bytes: usize,
    /// Source row stride for LOADTILE; LOADBLOCK uses the render tile's line.
    row_bytes: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct TlutLoad {
    tmem: u16,
    address: u32,
    entries: usize,
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    position: [f32; 3],
    /// Texture coordinates after the RSP scale, in 1/32 texel units.
    st: [f32; 2],
    color: [u8; 4],
    lights: Option<Lights>,
}

#[derive(Debug, Clone, Copy, Default)]
struct ImageRegister {
    size: u8,
    width: u16,
    address: u32,
}

/// Bound texture plus the render tile's shift factors, origin and size in texels.
type TileBinding = (TextureBinding, [f32; 2], [f32; 2], [f32; 2]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    address: u32,
    format: u8,
    size: u8,
    width: u16,
    height: u16,
    row_bytes: usize,
    palette: Option<(u32, bool)>,
}

#[derive(Debug, Clone)]
struct State {
    geometry_mode: u32,
    othermode_h: u32,
    othermode_l: u32,
    combine: (u32, u32),
    texture_on: bool,
    texture_tile: u8,
    texture_scale: [u16; 2],
    num_lights: u8,
    lights: [[u8; 16]; MAX_LIGHT_SLOTS],
    fog_factor: (i16, i16),
    fog_color: [u8; 4],
    prim_color: [u8; 4],
    env_color: [u8; 4],
    image: ImageRegister,
    tiles: [Tile; 8],
    loads: Vec<TmemLoad>,
    tluts: Vec<TlutLoad>,
    slots: [Option<Slot>; VERTEX_SLOTS],
}

impl Default for State {
    /// Pinned src/game/game_init.c init_rsp/init_rdp, then a 1-cycle level pass.
    fn default() -> Self {
        Self {
            geometry_mode: G_SHADE | G_SHADING_SMOOTH | G_CULL_BACK | G_LIGHTING,
            othermode_h: 0,
            othermode_l: 0,
            // G_CC_SHADE in both cycles.
            combine: (0x00FF_FFFF, 0xFFFE_793C),
            texture_on: false,
            texture_tile: 0,
            texture_scale: [0, 0],
            num_lights: 1,
            lights: [[0; 16]; MAX_LIGHT_SLOTS],
            fog_factor: (0, 0),
            fog_color: [0; 4],
            prim_color: [0; 4],
            env_color: [0; 4],
            image: ImageRegister::default(),
            tiles: [Tile::default(); 8],
            loads: vec![],
            tluts: vec![],
            slots: [None; VERTEX_SLOTS],
        }
    }
}

/// Decode the 16 combiner selectors of a G_SETCOMBINE command.
pub fn decode_combiner(w0: u32, w1: u32) -> [CombinerCycle; 2] {
    let f = |w: u32, shift: u32, bits: u32| ((w >> shift) & ((1 << bits) - 1)) as u8;
    [
        CombinerCycle {
            rgb: [f(w0, 20, 4), f(w1, 28, 4), f(w0, 15, 5), f(w1, 15, 3)],
            alpha: [f(w0, 12, 3), f(w1, 12, 3), f(w0, 9, 3), f(w1, 9, 3)],
        },
        CombinerCycle {
            rgb: [f(w0, 5, 4), f(w1, 24, 4), f(w0, 0, 5), f(w1, 6, 3)],
            alpha: [f(w1, 21, 3), f(w1, 3, 3), f(w1, 18, 3), f(w1, 0, 3)],
        },
    ]
}

/// Whether a combiner cycle samples TEXEL0 or TEXEL1 (by index 1 or 2).
fn uses_texel(cycle: &CombinerCycle, texel: u8) -> bool {
    let rgb = cycle.rgb;
    let alpha = cycle.alpha;
    rgb[0] == texel
        || rgb[1] == texel
        || rgb[2] == texel
        || rgb[2] == texel + 7
        || rgb[3] == texel
        || alpha.contains(&texel)
}

pub struct Builder<'a> {
    segments: &'a Segments,
    state: State,
    model: VisualModel,
    textures: HashMap<TextureKey, usize>,
    issues: BTreeSet<(u32, String)>,
    commands: usize,
}

impl<'a> Builder<'a> {
    pub fn new(segments: &'a Segments) -> Self {
        Self {
            segments,
            state: State::default(),
            model: VisualModel::default(),
            textures: HashMap::new(),
            issues: BTreeSet::new(),
            commands: 0,
        }
    }

    fn issue(&mut self, address: u32, feature: impl Into<String>) {
        self.issues.insert((address, feature.into()));
    }

    /// Run one master-list entry: the layer's render mode, then the display list.
    pub fn run(
        &mut self,
        display_list: u32,
        layer: u8,
        matrix: &Mat4,
        z_buffer: bool,
    ) -> Result<()> {
        let layer_index = usize::from(layer);
        if layer_index >= LAYER_COUNT {
            return Err(ImportError::new(
                "display list",
                display_list as usize,
                format!("invalid drawing layer {layer}"),
            ));
        }
        // geo_process_master_list_sub: G_ZBUFFER, then the layer's render mode.
        let modes = if z_buffer {
            self.state.geometry_mode |= G_ZBUFFER;
            &LAYER_RENDER_MODES_ZB
        } else {
            self.state.geometry_mode &= !G_ZBUFFER;
            &LAYER_RENDER_MODES
        };
        self.state.othermode_l = (self.state.othermode_l & 0x7) | (modes[layer_index] & !0x7);
        let mut stack = vec![];
        let mut address = display_list;
        loop {
            self.commands += 1;
            if self.commands > MAX_COMMANDS {
                return Err(ImportError::new(
                    "display list",
                    address as usize,
                    "command budget exceeded (possible cycle)",
                ));
            }
            let r = Reader::new(self.segments.read(address, 8)?, "display list");
            let w0 = r.u32(0)?;
            let w1 = r.u32(4)?;
            let opcode = (w0 >> 24) as u8;
            let next = address.checked_add(8).ok_or_else(|| {
                ImportError::new("display list", address as usize, "address overflow")
            })?;
            match opcode {
                0x00 => {}
                0x01 => self.issue(address, "G_MTX inside a display list is not applied"),
                0x03 => self.movemem(address, w0, w1)?,
                0x04 => self.vertex(address, w0, w1, matrix)?,
                0x06 => {
                    self.segments.read(w1, 8).map_err(|e| {
                        ImportError::new(
                            "display list",
                            address as usize,
                            format!("G_DL target 0x{w1:08X}: {e}"),
                        )
                    })?;
                    if (w0 >> 16) & 0xFF == 0 {
                        if stack.len() >= MAX_DL_DEPTH {
                            return Err(ImportError::new(
                                "display list",
                                address as usize,
                                "display-list call depth exceeded",
                            ));
                        }
                        stack.push(next);
                    }
                    address = w1;
                    continue;
                }
                0xB8 => match stack.pop() {
                    Some(ret) => {
                        address = ret;
                        continue;
                    }
                    None => return Ok(()),
                },
                0xB6 => self.state.geometry_mode &= !w1,
                0xB7 => self.state.geometry_mode |= w1,
                0xB9 | 0xBA => {
                    let shift = (w0 >> 8) & 0xFF;
                    let length = w0 & 0xFF;
                    if shift + length > 32 || length == 0 {
                        return Err(ImportError::new(
                            "display list",
                            address as usize,
                            "invalid othermode field",
                        ));
                    }
                    let mask = (((1u64 << length) - 1) << shift) as u32;
                    let target = if opcode == 0xB9 {
                        &mut self.state.othermode_l
                    } else {
                        &mut self.state.othermode_h
                    };
                    *target = (*target & !mask) | (w1 & mask);
                }
                0xBB => {
                    self.state.texture_on = w0 & 0xFF != 0;
                    self.state.texture_tile = ((w0 >> 8) & 7) as u8;
                    self.state.texture_scale = [(w1 >> 16) as u16, w1 as u16];
                }
                0xBC => self.moveword(address, w0, w1),
                // G_CULLDL only skips work for off-screen volumes; drawing everything
                // matches what is visible.
                0xBE => {}
                0xBF => self.triangle(address, w1, layer)?,
                0xBD => self.issue(address, "G_POPMTX is not applied"),
                0xE6..=0xE9 => {}
                0xEC | 0xED => {}
                0xEF => {
                    self.state.othermode_h = w0 & 0x00FF_FFFF;
                    self.state.othermode_l = w1;
                }
                0xF0 => self.load_tlut(w1),
                0xF2 => {
                    let tile = &mut self.state.tiles[((w1 >> 24) & 7) as usize];
                    tile.uls = ((w0 >> 12) & 0xFFF) as u16;
                    tile.ult = (w0 & 0xFFF) as u16;
                    tile.lrs = ((w1 >> 12) & 0xFFF) as u16;
                    tile.lrt = (w1 & 0xFFF) as u16;
                }
                0xF3 => self.load_block(address, w0, w1)?,
                0xF4 => self.load_tile(address, w0, w1)?,
                0xF5 => {
                    self.state.tiles[((w1 >> 24) & 7) as usize] = Tile {
                        format: ((w0 >> 21) & 7) as u8,
                        size: ((w0 >> 19) & 3) as u8,
                        line: ((w0 >> 9) & 0x1FF) as u16,
                        tmem: (w0 & 0x1FF) as u16,
                        palette: ((w1 >> 20) & 0xF) as u8,
                        cmt: ((w1 >> 18) & 3) as u8,
                        maskt: ((w1 >> 14) & 0xF) as u8,
                        shiftt: ((w1 >> 10) & 0xF) as u8,
                        cms: ((w1 >> 8) & 3) as u8,
                        masks: ((w1 >> 4) & 0xF) as u8,
                        shifts: (w1 & 0xF) as u8,
                        ..self.state.tiles[((w1 >> 24) & 7) as usize]
                    };
                }
                0xF7 | 0xF9 => {}
                0xF8 => self.state.fog_color = w1.to_be_bytes(),
                0xFA => self.state.prim_color = w1.to_be_bytes(),
                0xFB => self.state.env_color = w1.to_be_bytes(),
                0xFC => self.state.combine = (w0 & 0x00FF_FFFF, w1),
                0xFD => {
                    self.state.image = ImageRegister {
                        size: ((w0 >> 19) & 3) as u8,
                        width: ((w0 & 0xFFF) + 1) as u16,
                        address: w1,
                    }
                }
                0xFE | 0xFF => self.issue(
                    address,
                    format!("color/depth image command 0x{opcode:02X} ignored by the importer"),
                ),
                _ => self.issue(
                    address,
                    format!("unsupported Fast3D command 0x{opcode:02X} ({w0:08X} {w1:08X})"),
                ),
            }
            address = next;
        }
    }

    fn movemem(&mut self, address: u32, w0: u32, w1: u32) -> Result<()> {
        let index = (w0 >> 16) & 0xFF;
        match index {
            // G_MV_L0..G_MV_L7: one light per even index from 0x86.
            0x86..=0x94 if index.is_multiple_of(2) => {
                let slot = ((index - 0x86) / 2) as usize;
                let length = (w0 & 0xFFFF) as usize;
                // Ambient entries are 8 bytes; directional lights add a direction.
                let bytes = self.segments.read(w1, length.clamp(8, 16))?;
                let mut light = [0u8; 16];
                light[..bytes.len()].copy_from_slice(bytes);
                self.state.lights[slot] = light;
            }
            _ => self.issue(
                address,
                format!("G_MOVEMEM index 0x{index:02X} ignored (viewport/lookat/matrix)"),
            ),
        }
        Ok(())
    }

    fn moveword(&mut self, address: u32, w0: u32, w1: u32) {
        let index = w0 & 0xFF;
        match index {
            // G_MW_NUMLIGHT: NUML(n) = (n + 1) * 32 + 0x80000000 for F3D.
            0x02 => {
                let n = ((w1 & 0x7FFF_FFFF) / 32).saturating_sub(1);
                self.state.num_lights = n.min(7) as u8;
            }
            0x08 => self.state.fog_factor = ((w1 >> 16) as i16, w1 as i16),
            _ => self.issue(
                address,
                format!("G_MOVEWORD index 0x{index:02X} ignored (segment/clip/matrix)"),
            ),
        }
    }

    fn current_lights(&self) -> Option<Lights> {
        if self.state.geometry_mode & G_LIGHTING == 0 {
            return None;
        }
        let diffuse = &self.state.lights[0];
        let ambient_slot = usize::from(self.state.num_lights).min(MAX_LIGHT_SLOTS - 1);
        let ambient = &self.state.lights[ambient_slot];
        Some(Lights {
            ambient: [ambient[0], ambient[1], ambient[2]],
            diffuse: [diffuse[0], diffuse[1], diffuse[2]],
            direction: [diffuse[8] as i8, diffuse[9] as i8, diffuse[10] as i8],
        })
    }

    fn vertex(&mut self, address: u32, w0: u32, w1: u32, matrix: &Mat4) -> Result<()> {
        // F3D: p = ((n - 1) << 4) | v0, length = 16 * n.
        let p = (w0 >> 16) & 0xFF;
        let count = ((p >> 4) + 1) as usize;
        let first = (p & 0xF) as usize;
        let length = (w0 & 0xFFFF) as usize;
        if length != count * 16 || first + count > VERTEX_SLOTS {
            return Err(ImportError::new(
                "display list",
                address as usize,
                format!("invalid G_VTX: {count} vertices at slot {first}, {length} bytes"),
            ));
        }
        let r = Reader::new(self.segments.read(w1, length)?, "vertices");
        let lights = self.current_lights();
        let [scale_s, scale_t] = self.state.texture_scale.map(|s| f32::from(s) / 65536.0);
        for i in 0..count {
            let at = i * 16;
            let position = [r.i16(at)?, r.i16(at + 2)?, r.i16(at + 4)?];
            let st = [f32::from(r.i16(at + 8)?), f32::from(r.i16(at + 10)?)];
            self.state.slots[first + i] = Some(Slot {
                position: transform(matrix, position),
                st: [st[0] * scale_s, st[1] * scale_t],
                color: [
                    r.u8(at + 12)?,
                    r.u8(at + 13)?,
                    r.u8(at + 14)?,
                    r.u8(at + 15)?,
                ],
                lights,
            });
        }
        Ok(())
    }

    fn load_block(&mut self, address: u32, w0: u32, w1: u32) -> Result<()> {
        let tile = self.state.tiles[((w1 >> 24) & 7) as usize];
        let (uls, ult) = ((w0 >> 12) & 0xFFF, w0 & 0xFFF);
        let lrs = ((w1 >> 12) & 0xFFF) as usize;
        let image = self.state.image;
        let bits = texture::bits_per_texel(image.size).unwrap_or(16);
        if uls != 0 || ult != 0 {
            self.issue(
                address,
                "G_LOADBLOCK with a nonzero origin is not supported",
            );
            return Ok(());
        }
        self.store_load(TmemLoad {
            tmem: tile.tmem,
            address: image.address,
            bytes: ((lrs + 1) * bits).div_ceil(8),
            row_bytes: None,
        });
        Ok(())
    }

    fn load_tile(&mut self, address: u32, w0: u32, w1: u32) -> Result<()> {
        let tile = self.state.tiles[((w1 >> 24) & 7) as usize];
        let image = self.state.image;
        let Some(bits) = texture::bits_per_texel(image.size) else {
            self.issue(address, "G_LOADTILE with an invalid image size");
            return Ok(());
        };
        let (uls, ult) = (((w0 >> 12) & 0xFFF) >> 2, (w0 & 0xFFF) >> 2);
        let (lrs, lrt) = ((((w1 >> 12) & 0xFFF) >> 2), ((w1 & 0xFFF) >> 2));
        if lrs < uls || lrt < ult {
            return Err(ImportError::new(
                "display list",
                address as usize,
                "G_LOADTILE with an inverted rectangle",
            ));
        }
        let row_bytes = (usize::from(image.width) * bits).div_ceil(8);
        let start = image.address as usize + ult as usize * row_bytes + (uls as usize * bits) / 8;
        let rows = (lrt - ult + 1) as usize;
        self.store_load(TmemLoad {
            tmem: tile.tmem,
            address: start as u32,
            bytes: row_bytes * rows,
            row_bytes: Some(row_bytes),
        });
        Ok(())
    }

    fn store_load(&mut self, load: TmemLoad) {
        self.state.loads.retain(|l| l.tmem != load.tmem);
        self.state.loads.push(load);
    }

    fn load_tlut(&mut self, w1: u32) {
        let tile = self.state.tiles[((w1 >> 24) & 7) as usize];
        let entries = (((w1 >> 14) & 0x3FF) + 1) as usize;
        self.state.tluts.retain(|t| t.tmem != tile.tmem);
        self.state.tluts.push(TlutLoad {
            tmem: tile.tmem,
            address: self.state.image.address,
            entries,
        });
    }

    fn texture_binding(
        &mut self,
        address: u32,
        combiner: &[CombinerCycle; 2],
    ) -> Result<Option<TileBinding>> {
        let two_cycle = (self.state.othermode_h >> 20) & 3 == 1;
        let cycles = if two_cycle {
            &combiner[..]
        } else {
            &combiner[..1]
        };
        let texel0 = cycles.iter().any(|c| uses_texel(c, 1));
        let texel1 = cycles.iter().any(|c| uses_texel(c, 2));
        if !self.state.texture_on || !(texel0 || texel1) {
            return Ok(None);
        }
        if texel1 {
            self.issue(
                address,
                "TEXEL1 is sampled from the TEXEL0 tile (approximation)",
            );
        }
        let tile = self.state.tiles[usize::from(self.state.texture_tile)];
        let width = ((tile.lrs.wrapping_sub(tile.uls) >> 2) & 0x3FF) + 1;
        let height = ((tile.lrt.wrapping_sub(tile.ult) >> 2) & 0x3FF) + 1;
        let Some(load) = self
            .state
            .loads
            .iter()
            .find(|l| l.tmem == tile.tmem)
            .copied()
        else {
            self.issue(
                address,
                format!("no texture loaded at TMEM 0x{:03X}", tile.tmem),
            );
            return Ok(None);
        };
        let bits = texture::bits_per_texel(tile.size).ok_or_else(|| {
            ImportError::new("display list", address as usize, "invalid tile size")
        })?;
        let row_bytes = load
            .row_bytes
            .unwrap_or(usize::from(tile.line) * 8)
            .max((usize::from(width) * bits).div_ceil(8));
        let needed =
            row_bytes * (usize::from(height) - 1) + (usize::from(width) * bits).div_ceil(8);
        if needed > load.bytes {
            self.issue(
                address,
                format!(
                    "{width}x{height} tile reads {needed} bytes but only {} were loaded; decoded from source memory",
                    load.bytes
                ),
            );
        }
        let palette = if tile.format == texture::FMT_CI {
            let tlut_type = (self.state.othermode_h >> 14) & 3;
            let ia = tlut_type == 3;
            let (start, count) = if tile.size == texture::SIZ_4B {
                (256 + u16::from(tile.palette) * 16, 16)
            } else {
                (256, 256)
            };
            let Some(tlut) = self
                .state
                .tluts
                .iter()
                .find(|t| t.tmem <= start && usize::from(start - t.tmem) + count <= t.entries)
                .copied()
            else {
                self.issue(address, "color-indexed texture without a loaded palette");
                return Ok(None);
            };
            Some((tlut.address + u32::from(start - tlut.tmem) * 2, count, ia))
        } else {
            None
        };
        let key = TextureKey {
            address: load.address,
            format: tile.format,
            size: tile.size,
            width,
            height,
            row_bytes,
            palette: palette.map(|(a, _, ia)| (a, ia)),
        };
        let index = match self.textures.get(&key) {
            Some(&index) => index,
            None => {
                let source = self.segments.read(load.address, needed)?;
                let palette_bytes = match palette {
                    Some((a, count, _)) => Some(self.segments.read(a, count * 2)?),
                    None => None,
                };
                let decoded = texture::decode(
                    source,
                    tile.format,
                    tile.size,
                    width,
                    height,
                    Some(row_bytes),
                    palette_bytes.map(|p| {
                        (
                            p,
                            if palette.is_some_and(|(_, _, ia)| ia) {
                                texture::PaletteFormat::Ia16
                            } else {
                                texture::PaletteFormat::Rgba16
                            },
                        )
                    }),
                )
                .map_err(|e| ImportError::new("texture", load.address as usize, e.to_string()))?;
                let index = self.model.textures.len();
                self.model.textures.push(TextureImage {
                    width,
                    height,
                    rgba: decoded.rgba,
                    source: load.address,
                    format: tile.format,
                    size: tile.size,
                });
                self.textures.insert(key, index);
                index
            }
        };
        let wrap = |cm: u8, mask: u8| {
            if cm & 2 != 0 {
                WrapMode::Clamp
            } else if cm & 1 != 0 && mask != 0 {
                WrapMode::MirrorRepeat
            } else {
                WrapMode::Repeat
            }
        };
        let filter = if (self.state.othermode_h >> 12) & 3 == 0 {
            TextureFilter::Point
        } else {
            TextureFilter::Bilinear
        };
        let shift_factor = |shift: u8| match shift {
            0 => 1.0,
            1..=10 => 1.0 / f32::from(1u16 << shift),
            _ => f32::from(1u16 << (16 - shift)),
        };
        Ok(Some((
            TextureBinding {
                texture: index,
                wrap: [wrap(tile.cms, tile.masks), wrap(tile.cmt, tile.maskt)],
                filter,
            },
            [shift_factor(tile.shifts), shift_factor(tile.shiftt)],
            [f32::from(tile.uls) / 4.0, f32::from(tile.ult) / 4.0],
            [f32::from(width), f32::from(height)],
        )))
    }

    fn triangle(&mut self, address: u32, w1: u32, layer: u8) -> Result<()> {
        let mut indices = [0usize; 3];
        for (i, shift) in [16, 8, 0].into_iter().enumerate() {
            let byte = (w1 >> shift) & 0xFF;
            if !byte.is_multiple_of(10) || byte / 10 >= VERTEX_SLOTS as u32 {
                return Err(ImportError::new(
                    "display list",
                    address as usize,
                    format!("invalid G_TRI1 vertex byte {byte}"),
                ));
            }
            indices[i] = (byte / 10) as usize;
        }
        let mut slots = [None; 3];
        for (i, index) in indices.into_iter().enumerate() {
            slots[i] = Some(self.state.slots[index].ok_or_else(|| {
                ImportError::new(
                    "display list",
                    address as usize,
                    format!("G_TRI1 uses unloaded vertex slot {index}"),
                )
            })?);
        }
        let slots = slots.map(|s| s.expect("checked above"));
        let (cw0, cw1) = self.state.combine;
        let combiner = decode_combiner(cw0, cw1);
        let two_cycle = (self.state.othermode_h >> 20) & 3 == 1;
        let binding = self.texture_binding(address, &combiner)?;
        let mode = self.state.othermode_l;
        let geometry = self.state.geometry_mode;
        // Blender bits of the cycle that writes memory (cycle two in 2-cycle mode).
        let cycle_shift = if two_cycle { 16 } else { 18 };
        let blend_m = (mode >> (cycle_shift + 4)) & 3;
        let blend_b = (mode >> cycle_shift) & 3;
        let translucent = mode & FORCE_BL != 0 && blend_m == G_BL_CLR_MEM && blend_b == G_BL_1MA;
        let cutout = mode & CVG_X_ALPHA != 0 || mode & 3 == 1;
        let fog = (two_cycle && geometry & G_FOG != 0 && (mode >> 30) & 3 == G_BL_CLR_FOG)
            .then_some(Fog {
                multiplier: self.state.fog_factor.0,
                offset: self.state.fog_factor.1,
                color: self.state.fog_color,
            });
        let material = Material {
            layer,
            combiner,
            two_cycle,
            texture: binding.map(|b| b.0),
            lights: slots[0].lights,
            texture_gen: geometry & (G_TEXTURE_GEN | G_TEXTURE_GEN_LINEAR) != 0,
            prim_color: self.state.prim_color,
            env_color: self.state.env_color,
            fog,
            blend: if translucent {
                BlendMode::Translucent
            } else if cutout {
                BlendMode::Cutout
            } else {
                BlendMode::Opaque
            },
            depth_test: geometry & G_ZBUFFER != 0 && mode & Z_CMP != 0,
            depth_write: geometry & G_ZBUFFER != 0 && mode & Z_UPD != 0,
            decal: mode & ZMODE_MASK == ZMODE_DEC,
            cull_back: geometry & G_CULL_BACK != 0,
            cull_front: geometry & G_CULL_FRONT != 0,
        };
        let vertices = slots.map(|slot| {
            let uv = match binding {
                Some((_, shift, origin, size)) => [
                    (slot.st[0] / 32.0 * shift[0] - origin[0]) / size[0],
                    (slot.st[1] / 32.0 * shift[1] - origin[1]) / size[1],
                ],
                None => [0.0, 0.0],
            };
            VisualVertex {
                position: slot.position,
                uv,
                color: slot.color,
            }
        });
        match self.model.batches.last_mut() {
            Some(batch) if batch.material == material => batch.vertices.extend(vertices),
            _ => self.model.batches.push(DrawBatch {
                material,
                vertices: vertices.to_vec(),
                source: address,
            }),
        }
        Ok(())
    }

    pub fn finish(self) -> (VisualModel, Vec<ImportIssue>) {
        let issues = self
            .issues
            .into_iter()
            .map(|(address, feature)| ImportIssue { address, feature })
            .collect();
        (self.model, issues)
    }
}
