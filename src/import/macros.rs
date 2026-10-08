//! Modern macro-object records follow the pinned CC0 sm64 loader and macros.
//! See PROVENANCE.md. This imports placements, not spawn/respawn behavior.
use super::{ImportError, Result, reader::Reader, version};
use crate::content::MacroPlacement;

pub const MAX_MACRO_OBJECTS: usize = 4096;

fn record_address(base: u32, offset: usize, len: usize) -> Result<u32> {
    let start = (base & 0xFFFFFF) as usize;
    if start + offset + len > 0x1000000 {
        return Err(ImportError::new(
            "macro objects",
            base as usize,
            "record crosses segmented address boundary",
        ));
    }
    Ok(base + offset as u32)
}

/// Returns placements in source order and consumed bytes, including the terminator.
pub fn decode(bytes: &[u8], source_address: u32) -> Result<(Vec<MacroPlacement>, usize)> {
    let segment = source_address >> 24;
    if !(1..32).contains(&segment) || source_address & 1 != 0 {
        return Err(ImportError::new(
            "macro objects",
            source_address as usize,
            "expected an aligned segmented address",
        ));
    }
    let r = Reader::new(bytes, "macro objects");
    let mut placements = Vec::new();
    let mut at = 0;
    loop {
        let address = record_address(source_address, at, 2)?;
        let packed = r.u16(at)?;
        // load_area_terrain selects the legacy format using the entire first short.
        // Do not mistake its first preset for an empty modern list.
        if at == 0 && packed < 30 {
            return Err(ImportError::new(
                "macro objects",
                address as usize,
                "legacy hardcoded macro-object format is not implemented",
            ));
        }
        let biased_preset = packed & 0x1FF;
        // The loader accepts -1 and any negative decoded preset as end markers.
        if packed == 0xFFFF || biased_preset < 31 {
            return Ok((placements, at + 2));
        }
        if placements.len() == MAX_MACRO_OBJECTS {
            return Err(ImportError::new(
                "macro objects",
                address as usize,
                "placement limit exceeded",
            ));
        }
        let preset_id = biased_preset - 31;
        if preset_id >= version::MACRO_PRESET_COUNT {
            return Err(ImportError::new(
                "macro objects",
                address as usize,
                format!("preset {preset_id} outside pinned table"),
            ));
        }
        record_address(source_address, at, 10)?;
        r.slice(at, 10)?;
        // spawn_macro_objects converts the seven yaw bits to an even byte, then
        // convert_rotation shifts that byte by eight. Its odd-byte cardinal
        // corrections cannot occur for this format. Avoid a degrees/float roundtrip.
        let yaw = (packed & 0xFE00) as i16;
        placements.push(MacroPlacement {
            source_address: address,
            packed_preset_and_yaw: packed,
            preset_id,
            position: [r.i16(at + 2)?, r.i16(at + 4)?, r.i16(at + 6)?],
            yaw,
            raw_params: r.u16(at + 8)?,
        });
        at += 10;
    }
}
