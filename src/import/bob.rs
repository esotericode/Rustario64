use super::{
    ImportError, Result, collision, level, mio0,
    rom::Rom,
    segments::Segments,
    texture::{self, Texture},
    version,
};
use crate::content::{CollisionMesh, CourseId, ImportIssue, ImportedLevel, LevelId};

pub struct BobImport {
    pub level: ImportedLevel,
    pub collision: CollisionMesh,
    pub textures: Vec<Texture>,
    pub terrain_bytes: usize,
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
    decode_segments(terrain, script)
}

/// Same decoding path used by independently authored integration fixtures.
/// Does not validate a ROM or produce a claim about the original course.
pub fn decode_segments(terrain: Vec<u8>, script: Vec<u8>) -> Result<BobImport> {
    let entry = level::bob_entry(&script)?;
    let terrain_bytes = terrain.len();
    let mut segments = Segments::default();
    segments.insert(version::TERRAIN_SEGMENT, terrain)?;
    segments.insert(version::SCRIPT_SEGMENT, script)?;
    let mut level = level::extract(&segments, entry, CourseId(1), LevelId(9))?;
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
    Ok(BobImport {
        level,
        collision,
        textures,
        terrain_bytes,
    })
}
