//! Differential input-stage checks. Ordinary fixtures are independently authored.
use rustario64::{
    import::collision,
    presentation::GraphicsOptions,
    simulation::{
        TickInput,
        collision::{CollisionFlags, CollisionWorld},
        controller::{A_BUTTON, B_BUTTON, Controller, Z_TRIG},
        mario::{
            MarioState, StepWorld, constants as c,
            inputs::{self, InputContext, InputOutcome},
        },
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
    },
    trace,
};
use rustario64_oracle::{
    InputCall, Oracle, OracleInput, OracleMario,
    input_trace::{Replay, snapshot, world_digest},
};

fn computed_tables() -> TrigTables {
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
fn world(stream: &[i16]) -> CollisionWorld {
    let bytes: Vec<u8> = stream.iter().flat_map(|w| w.to_be_bytes()).collect();
    let (mesh, consumed) = collision::decode(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    CollisionWorld::load_area_terrain(&mesh).unwrap()
}
fn step_world<'a>(world: &'a CollisionWorld, trig: &'a TrigTables) -> StepWorld<'a> {
    StepWorld {
        collision: world,
        collision_flags: CollisionFlags::default(),
        trig,
        global_timer: 0,
        area_terrain_type: 0,
        level_num: c::LEVEL_BOB,
        water_pseudo_floor_origin_offset: 0.0,
    }
}
/// Sixteen sloped tiles, ceiling, two-sided wall, and overlapping water/gas.
fn authored_stream() -> Vec<i16> {
    let kinds = [
        c::SURFACE_DEFAULT,
        c::SURFACE_VERY_SLIPPERY,
        c::SURFACE_SLIPPERY,
        c::SURFACE_NOT_SLIPPERY,
        c::SURFACE_HARD_NOT_SLIPPERY,
        c::SURFACE_SWITCH,
        c::SURFACE_NOISE_SLIPPERY,
        c::SURFACE_HARD_SLIPPERY,
        c::SURFACE_NO_CAM_COL_SLIPPERY,
        c::SURFACE_ICE,
        c::SURFACE_HARD_VERY_SLIPPERY,
        c::SURFACE_NOISE_VERY_SLIPPERY_73,
        c::SURFACE_NOISE_VERY_SLIPPERY_74,
        c::SURFACE_NOISE_VERY_SLIPPERY,
        c::SURFACE_NO_CAM_COL_VERY_SLIPPERY,
        c::SURFACE_DEFAULT,
    ];
    let mut vertices = vec![];
    let mut groups = vec![];
    for (i, kind) in kinds.into_iter().enumerate() {
        let x = -6000 + (i as i16 % 4) * 3000;
        let z = -6000 + (i as i16 / 4) * 3000;
        let rise = [0, 500, 1200, 2800][i % 4];
        let base = vertices.len() as i16;
        vertices.extend([
            [x, 0, z],
            [x, 0, z + 2900],
            [x + 2900, rise, z],
            [x + 2900, rise, z + 2900],
        ]);
        groups.push((
            kind,
            vec![[base, base + 1, base + 2], [base + 2, base + 1, base + 3]],
        ));
    }
    let base = vertices.len() as i16;
    vertices.extend([
        [-5500, 150, -5500],
        [-3500, 150, -5500],
        [-5500, 150, -3500],
    ]);
    groups.push((0, vec![[base, base + 1, base + 2]]));
    let base = vertices.len() as i16;
    vertices.extend([
        [-5000, 0, -5800],
        [-5000, 2000, -5800],
        [-5000, 0, -3200],
        [-5000, 2000, -3200],
    ]);
    groups.push((
        0,
        vec![
            [base, base + 1, base + 2],
            [base + 1, base + 3, base + 2],
            [base, base + 2, base + 1],
            [base + 1, base + 2, base + 3],
        ],
    ));
    let mut stream = vec![0x40, vertices.len() as i16];
    stream.extend(vertices.into_iter().flatten());
    for (kind, tris) in groups {
        stream.extend([kind, tris.len() as i16]);
        stream.extend(tris.into_iter().flatten());
    }
    stream.extend([
        0x41, 0x44, 2, 0, -6000, -6000, -3000, 6000, 400, 50, -6000, -6000, 6000, -3000, 600, 0x42,
    ]);
    stream
}
fn controller_matches(ours: &Controller, reference: &OracleInput) {
    assert_eq!(
        ours.stick_x.to_bits(),
        reference.stick_x.to_bits(),
        "stick_x"
    );
    assert_eq!(
        ours.stick_y.to_bits(),
        reference.stick_y.to_bits(),
        "stick_y"
    );
    assert_eq!(
        ours.stick_mag.to_bits(),
        reference.stick_mag.to_bits(),
        "stick_mag"
    );
    assert_eq!(ours.button_down, reference.button_down, "button_down");
    assert_eq!(
        ours.button_pressed, reference.button_pressed,
        "button_pressed"
    );
}
fn exhaustive_controls(trig: &TrigTables) {
    let stream = [0x40, 0, 0x41, 0x42];
    let world = world(&stream);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let w = step_world(&world, trig);
    let mut checks = 0;
    for (squish, camera_yaw) in [(0, 0), (1, 32767), (255, -32768)] {
        let mut controller = Controller {
            button_down: A_BUTTON,
            ..Default::default()
        };
        for x in -128i16..=127 {
            for y in -128i16..=127 {
                let input = TickInput {
                    buttons: (x as u16).wrapping_mul(257) ^ y as u16,
                    stick: [x as i8, y as i8],
                    camera_yaw,
                };
                let mut m = MarioState {
                    squish_timer: squish,
                    frames_since_a: 254,
                    frames_since_b: 255,
                    face_angle: [0, -12345, 0],
                    ..Default::default()
                };
                let context = InputContext {
                    camera_yaw,
                    ..Default::default()
                };
                let mut reference = OracleMario::capture(&m, &w);
                let mut reference_input =
                    OracleInput::capture(&controller, &context, input, InputOutcome::Continue);
                oracle.input_tick(
                    &mut reference,
                    &mut reference_input,
                    InputCall::ButtonsAndJoystick,
                );
                controller.sample(input);
                inputs::update_mario_button_inputs(&mut m, &controller);
                inputs::update_mario_joystick_inputs(&mut m, &w, &controller, camera_yaw);
                controller_matches(&controller, &reference_input);
                assert_eq!(
                    m.intended_mag.to_bits(),
                    reference.intended_mag.to_bits(),
                    "{input:?}"
                );
                assert_eq!(m.intended_yaw, reference.intended_yaw, "{input:?}");
                assert_eq!(
                    (m.input, m.frames_since_a, m.frames_since_b),
                    (
                        reference.input,
                        reference.frames_since_a,
                        reference.frames_since_b
                    ),
                    "{input:?}"
                );
                checks += 1;
            }
        }
    }
    println!(
        "{checks} exact controller/button/joystick comparisons (all 65,536 raw stick pairs x 3 contexts)"
    );
}
#[test]
fn all_raw_stick_pairs_and_button_edges_match_the_decomp() {
    exhaustive_controls(&computed_tables());
}

#[derive(Default, Debug)]
struct Coverage {
    checks: usize,
    flags: u16,
    moved: usize,
    fallback: usize,
    deaths: usize,
    ceilings: usize,
}
fn geometry_cases(
    world: &CollisionWorld,
    trig: &TrigTables,
    oracle: &Oracle,
    count: usize,
) -> Coverage {
    let mut seed = 0x7265736561726368u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 32) as u32
    };
    let mut coverage = Coverage::default();
    for i in 0..count {
        let p = [
            (next() % 17000) as f32 - 8500.0,
            (next() % 4000) as f32 - 500.0,
            (next() % 17000) as f32 - 8500.0,
        ];
        let mut m = MarioState {
            action: if i % 3 == 0 {
                c::ACT_CRAWLING
            } else if i % 3 == 1 {
                c::ACT_IDLE
            } else {
                c::ACT_FREEFALL
            },
            pos: p,
            gfx_pos: if i % 5 == 0 {
                p
            } else {
                [-5400.0, 0.0, -5400.0]
            },
            input: next() as u16,
            flags: next(),
            particle_flags: next(),
            collided_obj_interact_types: next(),
            intended_mag: -0.0,
            intended_yaw: next() as i16,
            frames_since_a: next() as u8,
            frames_since_b: next() as u8,
            wall_kick_timer: (i % 256) as u8,
            double_jump_timer: next() as u8,
            squish_timer: if i % 2 == 0 { 0 } else { 3 },
            face_angle: [0, next() as i16, 0],
            ..Default::default()
        };
        let mut w = step_world(world, trig);
        w.area_terrain_type = (i % 7) as u16;
        w.collision_flags.find_floor_include_surface_intangible = i % 2 == 0;
        let mut context = InputContext {
            camera_yaw: next() as i16,
            camera_movement_flags: next() as u16,
            object_interact_status: next(),
            object_collided_interact_types: next(),
        };
        let input = TickInput {
            buttons: next() as u16,
            stick: [next() as i8, next() as i8],
            camera_yaw: context.camera_yaw,
        };
        let mut controller = Controller {
            button_down: next() as u16,
            ..Default::default()
        };
        let mut reference = OracleMario::capture(&m, &w);
        let mut reference_input =
            OracleInput::capture(&controller, &context, input, InputOutcome::Continue);
        oracle.input_tick(&mut reference, &mut reference_input, InputCall::Full);
        controller.sample(input);
        let outcome = inputs::update_mario_inputs(&mut m, &mut w, &controller, &mut context);
        let actual = snapshot(
            &OracleMario::capture(&m, &w),
            &OracleInput::capture(&controller, &context, input, outcome),
        );
        assert_eq!(
            snapshot(&reference, &reference_input),
            actual,
            "geometry case {i}, start {p:?}"
        );
        coverage.checks += 1;
        coverage.flags |= m.input;
        coverage.moved += usize::from(m.pos != p);
        coverage.fallback += usize::from(m.pos != p && m.pos == m.gfx_pos);
        coverage.deaths += usize::from(outcome == InputOutcome::DeathWarpRequested);
        coverage.ceilings += usize::from(m.ceil.is_some());
    }
    coverage
}
#[test]
fn authored_geometry_input_stage_matches_every_recorded_field() {
    let stream = authored_stream();
    let world = world(&stream);
    let trig = computed_tables();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let coverage = geometry_cases(&world, &trig, &oracle, 10_000);
    println!("Authored input-stage coverage: {coverage:?}");
    let required = c::INPUT_OFF_FLOOR
        | c::INPUT_ABOVE_SLIDE
        | c::INPUT_IN_WATER
        | c::INPUT_IN_POISON_GAS
        | c::INPUT_FIRST_PERSON
        | c::INPUT_STOMPED;
    assert_eq!(coverage.flags & required, required);
    assert!(
        coverage.moved > coverage.fallback
            && coverage.fallback > 0
            && coverage.deaths > 0
            && coverage.ceilings > 0
    );
}
#[test]
fn geometry_thresholds_keep_strict_floor_water_and_gas_bounds() {
    let stream = authored_stream();
    let world = world(&stream);
    let trig = computed_tables();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    for (height, flag, set) in [
        (99.5, c::INPUT_OFF_FLOOR, false),
        (100.0, c::INPUT_OFF_FLOOR, false),
        (100.5, c::INPUT_OFF_FLOOR, true),
        (389.5, c::INPUT_IN_WATER, true),
        (390.0, c::INPUT_IN_WATER, false),
        (390.5, c::INPUT_IN_WATER, false),
        (499.5, c::INPUT_IN_POISON_GAS, true),
        (500.0, c::INPUT_IN_POISON_GAS, false),
        (500.5, c::INPUT_IN_POISON_GAS, false),
    ] {
        let mut m = MarioState {
            action: c::ACT_IDLE,
            pos: [-5400.0, height, -3400.0],
            gfx_pos: [-5400.0, height, -3400.0],
            ..Default::default()
        };
        let mut w = step_world(&world, &trig);
        let mut context = InputContext::default();
        let controller = Controller::default();
        let input = TickInput::default();
        let mut reference = OracleMario::capture(&m, &w);
        let mut reference_input =
            OracleInput::capture(&controller, &context, input, InputOutcome::Continue);
        oracle.input_tick(&mut reference, &mut reference_input, InputCall::Full);
        let outcome = inputs::update_mario_inputs(&mut m, &mut w, &controller, &mut context);
        assert_eq!(outcome, InputOutcome::Continue);
        assert_eq!(m.input & flag != 0, set, "height {height} flag {flag:#x}");
        assert_eq!(
            snapshot(&reference, &reference_input),
            snapshot(
                &OracleMario::capture(&m, &w),
                &OracleInput::capture(&controller, &context, input, outcome)
            )
        );
    }
}
fn samples() -> Vec<TickInput> {
    (0..1200)
        .map(|i| TickInput {
            buttons: match i % 300 {
                0..=19 => A_BUTTON | B_BUTTON | Z_TRIG,
                20..=22 => 0,
                23..=45 => A_BUTTON,
                _ => 0,
            },
            stick: if i % 137 < 12 {
                [0, 0]
            } else {
                [(i * 17) as i8, (i * 37) as i8]
            },
            camera_yaw: (i * 419) as i16,
        })
        .collect()
}
#[test]
fn chained_input_traces_match_at_multiple_render_rates_and_settings() {
    let stream = authored_stream();
    let world = world(&stream);
    let trig = computed_tables();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let replay = Replay {
        collision: &world,
        trig: &trig,
        initial: MarioState {
            action: c::ACT_IDLE,
            pos: [-5400.0, 0.0, -5400.0],
            gfx_pos: [-5400.0, 0.0, -5400.0],
            frames_since_a: 255,
            frames_since_b: 255,
            wall_kick_timer: 5,
            double_jump_timer: 255,
            ..Default::default()
        },
        controller: Controller {
            button_down: A_BUTTON,
            ..Default::default()
        },
        context: InputContext {
            camera_movement_flags: c::CAM_MOVE_C_UP_MODE,
            ..Default::default()
        },
        area_terrain_type: 0,
        level_num: c::LEVEL_BOB,
        rom_sha1: "synthetic".into(),
        world_digest: world_digest(&stream, &trig),
        scenario: "authored-input-stage-40s".into(),
    };
    let samples = samples();
    let reference = replay
        .run(&samples, 30, GraphicsOptions::default(), Some(&oracle))
        .unwrap();
    for render_hz in [15, 30, 60, 120, 144] {
        for interpolation in [false, true] {
            let candidate = replay
                .run(
                    &samples,
                    render_hz,
                    GraphicsOptions {
                        interpolation,
                        enhanced_lighting: interpolation,
                        dynamic_shadows: interpolation,
                    },
                    None,
                )
                .unwrap();
            trace::compare(&reference, &candidate)
                .unwrap_or_else(|e| panic!("{render_hz} Hz: {e}"));
        }
    }
    assert_eq!(
        reference.frames[0].state.fields["button_pressed"] & u32::from(A_BUTTON),
        0
    );
    assert_eq!(reference.frames[260].state.fields["double_jump_timer"], 0);
    assert_eq!(reference.frames[299].state.fields["frames_since_a"], 255);
    let mut changed = reference.clone();
    *changed.frames[25]
        .state
        .fields
        .get_mut("intended_mag")
        .unwrap() ^= 1;
    let error = trace::compare(&reference, &changed).unwrap_err();
    assert_eq!(error.tick, Some(26));
    assert!(error.field.ends_with("intended_mag"));
}
#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn bob_input_stage_matches_with_rom_tables() {
    use rustario64::import::{bob, engine, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    exhaustive_controls(&trig);
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
    let world = CollisionWorld::load_area_terrain(&mesh).unwrap();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let coverage = geometry_cases(&world, &trig, &oracle, 20_000);
    println!("BOB input-stage coverage: {coverage:?}");
    assert!(coverage.moved > 0 && coverage.ceilings > 0 && coverage.deaths > 0);
    let (_, yaw_degrees, position) = imported.level.mario_start.unwrap();
    let replay = Replay {
        collision: &world,
        trig: &trig,
        initial: MarioState {
            action: c::ACT_IDLE,
            pos: position.map(f32::from),
            gfx_pos: position.map(f32::from),
            // level_cmd_set_mario_start_pos converts raw script degrees.
            face_angle: [0, (i32::from(yaw_degrees) * 0x8000 / 180) as i16, 0],
            frames_since_a: 255,
            frames_since_b: 255,
            ..Default::default()
        },
        controller: Controller::default(),
        context: InputContext::default(),
        area_terrain_type: imported.level.areas[0].terrain_type,
        level_num: c::LEVEL_BOB,
        rom_sha1: rom.fingerprint().into(),
        world_digest: world_digest(&stream, &trig),
        scenario: "bob-script-start-input-stage-40s-no-spawn-or-actions".into(),
    };
    let reference = replay
        .run(&samples(), 30, GraphicsOptions::default(), Some(&oracle))
        .unwrap();
    for hz in [30, 60, 120, 144] {
        trace::compare(
            &reference,
            &replay
                .run(&samples(), hz, GraphicsOptions::default(), None)
                .unwrap(),
        )
        .unwrap();
    }
    println!(
        "BOB: 1,200 chained input-stage ticks identical at 30/60/120/144 Hz; actions not executed"
    );
}
