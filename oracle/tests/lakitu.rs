//! Authored mode goals drive verbatim native update_lakitu. This validates the
//! persistent stage, NOT the mode dispatcher or original camera-relative play.
use rustario64::simulation::{
    FixedClock,
    camera::lakitu::{Camera, Lakitu, Rig, STATE_FIELD_NAMES, Transition, Unsupported},
    collision::CollisionFlags,
    mario::constants as c,
    math::TrigTables,
};
use rustario64_oracle::camera::CameraOracle;
use std::time::Duration;
#[path = "support/camera.rs"]
mod fixtures;
use fixtures::{Rng, tables, terrain, world};

fn compare(rig: &Rig, native: &[u32], context: &str) {
    for ((name, rust), native) in STATE_FIELD_NAMES.iter().zip(rig.state_words()).zip(native) {
        assert_eq!(rust, *native, "{context}: {name}");
    }
}
fn flags_words(flags: CollisionFlags) -> [u32; 2] {
    [
        flags.checking_for_camera.into(),
        flags.find_floor_include_surface_intangible.into(),
    ]
}
fn set_goal(rig: &mut Rig, oracle: &CameraOracle, mario: [f32; 3], action: u32, goal: Camera) {
    rig.camera = Camera {
        yaw: rig.camera.yaw,
        ..goal
    };
    oracle.lakitu_goal(mario, action, goal);
}
fn seed() -> Rig {
    let pos = [700.0, 900.0, 700.0];
    let focus = [0.0, 125.0, 0.0];
    Rig {
        camera: Camera {
            mode: c::CAMERA_MODE_RADIAL as u8,
            def_mode: c::CAMERA_MODE_RADIAL as u8,
            pos,
            focus,
            ..Camera::default()
        },
        lakitu: Lakitu {
            cur_pos: pos,
            cur_focus: focus,
            goal_pos: pos,
            goal_focus: focus,
            pos,
            focus,
            foc_h_speed: 0.8,
            foc_v_speed: 0.3,
            pos_h_speed: 0.3,
            pos_v_speed: 0.3,
            ..Lakitu::default()
        },
        transition: Transition {
            mario_pos: [0.0; 3],
            ..Transition::default()
        },
        status: c::CAM_FLAG_SMOOTH_MOVEMENT,
        yaw_speed: 0x400,
        old_pos: pos,
        old_focus: focus,
        ..Rig::default()
    }
}

fn random_cases(stream: &[i16], trig: &TrigTables, cases: usize) {
    let world = world(stream);
    let oracle = CameraOracle::new(stream, trig);
    let mut rng = Rng(65041);
    for i in 0..cases {
        let mut rig = seed();
        rig.camera.pos = rng.vec(10000.0);
        rig.camera.focus = rng.vec(5000.0);
        rig.camera.yaw = rng.next() as i16;
        rig.camera.next_yaw = rng.next() as i16;
        rig.camera.mode = [1, 4, 6, 16][i % 4];
        rig.camera.cutscene = u8::from(i % 7 == 0);
        rig.lakitu.cur_pos = rng.vec(7000.0);
        rig.lakitu.cur_focus = rng.vec(3000.0);
        rig.lakitu.pos = rig.lakitu.cur_pos;
        rig.lakitu.focus = rig.lakitu.cur_focus;
        rig.lakitu.yaw = rng.next() as i16;
        rig.lakitu.shake_magnitude = std::array::from_fn(|_| rng.next() as i16);
        rig.lakitu.shake_phase = std::array::from_fn(|_| rng.next() as i16);
        rig.lakitu.shake_velocity =
            std::array::from_fn(|_| [0, i16::MIN, -100, 0x1000, 0x4000][rng.next() as usize % 5]);
        rig.lakitu.shake_decay =
            std::array::from_fn(|_| [i16::MIN, -10, 0, 3, 200][rng.next() as usize % 5]);
        rig.lakitu.key_dance_roll = rng.next() as i16;
        rig.lakitu.foc_h_speed = rng.f(1.2);
        rig.lakitu.foc_v_speed = rng.f(1.2);
        rig.lakitu.pos_h_speed = rng.f(1.2);
        rig.lakitu.pos_v_speed = rng.f(1.2);
        rig.handheld_angles = std::array::from_fn(|_| rng.next() as i16);
        rig.handheld_increment = rng.f(1.0);
        rig.old_pos = rng.vec(7000.0);
        rig.old_focus = rng.vec(3000.0);
        rig.player2_focus_offset = rng.vec(30.0);
        rig.transition.pos_dist = rng.f(5000.0);
        rig.transition.foc_dist = rng.f(5000.0);
        rig.transition.pos_pitch = rng.next() as i16;
        rig.transition.pos_yaw = rng.next() as i16;
        rig.transition.foc_pitch = rng.next() as i16;
        rig.transition.foc_yaw = rng.next() as i16;
        rig.transition.mario_pos = rng.vec(5000.0);
        rig.transition.frames_left = [-10, 0, 1, 2, 30, 60, 90, 32767][i % 8];
        rig.status = [
            0,
            c::CAM_FLAG_SMOOTH_MOVEMENT,
            c::CAM_FLAG_BLOCK_SMOOTH_MOVEMENT,
            c::CAM_FLAG_SMOOTH_MOVEMENT
                | c::CAM_FLAG_START_TRANSITION
                | c::CAM_FLAG_UNUSED_CUTSCENE_ACTIVE,
            c::CAM_FLAG_TRANSITION_OUT_OF_C_UP | c::CAM_FLAG_START_TRANSITION,
        ][i % 5];
        rig.movement = if i % 19 == 0 {
            c::CAM_MOVE_PAUSE_SCREEN
        } else if i % 3 == 0 {
            c::CAM_MOVE_C_UP_MODE
        } else {
            0
        };
        if rig.movement & c::CAM_MOVE_PAUSE_SCREEN != 0 {
            rig.handheld_magnitude = 123;
        }
        let mario = rng.vec(5000.0);
        let action = if i % 3 == 0 { c::ACT_DIVE } else { c::ACT_IDLE };
        for checking_for_camera in [false, true] {
            for find_floor_include_surface_intangible in [false, true] {
                let mut flags = CollisionFlags {
                    checking_for_camera,
                    find_floor_include_surface_intangible,
                };
                let mut state = rig;
                oracle.reset_lakitu(state);
                compare(&state, &oracle.lakitu_snapshot(), "transport");
                oracle.lakitu_goal(mario, action, state.camera);
                state
                    .update(mario, action, &world, &mut flags, trig)
                    .unwrap();
                let (native, native_flags) = oracle.lakitu_update(CollisionFlags {
                    checking_for_camera,
                    find_floor_include_surface_intangible,
                });
                compare(&state, &native, &format!("random case {i}"));
                assert_eq!(flags_words(flags), native_flags, "random flags case {i}");
            }
        }
    }
    eprintln!(
        "{cases} randomized Lakitu states x 4 collision flag combinations: all 94 state words and flags exact"
    );
}

#[test]
fn randomized_lakitu_state_matches_native_update() {
    random_cases(&terrain(), &tables(), 10000);
}

#[test]
fn mode_transitions_and_damage_shake_priority_match() {
    let trig = tables();
    let oracle = CameraOracle::new(&terrain(), &trig);
    for initial in [0, 1, 4, 6, 16, 255] {
        for mode in [-1, 0, 1, 4, 6, 16, 32767] {
            for frames in [i16::MIN, -1, 0, 1, 30, i16::MAX] {
                for status in [0, c::CAM_FLAG_FRAME_AFTER_CAM_INIT] {
                    let mut rig = seed();
                    rig.camera.mode = initial;
                    rig.last_mode = 16;
                    rig.status = status;
                    rig.movement = u16::MAX;
                    rig.pan_distance = 32.0;
                    rig.cannon_y_offset = -5.0;
                    rig.mode_offset_yaw = -900;
                    rig.lakitu_dist = 300;
                    rig.lakitu_pitch = 250;
                    rig.c_up_pitch = 30;
                    rig.area_yaw_change = -50;
                    oracle.reset_lakitu(rig);
                    rig.transition_to_camera_mode(mode, frames);
                    oracle.lakitu_transition_mode(mode, frames);
                    compare(&rig, &oracle.lakitu_snapshot(), "mode transition");
                    rig.transition_next_state(frames);
                    oracle.lakitu_transition_next(frames);
                    compare(&rig, &oracle.lakitu_snapshot(), "next transition");
                }
            }
        }
    }
    for action in [c::ACT_IDLE, c::ACT_FLAG_SWIMMING, c::ACT_FLAG_METAL_WATER] {
        for shake in [
            -1,
            0,
            c::SHAKE_ATTACK,
            c::SHAKE_FALL_DAMAGE,
            c::SHAKE_GROUND_POUND,
            c::SHAKE_SMALL_DAMAGE,
            c::SHAKE_MED_DAMAGE,
            c::SHAKE_LARGE_DAMAGE,
            c::SHAKE_HIT_FROM_BELOW,
            32767,
        ] {
            for old in [i16::MIN, -0x800, 0, 0x80, 0x800, i16::MAX] {
                let mut rig = seed();
                rig.lakitu.shake_magnitude = [old; 3];
                oracle.reset_lakitu(rig);
                oracle.lakitu_goal([0.0; 3], action, rig.camera);
                rig.shake_from_hit(shake, action).unwrap();
                oracle.lakitu_hit(shake);
                compare(&rig, &oracle.lakitu_snapshot(), "hit shake");
                // Repeated/weaker requests must respect existing magnitude priority.
                rig.shake_from_hit(c::SHAKE_SMALL_DAMAGE, action).unwrap();
                oracle.lakitu_hit(c::SHAKE_SMALL_DAMAGE);
                compare(&rig, &oracle.lakitu_snapshot(), "weaker repeated shake");
            }
        }
    }
}

fn sequences(stream: &[i16], trig: &TrigTables, origin: [f32; 3]) {
    let world = world(stream);
    let mut baseline = None;
    for hz in [15, 30, 60, 120, 144] {
        for interpolate in [false, true] {
            let oracle = CameraOracle::new(stream, trig);
            let mut rig = seed();
            oracle.reset_lakitu(rig);
            let mut clock = FixedClock::default();
            let mut count = 0;
            let mut frame = 0_u64;
            let mut records = Vec::new();
            while count < 3600 {
                frame += 1;
                let end = Duration::from_nanos(frame * 1_000_000_000 / hz);
                let start = Duration::from_nanos((frame - 1) * 1_000_000_000 / hz);
                clock.add_elapsed(end - start).unwrap();
                for _ in 0..clock.drain(8) {
                    if count == 3600 {
                        break;
                    }
                    let previous = rig.lakitu.pos;
                    let yaw = (count as i16).wrapping_mul(67);
                    let mario = [
                        origin[0] + trig.sins(i32::from(yaw)) * 600.0,
                        origin[1] + (count % 71) as f32 * 5.0,
                        origin[2] + trig.coss(i32::from(yaw)) * 600.0,
                    ];
                    let action = if count % 130 < 12 {
                        c::ACT_DIVE
                    } else {
                        c::ACT_IDLE
                    };
                    let goal = Camera {
                        mode: if count % 500 < 250 { 1 } else { 16 },
                        def_mode: 1,
                        next_yaw: yaw.wrapping_add(i16::MIN),
                        pos: [
                            mario[0] + trig.sins(i32::from(yaw)) * 1100.0,
                            mario[1] + 300.0,
                            mario[2] + trig.coss(i32::from(yaw)) * 1100.0,
                        ],
                        focus: [mario[0], mario[1] + 125.0, mario[2]],
                        ..Camera::default()
                    };
                    set_goal(&mut rig, &oracle, mario, action, goal);
                    if count % 500 == 0 {
                        rig.transition_next_state(60);
                        oracle.lakitu_transition_next(60);
                    } else if count % 173 == 9 {
                        // Interrupt an in-flight transition (first at tick 9).
                        rig.transition_next_state(30);
                        oracle.lakitu_transition_next(30);
                    }
                    if count % 137 == 0 {
                        let shake = [
                            c::SHAKE_ATTACK,
                            c::SHAKE_GROUND_POUND,
                            c::SHAKE_SMALL_DAMAGE,
                            c::SHAKE_FALL_DAMAGE,
                        ][count / 137 % 4];
                        rig.shake_from_hit(shake, action).unwrap();
                        oracle.lakitu_hit(shake);
                    }
                    let mut flags = CollisionFlags::default();
                    rig.update(mario, action, &world, &mut flags, trig).unwrap();
                    let (native, native_flags) = oracle.lakitu_update(CollisionFlags::default());
                    compare(&rig, &native, &format!("sequence tick {count}, {hz}Hz"));
                    assert_eq!(flags_words(flags), native_flags);
                    rig.lakitu.last_frame_action = action;
                    oracle.lakitu_end_frame();
                    compare(&rig, &oracle.lakitu_snapshot(), "outer lastFrameAction");
                    records.push((rig.state_words(), flags_words(flags)));
                    let alpha = if interpolate { clock.alpha() } else { 1.0 };
                    let displayed = std::array::from_fn::<_, 3, _>(|i| {
                        previous[i] + (rig.lakitu.pos[i] - previous[i]) * alpha
                    });
                    std::hint::black_box(displayed);
                    count += 1;
                }
            }
            if let Some(expected) = &baseline {
                assert_eq!(&records, expected, "presentation rate changed tick state");
            } else {
                baseline = Some(records);
            }
        }
    }
    eprintln!(
        "3600 persistent Lakitu ticks x five presentation rates x interpolation on/off: all state exact"
    );
}

#[test]
fn persistent_lakitu_ticks_match_at_all_presentation_rates() {
    sequences(&terrain(), &tables(), [0.0; 3]);
}

#[test]
fn random_requests_stop_before_mutating_state() {
    let world = world(&terrain());
    let trig = tables();
    let mut rig = seed();
    let before = rig.state_words();
    assert_eq!(
        rig.shake_from_hit(c::SHAKE_SHOCK, c::ACT_IDLE),
        Err(Unsupported::ShockRandomShake)
    );
    assert_eq!(rig.state_words(), before);
    rig.handheld_magnitude = 1;
    let before = rig.state_words();
    assert_eq!(
        rig.update(
            [0.0; 3],
            c::ACT_IDLE,
            &world,
            &mut CollisionFlags::default(),
            &trig
        ),
        Err(Unsupported::HandheldRandomShake)
    );
    assert_eq!(rig.state_words(), before);
}

#[test]
fn camera_filter_flag_is_retained_when_floor_corrected_or_missing() {
    let trig = tables();
    for (stream, pos) in [
        (terrain(), [-2500.0, 10.0, 0.0]),
        (vec![0x40, 0, 0x41, 0x42], [100.0, 1.0, 100.0]),
    ] {
        let world = world(&stream);
        let mut rig = seed();
        rig.status = 0;
        rig.camera.pos = pos;
        let mut flags = CollisionFlags::default();
        rig.update([0.0; 3], c::ACT_IDLE, &world, &mut flags, &trig)
            .unwrap();
        assert!(flags.checking_for_camera);
    }
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; BOB collision and original trig tables"]
fn bob_lakitu_ticks_match_with_rom_tables() {
    use rustario64::import::{bob, collision, engine, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        rom.reader().slice(range.start, range.len()).unwrap(),
        version::MAX_SEGMENT_BYTES,
    )
    .unwrap();
    let start = (version::BOB_COLLISION & 0xffffff) as usize;
    let (mesh, consumed) = collision::decode(&terrain[start..]).unwrap();
    let imported = bob::import(&rom).unwrap();
    assert_eq!(mesh, imported.collision);
    let stream: Vec<i16> = terrain[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    let trig = engine::trig_tables(&rom).unwrap();
    random_cases(&stream, &trig, 20000);
    let spawn = imported.level.mario_start.as_ref().unwrap().2;
    sequences(&stream, &trig, spawn.map(f32::from));
}
