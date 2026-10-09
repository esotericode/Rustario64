//! Mario animation table decoding. Fixtures are independently authored; the
//! ignored owner-ROM test compares the real table with a digest reproduced from
//! the pinned decomp sources by tools/check_mario_anims_reference.py.
use rustario64::{
    content::animation::{ANIM_FLAG_NOLOOP, Animation},
    import::{animation, sha1_hex, version},
};

/// Author one DMA entry: header, index pairs, values, with offsets relative to
/// the header as in the original table.
fn entry(header: [i16; 5], index: &[u16], values: &[i16], patch: impl Fn(&mut Vec<u8>)) -> Vec<u8> {
    let bone_count = (index.len() / 6) as i16 - 1;
    let index_offset = 0x18u32;
    let values_offset = index_offset + 2 * index.len() as u32;
    let length = values_offset + 2 * values.len() as u32;
    let mut out = Vec::new();
    for v in header {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&bone_count.to_be_bytes());
    for v in [values_offset, index_offset, length] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    for v in index {
        out.extend_from_slice(&v.to_be_bytes());
    }
    for v in values {
        out.extend_from_slice(&v.to_be_bytes());
    }
    patch(&mut out);
    out
}

fn table(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (entries.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(&0u32.to_be_bytes());
    let mut at = 8 + 8 * entries.len() as u32;
    for e in entries {
        out.extend_from_slice(&at.to_be_bytes());
        out.extend_from_slice(&(e.len() as u32).to_be_bytes());
        at += e.len() as u32;
    }
    for e in entries {
        out.extend_from_slice(e);
    }
    out
}

/// Root translation plus one part: six (frames, offset) pairs.
const INDEX: [u16; 12] = [3, 0, 1, 3, 2, 4, 1, 6, 1, 7, 1, 8];
const VALUES: [i16; 9] = [10, -20, 30, 40, -5, 6, 7, 8, -32768];

#[test]
fn authored_table_decodes_headers_and_bounded_attributes() {
    let a = entry([1, 189, 0, 0, 34], &INDEX, &VALUES, |_| {});
    let b = entry([0x13, 189, 5, 2, 9], &INDEX[..6], &VALUES[..6], |_| {});
    let decoded = animation::decode_dma_table(&table(&[a, b])).unwrap();
    assert_eq!(decoded.animations.len(), 2);
    let first = decoded.get(0).unwrap();
    assert_eq!(
        (first.flags, first.y_trans_divisor, first.start_frame),
        (ANIM_FLAG_NOLOOP, 189, 0)
    );
    assert_eq!(
        (first.loop_start, first.loop_end, first.bone_count),
        (0, 34, 1)
    );
    assert_eq!(first.index, INDEX);
    assert_eq!(first.values, VALUES);
    // retrieve_animation_index: frames past an attribute's count repeat its last value.
    assert_eq!(first.value(0, 0), 10);
    assert_eq!(first.value(0, 2), 30);
    assert_eq!(first.value(0, 99), 30);
    assert_eq!(first.value(1, 5), 40);
    assert_eq!(first.value(5, 0), -32768);
    let second = decoded.get(1).unwrap();
    assert_eq!(second.bone_count, 0);
    assert_eq!(
        (second.start_frame, second.loop_start, second.loop_end),
        (5, 2, 9)
    );
    assert!(decoded.get(2).is_none());
}

#[test]
#[should_panic(expected = "before its value array")]
fn negative_frames_reading_before_values_are_outside_coverage() {
    let a = entry([0, 189, 0, 0, 4], &INDEX, &VALUES, |_| {});
    let decoded = animation::decode_dma_table(&table(&[a])).unwrap();
    let anim: &Animation = decoded.get(0).unwrap();
    anim.value(0, -1);
}

#[test]
fn malformed_tables_are_rejected_with_locations() {
    let good = || entry([0, 189, 0, 0, 4], &INDEX, &VALUES, |_| {});
    let rejected = |bytes: Vec<u8>, needle: &str| {
        let error = animation::decode_dma_table(&bytes).unwrap_err().to_string();
        assert!(error.contains(needle), "{error}");
    };
    rejected(table(&[]), "0 entries");
    let mut many = table(&[good()]);
    many[..4].copy_from_slice(&0x101u32.to_be_bytes());
    rejected(many, "257 entries");
    // Entry range past the block.
    let mut truncated = table(&[good()]);
    truncated.truncate(truncated.len() - 1);
    rejected(truncated, "need");
    // Header length disagrees with the DMA size.
    rejected(
        table(&[entry([0, 189, 0, 0, 4], &INDEX, &VALUES, |e| e[0x17] ^= 2)]),
        "differs from DMA size",
    );
    // Negative part count.
    rejected(
        table(&[entry([0, 189, 0, 0, 4], &INDEX, &VALUES, |e| {
            e[0x0A] = 0xFF
        })]),
        "negative part count",
    );
    // Index running into the values.
    rejected(
        table(&[entry([0, 189, 0, 0, 4], &INDEX, &VALUES, |e| e[0x0B] = 5)]),
        "do not fit",
    );
    // An attribute reading past the values, and one with zero frames.
    let mut wide = INDEX;
    wide[2] = 7;
    rejected(
        table(&[entry([0, 189, 0, 0, 4], &wide, &VALUES, |_| {})]),
        "attribute 1",
    );
    let mut empty = INDEX;
    empty[0] = 0;
    rejected(
        table(&[entry([0, 189, 0, 0, 4], &empty, &VALUES, |_| {})]),
        "attribute 0",
    );
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn local_us_rom_mario_animations() {
    use rustario64::import::rom::Rom;
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
    assert_eq!(anims.animations.len(), version::MARIO_ANIMATION_COUNT);
    // From the pinned assets/anims/anim_00.inc.c header.
    let first = anims.get(0).unwrap();
    assert_eq!(
        (
            first.flags,
            first.y_trans_divisor,
            first.start_frame,
            first.loop_end
        ),
        (1, 189, 0, 0x22)
    );
    assert_eq!(first.bone_count, 20);
    // Canonical compact JSON (sorted object keys) of every header field and
    // array, reproduced from the pinned decomp sources by
    // tools/check_mario_anims_reference.py, which also rebuilds the N64 table
    // and matches it byte for byte with the ROM. See docs/ROM_VALIDATION.md.
    let mut value = serde_json::to_value(&anims).unwrap();
    value.sort_all_objects();
    assert_eq!(
        sha1_hex(&serde_json::to_vec(&value).unwrap()),
        "919843c7438f964a888830607c04e37e433fb99b"
    );
}
