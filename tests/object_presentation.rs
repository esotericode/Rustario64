//! Authored geo/display-list fixtures; no game assets.
use rustario64::{
    import::{
        geo,
        objects::{ModelRegistration, ObjectContent},
        segments::Segments,
    },
    presentation::objects::{BillboardBasis, ObjectDrawer},
    simulation::{
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
        object::{
            ObjectId, ObjectList, ObjectPool, render::VisibleObject, script::authored_scripts,
        },
    },
};
use std::collections::BTreeMap;

fn tables() -> TrigTables {
    TrigTables::new(
        (0..SINE_ENTRIES)
            .map(|i| (i as f32 * std::f32::consts::TAU / 4096.0).sin())
            .collect(),
        vec![0; ARCTAN_ENTRIES],
    )
    .unwrap()
}

fn content() -> ObjectContent {
    let mut segment = vec![0; 0x80];
    for (i, p) in [[0i16, 0, 0], [10, 0, 0], [0, 10, 0]].iter().enumerate() {
        for (axis, v) in p.iter().enumerate() {
            segment[i * 16 + axis * 2..i * 16 + axis * 2 + 2].copy_from_slice(&v.to_be_bytes());
        }
        segment[i * 16 + 12..i * 16 + 16].copy_from_slice(&[255; 4]);
    }
    let words = |v: &[u32]| v.iter().flat_map(|w| w.to_be_bytes()).collect::<Vec<_>>();
    // Shade, no lighting; load three vertices, draw one triangle, end.
    segment.extend(words(&[
        0xFCFF_FFFF,
        0xFFFE_793C,
        0xB600_0000,
        0x0002_0000,
        0x0420_0030,
        0x0700_0000,
        0xBF00_0000,
        0x0000_0A14,
        0xB800_0000,
        0,
    ]));
    let mut segments = Segments::default();
    segments.insert(7, segment).unwrap();
    // Two switch children: display list translated by 0 or 100 on X.
    segments
        .insert(
            14,
            words(&[
                0x0E00_0002,
                0,
                0x0400_0000,
                0x1184_0000,
                0,
                0x0700_0080,
                0x1184_0064,
                0,
                0x0700_0080,
                0x0500_0000,
                0x0100_0000,
            ]),
        )
        .unwrap();
    ObjectContent {
        scripts: authored_scripts(),
        presets: vec![],
        segments,
        loads: vec![],
        main_models: BTreeMap::from([(
            20,
            ModelRegistration {
                address: 0,
                pointer: 0x0E00_0000,
                geometry_layout: true,
                layer: None,
            },
        )]),
    }
}

fn object(case: usize, generation: u64, pos: [f32; 3]) -> VisibleObject {
    VisibleObject {
        id: ObjectId(3),
        generation,
        behavior: 123,
        model: 20,
        pos,
        angle: [0; 3],
        scale: [1.0; 3],
        billboard: true,
        cases: vec![(0, case)],
    }
}

const BASIS: BillboardBasis = BillboardBasis {
    right: [1.0, 0.0, 0.0],
    up: [0.0, 1.0, 0.0],
    toward: [0.0, 0.0, 1.0],
};

#[test]
fn selected_mesh_switches_keep_interpolating_and_billboards_follow_displayed_camera() {
    let content = content();
    let trig = tables();
    let layout = geo::decode(&content.segments, 0x0E00_0000).unwrap();
    assert_eq!(layout.nodes[0].children, [1, 2]);
    let mut drawer = ObjectDrawer::new(&content, &trig);
    drawer.update(vec![object(1, 1, [0.0; 3])]).unwrap();
    drawer.update(vec![object(2, 1, [20.0, 0.0, 0.0])]).unwrap();
    let frame = drawer.frame(0.5, true, BASIS);
    assert_eq!(frame[0].vertices[0][0].position, [110.0, 0.0, 0.0]);
    assert_eq!(frame[0].vertices[0][1].position, [120.0, 0.0, 0.0]);
    assert_eq!(
        drawer.frame(0.0, true, BASIS)[0].vertices[0][0].position,
        [100.0, 0.0, 0.0]
    );
    assert_eq!(
        drawer.frame(0.0, false, BASIS)[0].vertices[0][0].position,
        [120.0, 0.0, 0.0]
    );
    let side = BillboardBasis {
        right: [0.0, 0.0, -1.0],
        up: [0.0, 1.0, 0.0],
        toward: [1.0, 0.0, 0.0],
    };
    assert_eq!(
        drawer.frame(0.5, true, side)[0].vertices[0][1].position,
        [10.0, 0.0, -110.0]
    );
    assert!(frame[0].template.batches[0].material.depth_test);
    assert_eq!(drawer.builds(), 2);
}

#[test]
fn despawns_reused_slots_and_presentation_resets_snap_and_instances_share_builds() {
    let content = content();
    let trig = tables();
    let mut drawer = ObjectDrawer::new(&content, &trig);
    drawer.update(vec![object(1, 1, [0.0; 3])]).unwrap();
    // Same behavior and pool index, different allocation: never interpolate.
    drawer
        .update(vec![object(1, 2, [1000.0, 0.0, 0.0])])
        .unwrap();
    assert_eq!(
        drawer.frame(0.0, true, BASIS)[0].vertices[0][0].position,
        [1000.0, 0.0, 0.0]
    );
    drawer.update(vec![]).unwrap();
    assert!(drawer.frame(0.0, true, BASIS).is_empty());
    let a = object(1, 2, [2000.0, 0.0, 0.0]);
    let b = VisibleObject {
        id: ObjectId(4),
        ..a.clone()
    };
    drawer.update(vec![a.clone(), b]).unwrap();
    assert_eq!(drawer.frame(0.0, true, BASIS)[0].vertices[0].len(), 6);
    drawer
        .update(vec![object(1, 2, [2100.0, 0.0, 0.0])])
        .unwrap();
    drawer.snap();
    assert_eq!(
        drawer.frame(0.0, true, BASIS)[0].vertices[0][0].position,
        [2100.0, 0.0, 0.0]
    );
    drawer.reset();
    assert!(drawer.frame(0.5, true, BASIS).is_empty());
    assert_eq!(drawer.builds(), 1);
    let mut pool = ObjectPool::new();
    let id = pool.allocate_object(ObjectList::Default, 9);
    let before = pool.generation(id);
    pool.unload_object(id);
    let reused = pool.allocate_object(ObjectList::Default, 9);
    assert_eq!(id, reused);
    assert_eq!(pool.generation(id), before + 1);
}
