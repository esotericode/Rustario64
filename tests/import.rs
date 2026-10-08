//! All bytes here are independently authored, never extracted from a ROM.
use rustario64::{
    content::{Act, ActMask, AreaId, CourseId, LevelId},
    diagnostics,
    import::{
        bob, collision, level, mio0,
        reader::Reader,
        rom::{self, ByteOrder, Rom},
        segments::Segments,
        texture, version,
    },
};

fn words(values: &[i16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}
fn packed(size: u32, mask: &[u8], tokens: &[u8], raw: &[u8]) -> Vec<u8> {
    let mut out = b"MIO0".to_vec();
    for n in [
        size,
        (16 + mask.len()) as u32,
        (16 + mask.len() + tokens.len()) as u32,
    ] {
        out.extend_from_slice(&n.to_be_bytes());
    }
    out.extend_from_slice(mask);
    out.extend_from_slice(tokens);
    out.extend_from_slice(raw);
    out
}

#[test]
fn reader_handles_signed_big_endian_and_boundaries() {
    let r = Reader::new(&[0xFF, 0x80, 0x12, 0x34], "fixture");
    assert_eq!(r.i16(0).unwrap(), -128);
    assert_eq!(r.u32(0).unwrap(), 0xFF801234);
    assert!(r.slice(4, 0).unwrap().is_empty());
    assert!(r.u16(3).is_err());
    assert!(r.slice(usize::MAX, 2).is_err());
}

#[test]
fn byte_orders_normalize_to_identical_bytes() {
    let z64 = vec![0x80, 0x37, 0x12, 0x40, 1, 2, 3, 4, 5, 6, 7, 8];
    assert_eq!(
        rom::normalize(z64.clone()).unwrap(),
        (z64.clone(), ByteOrder::Z64)
    );
    for (width, order) in [(2, ByteOrder::V64), (4, ByteOrder::N64)] {
        let mut swapped = z64.clone();
        for chunk in swapped.chunks_exact_mut(width) {
            chunk.reverse();
        }
        assert_eq!(rom::normalize(swapped).unwrap(), (z64.clone(), order));
    }
}

#[test]
fn byte_order_detection_rejects_unknown_and_partial_groups() {
    assert!(rom::normalize(vec![0, 0, 0, 0]).is_err());
    assert!(rom::normalize(vec![0x37, 0x80, 0x40, 0x12, 0]).is_err());
    assert!(rom::normalize(vec![0x40, 0x12, 0x37, 0x80, 0, 0]).is_err());
    assert!(rom::normalize(vec![0x80]).is_err());
}

#[test]
fn revision_requires_full_fingerprint_despite_a_matching_header() {
    let mut forged = vec![0; version::ROM_LEN];
    forged[..4].copy_from_slice(&0x80371240u32.to_be_bytes());
    forged[0x10..0x14].copy_from_slice(&0x635A2BFFu32.to_be_bytes());
    forged[0x14..0x18].copy_from_slice(&0x8B022326u32.to_be_bytes());
    assert!(
        Rom::from_bytes(forged)
            .err()
            .unwrap()
            .detail
            .contains("unsupported normalized SHA-1")
    );
    assert!(Rom::from_bytes(vec![0; version::ROM_LEN - 1]).is_err());
    assert!(Rom::from_bytes(vec![0; version::ROM_LEN + 1]).is_err());
}

#[test]
fn mio0_literal_stream_crosses_mask_boundaries() {
    let bytes: Vec<_> = (0..35).collect();
    assert_eq!(
        mio0::decode(&diagnostics::literal_mio0(&bytes), 35).unwrap(),
        bytes
    );
    assert!(
        mio0::decode(&packed(0, &[], &[], &[]), 0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn mio0_copies_overlapping_and_nonoverlapping_references() {
    assert_eq!(
        mio0::decode(&packed(6, &[0x80], &[0x20, 0], b"A"), 6).unwrap(),
        b"AAAAAA"
    );
    assert_eq!(
        mio0::decode(&packed(9, &[0xE0], &[0x30, 2], b"ABC"), 9).unwrap(),
        b"ABCABCABC"
    );
}

#[test]
fn mio0_rejects_every_truncated_literal_fixture() {
    let input = diagnostics::literal_mio0(b"a fixture crossing three mask bytes");
    for cut in 0..input.len() {
        assert!(mio0::decode(&input[..cut], 1024).is_err(), "cut {cut}");
    }
}

#[test]
fn mio0_rejects_unsafe_or_oversized_streams() {
    assert!(mio0::decode(&packed(1, &[0], &[0, 0], &[]), 1).is_err());
    assert!(mio0::decode(&packed(2, &[0x80], &[0x00, 0], b"A"), 2).is_err());
    assert!(mio0::decode(&packed(6, &[0x80], &[0x20], b"A"), 6).is_err());
    assert!(mio0::decode(&packed(1, &[], &[], b"A"), 1).is_err());
    assert!(mio0::decode(&packed(2, &[0xFF], &[], b"AB"), 1).is_err());
    let mut input = packed(1, &[0xFF], &[], b"A");
    input[8..12].copy_from_slice(&15u32.to_be_bytes());
    assert!(mio0::decode(&input, 1).is_err());
    input[8..12].copy_from_slice(&99u32.to_be_bytes());
    assert!(mio0::decode(&input, 1).is_err());
}

#[test]
fn segmented_reads_reject_bad_ids_duplicates_offsets_and_ranges() {
    let mut segments = Segments::default();
    assert!(segments.insert(0, vec![]).is_err());
    assert!(segments.insert(32, vec![]).is_err());
    segments.insert(7, vec![1, 2, 3]).unwrap();
    assert!(segments.insert(7, vec![]).is_err());
    assert_eq!(segments.read(0x07000001, 2).unwrap(), &[2, 3]);
    assert!(segments.read(0x08000000, 1).is_err());
    assert!(segments.read(0x07000002, 2).is_err());
    assert!(segments.tail(0x07FFFFFF).is_err());
}

#[test]
fn collision_preserves_signed_vertices_surface_order_specials_and_regions() {
    let input = diagnostics::collision_fixture();
    let (mesh, consumed) = collision::decode(&input).unwrap();
    assert_eq!(consumed, input.len());
    assert_eq!(
        mesh.vertices,
        [[-100, 0, -100], [100, 0, -100], [0, 0, 100]]
    );
    assert_eq!(mesh.triangles[0].indices, [0, 2, 1]);
    assert_eq!(mesh.triangles[0].surface, 0);
    assert_eq!(mesh.specials[0].position, [12, 34, 56]);
    assert_eq!(mesh.environment[0], [0, -50, -50, 50, 50, -10]);
    assert!(collision::to_obj(&mesh).contains("f 1 3 2"));
}

#[test]
fn collision_force_width_includes_the_unused_surface_0004() {
    for surface in [0x04, 0x0E, 0x24, 0x25, 0x27, 0x2C, 0x2D] {
        let input = words(&[0x40, 1, -3, 4, 5, surface, 1, 0, 0, 0, -32768, 0x41, 0x42]);
        let (mesh, consumed) = collision::decode(&input).unwrap();
        assert_eq!(mesh.triangles[0].force, Some(-32768));
        assert_eq!(consumed, input.len());
    }
}

#[test]
fn collision_appends_vertex_groups_with_rebased_indices() {
    let input = words(&[
        0x40, 1, 1, 2, 3, 0, 1, 0, 0, 0, 0x40, 1, 4, 5, 6, 0x65, 1, 0, 0, 0, 0x41, 0x42,
    ]);
    let (mesh, _) = collision::decode(&input).unwrap();
    assert_eq!(mesh.triangles[1].indices, [1; 3]);
    assert_eq!(mesh.triangles[1].surface, 0x65);
}

#[test]
fn collision_handles_each_special_record_width() {
    for (preset, extra) in [(1, 0), (0x65, 1), (0x83, 2), (30, 3), (0x8A, 1)] {
        let mut values = vec![0x40, 0, 0x41, 0x43, 1, preset, -1, -2, -3];
        values.extend(std::iter::repeat_n(17, extra));
        values.push(0x42);
        let (mesh, consumed) = collision::decode(&words(&values)).unwrap();
        assert_eq!(mesh.specials[0].extra.len(), extra);
        assert_eq!(consumed, values.len() * 2);
    }
}

#[test]
fn collision_rejects_negative_counts_invalid_indices_commands_and_presets() {
    for values in [
        vec![0x40, -1],
        vec![0x40, 0, 0, 1, 0, 0, 0],
        vec![0x40, 0, 0x50],
        vec![0x40, 0, 0x43, 1, 0x55, 0, 0, 0, 0x42],
        vec![0x40, 0, 0x41, 0, 0, 0x42],
        vec![0x40, 0, 0, -1],
    ] {
        assert!(collision::decode(&words(&values)).is_err(), "{values:?}");
    }
    let input = diagnostics::collision_fixture();
    for cut in 0..input.len() {
        assert!(collision::decode(&input[..cut]).is_err(), "cut {cut}");
    }
}

#[test]
fn texture_rgba5551_expands_channels_and_retains_alpha() {
    let texture = texture::rgba16(&[0xF8, 1, 0x07, 0xC0, 0, 0x3F], 3, 1).unwrap();
    assert_eq!(texture.rgba, [255, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 255]);
    assert!(texture.to_ppm().starts_with(b"P6\n3 1\n255\n"));
    assert!(texture::rgba16(&[0], 1, 1).is_err());
    assert!(texture::rgba16(&[], 0, 1).is_err());
    assert!(texture::rgba16(&[], 65535, 65535).is_err());
}

#[test]
fn act_mask_preserves_original_all_acts_sentinel() {
    assert!(Act::new(0).is_none());
    assert!(Act::new(7).is_none());
    assert!(ActMask(0x1F).includes(Act::new(6).unwrap()));
    assert!(ActMask(0x20).includes(Act::new(6).unwrap()));
    assert!(!ActMask(0x20).includes(Act::new(1).unwrap()));
    assert!(!ActMask(0).includes(Act::new(1).unwrap()));
}

fn extract(script: Vec<u8>) -> rustario64::import::Result<rustario64::content::ImportedLevel> {
    let mut segments = Segments::default();
    segments.insert(0x0E, script)?;
    level::extract(&segments, 0x0E000000, CourseId(1), LevelId(9))
}

#[test]
fn level_rejects_bad_lengths_opcodes_truncation_and_unbalanced_calls() {
    for script in [
        vec![0x1B, 0, 0, 0],
        vec![0x1F, 8, 1, 0],
        vec![0xFF, 4, 0, 0],
        vec![7, 4, 0, 0],
        vec![0x20, 4, 0, 0],
        vec![0x1F, 8, 1, 0, 0, 0, 0, 0, 4, 4, 0, 0],
    ] {
        assert!(extract(script).is_err());
    }
}

#[test]
fn level_cycles_and_recursive_calls_are_bounded() {
    let cycle = vec![5, 8, 0, 0, 0x0E, 0, 0, 0];
    assert!(extract(cycle).err().unwrap().detail.contains("budget"));
    let recurse = vec![6, 8, 0, 0, 0x0E, 0, 0, 0];
    assert!(extract(recurse).err().unwrap().detail.contains("depth"));
}

#[test]
fn unresolved_global_calls_are_visible_and_bad_local_pointers_fail() {
    let global = vec![6, 8, 0, 0, 0x15, 0, 0, 0, 4, 4, 0, 0];
    let decoded = extract(global).unwrap();
    assert!(decoded.issues[0].feature.contains("0x15000000"));
    let local = vec![6, 8, 0, 0, 0x0E, 0, 0x10, 0, 4, 4, 0, 0];
    assert!(extract(local).is_err());
}

#[test]
fn bob_entry_requires_unique_aligned_reference_load_pair() {
    let (_, script) = diagnostics::bob_segments_fixture();
    assert_eq!(level::bob_entry(&script).unwrap(), 0x0E000000);
    assert!(level::bob_entry(&script[1..]).is_err());
    let mut duplicated = script.clone();
    duplicated.extend_from_slice(&script);
    assert!(level::bob_entry(&duplicated).is_err());
}

#[test]
fn bob_static_import_fixture_traverses_local_calls_and_retains_content() {
    let result = diagnostics::parser_smoke().unwrap();
    assert_eq!(result.level.course, CourseId(1));
    assert_eq!(result.level.level, LevelId(9));
    assert_eq!(result.level.areas[0].id, AreaId(1));
    assert_eq!(result.level.areas[0].spawns[0].position, [-7, 23, -45]);
    assert_eq!(result.level.areas[0].spawns[1].position, [8, 23, -45]);
    assert_eq!(result.level.areas[0].spawns[0].behavior_params, 0x12345678);
    assert_eq!(
        result.level.areas[0].spawns[0].angles_degrees,
        [0, 90, -180]
    );
    assert_eq!(
        result.level.mario_start,
        Some((AreaId(1), 90, [11, 22, 33]))
    );
    assert_eq!(result.level.areas[0].warps[0].destination_node, 10);
    assert_eq!(result.textures[0].rgba[..4], [0, 255, 0, 255]);
    assert_eq!(result.collision.triangles.len(), 1);
    assert!(
        result
            .level
            .issues
            .iter()
            .any(|i| i.feature.contains("native callback"))
    );
    assert!(
        result
            .level
            .issues
            .iter()
            .any(|i| i.feature.contains("unimplemented behavior"))
    );
}

#[test]
fn bob_mapping_mismatch_and_truncated_collision_are_rejected() {
    let (terrain, mut script) = diagnostics::bob_segments_fixture();
    let i = script
        .windows(4)
        .position(|b| b == version::BOB_COLLISION.to_be_bytes())
        .unwrap();
    script[i..i + 4].copy_from_slice(&0x07001000u32.to_be_bytes());
    assert!(bob::decode_segments(terrain, script).is_err());
    let (mut terrain, script) = diagnostics::bob_segments_fixture();
    terrain.pop();
    assert!(bob::decode_segments(terrain, script).is_err());
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn local_us_rom_import() {
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let imported = bob::import(&rom).unwrap();
    assert!(imported.collision.vertices.len() > 100);
    assert!(imported.collision.triangles.len() > 100);
    assert_eq!(imported.textures.len(), 5);
    assert_eq!(imported.level.areas.len(), 1);
    assert!(imported.level.areas[0].spawns.len() > 10);
    assert!(imported.level.areas[0].warps.len() >= 7);
    assert_eq!(
        imported.level.mario_start,
        Some((AreaId(1), 135, [-6558, 0, 6464]))
    );
    // Smoke coverage only: counts and content still need independent ROM-owner validation.
}
