//! Mario's DMA animation table (`gMarioAnims`), decoded with bounds checks.
//!
//! Layout from the pinned CC0 decomp: `struct DmaTable`/`load_patchable_table`
//! in src/game/memory.c, `struct Animation` in include/types.h, and the table
//! built by tools/mario_anims_converter.py: a count, a placeholder word, then
//! (offset, size) pairs relative to the table start. Each entry is one DMA
//! unit: a 0x18-byte header whose `values`/`index` words are offsets from the
//! header, followed by the arrays. Two IDs may share arrays (the original's
//! anim_01_02); each entry is decoded from its own DMA range, as the game
//! loads it. The ROM range comes from the version adapter.
use super::{ImportError, Result, reader::Reader, rom::Rom, segments::Segments, version};
use crate::content::animation::{Animation, MarioAnimations, ObjectAnimations};

const HEADER_BYTES: usize = 0x18;
/// The US table has 209 entries; anything above this is rejected as corrupt.
pub const MAX_ANIMATIONS: usize = 0x100;
const CONTEXT: &str = "Mario animation table";

fn error(offset: usize, detail: impl Into<String>) -> ImportError {
    ImportError::new(CONTEXT, offset, detail)
}

/// Decode one DMA table. Every attribute must stay inside its entry's values,
/// so later frame lookups are bounded for non-negative frames.
pub fn decode_dma_table(block: &[u8]) -> Result<MarioAnimations> {
    let table = Reader::new(block, CONTEXT);
    let count = table.u32(0)? as usize;
    if count == 0 || count > MAX_ANIMATIONS {
        return Err(error(
            0,
            format!("{count} entries; expected 1..={MAX_ANIMATIONS}"),
        ));
    }
    // The placeholder word at offset 4 is overwritten by load_dma_table_address
    // and never read, so its value is not validated.
    let mut animations = Vec::with_capacity(count);
    for id in 0..count {
        let pair = 8 + id * 8;
        let offset = table.u32(pair)? as usize;
        let size = table.u32(pair + 4)? as usize;
        let entry = Reader::new(table.slice(offset, size)?, CONTEXT);
        animations.push(decode_entry(entry, offset, id)?);
    }
    Ok(MarioAnimations { animations })
}

fn decode_entry(entry: Reader<'_>, base: usize, id: usize) -> Result<Animation> {
    let at = |offset: usize| base + offset;
    let fail =
        |offset: usize, detail: String| error(at(offset), format!("animation {id:#X}: {detail}"));
    if entry.len() < HEADER_BYTES {
        return Err(fail(
            0,
            format!("{} bytes cannot hold a header", entry.len()),
        ));
    }
    let bone_count = entry.i16(0x0A)?;
    let values_offset = entry.u32(0x0C)? as usize;
    let index_offset = entry.u32(0x10)? as usize;
    let length = entry.u32(0x14)? as usize;
    if length != entry.len() {
        return Err(fail(
            0x14,
            format!(
                "length {length:#X} differs from DMA size {:#X}",
                entry.len()
            ),
        ));
    }
    let attributes = usize::try_from(bone_count)
        .map_err(|_| fail(0x0A, format!("negative part count {bone_count}")))?
        .checked_add(1)
        .map(|parts| parts * 3)
        .ok_or_else(|| fail(0x0A, "part count overflow".into()))?;
    let index_end = index_offset + attributes * 4;
    if index_offset < HEADER_BYTES || index_end > values_offset || values_offset >= entry.len() {
        return Err(fail(
            0x0C,
            format!(
                "index {index_offset:#X}..{index_end:#X} and values {values_offset:#X} do not fit {:#X} bytes",
                entry.len()
            ),
        ));
    }
    if !(entry.len() - values_offset).is_multiple_of(2) {
        return Err(fail(values_offset, "values end mid-halfword".into()));
    }
    let index: Vec<u16> = entry
        .slice(index_offset, attributes * 4)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_be_bytes(*b))
        .collect();
    let values: Vec<i16> = entry
        .slice(values_offset, entry.len() - values_offset)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    for (attribute, pair) in index.as_chunks::<2>().0.iter().enumerate() {
        let [frames, offset] = pair.map(usize::from);
        if frames == 0 || offset + frames > values.len() {
            return Err(fail(
                index_offset + attribute * 4,
                format!(
                    "attribute {attribute} reads values {offset}+{frames} of {}",
                    values.len()
                ),
            ));
        }
    }
    Ok(Animation {
        flags: entry.i16(0)?,
        y_trans_divisor: entry.i16(2)?,
        start_frame: entry.i16(4)?,
        loop_start: entry.i16(6)?,
        loop_end: entry.i16(8)?,
        bone_count,
        index,
        values,
    })
}

/// Mario's animations from the identified ROM.
pub fn mario_animations(rom: &Rom) -> Result<MarioAnimations> {
    let range = version::MARIO_ANIMATIONS;
    let animations = decode_dma_table(rom.reader().slice(range.start, range.len())?)
        .map_err(|e| ImportError::new(e.context, range.start + e.offset, e.detail))?;
    if animations.animations.len() != version::MARIO_ANIMATION_COUNT {
        return Err(ImportError::new(
            CONTEXT,
            range.start,
            format!(
                "{} entries; the US table has {}",
                animations.animations.len(),
                version::MARIO_ANIMATION_COUNT
            ),
        ));
    }
    Ok(animations)
}

/// Object animation tables hold a few entries before their NULL; anything
/// longer is rejected as corrupt.
pub const MAX_OBJECT_TABLE_ENTRIES: usize = 64;
const OBJECT_CONTEXT: &str = "object animation";

/// Decode the struct Animation at a segmented address: a 0x18-byte header
/// whose `values` and `index` are segmented pointers (`length` is 0 outside
/// Mario's DMA table). The index holds (part count + 1) * 3 attributes, as
/// the pinned ANIMINDEX_NUMPARTS sizes `unusedBoneCount`; each attribute's
/// frames must lie inside the mapped values, so later frame lookups are
/// bounded for non-negative frames.
pub fn decode_object_animation(segments: &Segments, address: u32) -> Result<Animation> {
    let fail = |detail: String| ImportError::new(OBJECT_CONTEXT, address as usize, detail);
    let header = Reader::new(segments.read(address, HEADER_BYTES)?, OBJECT_CONTEXT);
    let bone_count = header.i16(0x0A)?;
    let values_address = header.u32(0x0C)?;
    let index_address = header.u32(0x10)?;
    let length = header.u32(0x14)?;
    if length != 0 {
        return Err(fail(format!(
            "length {length:#X}; object animations are not DMA entries"
        )));
    }
    let attributes = usize::try_from(bone_count)
        .ok()
        .and_then(|parts| parts.checked_add(1))
        .map(|parts| parts * 3)
        .ok_or_else(|| fail(format!("part count {bone_count}")))?;
    let index: Vec<u16> = segments
        .read(index_address, attributes * 4)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_be_bytes(*b))
        .collect();
    let used = index
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[frames, offset]| {
            if *frames == 0 {
                Err(fail("an attribute with no frames".into()))
            } else {
                Ok(usize::from(*offset) + usize::from(*frames))
            }
        })
        .try_fold(0, |end, used| used.map(|u| end.max(u)))?;
    let values: Vec<i16> = segments
        .read(values_address, used * 2)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    Ok(Animation {
        flags: header.i16(0)?,
        y_trans_divisor: header.i16(2)?,
        start_frame: header.i16(4)?,
        loop_start: header.i16(6)?,
        loop_end: header.i16(8)?,
        bone_count,
        index,
        values,
    })
}

/// Decode an object animation pointer table at a segmented address (its
/// entries up to the NULL) and every animation it names into `out`.
pub fn decode_object_table(
    segments: &Segments,
    table: u32,
    out: &mut ObjectAnimations,
) -> Result<()> {
    if out.tables.contains_key(&table) {
        return Ok(());
    }
    let mut entries = vec![];
    for i in 0..=MAX_OBJECT_TABLE_ENTRIES {
        let entry = u32::from_be_bytes(
            segments
                .read(table + 4 * i as u32, 4)?
                .try_into()
                .expect("four bytes"),
        );
        if entry == 0 {
            if entries.is_empty() {
                return Err(ImportError::new(
                    OBJECT_CONTEXT,
                    table as usize,
                    "empty animation table",
                ));
            }
            for &address in &entries {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    out.animations.entry(address)
                {
                    entry.insert(decode_object_animation(segments, address)?);
                }
            }
            out.tables.insert(table, entries);
            return Ok(());
        }
        entries.push(entry);
    }
    Err(ImportError::new(
        OBJECT_CONTEXT,
        table as usize,
        format!("no NULL within {MAX_OBJECT_TABLE_ENTRIES} entries"),
    ))
}
