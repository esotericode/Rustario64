//! Independently authored camera fixtures; exact native-C differential checks.
//! These compare components and radial goals, not a complete camera tick.
use rustario64::{
    import::collision,
    simulation::{
        FixedClock,
        camera::{self, PlayerGeometry, RadialState},
        collision::{CollisionFlags, CollisionWorld},
        mario::constants as c,
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
    },
};
use rustario64_oracle::camera::{CameraAngle, CameraFloat, CameraOracle, CameraVector};
use std::time::Duration;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn f(&mut self, scale: f32) -> f32 {
        (self.next() as f32 / u32::MAX as f32 - 0.5) * 2.0 * scale
    }
    fn vec(&mut self, scale: f32) -> [f32; 3] {
        std::array::from_fn(|_| self.f(scale))
    }
}

/// Computed approximations, not original ROM table values.
fn tables() -> TrigTables {
    TrigTables::new(
        (0..SINE_ENTRIES)
            .map(|i| (i as f64 * std::f64::consts::TAU / 4096.0).sin() as f32)
            .collect(),
        (0..ARCTAN_ENTRIES)
            .map(|i| ((i as f64 / 1024.0).atan() * 32768.0 / std::f64::consts::PI).round() as u16)
            .collect(),
    )
    .unwrap()
}

/// Sloped bands, camera-filtered floors/ceilings, narrow tunnels, intersecting
/// walls and a water region. Every vertex and word is authored here.
fn terrain() -> Vec<i16> {
    let mut vertices = Vec::<[i16; 3]>::new();
    let mut groups = vec![];
    let mut triangle = |surface: i16, points: [[i16; 3]; 3]| {
        let base = vertices.len() as i16;
        vertices.extend(points);
        groups.push([surface, 1, base, base + 1, base + 2]);
    };
    for i in 0..6 {
        let x = -3000 + i * 1000;
        let y = i * 70;
        let floor_type = [
            0,
            c::SURFACE_NO_CAM_COLLISION,
            c::SURFACE_INTANGIBLE,
            c::SURFACE_WALL_MISC,
            c::SURFACE_CAMERA_BOUNDARY,
            0,
        ][i as usize];
        triangle(
            floor_type,
            [[x + 1000, y + 120, -3000], [x, y, -3000], [x, y, 3000]],
        );
        triangle(
            floor_type,
            [
                [x + 1000, y + 120, -3000],
                [x, y, 3000],
                [x + 1000, y + 120, 3000],
            ],
        );
        if floor_type == c::SURFACE_INTANGIBLE {
            // The reference retries 200 below an intangible floor; without a
            // lower floor it retains a height with a NULL pointer and camera.c
            // dereferences it. Keep random comparisons in the defined domain.
            triangle(
                0,
                [[x, -500, -3000], [x, -500, 3000], [x + 1000, -500, -3000]],
            );
            triangle(
                0,
                [
                    [x + 1000, -500, -3000],
                    [x, -500, 3000],
                    [x + 1000, -500, 3000],
                ],
            );
        }
        let ceil_y = y + if i % 2 == 0 { 200 } else { 1100 };
        let ceil_type = if i == 1 {
            c::SURFACE_NO_CAM_COLLISION
        } else {
            0
        };
        triangle(
            ceil_type,
            [
                [x, ceil_y, -3000],
                [x + 1000, ceil_y, -3000],
                [x, ceil_y, 3000],
            ],
        );
        triangle(
            ceil_type,
            [
                [x + 1000, ceil_y, -3000],
                [x + 1000, ceil_y, 3000],
                [x, ceil_y, 3000],
            ],
        );
    }
    // Four intersecting walls exercise last-wall reuse and query-radius clamps.
    for (surface, x) in [
        (0, -50),
        (0, 50),
        (c::SURFACE_CAMERA_BOUNDARY, 100),
        (c::SURFACE_NO_CAM_COLLISION, 150),
    ] {
        triangle(surface, [[x, -500, -700], [x, 1700, -700], [x, -500, 700]]);
        triangle(surface, [[x, -500, 700], [x, 1700, -700], [x, 1700, 700]]);
        triangle(surface, [[-700, -500, x], [-700, 1700, x], [700, -500, x]]);
        triangle(surface, [[700, -500, x], [-700, 1700, x], [700, 1700, x]]);
    }
    let mut stream = vec![0x40, vertices.len() as i16];
    stream.extend(vertices.into_iter().flatten());
    stream.extend(groups.into_iter().flatten());
    stream.extend([0x41, 0x44, 1, 0, -1200, -1800, 900, 1600, 500, 0x42]);
    stream
}

fn world(stream: &[i16]) -> CollisionWorld {
    let bytes: Vec<u8> = stream.iter().flat_map(|x| x.to_be_bytes()).collect();
    let (mesh, _) = collision::decode(&bytes).unwrap();
    CollisionWorld::load_area_terrain(&mesh).unwrap()
}
fn flag_words(flags: CollisionFlags) -> [u32; 2] {
    [
        flags.checking_for_camera.into(),
        flags.find_floor_include_surface_intangible.into(),
    ]
}
fn geometry_words(g: PlayerGeometry, flags: CollisionFlags) -> [u32; 9] {
    [
        g.floor.map_or(u32::MAX, u32::from),
        g.floor_height.to_bits(),
        g.floor_type as i32 as u32,
        g.ceil.map_or(u32::MAX, u32::from),
        g.ceil_height.to_bits(),
        g.ceil_type as i32 as u32,
        g.water_height.to_bits(),
        flags.checking_for_camera.into(),
        flags.find_floor_include_surface_intangible.into(),
    ]
}

#[test]
fn c_button_precedence_matches_all_combinations() {
    let trig = tables();
    let oracle = CameraOracle::new(&terrain(), &trig);
    for history in 0..16 {
        for pressed in 0..16 {
            for down in 0..16 {
                for high in [0, 0xa5f0] {
                    assert_eq!(
                        camera::find_c_buttons_pressed(
                            history | high,
                            pressed | 0x8000,
                            down | 0x8000
                        ),
                        oracle.buttons(history | high, pressed | 0x8000, down | 0x8000)
                    );
                }
            }
        }
    }
}

#[test]
fn angle_approaches_match_wraps_and_promotions() {
    let trig = tables();
    let oracle = CameraOracle::new(&terrain(), &trig);
    let mut rng = Rng(717);
    let boundaries = [i16::MIN, -32767, -16384, -1, 0, 1, 16384, 32766, i16::MAX];
    let amounts = [i16::MIN, -1024, -16, -1, 0, 1, 3, 16, 1024, i16::MAX];
    for raw in 0..=u16::MAX {
        let current = raw as i16;
        let target = if raw % 2 == 0 {
            boundaries[raw as usize % boundaries.len()]
        } else {
            rng.next() as i16
        };
        for &amount in &amounts {
            for status in [0, c::CAM_FLAG_SMOOTH_MOVEMENT] {
                for call in [
                    CameraAngle::AsymptoticBool,
                    CameraAngle::Asymptotic,
                    CameraAngle::SymmetricBool,
                    CameraAngle::Symmetric,
                    CameraAngle::SetOrApproach,
                ] {
                    let mut value = current;
                    let active = match call {
                        CameraAngle::AsymptoticBool => {
                            camera::approach_s16_asymptotic_bool(&mut value, target, amount)
                        }
                        CameraAngle::Asymptotic => {
                            value = camera::approach_s16_asymptotic(value, target, amount);
                            false
                        }
                        CameraAngle::SymmetricBool => {
                            camera::camera_approach_s16_symmetric_bool(&mut value, target, amount)
                        }
                        CameraAngle::Symmetric => {
                            value = camera::camera_approach_s16_symmetric(value, target, amount);
                            false
                        }
                        CameraAngle::SetOrApproach => camera::set_or_approach_s16_symmetric(
                            &mut value, target, amount, status,
                        ),
                    };
                    assert_eq!(
                        (value, active),
                        oracle.angle(call, current, target, amount, status),
                        "{call:?} current={current} target={target} amount={amount} status={status}"
                    );
                }
            }
        }
    }
    for &a in &boundaries {
        for &b in &boundaries {
            for &d in &amounts {
                let mut v = [a, b, d];
                let target = [b, d, a];
                let divisor = [d, a, b];
                camera::approach_vec3s_asymptotic(&mut v, target, divisor);
                assert_eq!(v, oracle.vec3s([a, b, d], target, divisor));
            }
        }
    }
}

#[test]
fn float_approaches_match_bits_and_return_values() {
    let trig = tables();
    let oracle = CameraOracle::new(&terrain(), &trig);
    let special = [
        0.0,
        -0.0,
        f32::from_bits(1),
        -f32::from_bits(1),
        -1000.0,
        1000.0,
        f32::MAX,
        -f32::MAX,
    ];
    let mut rng = Rng(901);
    let mut cases = vec![];
    for &a in &special {
        for &b in &special {
            for d in [-20.0, -1.0, -0.0, 0.0, 0.01, 0.3, 1.0, 1.5, 100.0] {
                cases.push((a, b, d));
            }
        }
    }
    for _ in 0..10000 {
        cases.push((rng.f(10000.0), rng.f(10000.0), rng.f(1.5)));
    }
    for (a, b, d) in cases {
        for status in [0, c::CAM_FLAG_SMOOTH_MOVEMENT] {
            for call in [
                CameraFloat::AsymptoticBool,
                CameraFloat::Asymptotic,
                CameraFloat::SetOrApproach,
                CameraFloat::SymmetricBool,
                CameraFloat::Symmetric,
            ] {
                let mut value = a;
                let active = match call {
                    CameraFloat::AsymptoticBool => {
                        camera::approach_f32_asymptotic_bool(&mut value, b, d)
                    }
                    CameraFloat::Asymptotic => {
                        value = camera::approach_f32_asymptotic(value, b, d);
                        false
                    }
                    CameraFloat::SetOrApproach => {
                        camera::set_or_approach_f32_asymptotic(&mut value, b, d, status)
                    }
                    CameraFloat::SymmetricBool => {
                        camera::camera_approach_f32_symmetric_bool(&mut value, b, d)
                    }
                    CameraFloat::Symmetric => {
                        value = camera::camera_approach_f32_symmetric(value, b, d);
                        false
                    }
                };
                let (native, busy) = oracle.float(call, a, b, d, status);
                assert_eq!(
                    (value.to_bits(), active),
                    (native.to_bits(), busy),
                    "{call:?} {a} {b} {d}"
                );
            }
        }
    }
}

#[test]
fn vectors_angles_and_strict_trigger_bounds_match() {
    let trig = tables();
    let oracle = CameraOracle::new(&terrain(), &trig);
    let mut rng = Rng(423);
    for i in 0..20000 {
        let a = rng.vec(10000.0);
        let b = if i % 5 == 0 { a } else { rng.vec(10000.0) };
        let mul = rng.vec(1.5);
        let angle = rng.next() as i16;
        let status = if i % 2 == 0 {
            0
        } else {
            c::CAM_FLAG_SMOOTH_MOVEMENT
        };
        let [pitch, yaw] = camera::calculate_angles(a, b, &trig);
        assert_eq!(
            [
                pitch as i32 as u32,
                yaw as i32 as u32,
                camera::calc_abs_dist(a, b).to_bits(),
                camera::calc_hor_dist(a, b).to_bits()
            ],
            oracle.vectors(CameraVector::AnglesAndDistances, a, b, mul, angle, status)
        );
        for call in [
            CameraVector::RotateXz,
            CameraVector::RotateYz,
            CameraVector::ScaleLine,
            CameraVector::Approach,
            CameraVector::SetOrApproach,
        ] {
            let mut v = a;
            match call {
                CameraVector::RotateXz => v = camera::rotate_in_xz(v, angle, &trig),
                CameraVector::RotateYz => v = camera::rotate_in_yz(v, angle, &trig),
                CameraVector::ScaleLine => v = camera::scale_along_line(v, b, mul[0]),
                CameraVector::Approach => camera::approach_vec3f_asymptotic(&mut v, b, mul),
                CameraVector::SetOrApproach => {
                    camera::set_or_approach_vec3f_asymptotic(&mut v, b, mul, status)
                }
                _ => unreachable!(),
            }
            assert_eq!(
                [v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), 0],
                oracle.vectors(call, a, b, mul, angle, status),
                "{call:?} case={i}"
            );
        }
        let mut v = a;
        let n = camera::clamp_pitch(b, &mut v, angle, status, &trig);
        assert_eq!(
            [v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), n as u32],
            oracle.vectors(CameraVector::ClampPitch, a, b, mul, angle, status)
        );
        for area in [
            c::AREA_BOB,
            c::AREA_WDW_MAIN,
            c::AREA_THI_HUGE,
            c::AREA_THI_TINY,
            -1,
        ] {
            let mut v = a;
            let yaw = camera::find_in_bounds_yaw_wdw_bob_thi(area, &mut v, b, angle, &trig);
            assert_eq!(
                [
                    v[0].to_bits(),
                    v[1].to_bits(),
                    v[2].to_bits(),
                    yaw as i32 as u32
                ],
                oracle.vectors(CameraVector::AreaBounds, a, b, mul, angle, area as i16)
            );
        }
        let bounds = [9000.0, 8500.0, 7500.0];
        let inside = camera::is_pos_in_bounds(a, b, bounds, angle, &trig);
        assert_eq!(
            u32::from(inside),
            oracle.vectors(CameraVector::InBounds, a, b, bounds, angle, status)[3]
        );
    }
    for yaw in [0, 0x4000, i16::MIN, -0x4000, 0x2134] {
        for axis in 0..3 {
            for edge in [-100.001, -100.0, -99.999, 0.0, 99.999, 100.0, 100.001] {
                let mut pos = [0.0; 3];
                pos[axis] = edge;
                let inside = camera::is_pos_in_bounds(pos, [0.0; 3], [100.0; 3], yaw, &trig);
                assert_eq!(
                    u32::from(inside),
                    oracle.vectors(CameraVector::InBounds, pos, [0.0; 3], [100.0; 3], yaw, 0)[3]
                );
            }
        }
    }
    // These assertions make boundary semantics visible independently of the C oracle.
    assert!(!camera::is_pos_in_bounds(
        [100.0, 0.0, 0.0],
        [0.0; 3],
        [100.0; 3],
        0,
        &trig
    ));
    assert!(camera::is_pos_in_bounds(
        [99.999, 0.0, 0.0],
        [0.0; 3],
        [100.0; 3],
        0,
        &trig
    ));
}

fn compare_terrain(stream: &[i16], trig: &TrigTables, n: usize) -> (i32, usize) {
    let world = world(stream);
    let oracle = CameraOracle::new(stream, trig);
    let mut rng = Rng(591);
    let mut wall_hits = 0;
    let mut type_height_difference = 0;
    for i in 0..n {
        let pos = if i % 3 == 0 {
            [rng.f(180.0), rng.f(1000.0) + 500.0, rng.f(180.0)]
        } else {
            [rng.f(9000.0), rng.f(1500.0), rng.f(9000.0)]
        };
        for for_camera in [false, true] {
            for intangible in [false, true] {
                let incoming = CollisionFlags {
                    checking_for_camera: for_camera,
                    find_floor_include_surface_intangible: intangible,
                };
                let mut flags = incoming;
                let g = camera::find_mario_floor_and_ceil(pos, &world, &mut flags);
                assert_eq!(
                    geometry_words(g, flags),
                    oracle.geometry(pos, incoming),
                    "geometry case={i} {pos:?}"
                );
                if let Some(floor) = g.floor
                    && world.surfaces()[floor as usize].surface_type != g.floor_type
                {
                    type_height_difference += 1;
                }
                let offset = [0.0, 10.0, -80.0, 160.0][i % 4];
                let radius = [0.0, 50.0, 100.0, 200.0, 250.0][i % 5];
                let mut corrected = pos;
                let hits = camera::collide_with_walls(
                    &mut corrected,
                    offset,
                    radius,
                    &world,
                    incoming,
                    false,
                );
                let (native, count, native_flags) = oracle.walls(pos, offset, radius, incoming);
                wall_hits += hits;
                assert_eq!(
                    (corrected.map(f32::to_bits), hits, flag_words(incoming)),
                    (native.map(f32::to_bits), count, native_flags),
                    "walls case={i}"
                );
                let mut flags = incoming;
                let mut resolved = pos;
                camera::resolve_geometry_collisions(&mut resolved, &world, &mut flags, false);
                let (native, native_flags) = oracle.resolve_geometry(pos, incoming);
                assert_eq!(
                    (resolved.map(f32::to_bits), flag_words(flags)),
                    (native.map(f32::to_bits), native_flags),
                    "resolve case={i}"
                );
                let state = RadialState {
                    area: [c::AREA_BOB, c::AREA_THI_TINY, -1][i % 3],
                    center_x: rng.f(2500.0),
                    center_z: rng.f(2500.0),
                    mode_offset_yaw: rng.next() as i16,
                    lakitu_dist: (rng.next() % 700) as i16,
                    lakitu_pitch: (rng.next() % 4096) as i16,
                    area_yaw: 0,
                };
                let action = if i % 2 == 0 {
                    c::ACT_IDLE
                } else {
                    c::ACT_FLAG_METAL_WATER
                };
                let native = oracle.radial(state, pos, action, g, incoming);
                let mut rust = state;
                let mut flags = incoming;
                let goal = camera::update_radial_camera(
                    &mut rust, pos, action, g, &world, &mut flags, trig,
                );
                assert_eq!(
                    [
                        goal.focus[0].to_bits(),
                        goal.focus[1].to_bits(),
                        goal.focus[2].to_bits(),
                        goal.pos[0].to_bits(),
                        goal.pos[1].to_bits(),
                        goal.pos[2].to_bits(),
                        goal.yaw as i32 as u32,
                        rust.area_yaw as i32 as u32,
                        flags.checking_for_camera.into(),
                        flags.find_floor_include_surface_intangible.into()
                    ],
                    native,
                    "radial case={i}"
                );
            }
        }
    }
    eprintln!(
        "{n} positions x 4 flag combinations: geometry, walls, resolution, radial goals exact"
    );
    (wall_hits, type_height_difference)
}

#[test]
fn authored_geometry_and_radial_goals_match() {
    let (wall_hits, type_height_difference) = compare_terrain(&terrain(), &tables(), 10000);
    assert!(wall_hits > 0, "fixture did not hit walls");
    assert!(
        type_height_difference > 0,
        "fixture did not exercise filtered types"
    );
}

#[test]
#[should_panic(expected = "retained intangible height")]
fn missing_contact_with_intangible_height_is_an_explicit_boundary() {
    let stream = [
        0x40,
        3,
        -500,
        0,
        -500,
        -500,
        0,
        500,
        500,
        0,
        -500,
        c::SURFACE_INTANGIBLE,
        1,
        0,
        1,
        2,
        0x41,
        0x42,
    ];
    let world = world(&stream);
    let mut flags = CollisionFlags::default();
    // The native function dereferences NULL here; do not invoke it.
    camera::find_mario_floor_and_ceil([0.0, 10.0, 0.0], &world, &mut flags);
}

#[test]
#[should_panic(expected = "require the object runtime")]
fn pole_height_offsets_require_objects() {
    let world = world(&terrain());
    camera::calc_y_to_curr_floor(
        [0.0; 3],
        c::ACT_FLAG_ON_POLE,
        PlayerGeometry::default(),
        1.0,
        200.0,
        0.9,
        200.0,
        &world,
    );
}

/// A persistent helper sequence on an authored Mario path. Native scalar and
/// vector state persists independently. This is deliberately NOT update_camera.
#[test]
fn chained_camera_components_match_at_all_presentation_rates() {
    let trig = tables();
    let stream = terrain();
    let world = world(&stream);
    let oracle = CameraOracle::new(&stream, &trig);
    let mut baseline = None;
    for hz in [15, 30, 60, 120, 144] {
        for interpolate in [false, true] {
            let mut clock = FixedClock::default();
            let mut frame = 0u64;
            let mut time = 0;
            let mut rust_pos = [0.0; 3];
            let mut native_pos = [0.0; 3];
            let mut rust_yaw = 0i16;
            let mut native_yaw = 0i16;
            let mut rust_buttons = 0;
            let mut native_buttons = 0;
            let mut records = Vec::new();
            while records.len() < 3600 {
                frame += 1;
                let next = frame * 1_000_000_000 / hz;
                clock
                    .add_elapsed(Duration::from_nanos(next - time))
                    .unwrap();
                time = next;
                for _ in 0..clock.drain(8) {
                    if records.len() == 3600 {
                        break;
                    }
                    let tick = records.len() as i32;
                    let pos = [
                        ((tick * 7) % 7000 - 3500) as f32,
                        ((tick * 11) % 1600 - 300) as f32,
                        ((tick * 13) % 7000 - 3500) as f32,
                    ];
                    let mut flags = CollisionFlags::default();
                    let g = camera::find_mario_floor_and_ceil(pos, &world, &mut flags);
                    let mut state = RadialState {
                        area: c::AREA_BOB,
                        mode_offset_yaw: (tick * 83) as i16,
                        lakitu_dist: (tick % 700) as i16,
                        ..RadialState::default()
                    };
                    let native_goal = oracle.radial(state, pos, c::ACT_IDLE, g, flags);
                    let goal = camera::update_radial_camera(
                        &mut state,
                        pos,
                        c::ACT_IDLE,
                        g,
                        &world,
                        &mut flags,
                        &trig,
                    );
                    let target = std::array::from_fn(|i| f32::from_bits(native_goal[3 + i]));
                    let status = if tick % 97 == 0 {
                        0
                    } else {
                        c::CAM_FLAG_SMOOTH_MOVEMENT
                    };
                    let previous = rust_pos;
                    camera::set_or_approach_vec3f_asymptotic(
                        &mut rust_pos,
                        goal.pos,
                        [0.3, 0.3, 0.3],
                        status,
                    );
                    let native = oracle.vectors(
                        CameraVector::SetOrApproach,
                        native_pos,
                        target,
                        [0.3; 3],
                        0,
                        status,
                    );
                    native_pos = std::array::from_fn(|i| f32::from_bits(native[i]));
                    camera::set_or_approach_s16_symmetric(&mut rust_yaw, goal.yaw, 0x400, status);
                    native_yaw = oracle
                        .angle(
                            CameraAngle::SetOrApproach,
                            native_yaw,
                            native_goal[6] as i16,
                            0x400,
                            status,
                        )
                        .0;
                    let pressed = if tick % 9 == 0 { (tick % 16) as u16 } else { 0 };
                    let down = (tick % 16) as u16;
                    rust_buttons = camera::find_c_buttons_pressed(rust_buttons, pressed, down);
                    native_buttons = oracle.buttons(native_buttons, pressed, down);
                    let record = (rust_pos.map(f32::to_bits), rust_yaw, rust_buttons);
                    assert_eq!(
                        record,
                        (native_pos.map(f32::to_bits), native_yaw, native_buttons),
                        "helper tick={tick} hz={hz}"
                    );
                    records.push(record);
                    // Presentation consumes copies; it cannot feed interpolation into helpers.
                    let alpha = if interpolate { clock.alpha() } else { 1.0 };
                    let displayed = std::array::from_fn::<_, 3, _>(|i| {
                        previous[i] + (rust_pos[i] - previous[i]) * alpha
                    });
                    std::hint::black_box(displayed);
                }
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
#[ignore = "requires RUSTARIO64_ROM; private original BOB collision and trig tables"]
fn bob_camera_components_match_with_rom_tables() {
    use rustario64::import::{bob, engine, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        rom.reader().slice(range.start, range.len()).unwrap(),
        version::MAX_SEGMENT_BYTES,
    )
    .unwrap();
    let start = (version::BOB_COLLISION & 0xFFFFFF) as usize;
    let (mesh, consumed) = collision::decode(&terrain[start..]).unwrap();
    assert_eq!(mesh, bob::import(&rom).unwrap().collision);
    let stream: Vec<i16> = terrain[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes([b[0], b[1]]))
        .collect();
    let trig = engine::trig_tables(&rom).unwrap();
    compare_terrain(&stream, &trig, 20000);
}
