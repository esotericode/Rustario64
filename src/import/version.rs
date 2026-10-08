//! The only revision adapter. Offsets are from pinned sm64tools configs/sm64.u.yaml.
//! The fingerprint is from pinned n64decomp/sm64 sm64.us.sha1. See PROVENANCE.md.
use std::ops::Range;

pub const US_SHA1: &str = "9bef1128717f958171a4afac3ed78ee2bb4e86ce";
pub const ROM_LEN: usize = 0x800000;
pub const IMPORT_SCHEMA: u32 = 3;
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
