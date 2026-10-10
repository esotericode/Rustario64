//! A bounded static-content extractor, not a level-script gameplay VM.
//! Local JUMP_LINK/RETURN traversals preserve placement encounter order.
//! Commands follow pinned CC0 sm64 include/level_commands.h and level_script.c.
use super::{ImportError, Result, macros, reader::Reader, segments::Segments, version};
use crate::content::*;
use std::collections::BTreeMap;

const MAX_COMMANDS: usize = 4096;
const MAX_CALL_DEPTH: usize = 64;

pub(crate) fn expected_length(opcode: u8) -> Option<usize> {
    Some(match opcode {
        0x00 | 0x01 | 0x16 => 16,
        0x02..=0x04
        | 0x07..=0x0A
        | 0x0F..=0x10
        | 0x13..=0x15
        | 0x19
        | 0x1B..=0x1E
        | 0x20
        | 0x29..=0x2A
        | 0x2C..=0x2D
        | 0x30..=0x32
        | 0x34..=0x35
        | 0x37..=0x38
        | 0x3C => 4,
        0x05..=0x06
        | 0x0B
        | 0x0E
        | 0x11..=0x12
        | 0x1F
        | 0x21..=0x22
        | 0x26..=0x27
        | 0x2E..=0x2F
        | 0x33
        | 0x36
        | 0x39 => 8,
        0x0C..=0x0D | 0x17..=0x18 | 0x1A | 0x23 | 0x25 | 0x28 | 0x2B | 0x3A..=0x3B => 12,
        0x24 => 24,
        _ => return None,
    })
}

/// The exact verified ROM's entry is found from the upstream INIT_LEVEL/load pair.
/// No guessed symbol offset: match must be unique and aligned in the bounded blob.
pub fn bob_entry(script: &[u8]) -> Result<u32> {
    let mut needle = [
        0x1B,
        4,
        0,
        0,
        0x18,
        12,
        0,
        version::TERRAIN_SEGMENT,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    needle[8..12].copy_from_slice(&(version::BOB_TERRAIN.start as u32).to_be_bytes());
    needle[12..16].copy_from_slice(&(version::BOB_TERRAIN.end as u32).to_be_bytes());
    let matches: Vec<_> = script
        .windows(needle.len())
        .enumerate()
        .filter(|(offset, bytes)| offset % 4 == 0 && *bytes == needle)
        .map(|(offset, _)| offset)
        .collect();
    if matches.len() != 1 {
        return Err(ImportError::new(
            "BOB entry",
            0,
            format!(
                "expected unique INIT_LEVEL/terrain-load pair, found {}",
                matches.len()
            ),
        ));
    }
    Ok((u32::from(version::SCRIPT_SEGMENT) << 24) | matches[0] as u32)
}

pub fn extract(
    segments: &Segments,
    entry: u32,
    course: CourseId,
    level: LevelId,
) -> Result<ImportedLevel> {
    let mut result = ImportedLevel {
        course,
        level,
        areas: vec![],
        mario_start: None,
        segment_loads: vec![],
        models: vec![],
        commands: BTreeMap::new(),
        issues: vec![],
    };
    let mut address = entry;
    let mut stack = Vec::new();
    let mut area_index = None;
    for _ in 0..MAX_COMMANDS {
        let header = segments.read(address, 2)?;
        let opcode = header[0];
        let length = usize::from(header[1]);
        let expected = expected_length(opcode).ok_or_else(|| {
            ImportError::new(
                "level script",
                address as usize,
                format!("unsupported opcode 0x{opcode:02X}"),
            )
        })?;
        if length != expected {
            return Err(ImportError::new(
                "level script",
                address as usize,
                format!("opcode 0x{opcode:02X}: length {length}, expected {expected}"),
            ));
        }
        let r = Reader::new(segments.read(address, length)?, "level command");
        let offset = address & 0xFFFFFF;
        if offset + length as u32 > 0xFFFFFF {
            return Err(ImportError::new(
                "level script",
                address as usize,
                "command crosses segment",
            ));
        }
        let next = address + length as u32;
        *result.commands.entry(opcode).or_default() += 1;
        match opcode {
            0x02 | 0x04 => {
                if area_index.is_some() || !stack.is_empty() {
                    return Err(ImportError::new(
                        "level script",
                        address as usize,
                        "unbalanced area or calls at exit",
                    ));
                }
                return Ok(result);
            }
            0x05 | 0x06 => {
                let target = r.u32(4)?;
                if !segments.is_mapped((target >> 24) as u8) {
                    result.issues.push(ImportIssue {
                        address,
                        feature: format!("unresolved script call/jump 0x{target:08X}"),
                    });
                    if opcode == 0x05 {
                        return Ok(result);
                    }
                } else {
                    segments.read(target, 2)?;
                    if opcode == 0x06 {
                        if stack.len() >= MAX_CALL_DEPTH {
                            return Err(ImportError::new(
                                "level script",
                                address as usize,
                                "call depth exceeded",
                            ));
                        }
                        stack.push(next);
                    }
                    address = target;
                    continue;
                }
            }
            0x07 => {
                address = stack.pop().ok_or_else(|| {
                    ImportError::new("level script", address as usize, "RETURN without caller")
                })?;
                continue;
            }
            0x11 | 0x12 => {
                result.issues.push(ImportIssue {
                    address,
                    feature: format!(
                        "native callback 0x{:08X}, arg {}; not executed",
                        r.u32(4)?,
                        r.i16(2)?
                    ),
                });
            }
            0x17 | 0x18 | 0x1A => {
                let segment = u8::try_from(r.u16(2)?).map_err(|_| {
                    ImportError::new("level script", address as usize, "segment ID out of range")
                })?;
                if segment == 0 || segment >= 32 || r.u32(4)? >= r.u32(8)? {
                    return Err(ImportError::new(
                        "level script",
                        address as usize,
                        "invalid segment load",
                    ));
                }
                result.segment_loads.push(SegmentLoad {
                    address,
                    segment,
                    rom_start: r.u32(4)?,
                    rom_end: r.u32(8)?,
                    mio0: opcode != 0x17,
                    texture: opcode == 0x1A,
                });
            }
            0x1F => {
                if area_index.is_some() {
                    return Err(ImportError::new(
                        "level script",
                        address as usize,
                        "nested AREA",
                    ));
                }
                let id = AreaId(r.u8(2)?);
                if id.0 == 0 || result.areas.iter().any(|a| a.id == id) {
                    return Err(ImportError::new(
                        "level script",
                        address as usize,
                        "invalid or duplicate area",
                    ));
                }
                result.areas.push(ImportedArea {
                    id,
                    geometry_layout: r.u32(4)?,
                    terrain: None,
                    macro_objects: None,
                    macro_spawns: vec![],
                    spawns: vec![],
                    warps: vec![],
                    terrain_type: 0,
                    dialog_ids: [0xFF; 2],
                    background_music: AreaMusic::default(),
                });
                area_index = Some(result.areas.len() - 1);
            }
            0x20 => {
                if area_index.take().is_none() {
                    return Err(ImportError::new(
                        "level script",
                        address as usize,
                        "END_AREA without AREA",
                    ));
                }
            }
            0x21 | 0x22 => {
                let field = r.u16(2)?;
                result.models.push(ModelDefinition {
                    model: field & 0xFFF,
                    pointer: r.u32(4)?,
                    geometry_layout: opcode == 0x22,
                    layer: (opcode == 0x21).then_some((field >> 12) as u8),
                });
            }
            0x24 => {
                let i = area_index.ok_or_else(|| {
                    ImportError::new("level script", address as usize, "OBJECT outside AREA")
                })?;
                let position = [r.i16(4)?, r.i16(6)?, r.i16(8)?];
                let angles_degrees = [r.i16(10)?, r.i16(12)?, r.i16(14)?];
                let behavior_script = r.u32(20)?;
                result.areas[i].spawns.push(ImportedSpawn {
                    source_address: address,
                    acts: ActMask(r.u8(2)?),
                    model: r.u8(3)?,
                    position,
                    angles_degrees,
                    behavior_params: r.u32(16)?,
                    behavior_script,
                });
                result.issues.push(ImportIssue {
                    address,
                    feature: format!("unimplemented behavior script 0x{behavior_script:08X}"),
                });
            }
            0x25 => result.issues.push(ImportIssue {
                address,
                feature: format!(
                    "Mario model {}, params 0x{:08X}, behavior 0x{:08X}; runtime not implemented",
                    r.u8(3)?,
                    r.u32(4)?,
                    r.u32(8)?
                ),
            }),
            0x26 => {
                let i = area_index.ok_or_else(|| {
                    ImportError::new("level script", address as usize, "WARP_NODE outside AREA")
                })?;
                result.areas[i].warps.push(WarpNode {
                    id: r.u8(2)?,
                    destination_level: LevelId(r.u8(3)?),
                    destination_area: AreaId(r.u8(4)?),
                    destination_node: r.u8(5)?,
                    flags: r.u8(6)?,
                });
            }
            0x2B => {
                if result.mario_start.is_some() {
                    return Err(ImportError::new(
                        "level script",
                        address as usize,
                        "duplicate Mario start",
                    ));
                }
                result.mario_start = Some((
                    AreaId(r.u8(2)?),
                    r.i16(4)?,
                    [r.i16(6)?, r.i16(8)?, r.i16(10)?],
                ));
            }
            0x2E | 0x39 => {
                let i = area_index.ok_or_else(|| {
                    ImportError::new(
                        "level script",
                        address as usize,
                        "terrain/macro objects outside AREA",
                    )
                })?;
                let pointer = r.u32(4)?;
                if opcode == 0x2E {
                    result.areas[i].terrain = Some(pointer);
                } else {
                    result.areas[i].macro_objects = Some(pointer);
                    if segments.is_mapped((pointer >> 24) as u8) {
                        let (placements, _) = macros::decode(segments.tail(pointer)?, pointer)?;
                        for placement in &placements {
                            result.issues.push(ImportIssue {
                                address: placement.source_address,
                                feature: format!(
                                    "macro preset {} parsed; preset defaults, behavior and respawn runtime not implemented",
                                    placement.preset_id
                                ),
                            });
                        }
                        result.areas[i].macro_spawns = placements;
                    } else {
                        result.issues.push(ImportIssue {
                            address,
                            feature: format!("macro objects 0x{pointer:08X}; segment not imported"),
                        });
                    }
                }
            }
            0x30 | 0x31 | 0x36 => {
                let i = area_index.ok_or_else(|| {
                    ImportError::new(
                        "level script",
                        address as usize,
                        "area metadata outside AREA",
                    )
                })?;
                let area = &mut result.areas[i];
                match opcode {
                    0x30 => {
                        let slot = usize::from(r.u8(2)?);
                        if let Some(dialog) = area.dialog_ids.get_mut(slot) {
                            *dialog = r.u8(3)?;
                        } else {
                            result.issues.push(ImportIssue {
                                address,
                                feature: format!(
                                    "SHOW_DIALOG slot {slot} ignored by the original; dialog {}",
                                    r.u8(3)?
                                ),
                            });
                        }
                    }
                    0x31 => area.terrain_type |= r.u16(2)?,
                    0x36 => {
                        area.background_music = AreaMusic {
                            settings_preset: r.i16(2)?,
                            sequence: r.i16(4)?,
                        }
                    }
                    _ => unreachable!(),
                }
            }
            // Import scaffolding and NOP: no gameplay execution.
            0x1B..=0x1E | 0x32 => {}
            _ => {
                return Err(ImportError::new(
                    "level script",
                    address as usize,
                    format!("opcode 0x{opcode:02X} has no static extraction implementation"),
                ));
            }
        }
        address = next;
    }
    Err(ImportError::new(
        "level script",
        address as usize,
        "command budget exceeded (possible cycle)",
    ))
}
