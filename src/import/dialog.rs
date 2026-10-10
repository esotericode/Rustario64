//! The dialog table (seg2_dialog_table) from segment 2 of the owner's ROM.
//! Its decoded offset is version data located and checked against the
//! pinned text/us/dialogs.h by tools/check_dialog_reference.py.
use super::{ImportError, Result, mio0, rom::Rom, version};
use crate::content::dialog::{DIALOG_CHAR_TERMINATOR, DialogEntry, DialogTable};

/// The longest string accepted before its terminator.
const MAX_TEXT: usize = 4096;

pub fn import(rom: &Rom) -> Result<DialogTable> {
    let range = version::SEGMENT2_MIO0;
    let segment = mio0::decode(
        rom.reader().slice(range.start, range.len())?,
        version::MAX_SEGMENT_BYTES,
    )?;
    decode(&segment, version::DIALOG_TABLE, version::DIALOG_COUNT)
}

/// Reads `count` entries from the table at `table` in a decoded segment 2,
/// then requires its NULL terminator.
pub fn decode(segment: &[u8], table: usize, count: usize) -> Result<DialogTable> {
    let word = |at: usize, what: &'static str| -> Result<u32> {
        segment
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| ImportError::new(what, at, "truncated"))
    };
    let offset_of = |pointer: u32, what: &'static str, at: usize| -> Result<usize> {
        if pointer >> 24 != 2 {
            return Err(ImportError::new(
                what,
                at,
                format!("0x{pointer:08X} is not a segment 2 address"),
            ));
        }
        Ok((pointer & 0x00FF_FFFF) as usize)
    };
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let at = table + 4 * i;
        let entry = offset_of(word(at, "dialog table")?, "dialog table", at)?;
        let bytes = segment
            .get(entry..entry + 0x10)
            .ok_or_else(|| ImportError::new("dialog entry", entry, "truncated"))?;
        let str_at = offset_of(word(entry + 0x0C, "dialog entry")?, "dialog entry", entry)?;
        let tail = segment
            .get(str_at..)
            .ok_or_else(|| ImportError::new("dialog text", str_at, "outside segment 2"))?;
        let end = tail
            .iter()
            .take(MAX_TEXT)
            .position(|&b| b == DIALOG_CHAR_TERMINATOR)
            .ok_or_else(|| ImportError::new("dialog text", str_at, "no terminator"))?;
        entries.push(DialogEntry {
            unused: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            lines_per_box: bytes[4] as i8,
            left_offset: i16::from_be_bytes([bytes[6], bytes[7]]),
            width: i16::from_be_bytes([bytes[8], bytes[9]]),
            text: tail[..=end].to_vec(),
        });
    }
    let end = table + 4 * count;
    if word(end, "dialog table")? != 0 {
        return Err(ImportError::new(
            "dialog table",
            end,
            "missing NULL terminator",
        ));
    }
    Ok(DialogTable { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_follow_segmented_pointers_to_terminated_text() {
        // Table at 0x00: two entries and NULL; entries at 0x10/0x20; text
        // at 0x30.
        let mut seg = vec![0u8; 0x40];
        seg[0..4].copy_from_slice(&0x0200_0010u32.to_be_bytes());
        seg[4..8].copy_from_slice(&0x0200_0020u32.to_be_bytes());
        for (at, lines) in [(0x10, 4u8), (0x20, 5)] {
            seg[at..at + 4].copy_from_slice(&1u32.to_be_bytes());
            seg[at + 4] = lines;
            seg[at + 6..at + 8].copy_from_slice(&95i16.to_be_bytes());
            seg[at + 8..at + 10].copy_from_slice(&200i16.to_be_bytes());
            seg[at + 12..at + 16].copy_from_slice(&0x0200_0030u32.to_be_bytes());
        }
        seg[0x30..0x34].copy_from_slice(&[0x0A, 0xFE, 0x0B, 0xFF]);
        let table = decode(&seg, 0, 2).unwrap();
        assert_eq!(table.entries.len(), 2);
        assert_eq!(table.entries[1].lines_per_box, 5);
        assert_eq!(table.entries[0].text, [0x0A, 0xFE, 0x0B, 0xFF]);
        assert_eq!(table.get(2), None);
        assert_eq!(
            crate::content::dialog::decode_us(&table.entries[0].text),
            "A\nB"
        );
        // A missing NULL or an unterminated string is an import error.
        assert!(decode(&seg, 0, 1).is_err());
        seg[0x33] = 0;
        assert!(decode(&seg, 0, 2).is_err());
    }
}
