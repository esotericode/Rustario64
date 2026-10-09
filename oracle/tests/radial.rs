//! Native-C comparisons of obstruction/radial stages. Persistent compositions
//! below deliberately omit full mode input, camera height and dispatch.
use rustario64::simulation::{
    FixedClock,
    camera::{
        self, PlayerGeometry, RadialState,
        lakitu::{Camera, Lakitu, Rig, STATE_FIELD_NAMES},
        obstruction as o,
        radial::RadialMovement,
    },
    collision::{CollisionFlags, Surface},
    mario::constants as c,
    math::TrigTables,
    rng::Rng as GameRng,
};
use rustario64_oracle::camera::CameraOracle;
use std::time::Duration;
#[path = "support/camera.rs"]
mod fixtures;
use fixtures::{Rng, tables, terrain, world};

fn compare(rig: &Rig, native: &[u32], context: &str) {
    for ((name, word), native) in STATE_FIELD_NAMES.iter().zip(rig.state_words()).zip(native) {
        assert_eq!(word, *native, "{context}: {name}");
    }
}
fn flags_words(flags: CollisionFlags) -> [u32; 2] {
    [
        flags.checking_for_camera.into(),
        flags.find_floor_include_surface_intangible.into(),
    ]
}
fn movement() -> RadialMovement {
    RadialMovement {
        area: c::AREA_BOB,
        center: [0.0; 2],
        second_rotate: 0,
    }
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
        status: c::CAM_FLAG_SMOOTH_MOVEMENT,
        yaw_speed: 0x400,
        old_pos: pos,
        old_focus: focus,
        ..Rig::default()
    }
}

fn pan_cases(trig: &TrigTables, count: usize) {
    let oracle = CameraOracle::new(&terrain(), trig);
    let mut rng = Rng(7351);
    let actions = [
        c::ACT_WALKING,
        c::ACT_LONG_JUMP,
        c::ACT_HOLDING_POLE,
        c::ACT_TOP_OF_POLE,
        c::ACT_TOP_OF_POLE_TRANSITION,
        c::ACT_FREEFALL,
    ];
    for i in 0..count {
        let mario = rng.vec(8000.0);
        let action = actions[i % actions.len()];
        let face_yaw = [i16::MIN, i16::MAX, 0, -1, rng.next() as i16][i % 5];
        let mut initial = seed();
        initial.camera.pos = match i % 5 {
            0 => mario,
            1 => [mario[0], mario[1] + 500.0, mario[2]],
            _ => rng.vec(8000.0),
        };
        initial.camera.focus = if i.is_multiple_of(11) {
            [-0.0; 3]
        } else {
            rng.vec(8000.0)
        };
        initial.pan_distance = if i.is_multiple_of(13) {
            -0.0
        } else {
            rng.f(2000.0)
        };
        for status in [
            0,
            c::CAM_FLAG_SLEEPING,
            c::CAM_FLAG_SMOOTH_MOVEMENT,
            c::CAM_FLAG_SLEEPING | c::CAM_FLAG_SMOOTH_MOVEMENT,
        ] {
            let mut rig = initial;
            rig.status = status;
            oracle.reset_lakitu(rig);
            rig.pan_ahead_of_player(mario, action, face_yaw, trig);
            compare(
                &rig,
                &oracle.pan_ahead(mario, action, face_yaw),
                &format!("pan case {i} status {status}"),
            );
            // Pan must not change camera-relative movement yaw or eye position.
            assert_eq!(rig.camera.yaw, initial.camera.yaw);
            assert_eq!(rig.camera.next_yaw, initial.camera.next_yaw);
            assert_eq!(
                rig.camera.pos.map(f32::to_bits),
                initial.camera.pos.map(f32::to_bits)
            );
        }
    }
}

#[test]
fn look_ahead_pan_matches_actions_sleeping_and_float_order() {
    pan_cases(&tables(), 10000);
}

#[test]
fn avoid_yaw_matches_signed_half_turn_boundaries() {
    let trig = tables();
    let oracle = CameraOracle::new(&terrain(), &trig);
    for raw in 0..=u16::MAX {
        for delta in [
            i16::MIN,
            -0x4001,
            -0x4000,
            -0x3fff,
            -1,
            0,
            1,
            0x3fff,
            0x4000,
            i16::MAX,
        ] {
            let yaw = raw as i16;
            let wall = yaw.wrapping_add(delta);
            assert_eq!(
                o::calc_avoid_yaw(yaw, wall),
                oracle.avoid_yaw(yaw, wall),
                "yaw={yaw} wall={wall}"
            );
        }
    }
}

#[test]
fn vertex_tests_match_strict_extents_integer_products_and_sectors() {
    let trig = tables();
    let stream = terrain();
    let world = world(&stream);
    let oracle = CameraOracle::new(&stream, &trig);
    let mut rng = Rng(41839);
    let check = |surface: &Surface, from, to, range, exclude, bounds, present| {
        let actual: [u32; 3] = [
            o::is_surf_within_bounding_box(surface, bounds).into(),
            o::is_behind_surface(to, surface).into(),
            o::is_range_behind_surface(
                from,
                to,
                if present { Some(surface) } else { None },
                range,
                exclude,
                &trig,
            )
            .into(),
        ];
        assert_eq!(
            actual,
            oracle.surface_checks(surface, present, from, to, range, exclude, bounds),
            "vertices={:?}/{:?}/{:?} from={from:?} to={to:?} range={range} bounds={bounds:?}",
            surface.vertex1,
            surface.vertex2,
            surface.vertex3
        );
    };
    for surface in world.surfaces() {
        for to in [
            surface.vertex1.map(f32::from),
            [0.0; 3],
            surface.vertex2.map(f32::from),
        ] {
            for range in [0, -1, 1, 0x400, 0x600, i16::MIN, i16::MAX] {
                for exclude in [-1, c::SURFACE_WALL_MISC, surface.surface_type] {
                    check(
                        surface,
                        [600.0, 300.0, -200.0],
                        to,
                        range,
                        exclude,
                        [-1.0, 150.0, -1.0],
                        true,
                    );
                }
            }
        }
    }
    for i in 0..20000 {
        let mut surface = world.surfaces()[i % world.surfaces().len()];
        if i % 2 == 0 {
            surface.vertex1 = std::array::from_fn(|_| rng.next() as i16);
            surface.vertex2 = std::array::from_fn(|_| rng.next() as i16);
            surface.vertex3 = std::array::from_fn(|_| rng.next() as i16);
        }
        let from = rng.vec(10000.0);
        let to = rng.vec(10000.0);
        let range = rng.next() as i16;
        let bounds = std::array::from_fn(|_| {
            if rng.next().is_multiple_of(3) {
                -1.0
            } else {
                rng.f(40000.0)
            }
        });
        check(
            &surface,
            from,
            to,
            range,
            if i % 3 == 0 { surface.surface_type } else { -1 },
            bounds,
            i % 7 != 0,
        );
    }
    // Low walls use STRICT height <150, not <=150; ignored X/Z bounds do not
    // test just one horizontal axis. Reverse winding and exact-plane points.
    for height in [149, 150, 151] {
        let mut surface = world.surfaces()[0];
        surface.vertex1 = [0, 0, -1000];
        surface.vertex2 = [0, height, -1000];
        surface.vertex3 = [0, 0, 1000];
        assert_eq!(
            o::is_surf_within_bounding_box(&surface, [-1.0, 150.0, -1.0]),
            height < 150
        );
        for to in [[-1.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0]] {
            check(
                &surface,
                [1000.0, 0.0, 0.0],
                to,
                0x400,
                -1,
                [-1.0, 150.0, -1.0],
                true,
            );
            std::mem::swap(&mut surface.vertex2, &mut surface.vertex3);
            check(
                &surface,
                [1000.0, 0.0, 0.0],
                to,
                0x400,
                -1,
                [-1.0, 150.0, -1.0],
                true,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scan(
    oracle: &CameraOracle,
    world: &rustario64::simulation::collision::CollisionWorld,
    trig: &TrigTables,
    mario: [f32; 3],
    pos: [f32; 3],
    range: i16,
    flags: CollisionFlags,
    initial_yaw: i16,
    initial_status: i16,
) -> i32 {
    let mut yaw = initial_yaw;
    let mut status = initial_status;
    let result = o::rotate_camera_around_walls(
        mario,
        pos,
        &mut yaw,
        range,
        &mut status,
        world,
        flags,
        false,
        trig,
    );
    assert_eq!(
        [
            result as u32,
            yaw as i32 as u32,
            status as i32 as u32,
            flags.checking_for_camera.into(),
            flags.find_floor_include_surface_intangible.into()
        ],
        oracle.obstruction(mario, pos, initial_yaw, range, initial_status, flags),
        "mario={mario:?} eye={pos:?} range={range} flags={flags:?}"
    );
    if result == 0 {
        assert_eq!(yaw, initial_yaw);
    }
    result
}

fn obstruction_cases(stream: &[i16], trig: &TrigTables, cases: usize) {
    let world = world(stream);
    let oracle = CameraOracle::new(stream, trig);
    let mut rng = Rng(9361);
    let mut counts = [0usize; 4];
    for i in 0..cases {
        let mario = rng.vec(if i % 2 == 0 { 1200.0 } else { 7500.0 });
        let pos =
            std::array::from_fn(|axis| mario[axis] + rng.f(if axis == 1 { 500.0 } else { 1500.0 }));
        for checking_for_camera in [false, true] {
            for find_floor_include_surface_intangible in [false, true] {
                let flags = CollisionFlags {
                    checking_for_camera,
                    find_floor_include_surface_intangible,
                };
                let result = scan(
                    &oracle,
                    &world,
                    trig,
                    mario,
                    pos,
                    [0, 0x400, 0x600, -0x400, i16::MIN][i % 5],
                    flags,
                    rng.next() as i16,
                    rng.next() as i16,
                );
                counts[result as usize] += 1;
            }
        }
    }
    assert!(
        counts[0] > 0 && counts[1] > 0 && counts[3] > 0,
        "all scan results must occur: {counts:?}"
    );
    assert_eq!(counts[2], 0);
    eprintln!("obstruction cases: {counts:?}");
}

#[test]
fn obstruction_matches_ordered_wall_queries_and_camera_filters() {
    obstruction_cases(&terrain(), &tables(), 10000);
}

fn wall(height: i16, surface: i16) -> Vec<i16> {
    vec![
        0x40, 4, 0, 0, -2000, 0, height, -2000, 0, 0, 2000, 0, height, 2000, surface, 2, 0, 1, 2,
        2, 1, 3, 0x41, 0x42,
    ]
}

#[test]
fn low_wall_cutoff_near_wall_and_clear_flags_have_distinct_results() {
    let trig = tables();
    for height in [149, 150, 151, 2000] {
        for surface in [
            0,
            c::SURFACE_WALL_MISC,
            c::SURFACE_NO_CAM_COLLISION,
            c::SURFACE_CAMERA_BOUNDARY,
        ] {
            let stream = wall(height, surface);
            let world = world(&stream);
            let oracle = CameraOracle::new(&stream, &trig);
            for checking_for_camera in [false, true] {
                let flags = CollisionFlags {
                    checking_for_camera,
                    find_floor_include_surface_intangible: true,
                };
                let result = scan(
                    &oracle,
                    &world,
                    &trig,
                    [500.0, 0.0, 0.0],
                    [-500.0, 0.0, 0.0],
                    0x400,
                    flags,
                    77,
                    -1,
                );
                if surface == 0 {
                    assert_eq!(result, if height >= 150 { 3 } else { 1 });
                }
                scan(
                    &oracle,
                    &world,
                    &trig,
                    [1100.0, 0.0, 0.0],
                    [25.0, 0.0, 0.0],
                    0x400,
                    flags,
                    -123,
                    -1,
                );
                assert_eq!(
                    scan(
                        &oracle,
                        &world,
                        &trig,
                        [2000.0, 0.0, 0.0],
                        [3000.0, 0.0, 0.0],
                        0x400,
                        flags,
                        314,
                        -1
                    ),
                    0
                );
            }
        }
    }
}

fn random_movement(stream: &[i16], trig: &TrigTables, cases: usize) {
    let world = world(stream);
    let oracle = CameraOracle::new(stream, trig);
    let mut rng = Rng(63823);
    for i in 0..cases {
        let mario = rng.vec(7500.0);
        let forward_vel = [0.0, -0.25, 0.25, -48.0, 32.0, 48.5, 8192.0][i % 7];
        let mut rig = seed();
        rig.camera.pos = if i % 2 == 0 {
            rng.vec(7500.0)
        } else {
            std::array::from_fn(|j| mario[j] + rng.f(1200.0))
        };
        rig.mode_offset_yaw = rng.next() as i16;
        rig.camera.mode = [c::CAMERA_MODE_RADIAL, c::CAMERA_MODE_OUTWARD_RADIAL][i % 2] as u8;
        rig.movement = rng.next() as u16;
        rig.status = rng.next() as i16;
        rig.area_yaw_change = rng.next() as i16;
        rig.lakitu_dist = rng.next() as i16;
        rig.lakitu_pitch = rng.next() as i16;
        let mut radial = RadialMovement {
            area: [
                c::AREA_BOB,
                c::AREA_TTC,
                c::AREA_SSL_PYRAMID,
                c::AREA_LLL_OUTSIDE,
                -1,
            ][i % 5],
            center: [rng.f(7500.0), rng.f(7500.0)],
            second_rotate: rng.next() as u16,
        };
        let floors = [
            0,
            c::SURFACE_CAMERA_MIDDLE,
            c::SURFACE_CAMERA_ROTATE_LEFT,
            c::SURFACE_CAMERA_ROTATE_RIGHT,
        ];
        let curr = floors[i % 4];
        let prev = floors[(i / 4) % 4];
        let flags = CollisionFlags {
            checking_for_camera: i % 3 == 0,
            find_floor_include_surface_intangible: i % 7 == 0,
        };
        oracle.reset_radial(rig, radial);
        let area_yaw = rng.next() as i16;
        assert_eq!(
            radial.outward_offset(&rig, mario, forward_vel, area_yaw),
            oracle.radial_offset(mario, forward_vel, area_yaw)
        );
        radial.move_camera(
            &mut rig,
            mario,
            forward_vel,
            curr,
            prev,
            &world,
            flags,
            trig,
        );
        let (native, extra) = oracle.radial_move(mario, forward_vel, curr, prev, flags);
        compare(&rig, &native, &format!("random movement {i}"));
        assert_eq!(
            [
                u32::from(radial.second_rotate),
                flags_words(flags)[0],
                flags_words(flags)[1]
            ],
            extra
        );
        let range_dist = [400.0, 399.5, -30.5, 0.0, 500.25][i % 5];
        let range_pitch = [0x900, -0x900, i16::MIN, i16::MAX, 0, 12, 13][i % 7];
        radial.zoom(&mut rig, range_dist, range_pitch);
        compare(
            &rig,
            &oracle.radial_zoom(range_dist, range_pitch),
            &format!("random zoom {i}"),
        );
    }
}

#[test]
fn radial_rotation_surface_entries_outward_offsets_and_zoom_match() {
    random_movement(&terrain(), &tables(), 20000);
}

#[test]
fn surface_rotation_and_second_rotation_finish_at_original_limits() {
    let stream = wall(2000, 0);
    let world = world(&stream);
    let trig = tables();
    let oracle = CameraOracle::new(&stream, &trig);
    for (surface, direction) in [
        (c::SURFACE_CAMERA_ROTATE_RIGHT, c::CAM_MOVE_ROTATE_RIGHT),
        (c::SURFACE_CAMERA_ROTATE_LEFT, c::CAM_MOVE_ROTATE_LEFT),
    ] {
        for second in [false, true] {
            let mut rig = seed();
            let mut radial = movement();
            rig.camera.pos = [3000.0, 200.0, 3000.0];
            radial.second_rotate = if second { direction } else { 0 };
            oracle.reset_radial(rig, radial);
            for tick in 0..60 {
                let prev = if tick == 0 { 0 } else { surface };
                radial.move_camera(
                    &mut rig,
                    [2000.0, 0.0, 2000.0],
                    0.0,
                    surface,
                    prev,
                    &world,
                    CollisionFlags::default(),
                    &trig,
                );
                let (native, extra) = oracle.radial_move(
                    [2000.0, 0.0, 2000.0],
                    0.0,
                    surface,
                    prev,
                    CollisionFlags::default(),
                );
                compare(&rig, &native, "surface sequence");
                assert_eq!(u32::from(radial.second_rotate), extra[0]);
            }
            let magnitude = if second {
                105 * 0x10000 / 360
            } else {
                60 * 0x10000 / 360
            };
            let sign = if direction == c::CAM_MOVE_ROTATE_RIGHT {
                1
            } else {
                -1
            };
            assert_eq!(rig.mode_offset_yaw, (magnitude * sign) as i16);
            assert_eq!(
                rig.movement & (direction | c::CAM_MOVE_ENTERED_ROTATE_SURFACE),
                0
            );
            assert_eq!(radial.second_rotate & direction, 0);
        }
    }
}

fn persistent_stages(stream: &[i16], trig: &TrigTables) {
    let world = world(stream);
    let oracle = CameraOracle::new(stream, trig);
    let mut baseline = None;
    for hz in [15, 30, 60, 120, 144] {
        for interpolation in [false, true] {
            let mut rig = seed();
            let mut radial = movement();
            let mut area_yaw = 0i16;
            let mut previous_floor = 0;
            let mut flags = CollisionFlags::default();
            oracle.reset_radial(rig, radial);
            let mut clock = FixedClock::default();
            let mut frame = 0u64;
            let mut tick = 0usize;
            let mut records = Vec::new();
            let mut prev_display = rig.lakitu.pos;
            while tick < 1800 {
                frame += 1;
                let end = Duration::from_nanos(frame * 1_000_000_000 / hz);
                let start = Duration::from_nanos((frame - 1) * 1_000_000_000 / hz);
                clock.add_elapsed(end - start).unwrap();
                for _ in 0..clock.drain(8) {
                    if tick == 1800 {
                        break;
                    }
                    let yaw = (tick as i16).wrapping_mul(83);
                    let mario = [
                        trig.sins(i32::from(yaw)) * 2000.0,
                        600.0 + trig.sins(i32::from(yaw.wrapping_mul(3))) * 400.0,
                        trig.coss(i32::from(yaw)) * 2000.0,
                    ];
                    let action = if tick % 120 < 60 {
                        c::ACT_WALKING
                    } else {
                        c::ACT_FREEFALL
                    };
                    let forward_vel = if tick % 91 < 60 { 32.0 } else { 0.0 };
                    let floor = [
                        0,
                        c::SURFACE_CAMERA_ROTATE_RIGHT,
                        c::SURFACE_CAMERA_MIDDLE,
                        c::SURFACE_CAMERA_ROTATE_LEFT,
                    ][(tick / 90) % 4];
                    if tick.is_multiple_of(120) {
                        rig.movement ^= c::CAM_MOVE_ZOOMED_OUT;
                        // Apply the same authored event to each independent state.
                        oracle.radial_toggle_zoom();
                    }
                    if tick.is_multiple_of(173) {
                        rig.transition_next_state(15);
                        oracle.lakitu_transition_next(15);
                    }
                    radial.move_camera(
                        &mut rig,
                        mario,
                        forward_vel,
                        floor,
                        previous_floor,
                        &world,
                        flags,
                        trig,
                    );
                    let (native, extra) =
                        oracle.radial_move(mario, forward_vel, floor, previous_floor, flags);
                    compare(&rig, &native, &format!("hz={hz} tick={tick} movement"));
                    assert_eq!(u32::from(radial.second_rotate), extra[0]);
                    radial.zoom(&mut rig, 400.0, 0x900);
                    compare(&rig, &oracle.radial_zoom(400.0, 0x900), "persistent zoom");
                    let geometry = PlayerGeometry {
                        floor_height: mario[1] - 100.0,
                        ..PlayerGeometry::default()
                    };
                    let mut state = RadialState {
                        area: radial.area,
                        center_x: radial.center[0],
                        center_z: radial.center[1],
                        mode_offset_yaw: rig.mode_offset_yaw,
                        lakitu_dist: rig.lakitu_dist,
                        lakitu_pitch: rig.lakitu_pitch,
                        area_yaw,
                    };
                    let incoming_flags = flags;
                    let goal = camera::update_radial_camera(
                        &mut state, mario, action, geometry, &world, &mut flags, trig,
                    );
                    area_yaw = state.area_yaw;
                    rig.camera.pos = goal.pos;
                    rig.camera.focus = goal.focus;
                    rig.camera.next_yaw = goal.yaw;
                    let (native, extra) = oracle.radial_goal_stage(
                        mario,
                        action,
                        geometry.floor_height,
                        incoming_flags,
                    );
                    compare(&rig, &native, "persistent radial goal");
                    assert_eq!(
                        extra,
                        [
                            area_yaw as i32 as u32,
                            flags_words(flags)[0],
                            flags_words(flags)[1]
                        ]
                    );
                    prev_display = rig.lakitu.pos;
                    let face_yaw = yaw.wrapping_add(0x2222);
                    rig.pan_ahead_of_player(mario, action, face_yaw, trig);
                    compare(
                        &rig,
                        &oracle.pan_ahead(mario, action, face_yaw),
                        "persistent pan",
                    );
                    let incoming_flags = flags;
                    rig.update(
                        mario,
                        action,
                        &world,
                        &mut flags,
                        trig,
                        &mut GameRng::default(),
                        false,
                    );
                    let (native, native_flags) = oracle.lakitu_update(incoming_flags);
                    compare(&rig, &native, &format!("hz={hz} tick={tick} Lakitu"));
                    assert_eq!(flags_words(flags), native_flags);
                    rig.lakitu.last_frame_action = action;
                    oracle.lakitu_end_frame();
                    compare(&rig, &oracle.lakitu_snapshot(), "outer assignment");
                    previous_floor = floor;
                    records.push((
                        rig.state_words(),
                        radial.second_rotate,
                        area_yaw,
                        flags_words(flags),
                    ));
                    tick += 1;
                }
                let alpha = if interpolation { clock.alpha() } else { 1.0 };
                let displayed = std::array::from_fn::<_, 3, _>(|i| {
                    prev_display[i] + (rig.lakitu.pos[i] - prev_display[i]) * alpha
                });
                std::hint::black_box(displayed);
            }
            if let Some(expected) = &baseline {
                assert_eq!(&records, expected);
            } else {
                baseline = Some(records);
            }
        }
    }
}

#[test]
fn radial_goal_and_lakitu_stages_are_persistent_and_render_rate_independent() {
    persistent_stages(&terrain(), &tables());
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; private BOB collision and original trig tables"]
fn bob_radial_and_obstruction_stages_match_with_rom_tables() {
    use rustario64::import::{bob, collision, engine, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let range = version::BOB_TERRAIN;
    let bytes = mio0::decode(
        rom.reader().slice(range.start, range.len()).unwrap(),
        version::MAX_SEGMENT_BYTES,
    )
    .unwrap();
    let start = (version::BOB_COLLISION & 0xffffff) as usize;
    let (mesh, consumed) = collision::decode(&bytes[start..]).unwrap();
    assert_eq!(mesh, bob::import(&rom).unwrap().collision);
    let stream: Vec<i16> = bytes[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    let trig = engine::trig_tables(&rom).unwrap();
    obstruction_cases(&stream, &trig, 20000);
    random_movement(&stream, &trig, 40000);
    persistent_stages(&stream, &trig);
    pan_cases(&trig, 10000);
}
