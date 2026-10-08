//! Game data embedded in the engine segment, loaded from the identified ROM.
use super::{ImportError, Result, rom::Rom, version};
use crate::simulation::math::TrigTables;
use sha1::{Digest, Sha1};

fn verified(
    rom: &Rom,
    range: std::ops::Range<usize>,
    sha1: &str,
    what: &'static str,
) -> Result<Vec<u8>> {
    let bytes = rom.reader().slice(range.start, range.len())?;
    let digest = format!("{:x}", Sha1::digest(bytes));
    if digest != sha1 {
        return Err(ImportError::new(
            what,
            range.start,
            format!("table digest {digest} does not match {sha1}"),
        ));
    }
    Ok(bytes.to_vec())
}

/// gSineTable/gCosineTable (big-endian f32) and gArctanTable (big-endian s16).
pub fn trig_tables(rom: &Rom) -> Result<TrigTables> {
    let sine = verified(
        rom,
        version::SINE_COSINE_TABLE,
        version::SINE_COSINE_SHA1,
        "sine table",
    )?
    .chunks_exact(4)
    .map(|b| f32::from_bits(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
    .collect();
    let arctan = verified(
        rom,
        version::ARCTAN_TABLE,
        version::ARCTAN_SHA1,
        "arctan table",
    )?
    .chunks_exact(2)
    .map(|b| u16::from_be_bytes([b[0], b[1]]))
    .collect();
    TrigTables::new(sine, arctan).ok_or_else(|| {
        ImportError::new("trig tables", version::SINE_COSINE_TABLE.start, "bad size")
    })
}
