//! Engine-owned visible geometry produced by the Fast3D importer.
//!
//! These meshes are presentation content only. Gameplay collision stays in
//! `CollisionMesh`; nothing here feeds simulation, and no ROM pointer survives
//! except in `source` fields kept for diagnostics.
use serde::{Deserialize, Serialize};

/// Original geo-layout drawing layers (pinned sm64 include/sm64.h).
pub const LAYER_FORCE: u8 = 0;
pub const LAYER_OPAQUE: u8 = 1;
pub const LAYER_OPAQUE_DECAL: u8 = 2;
pub const LAYER_OPAQUE_INTER: u8 = 3;
pub const LAYER_ALPHA: u8 = 4;
pub const LAYER_TRANSPARENT: u8 = 5;
pub const LAYER_TRANSPARENT_DECAL: u8 = 6;
pub const LAYER_TRANSPARENT_INTER: u8 = 7;
pub const LAYER_COUNT: usize = 8;

/// Decoded RGBA8 image, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureImage {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
    /// Segmented source address and N64 format/size, for diagnostics only.
    pub source: u32,
    pub format: u8,
    pub size: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WrapMode {
    Repeat,
    MirrorRepeat,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureFilter {
    Point,
    Bilinear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextureBinding {
    /// Index into `VisualModel::textures`.
    pub texture: usize,
    pub wrap: [WrapMode; 2],
    pub filter: TextureFilter,
}

/// One directional light plus ambient, as loaded by gsSPLight with one light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Lights {
    pub ambient: [u8; 3],
    pub diffuse: [u8; 3],
    /// Signed direction in the modelview output space (camera space in SM64).
    pub direction: [i8; 3],
}

/// Color-combiner selectors for one cycle: RGB (a, b, c, d) and alpha (a, b, c, d),
/// using the RDP's G_CCMUX / G_ACMUX numbering from the pinned gbi.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CombinerCycle {
    pub rgb: [u8; 4],
    pub alpha: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    Opaque,
    /// Alpha-tested cutout (coverage times alpha, or an alpha-compare threshold).
    Cutout,
    Translucent,
}

/// Everything the renderer needs to draw a batch, decoded from RSP/RDP state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Material {
    pub layer: u8,
    pub combiner: [CombinerCycle; 2],
    pub two_cycle: bool,
    pub texture: Option<TextureBinding>,
    /// Vertex colors are normals and shade comes from these lights.
    pub lights: Option<Lights>,
    pub texture_gen: bool,
    pub prim_color: [u8; 4],
    pub env_color: [u8; 4],
    pub fog: Option<Fog>,
    pub blend: BlendMode,
    pub depth_test: bool,
    pub depth_write: bool,
    pub decal: bool,
    pub cull_back: bool,
    pub cull_front: bool,
}

/// gsSPFogPosition-encoded multiplier/offset and the RDP fog color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fog {
    pub multiplier: i16,
    pub offset: i16,
    pub color: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisualVertex {
    pub position: [f32; 3],
    /// Texture coordinates normalized to the bound texture's dimensions.
    pub uv: [f32; 2],
    /// Vertex color, or a signed normal in xyz when the material has lights.
    pub color: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawBatch {
    pub material: Material,
    /// Non-indexed triangle list in original draw order.
    pub vertices: Vec<VisualVertex>,
    /// Segmented display list that produced the first triangle, for diagnostics.
    pub source: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VisualModel {
    pub textures: Vec<TextureImage>,
    /// Batches in original order; the renderer draws layers 0..7 in turn.
    pub batches: Vec<DrawBatch>,
}

/// A model whose vertices stay in the space of the transform node that was
/// current when they were loaded, so a pose can place them each frame.
/// `bones[b][v]` is the node of `model.batches[b].vertices[v]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SkinnedModel {
    pub model: VisualModel,
    pub bones: Vec<Vec<u16>>,
}

impl VisualModel {
    pub fn triangle_count(&self) -> usize {
        self.batches.iter().map(|b| b.vertices.len() / 3).sum()
    }

    /// Axis-aligned bounds of every visible vertex, or None for an empty model.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut vertices = self.batches.iter().flat_map(|b| &b.vertices);
        let first = vertices.next()?.position;
        Some(vertices.fold((first, first), |(mut lo, mut hi), v| {
            for i in 0..3 {
                lo[i] = lo[i].min(v.position[i]);
                hi[i] = hi[i].max(v.position[i]);
            }
            (lo, hi)
        }))
    }
}

/// Sky background selected by the area geo layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Background {
    /// Original skybox ID (BACKGROUND_OCEAN_SKY = 0, ...); drawn by a native callback.
    Skybox(i16),
    /// RGBA5551 clear color.
    Color(u16),
}

/// Camera node values from the area geo layout (original integer units),
/// with the native callbacks of the camera node and of the perspective node
/// that encloses it (ROM addresses; `import::version::AREA_CAMERA_CALLBACKS`
/// names them).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoCamera {
    pub mode: i16,
    pub position: [i16; 3],
    pub focus: [i16; 3],
    pub fov_degrees: i16,
    pub near: i16,
    pub far: i16,
    pub callback: u32,
    pub perspective_callback: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreaVisual {
    pub area: u8,
    pub model: VisualModel,
    pub background: Option<Background>,
    pub camera: Option<GeoCamera>,
}
