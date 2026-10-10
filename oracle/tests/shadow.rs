//! Exact original Vtx coordinates, texture coordinates, alpha and layer.
//! CI fixtures are authored. The ignored BOB check imports private ROM data.
use rustario64::{
    content::animation::{Animation, MarioAnimations},
    import::{animation, bob, collision, engine, mario, mio0, rom::Rom, shadow, version},
    play::{Pad, Session},
    presentation::{
        mario::{AnimationPose, MarioPose, shadow_origin},
        shadow::{ShadowDrawer, player_shadow},
    },
    simulation::{
        collision::CollisionWorld,
        game::GameEntry,
        mario::{MarioBodyState, constants::*},
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
    },
};
use rustario64_oracle::Oracle;

fn tables() -> TrigTables {
    // Analytically generated host tables, never copied from game data.
    TrigTables::new(
        (0..SINE_ENTRIES)
            .map(|i| (i as f32 * std::f32::consts::TAU / 4096.0).sin())
            .collect(),
        (0..ARCTAN_ENTRIES)
            .map(|i| ((i as f64 / 1024.0).atan() * 32768.0 / std::f64::consts::PI) as u16)
            .collect(),
    )
    .unwrap()
}

fn stream(kind: i16, slope: i16, water: bool) -> Vec<i16> {
    let mut s = vec![0x40, 4];
    for v in [
        [-1000, -slope, -1000],
        [-1000, -slope, 1000],
        [1000, slope, -1000],
        [1000, slope, 1000],
    ] {
        s.extend(v);
    }
    s.extend([kind, 2, 0, 1, 2, 2, 1, 3, 0x41]);
    if water {
        s.extend([0x44, 1, 0, -900, -900, 900, 900, 300]);
    }
    s.push(0x42);
    s
}

fn world(stream: &[i16]) -> CollisionWorld {
    let bytes: Vec<u8> = stream.iter().flat_map(|v| v.to_be_bytes()).collect();
    CollisionWorld::load_area_terrain(&collision::decode(&bytes).unwrap().0).unwrap()
}

#[test]
fn original_shadow_vertices_match_on_slopes_edges_water_ice_and_ledge_frames() {
    let trig = tables();
    let mut count = 0;
    for kind in [SURFACE_DEFAULT, SURFACE_ICE] {
        for slope in [0, 200, -500, 1500] {
            for water in [false, true] {
                let stream = stream(kind, slope, water);
                let world = world(&stream);
                let oracle = Oracle::load(&stream);
                oracle.set_trig(trig.sine_table(), trig.arctan_table());
                for (x, z) in [
                    (0.25, -0.75),
                    (930.5, 50.0),
                    (999.0, 999.0),
                    (-999.0, -500.0),
                    (1100.0, 0.0),
                ] {
                    for height in [-100.5, 0.0, 70.0, 299.9, 300.0, 600.0, 1200.0, 3000.0] {
                        for anim in [
                            MARIO_ANIM_RUNNING,
                            MARIO_ANIM_IDLE_ON_LEDGE,
                            MARIO_ANIM_FAST_LEDGE_GRAB,
                            MARIO_ANIM_SLOW_LEDGE_GRAB,
                            MARIO_ANIM_CLIMB_DOWN_LEDGE,
                        ] {
                            for frame in [0, 4, 5, 6, 13, 14, 15, 20, 21, 32, 33, 34] {
                                let origin = [x, height, z];
                                let rust = player_shadow(
                                    &world,
                                    &trig,
                                    origin,
                                    100,
                                    150,
                                    anim as i16,
                                    frame,
                                );
                                let native =
                                    oracle.player_shadow(origin, 100, 150, anim as i16, frame);
                                assert_eq!(
                                    rust, native,
                                    "kind={kind} slope={slope} water={water} p={origin:?} anim={anim} frame={frame}"
                                );
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 38400);
    let stream = stream(SURFACE_DEFAULT, 0, false);
    let world = world(&stream);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    for solidity in [90, 120, 121, 180, 255] {
        for scale in [64, 100, 327] {
            for height in [0.0, 0.25, 599.999, 600.0, 1200.0] {
                let origin = [0.25, height, -0.75];
                assert_eq!(
                    player_shadow(
                        &world,
                        &trig,
                        origin,
                        scale,
                        solidity,
                        MARIO_ANIM_RUNNING as i16,
                        0
                    ),
                    oracle.player_shadow(origin, scale, solidity, MARIO_ANIM_RUNNING as i16, 0)
                );
            }
        }
    }
}

fn pose(frame: i16) -> MarioPose {
    MarioPose {
        visible: true,
        position: [3.25, 200.0, -2.5],
        throw_matrix: None,
        angle: [0, 0, 0],
        scale: [1.0; 3],
        animation: Some(AnimationPose {
            entry: 0,
            frame,
            y_trans: 189,
        }),
        body: MarioBodyState::default(),
        area_update_counter: 0,
        camera_c_up: false,
        head_rotation: [0; 3],
    }
}

#[test]
fn lateral_animation_origin_matches_the_original_geo_callback() {
    let trig = tables();
    let stream = stream(0, 0, false);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut animation = Animation {
        flags: 0,
        y_trans_divisor: 256,
        start_frame: 0,
        loop_start: 0,
        loop_end: 3,
        bone_count: 0,
        index: vec![3, 0, 1, 3, 2, 4],
        values: vec![-150, 13, 377, 99, 250, -33],
    };
    for flags in 0..128 {
        animation.flags = flags;
        let anims = MarioAnimations {
            animations: vec![animation.clone()],
        };
        for frame in [0, 1, 2, 7] {
            for yaw in [0, 0x4000, -0x4000, i16::MIN, 12345] {
                for child_scale in [0.25, 1.0, 1.7] {
                    let mut p = pose(frame);
                    p.angle[1] = yaw;
                    let rust = shadow_origin(&p, &anims, &trig, child_scale);
                    let native = oracle.shadow_origin(&p, &animation, child_scale);
                    assert_eq!(
                        rust.map(f32::to_bits),
                        native.map(f32::to_bits),
                        "flags={flags} frame={frame} yaw={yaw}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn bob_shadows_match_native_vertices_and_leave_played_state_unchanged() {
    let rom = Rom::open(std::path::Path::new(
        &std::env::var_os("RUSTARIO64_ROM").unwrap(),
    ))
    .unwrap();
    let imported = bob::import(&rom).unwrap();
    let visual = imported.visual.as_ref().unwrap();
    let entry = GameEntry::script_start(&imported.level, &visual.camera.unwrap()).unwrap();
    let world = CollisionWorld::load_area_terrain(&imported.collision).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
    let source = shadow::import(&rom, &mario::import(&rom).unwrap()).unwrap();
    assert_eq!(
        (source.scale, source.solidity, source.child_scale),
        (100, 180, 0.25)
    );
    assert_eq!(
        (
            source.texture.width,
            source.texture.height,
            source.texture.rgba.len()
        ),
        (16, 16, 1024)
    );
    // Independently reproduced by tools/check_shadow_reference.py from the
    // pinned assets.json metadata and its own MIO0/IA8 decode.
    assert_eq!(
        rustario64::import::sha1_hex(&source.texture.rgba),
        "8bf7a3d2b651a090b0aadeff3979213f5b782875"
    );
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        rom.reader().slice(range.start, range.len()).unwrap(),
        version::MAX_SEGMENT_BYTES,
    )
    .unwrap();
    let start = (version::BOB_COLLISION & 0xFFFFFF) as usize;
    let (_, len) = collision::decode(&terrain[start..]).unwrap();
    let words: Vec<i16> = terrain[start..start + len]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| i16::from_be_bytes([v[0], v[1]]))
        .collect();
    let oracle = Oracle::load(&words);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut compared = 0;
    for x in (-7800..=7800).step_by(379) {
        for z in (-7800..=7800).step_by(397) {
            for y in [0.0, 500.0, 1200.0, 2700.0, 4600.0] {
                let p = [x as f32 + 0.25, y, z as f32 - 0.75];
                assert_eq!(
                    player_shadow(&world, &trig, p, 100, 150, MARIO_ANIM_RUNNING as i16, 0),
                    oracle.player_shadow(p, 100, 150, MARIO_ANIM_RUNNING as i16, 0),
                    "BOB {p:?}"
                );
                compared += 1;
            }
        }
    }
    let content = rustario64::import::objects::bob(
        &rom,
        &imported.level,
        rustario64::content::Act::new(1).unwrap(),
    )
    .unwrap();
    let mut session = Session::new(&world, &trig, &anims, content.level_objects(), entry);
    let mut baseline = Session::new(&world, &trig, &anims, content.level_objects(), entry);
    let mut drawer = ShadowDrawer::new(&source);
    let mut shown = 0;
    for tick in 0..360 {
        let pad = Pad {
            up: tick % 180 < 120,
            left: tick % 180 >= 120,
            a: tick % 60 < 3,
            ..Pad::default()
        };
        assert!(session.step(&pad));
        assert!(baseline.step(&pad));
        let pose = session.mario_pose();
        let anim = &session.mario().obj.gfx.anim;
        let p = shadow_origin(&pose, &anims, &trig, source.child_scale);
        if let Some(a) = pose.animation {
            assert_eq!(
                p.map(f32::to_bits),
                oracle
                    .shadow_origin(&pose, anims.get(a.entry).unwrap(), source.child_scale)
                    .map(f32::to_bits)
            );
        }
        let shadow = player_shadow(
            &world,
            &trig,
            p,
            source.scale,
            source.solidity,
            anim.anim_id,
            anim.anim_frame,
        );
        assert_eq!(
            shadow,
            oracle.player_shadow(
                p,
                source.scale,
                source.solidity,
                anim.anim_id,
                anim.anim_frame
            )
        );
        if shadow.is_some() {
            shown += 1;
        }
        drawer.update(shadow);
        for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let _ = drawer.frame(alpha, true);
        }
        assert_eq!(session.mario(), baseline.mario());
        assert_eq!(
            rustario64_oracle::camera_trace::capture_game(session.game()),
            rustario64_oracle::camera_trace::capture_game(baseline.game())
        );
    }
    assert!(shown > 300);
    eprintln!("Compared {compared} BOB grid shadows and 360 played shadows ({shown} present)");
}
