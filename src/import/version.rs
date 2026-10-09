//! The only revision adapter. Offsets are from pinned sm64tools configs/sm64.u.yaml.
//! The fingerprint is from pinned n64decomp/sm64 sm64.us.sha1. See PROVENANCE.md.
use std::ops::Range;

pub const US_SHA1: &str = "9bef1128717f958171a4afac3ed78ee2bb4e86ce";
pub const ROM_LEN: usize = 0x800000;
pub const IMPORT_SCHEMA: u32 = 4;
pub const REFERENCE_REVISION: &str = "9921382a68bb0c865e5e45eb594d9c64db59b1af";
pub const BOB_TERRAIN: Range<usize> = 0x3FC2B0..0x405A60;
pub const BOB_LEVEL: Range<usize> = 0x405A60..0x405FB0;
pub const BOB_COLLISION: u32 = 0x0700E958;
pub const SCRIPT_SEGMENT: u8 = 0x0E;
pub const TERRAIN_SEGMENT: u8 = 0x07;
pub const MAX_SEGMENT_BYTES: usize = 4 * 1024 * 1024;
pub const BOB_TEXTURE_OFFSETS: [usize; 5] = [0, 0x800, 0x1000, 0x1800, 0x2000];
/// Number of entries in the pinned US sMacroObjectPresets table (CC0 sm64).
pub const MACRO_PRESET_COUNT: u16 = 366;
/// Engine-segment trig tables (gSineTable followed by gCosineTable, then
/// gArctanTable). Located by matching the pinned decomp's
/// include/trig_tables.inc.c against the identified ROM; the ranges map to VRAM
/// 0x80386000 and 0x8038B000 under sm64tools' engine segment (ROM 0xF5580 at
/// 0x80378800). Digests guard the contents; the values are not stored here.
pub const SINE_COSINE_TABLE: Range<usize> = 0x102D80..0x107D80;
pub const SINE_COSINE_SHA1: &str = "ada98573b7792b42e28667a452233c7275a81782";
pub const ARCTAN_TABLE: Range<usize> = 0x107D80..0x108582;
pub const ARCTAN_SHA1: &str = "c282767b1d02c68afe1e6c65e091540d868bc032";
/// Mario's DMA animation table (`gMarioAnims`): sm64tools' "mario_animation"
/// block. The decoder checks its structure; the content is validated against
/// the pinned decomp's animation sources (tools/check_mario_anims_reference.py).
pub const MARIO_ANIMATIONS: Range<usize> = 0x4EC000..0x579C26;
/// MARIO_ANIM_* IDs 0x00..=0xD0 in the pinned include/mario_animation_ids.h.
pub const MARIO_ANIMATION_COUNT: usize = 0xD1;
/// The main level scripts, `level_main_scripts_entry` first (sm64tools
/// "main_level_scripts"), executed as segment 0x15.
pub const MAIN_LEVEL_SCRIPTS: Range<usize> = 0x2ABCA0..0x2AC6B0;
pub const MAIN_SCRIPTS_SEGMENT: u8 = 0x15;
/// group0 (sm64tools "mario_water_sparkles"): Mario's display lists and
/// textures as MIO0 segment 4, and the group's geo layouts as raw segment
/// 0x17. The importer checks that the main scripts load exactly these.
pub const GROUP0_MIO0: Range<usize> = 0x114750..0x1279B0;
pub const GROUP0_SEGMENT: u8 = 0x04;
/// sm64tools' font_graphics MIO0 segment; pinned sm64 assets.json names
/// the 16x16 IA8 quarter-circle at decoded offset 0x120B8.
pub const SEGMENT2_MIO0: Range<usize> = 0x108A40..0x114750;
pub const SHADOW_CIRCLE_TEXTURE: usize = 0x120B8;
pub const GROUP0_GEO: Range<usize> = 0x1279B0..0x12A7E0;
pub const GROUP0_GEO_SEGMENT: u8 = 0x17;
/// MODEL_MARIO in the pinned include/model_ids.h.
pub const MODEL_MARIO: u16 = 1;
/// The native callbacks of an area geo layout's camera nodes, by decomp name
/// and address in this revision: the GEO_CAMERA node's and the enclosing
/// GEO_CAMERA_FRUSTUM_WITH_FUNC node's. BOB area 1's nodes hold these
/// addresses where the pinned levels/bob/areas/1/geo.inc.c names the
/// functions (the owner-ROM import test pins the node values that align them).
pub const AREA_CAMERA_CALLBACKS: [(&str, u32); 2] = [
    ("geo_camera_main", 0x80287D30),
    ("geo_camera_fov", 0x8029AA3C),
];

/// The native callbacks in Mario's geo layout, by their decomp names and their
/// addresses in this revision. Located by tools/check_mario_geo_reference.py,
/// which walks `mario_geo` from the ROM alongside the pinned
/// actors/mario/geo.inc.c; five also carry sm64tools' unnamed GeoSwitchCase
/// labels.
pub const MARIO_GEO_CALLBACKS: [(&str, u32); 13] = [
    ("geo_mirror_mario_set_alpha", 0x802770A4),
    ("geo_switch_mario_stand_run", 0x80277150),
    ("geo_switch_mario_eyes", 0x802771BC),
    ("geo_mario_tilt_torso", 0x80277294),
    ("geo_mario_head_rotation", 0x802773A4),
    ("geo_switch_mario_hand", 0x802774F4),
    ("geo_mario_hand_foot_scaler", 0x802775CC),
    ("geo_switch_mario_cap_effect", 0x802776D8),
    ("geo_switch_mario_cap_on_off", 0x80277740),
    ("geo_mario_rotate_wing_cap_wings", 0x80277824),
    ("geo_switch_mario_hand_grab_pos", 0x8027795C),
    ("geo_mirror_mario_backface_culling", 0x80277D6C),
    ("geo_move_mario_part_from_parent", 0x802B1BB0),
];
