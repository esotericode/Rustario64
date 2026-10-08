//! All bytes here are independently authored, never extracted from a ROM.
use rustario64::{
    content::{Act, ActMask, AreaId, CourseId, LevelId, visual::Background},
    diagnostics,
    import::{
        bob, collision, level, macros, mio0,
        reader::Reader,
        rom::{self, ByteOrder, Rom},
        segments::Segments,
        sha1_hex, texture, version,
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
fn macro_records_preserve_packed_yaw_signed_positions_params_and_order() {
    let input = words(&[
        0x8026u16 as i16,
        -123,
        321,
        -456,
        0xA5B6u16 as i16,
        396,
        111,
        222,
        333,
        0,
        30,
    ]);
    let (placements, consumed) = macros::decode(&input, 0x07001234).unwrap();
    assert_eq!(consumed, 22);
    assert_eq!(placements.len(), 2);
    assert_eq!(placements[0].source_address, 0x07001234);
    assert_eq!(placements[1].source_address, 0x0700123E);
    assert_eq!(placements[0].packed_preset_and_yaw, 0x8026);
    assert_eq!(placements[0].preset_id, 7);
    assert_eq!(placements[1].preset_id, 365);
    assert_eq!(placements[0].position, [-123, 321, -456]);
    assert_eq!(placements[0].yaw, i16::MIN);
    assert_eq!(placements[0].raw_params, 0xA5B6);
}

#[test]
fn macro_rotation_retains_every_quantized_angle_without_rounding() {
    for yaw in 0..128u16 {
        let packed = (yaw << 9) | 31;
        let input = words(&[packed as i16, 0, 0, 0, 0, 30]);
        let (placements, _) = macros::decode(&input, 0x07000000).unwrap();
        assert_eq!(placements[0].yaw, (yaw << 9) as i16);
    }
}

#[test]
fn macro_terminators_and_legacy_dispatch_follow_the_reference() {
    for terminator in [30, -1, 0x401E] {
        let input = words(&[terminator, 99, 99]);
        let (placements, consumed) = macros::decode(&input, 0x07000000).unwrap();
        assert!(placements.is_empty());
        assert_eq!(consumed, 2);
    }
    for first in 0..30 {
        let error = macros::decode(&words(&[first]), 0x07000000).unwrap_err();
        assert!(error.detail.contains("legacy"));
    }
    // A small preset word after a modern record ends that list, without dispatch.
    let (_, consumed) = macros::decode(&words(&[31, 1, 2, 3, 4, 0]), 0x07000000).unwrap();
    assert_eq!(consumed, 12);
}

#[test]
fn macro_decoder_rejects_truncation_invalid_presets_and_segment_wrap() {
    let input = words(&[31, 1, 2, 3, 4, 30]);
    for cut in 0..input.len() {
        assert!(
            macros::decode(&input[..cut], 0x07000000).is_err(),
            "cut {cut}"
        );
    }
    assert!(macros::decode(&words(&[397, 0, 0, 0, 0, 30]), 0x07000000).is_err());
    for address in [0, 0x20000000, 0x07000001, 0x07FFFFF8] {
        assert!(macros::decode(&input, address).is_err());
    }
    assert_eq!(macros::decode(&words(&[30]), 0x07FFFFFE).unwrap().1, 2);
}

#[test]
fn macro_decoder_bounds_record_count_but_accepts_termination_at_the_limit() {
    let record = words(&[31, 1, 2, 3, 4]);
    let mut input = record.repeat(macros::MAX_MACRO_OBJECTS);
    assert!(macros::decode(&input, 0x07000000).is_err());
    input.extend_from_slice(&words(&[30]));
    assert_eq!(
        macros::decode(&input, 0x07000000).unwrap().0.len(),
        macros::MAX_MACRO_OBJECTS
    );
    input.truncate(input.len() - 2);
    input.extend_from_slice(&record);
    input.extend_from_slice(&words(&[30]));
    assert!(
        macros::decode(&input, 0x07000000)
            .unwrap_err()
            .detail
            .contains("limit")
    );
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

fn records_sha1(records: &impl serde::Serialize) -> String {
    let mut value = serde_json::to_value(records).unwrap();
    value.sort_all_objects();
    sha1_hex(&serde_json::to_vec(&value).unwrap())
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
    let mut wrong_range = script;
    wrong_range[8..12].copy_from_slice(&((version::BOB_TERRAIN.start + 4) as u32).to_be_bytes());
    assert!(level::bob_entry(&wrong_range).is_err());
}

#[test]
fn area_metadata_keeps_original_defaults_or_updates_and_signed_music_words() {
    // Authored two-area script, deliberately exercising repeated commands.
    let decoded = extract(vec![
        0x1F, 8, 1, 0, 0x0E, 0, 0, 0, 0x30, 4, 1, 17, 0x30, 4, 1, 23, 0x30, 4, 2,
        99, // Original ignores out-of-range dialog slots.
        0x31, 4, 0, 2, 0x31, 4, 0x80, 1, 0x36, 8, 0, 7, 0, 9, 0, 0, 0x36, 8, 0xFF, 0xFE, 0x80, 0,
        0, 0, 0x20, 4, 0, 0, 0x1F, 8, 2, 0, 0x0E, 0, 0, 0, 0x20, 4, 0, 0, 4, 4, 0, 0,
    ])
    .unwrap();
    let first = &decoded.areas[0];
    assert_eq!(first.dialog_ids, [0xFF, 23]);
    assert_eq!(first.terrain_type, 0x8003);
    assert_eq!(first.background_music.settings_preset, -2);
    assert_eq!(first.background_music.sequence, i16::MIN);
    assert!(
        decoded.issues[0]
            .feature
            .contains("slot 2 ignored by the original")
    );
    let second = &decoded.areas[1];
    assert_eq!(second.dialog_ids, [0xFF; 2]);
    assert_eq!(second.terrain_type, 0);
    assert_eq!(second.background_music, Default::default());
    for command in [
        vec![0x30, 4, 0, 1],
        vec![0x31, 4, 0, 2],
        vec![0x36, 8, 0, 1, 0, 2, 0, 0],
    ] {
        assert!(
            extract(command)
                .unwrap_err()
                .detail
                .contains("outside AREA")
        );
    }
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
    assert_eq!(result.level.areas[0].macro_spawns.len(), 2);
    assert_eq!(result.level.areas[0].macro_spawns[0].preset_id, 7);
    assert!(
        result
            .level
            .issues
            .iter()
            .any(|i| i.feature.contains("respawn runtime"))
    );
    assert!(
        !result
            .level
            .issues
            .iter()
            .any(|i| i.feature.contains("macro objects") && i.feature.contains("not decoded"))
    );
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
fn bob_mapping_mismatch_and_truncated_terrain_are_rejected() {
    let (terrain, mut script) = diagnostics::bob_segments_fixture();
    let i = script
        .windows(4)
        .position(|b| b == version::BOB_COLLISION.to_be_bytes())
        .unwrap();
    script[i..i + 4].copy_from_slice(&0x07001000u32.to_be_bytes());
    assert!(bob::decode_segments(terrain, script).is_err());
    let (mut terrain, script) = diagnostics::bob_segments_fixture();
    terrain.truncate(
        (version::BOB_COLLISION & 0xFFFFFF) as usize + diagnostics::collision_fixture().len() - 1,
    );
    assert!(bob::decode_segments(terrain, script).is_err());
}

#[test]
fn bob_macro_import_fails_on_truncation_and_reports_unmapped_lists() {
    let (mut terrain, script) = diagnostics::bob_segments_fixture();
    terrain.pop();
    assert!(bob::decode_segments(terrain, script).is_err());
    let (terrain, mut script) = diagnostics::bob_segments_fixture();
    let at = script
        .windows(4)
        .position(|b| b == [0x39, 8, 0, 0])
        .unwrap();
    script[at + 4..at + 8].copy_from_slice(&0x0B000000u32.to_be_bytes());
    let decoded = bob::decode_segments(terrain, script).unwrap();
    assert!(decoded.level.areas[0].macro_spawns.is_empty());
    assert!(
        decoded
            .level
            .issues
            .iter()
            .any(|i| i.feature == "macro objects 0x0B000000; segment not imported")
    );
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn local_us_rom_import() {
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let imported = bob::import(&rom).unwrap();
    assert_eq!(imported.terrain_bytes, 71618);
    assert_eq!(imported.collision.vertices.len(), 570);
    assert_eq!(imported.collision.triangles.len(), 1060);
    assert_eq!(imported.collision.specials.len(), 17);
    assert!(imported.collision.environment.is_empty());
    assert_eq!(imported.textures.len(), 5);
    assert_eq!(imported.level.areas.len(), 1);
    let area = &imported.level.areas[0];
    assert_eq!(area.id, AreaId(1));
    assert_eq!(area.geometry_layout, 0x0E000488);
    assert_eq!(area.terrain, Some(version::BOB_COLLISION));
    assert_eq!(area.macro_objects, Some(0x0701104C));
    assert_eq!(area.spawns.len(), 30);
    assert_eq!(area.macro_spawns.len(), 88);
    assert_eq!(area.warps.len(), 7);
    // From the pinned BOB level script and seq_ids.h, not parser-generated output.
    assert_eq!(area.terrain_type, 0); // TERRAIN_GRASS
    assert_eq!(area.dialog_ids, [0, 0xFF]);
    assert_eq!(area.background_music.settings_preset, 0);
    assert_eq!(area.background_music.sequence, 3); // SEQ_LEVEL_GRASS
    assert_eq!(
        imported.level.mario_start,
        Some((AreaId(1), 135, [-6558, 0, 6464]))
    );
    // Canonical compact JSON digests independently computed from expanded macros
    // by tools/check_bob_reference.py. Object keys are sorted; array order stays
    // authoritative. A struct-field reorder does not alter content expectations.
    // See docs/ROM_VALIDATION.md; these are asset checks, not gameplay comparisons.
    assert_eq!(
        records_sha1(&imported.collision),
        "dfe37da1b39dada6ebf6c31cab9af3ca16c6e269"
    );
    assert_eq!(
        records_sha1(&area.macro_spawns),
        "ea3910a778a11c1f1bdd5c6d6e23af177b93a7c8"
    );
    // Dependent segments come from ROM ranges named by the level script itself;
    // these match the pinned sm64tools US configuration's block boundaries.
    let ranges: Vec<_> = imported
        .loaded_segments
        .iter()
        .map(|s| (s.segment, s.rom_start, s.rom_end, s.mio0))
        .collect();
    assert_eq!(
        ranges,
        [
            (0x09, 0x32D070, 0x334B30, true),
            (0x0A, 0x2AC6B0, 0x2B8F10, true),
            (0x05, 0x134D20, 0x13B5D0, true),
            (0x0C, 0x13B5D0, 0x13B910, false),
            (0x06, 0x1C4230, 0x1D7C90, true),
            (0x0D, 0x1D7C90, 0x1D8310, false),
            (0x08, 0x1F2200, 0x2008D0, true),
            (0x0F, 0x2008D0, 0x201410, false),
        ]
    );
    let visual = imported.visual.as_ref().unwrap();
    assert_eq!(visual.model.triangle_count(), 1101);
    assert_eq!(visual.model.batches.len(), 24);
    assert_eq!(visual.background, Some(Background::Skybox(0)));
    // Every drawn triangle as [x, y, z, r, g, b, a] per vertex, in draw order.
    // Digest from an independent expansion of the pinned decompilation's BOB
    // display-list source in geo order; see docs/ROM_VALIDATION.md.
    let stream: Vec<Vec<Vec<i64>>> = visual
        .model
        .batches
        .iter()
        .flat_map(|b| b.vertices.as_chunks::<3>().0.iter())
        .map(|tri| {
            tri.iter()
                .map(|v| {
                    let mut row: Vec<i64> = v.position.iter().map(|&p| p as i64).collect();
                    row.extend(v.color.iter().map(|&c| i64::from(c)));
                    row
                })
                .collect()
        })
        .collect();
    assert_eq!(
        sha1_hex(&serde_json::to_vec(&stream).unwrap()),
        "f6ac0b00e30b5bb0583fb4f3dfc1670f411b3f74"
    );
    // RGBA8 of all 18 bound textures in source-address order; matches PNGs made by
    // the pinned decompilation's own mio0/n64graphics extraction tools.
    let mut textures: Vec<_> = visual.model.textures.iter().collect();
    textures.sort_by_key(|t| t.source);
    let texture_bytes: Vec<u8> = textures
        .iter()
        .flat_map(|t| t.rgba.iter().copied())
        .collect();
    assert_eq!(textures.len(), 18);
    assert_eq!(
        sha1_hex(&texture_bytes),
        "cc0c962ef7fa0a8ae9f2d6ec1f7fb0ec8ee1cf14"
    );
    let models: Vec<_> = imported
        .models
        .iter()
        .map(|m| (m.model, m.geometry_layout, m.visual.triangle_count()))
        .collect();
    assert_eq!(
        models,
        [
            (54, 0x0E000440, 2),
            (55, 0x0E000458, 12),
            (56, 0x0E000470, 3)
        ]
    );
}
