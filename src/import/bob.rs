use super::{
    ImportError, Result, collision, level, mio0, model,
    rom::Rom,
    segments::Segments,
    texture::{self, Texture},
    version,
};
use crate::content::{
    CollisionMesh, CourseId, ImportIssue, ImportedLevel, LevelId, SegmentLoad,
    visual::{AreaVisual, VisualModel},
};
use serde::Serialize;

/// A geo-layout model registered by LOAD_MODEL_FROM_GEO in the level script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelVisual {
    pub model: u16,
    pub geometry_layout: u32,
    pub visual: VisualModel,
}

/// A dependent segment loaded from the ROM range named by the level script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LoadedSegment {
    pub segment: u8,
    pub rom_start: u32,
    pub rom_end: u32,
    pub mio0: bool,
    pub bytes: usize,
}

pub struct BobImport {
    pub level: ImportedLevel,
    pub collision: CollisionMesh,
    pub textures: Vec<Texture>,
    pub terrain_bytes: usize,
    /// Visible area geometry; present only when dependent segments were loaded.
    pub visual: Option<AreaVisual>,
    pub models: Vec<ModelVisual>,
    pub loaded_segments: Vec<LoadedSegment>,
}

/// Load one LOAD_RAW/LOAD_MIO0 range from the identified ROM, bounded.
pub fn load_segment(rom: &Rom, load: &SegmentLoad) -> Result<Vec<u8>> {
    let start = load.rom_start as usize;
    let len = (load.rom_end - load.rom_start) as usize;
    let bytes = rom.reader().slice(start, len).map_err(|e| {
        ImportError::new(
            "segment load",
            load.address as usize,
            format!("segment 0x{:02X} range: {e}", load.segment),
        )
    })?;
    if load.mio0 {
        mio0::decode(bytes, version::MAX_SEGMENT_BYTES)
    } else if len > version::MAX_SEGMENT_BYTES {
        Err(ImportError::new(
            "segment load",
            load.address as usize,
            "raw segment exceeds limit",
        ))
    } else {
        Ok(bytes.to_vec())
    }
}

pub fn import(rom: &Rom) -> Result<BobImport> {
    let r = rom.reader();
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        r.slice(range.start, range.len())?,
        version::MAX_SEGMENT_BYTES,
    )?;
    let range = version::BOB_LEVEL;
    let script = r.slice(range.start, range.len())?.to_vec();
    let (mut segments, entry) = base_segments(terrain, script)?;
    let mut level = level::extract(&segments, entry, CourseId(1), LevelId(9))?;
    let mut loaded_segments = vec![];
    for load in level.segment_loads.clone() {
        if load.segment == version::TERRAIN_SEGMENT {
            // The pinned manifest range was used above; the script must agree.
            if (load.rom_start as usize, load.rom_end as usize)
                != (version::BOB_TERRAIN.start, version::BOB_TERRAIN.end)
                || !load.mio0
            {
                return Err(ImportError::new(
                    "BOB",
                    load.address as usize,
                    "segment 7 load disagrees with pinned manifest",
                ));
            }
            continue;
        }
        let bytes = load_segment(rom, &load)?;
        loaded_segments.push(LoadedSegment {
            segment: load.segment,
            rom_start: load.rom_start,
            rom_end: load.rom_end,
            mio0: load.mio0,
            bytes: bytes.len(),
        });
        segments
            .insert(load.segment, bytes)
            .map_err(|e| ImportError::new("segment load", load.address as usize, e.to_string()))?;
    }
    let (collision, textures, terrain_bytes) = decode_static(&segments, &mut level, entry)?;
    let area = level
        .areas
        .iter()
        .find(|a| a.id.0 == 1)
        .expect("decode_static checked area 1");
    let (visual, issues) = model::area(&segments, 1, area.geometry_layout)?;
    level.issues.extend(issues);
    let mut models = vec![];
    for definition in level.models.clone() {
        if !definition.geometry_layout {
            level.issues.push(ImportIssue {
                address: definition.pointer,
                feature: format!(
                    "model {} display-list model not decoded (layer {:?})",
                    definition.model, definition.layer
                ),
            });
            continue;
        }
        // Models in segments loaded by global scripts stay reported gaps.
        let built = super::geo::decode(&segments, definition.pointer)
            .and_then(|layout| model::build(&segments, &layout));
        match built {
            Ok(built) => {
                level.issues.extend(built.issues);
                models.push(ModelVisual {
                    model: definition.model,
                    geometry_layout: definition.pointer,
                    visual: built.model,
                });
            }
            Err(e) => level.issues.push(ImportIssue {
                address: definition.pointer,
                feature: format!("model {} geometry not imported: {e}", definition.model),
            }),
        }
    }
    report_unloaded(&mut level, &segments);
    Ok(BobImport {
        level,
        collision,
        textures,
        terrain_bytes,
        visual: Some(visual),
        models,
        loaded_segments,
    })
}

fn base_segments(terrain: Vec<u8>, script: Vec<u8>) -> Result<(Segments, u32)> {
    let entry = level::bob_entry(&script)?;
    let mut segments = Segments::default();
    segments.insert(version::TERRAIN_SEGMENT, terrain)?;
    segments.insert(version::SCRIPT_SEGMENT, script)?;
    Ok((segments, entry))
}

fn report_unloaded(level: &mut ImportedLevel, segments: &Segments) {
    for load in &level.segment_loads {
        if !segments.is_mapped(load.segment) {
            level.issues.push(ImportIssue {
                address: load.address,
                feature: format!(
                    "segment 0x{:02X} load recorded; dependency not imported",
                    load.segment
                ),
            });
        }
    }
}

fn decode_static(
    segments: &Segments,
    level: &mut ImportedLevel,
    entry: u32,
) -> Result<(CollisionMesh, Vec<Texture>, usize)> {
    let area = level
        .areas
        .iter()
        .find(|a| a.id.0 == 1)
        .ok_or_else(|| ImportError::new("BOB", entry as usize, "missing area 1"))?;
    if area.terrain != Some(version::BOB_COLLISION) {
        return Err(ImportError::new(
            "BOB",
            entry as usize,
            "terrain pointer disagrees with pinned manifest",
        ));
    }
    let (collision, _) = collision::decode(segments.tail(version::BOB_COLLISION)?)?;
    for placement in &collision.specials {
        level.issues.push(ImportIssue {
            address: version::BOB_COLLISION,
            feature: format!(
                "special preset 0x{:02X} parsed; spawn behavior not implemented",
                placement.preset
            ),
        });
    }
    let mut textures = Vec::new();
    for offset in version::BOB_TEXTURE_OFFSETS {
        let address = (u32::from(version::TERRAIN_SEGMENT) << 24) | offset as u32;
        textures.push(texture::rgba16(
            segments.read(address, 32 * 32 * 2)?,
            32,
            32,
        )?);
    }
    let terrain_bytes = segments
        .tail(u32::from(version::TERRAIN_SEGMENT) << 24)?
        .len();
    Ok((collision, textures, terrain_bytes))
}

/// Same static decoding path used by independently authored integration fixtures.
/// Does not validate a ROM or produce a claim about the original course. Visible
/// geometry needs dependent segments and is decoded only by `import`.
pub fn decode_segments(terrain: Vec<u8>, script: Vec<u8>) -> Result<BobImport> {
    let (segments, entry) = base_segments(terrain, script)?;
    let mut level = level::extract(&segments, entry, CourseId(1), LevelId(9))?;
    let (collision, textures, terrain_bytes) = decode_static(&segments, &mut level, entry)?;
    report_unloaded(&mut level, &segments);
    let mut not_decoded: Vec<ImportIssue> = level
        .areas
        .iter()
        .map(|area| ImportIssue {
            address: area.geometry_layout,
            feature: format!(
                "area {} geometry layout not decoded without dependent segments",
                area.id.0
            ),
        })
        .collect();
    not_decoded.extend(level.models.iter().map(|m| ImportIssue {
        address: m.pointer,
        feature: format!(
            "model {} geometry not decoded without dependent segments",
            m.model
        ),
    }));
    level.issues.extend(not_decoded);
    Ok(BobImport {
        level,
        collision,
        textures,
        terrain_bytes,
        visual: None,
        models: vec![],
        loaded_segments: vec![],
    })
}
