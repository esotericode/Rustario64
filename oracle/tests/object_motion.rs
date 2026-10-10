//! Bitwise object_step comparisons: authored geometry in CI, and local BOB
//! collision/trig from the owner's ROM. These are motion component calls,
//! not complete actor/frame comparisons or mission-completion evidence.
#[path = "support/playground.rs"]
mod playground;

use playground::{Builder, Lcg, computed_tables, world};
use rustario64::{
    content::animation::MarioAnimations,
    simulation::{
        collision::{CollisionFlags, CollisionWorld},
        mario::{StepWorld, constants::*},
        math::TrigTables,
        object::{Object, motion::*},
    },
};
use rustario64_oracle::Oracle;

static NO_ANIMATIONS: MarioAnimations = MarioAnimations {
    animations: Vec::new(),
};

fn stream() -> Vec<i16> {
    let mut b = Builder::default();
    b.flat(SURFACE_DEFAULT, [-7000, 7000], [-7000, 7000], -300, true);
    b.flat(SURFACE_DEFAULT, [-3000, 0], [-3000, 3000], 0, true);
    b.ramp(SURFACE_DEFAULT, [1000, 2000], [-2000, 2000], [0, 1000]);
    // The last two panels have normal.y below 0.5 and below 0.2.
    b.ramp(SURFACE_DEFAULT, [3000, 4000], [-1000, 1000], [0, 4000]);
    b.ramp(SURFACE_DEFAULT, [4500, 5500], [-500, 500], [0, 7000]);
    b.flat(SURFACE_INTANGIBLE, [-1500, -500], [1000, 2000], 300, true);
    b.block(SURFACE_DEFAULT, [-1500, -1000], [-1000, -500], [0, 600]);
    for (x, surface) in [
        (600, SURFACE_VANISH_CAP_WALLS),
        (650, SURFACE_DEFAULT),
        (-2500, SURFACE_NO_CAM_COLLISION),
    ] {
        b.quad(
            surface,
            [
                [x, -300, -2000],
                [x, 3000, -2000],
                [x, -300, 2000],
                [x, 3000, 2000],
            ],
            [-1, 0, 0],
            None,
        );
    }
    let mut s = b.stream();
    s.pop(); // Replace terrain end with environment boxes followed by terrain end.
    s.extend([
        0x44, 2, 0, -3000, -3000, 0, 3000, 1000, 50, 1000, -3000, 2000, 3000, 1400, 0x42,
    ]);
    s
}

fn object(pos: [f32; 3]) -> Object {
    let mut o = Object {
        hitbox_radius: 65.0,
        hitbox_height: 113.0,
        ..Default::default()
    };
    o.active_flags = ACTIVE_FLAG_ACTIVE;
    for (field, value) in [
        (O_POS_X, pos[0]),
        (O_POS_Y, pos[1]),
        (O_POS_Z, pos[2]),
        (O_GRAVITY, 2.5),
        (O_FRICTION, 0.8),
        (O_BUOYANCY, 1.3),
    ] {
        o.raw.set_f32(field, value);
    }
    o
}

fn compare(
    oracle: &Oracle,
    o: &mut Object,
    w: &mut StepWorld<'_>,
    options: ObjectStepOptions,
    case: usize,
) -> ObjectStepResult {
    let before = o.clone();
    let mut native = before.clone();
    let mut native_flags = w.collision_flags;
    let expected = oracle.object_step(&mut native, &mut native_flags, w.global_timer, options);
    let actual = object_step_with_options(o, w, options);
    for field in 0..0x50 {
        assert_eq!(
            o.raw.u32(field),
            native.raw.u32(field),
            "case {case}, raw[{field:#x}], before {before:?}, result {actual:?}"
        );
    }
    assert_eq!(
        o, &native,
        "case {case}: motion must leave other object fields alone"
    );
    assert_eq!(w.collision_flags, native_flags, "case {case}: query flags");
    assert_eq!(
        actual.collision_flags, expected.collision_flags,
        "case {case}: movement flags"
    );
    assert_eq!(actual.floor, expected.floor, "case {case}: sObjFloor");
    assert_eq!(
        actual.effects, expected.effects,
        "case {case}: ordered splash requests"
    );
    let bits = |m: Option<[[f32; 4]; 4]>| m.map(|m| m.map(|row| row.map(f32::to_bits)));
    assert_eq!(
        bits(actual.terrain_matrix),
        bits(expected.terrain_matrix),
        "case {case}: terrain matrix, before {before:?}"
    );
    actual
}

#[test]
fn object_step_boundaries_match_the_original() {
    let stream = stream();
    let collision = world(&stream);
    let trig = computed_tables();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut w = StepWorld::new(&collision, &trig, &NO_ANIMATIONS);
    let mut cases = 0;
    let mut observed = [false; 4]; // orientation, wave, bubble, sound
    for y in [
        -5.9, 0.0, 0.9, 36.9, 37.0, 38.0, 970.0, 999.9, 1029.9, 1100.0,
    ] {
        for vy in [
            -200.0, -20.0, -15.000001, -15.0, -14.999999, -0.000001, -0.0, 0.000001, 30.0, 200.0,
        ] {
            for speed in [0.0, 0.000001, 12.5, 12.500001, 20.0] {
                for buoyancy in [0.5, 1.0, 1.3] {
                    let mut o = object([-2000.0, y, 0.0]);
                    o.raw.set_f32(O_VEL_Y, vy);
                    o.raw.set_f32(O_FORWARD_VEL, speed);
                    o.raw.set_f32(O_BUOYANCY, buoyancy);
                    w.global_timer = 32;
                    let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
                    observed[0] |= result.terrain_matrix.is_some();
                    for effect in result.effects {
                        match effect {
                            ObjectStepEffect::WaterWave => observed[1] = true,
                            ObjectStepEffect::SmallBubble => observed[2] = true,
                            ObjectStepEffect::Sound(_) => observed[3] = true,
                        }
                    }
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(
        observed, [true; 4],
        "water and matrix boundaries must be exercised"
    );
    // On a dry floor, matrix suppression changes only the returned matrix.
    let mut original = object([0.0, -300.0, 4000.0]);
    original.raw.set_f32(O_FORWARD_VEL, 20.0);
    original.raw.set_s32(O_FACE_ANGLE_YAW, 0x12345);
    original.raw.set_f32(O_GRAPH_Y_OFFSET, 35.0);
    let mut normal = original.clone();
    let result = compare(&oracle, &mut normal, &mut w, Default::default(), cases);
    assert!(result.terrain_matrix.is_some());
    cases += 1;
    for (options, billboard) in [
        (
            ObjectStepOptions {
                orient_with_floor: false,
                ..Default::default()
            },
            false,
        ),
        (
            ObjectStepOptions {
                terrain_matrix_available: false,
                ..Default::default()
            },
            false,
        ),
        (ObjectStepOptions::default(), true),
    ] {
        let mut o = original.clone();
        if billboard {
            o.gfx.node_flags |= GRAPH_RENDER_BILLBOARD;
        }
        let result = compare(&oracle, &mut o, &mut w, options, cases);
        assert!(result.terrain_matrix.is_none());
        assert_eq!(o.raw, normal.raw);
        cases += 1;
    }
    // Preserve the no-floor double addition even for negative integer yaws.
    for yaw in [-65536, -32768, -1, 0, 1, 32767, 65536] {
        let mut o = object([8000.0, 500.0, 8000.0]);
        o.raw.set_s32(O_MOVE_ANGLE_YAW, yaw);
        let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
        assert!(result.floor.is_none());
        assert_eq!(
            result.collision_flags,
            OBJ_COL_FLAG_HIT_WALL | OBJ_COL_FLAG_NO_Y_VEL
        );
        cases += 1;
    }
    // Strict bounce/grounded flags and close-floor integer thresholds on dry ground.
    for y in [-301.9, -300.0, -299.1, -263.1, -263.0, -262.1] {
        for vy in [
            -15.000001, -15.0, -14.999999, -0.000001, -0.0, 0.000001, 200.0,
        ] {
            let mut o = object([0.0, y, 4000.0]);
            o.raw.set_f32(O_VEL_Y, vy);
            compare(&oracle, &mut o, &mut w, Default::default(), cases);
            cases += 1;
        }
    }
    // Two f32 ULPs below -15 survive subtraction at -17.5; one ULP rounds
    // back onto the cutoff (already compared in the broader grid above).
    for vy in [-15.000002, -15.0, -14.999999] {
        let mut o = object([0.0, -300.0, 4000.0]);
        o.raw.set_f32(O_VEL_Y, vy);
        let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
        assert_eq!(o.raw.f32(O_VEL_Y) > 0.0, vy < -15.0);
        assert_eq!(
            result.collision_flags,
            if vy < -15.0 {
                OBJ_COL_FLAG_GROUNDED
            } else {
                OBJ_COL_FLAGS_LANDED
            }
        );
        cases += 1;
    }
    for (floor, pos, heights) in [
        (
            -300.0,
            [0.0, 0.0, 4000.0],
            [-300.0, -299.9, -264.0, -263.9, -263.0],
        ),
        (0.0, [-2000.0, 0.0, 0.0], [0.0, 0.9, 36.9, 37.0, 37.1]),
    ] {
        for y in heights {
            let mut o = object([pos[0], y, pos[2]]);
            o.raw.set_f32(O_GRAVITY, 0.0);
            o.raw.set_f32(O_BUOYANCY, 1.0);
            let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
            assert_eq!(
                result.terrain_matrix.is_some(),
                (y as i32) < floor as i32 + 37
            );
            cases += 1;
        }
    }
    // Steep floor approached from below: treat it as a wall, leave Y alone.
    for (x, z, y) in [(3500.0, 0.0, 1950.0), (5000.0, 0.0, 3480.0)] {
        let mut o = object([x, y, z]);
        o.raw.set_f32(O_FORWARD_VEL, 5.0);
        let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
        assert!(result.floor.is_some());
        assert!(result.collision_flags & OBJ_COL_FLAG_HIT_WALL != 0);
        assert_eq!(o.raw.f32(O_POS_Y), y);
        assert!(result.terrain_matrix.is_none());
        cases += 1;
    }
    // On the very steep panel from above, friction either zeroes motion or
    // keeps it according to the original double 0.9999 threshold.
    for friction in [0.8, 0.9998999, 0.9999, 1.0] {
        let mut o = object([5000.0, 3500.0, 0.0]);
        o.raw.set_f32(O_FRICTION, friction);
        let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
        assert!(result.terrain_matrix.is_some());
        assert_eq!(
            o.raw.f32(O_FORWARD_VEL) == 0.0,
            f64::from(friction) < 0.9999
        );
        cases += 1;
    }
    // Two parallel walls lie inside the radius. Passing the grate changes
    // the push without skipping the ordinary wall.
    for grate in [false, true] {
        let mut o = object([550.0, 100.0, 0.0]);
        o.raw.set_f32(O_FORWARD_VEL, 20.0);
        o.raw.set_s32(O_MOVE_ANGLE_YAW, 0x4000);
        o.hitbox_radius = 100.0;
        if grate {
            o.active_flags |= ACTIVE_FLAG_MOVE_THROUGH_GRATE;
        }
        let result = compare(&oracle, &mut o, &mut w, Default::default(), cases);
        assert!(result.collision_flags & OBJ_COL_FLAG_HIT_WALL != 0);
        cases += 1;
    }
    eprintln!("{cases} targeted original object_step comparisons");
}

fn random_object(rng: &mut Lcg, collision: &CollisionWorld) -> Object {
    let fraction = |rng: &mut Lcg| rng.range(-99, 99) as f32 / 100.0;
    let x = rng.range(-8300, 8300) as f32 + fraction(rng);
    let z = rng.range(-8300, 8300) as f32 + fraction(rng);
    let (floor, _) = collision.find_floor(x, 10000.0, z, &mut Default::default());
    let y = if rng.chance(3) {
        rng.range(-12000, 7000) as f32
    } else {
        floor + rng.range(-77, 1500) as f32 + fraction(rng)
    };
    let mut o = object([x, y, z]);
    // Protect every raw field the original does not write, including aliases.
    for field in 0..0x50 {
        o.raw.set_u32(field, rng.next());
    }
    for (field, value) in [
        (O_POS_X, x),
        (O_POS_Y, y),
        (O_POS_Z, z),
        (O_FORWARD_VEL, rng.range(-60, 80) as f32 + fraction(rng)),
        (O_VEL_Y, rng.range(-150, 150) as f32 + fraction(rng)),
        (O_GRAVITY, rng.range(-6, 12) as f32 / 2.0),
        (
            O_FRICTION,
            [0.0, 0.8, 0.9998999, 0.9999, 1.0][rng.next() as usize % 5],
        ),
        (O_BUOYANCY, rng.range(-5, 20) as f32 / 10.0),
        (O_GRAPH_Y_OFFSET, rng.range(-50, 100) as f32),
    ] {
        o.raw.set_f32(field, value);
    }
    o.raw.set_s32(O_MOVE_ANGLE_YAW, rng.range(-100000, 100000));
    o.hitbox_radius = rng.range(0, 250) as f32;
    o.hitbox_height = rng.range(0, 500) as f32;
    if rng.chance(4) {
        o.gfx.node_flags |= GRAPH_RENDER_BILLBOARD;
    }
    if rng.chance(4) {
        o.active_flags |= ACTIVE_FLAG_MOVE_THROUGH_GRATE;
    }
    o
}

fn generated_calls(
    stream: &[i16],
    collision: &CollisionWorld,
    trig: &TrigTables,
    calls: usize,
    ticks: usize,
) {
    let oracle = Oracle::load(stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut w = StepWorld::new(collision, trig, &NO_ANIMATIONS);
    let mut rng = Lcg(0xB0B0_B1AC_2026);
    let mut mask = 0;
    let mut matrices = 0;
    let mut missing_floor = 0;
    for case in 0..calls {
        let mut o = random_object(&mut rng, collision);
        w.global_timer = rng.next();
        w.collision_flags = CollisionFlags {
            checking_for_camera: rng.chance(5),
            find_floor_include_surface_intangible: rng.chance(4),
        };
        let options = ObjectStepOptions {
            orient_with_floor: !rng.chance(6),
            terrain_matrix_available: !rng.chance(6),
        };
        let result = compare(&oracle, &mut o, &mut w, options, case);
        mask |= result.collision_flags;
        matrices += usize::from(result.terrain_matrix.is_some());
        missing_floor += usize::from(result.floor.is_none());
    }
    assert!(mask & OBJ_COL_FLAGS_LANDED == OBJ_COL_FLAGS_LANDED);
    assert!(mask & OBJ_COL_FLAG_HIT_WALL != 0);
    assert!(matrices > 100 && missing_floor > 100);
    // Carry Rust and native words through 120 trajectories, with sustained
    // patrol speed, chase speed or free thrown motion; never reset between ticks.
    for path in 0..120 {
        let mut o = random_object(&mut rng, collision);
        o.raw.set_f32(O_GRAVITY, 2.5);
        o.raw.set_f32(O_FRICTION, 0.8);
        o.raw.set_f32(O_BUOYANCY, 1.3);
        w.collision_flags = Default::default();
        for tick in 0..ticks {
            w.global_timer = tick as u32;
            if path % 3 != 0 {
                o.raw
                    .set_f32(O_FORWARD_VEL, if path % 3 == 1 { 5.0 } else { 20.0 });
            }
            compare(
                &oracle,
                &mut o,
                &mut w,
                Default::default(),
                calls + path * ticks + tick,
            );
        }
    }
    eprintln!(
        "{} original object_step comparisons; flags {mask:#x}, {matrices} matrices, {missing_floor} missing floors",
        calls + 120 * ticks
    );
}

#[test]
fn authored_object_motion_matches_the_decomp() {
    let s = stream();
    generated_calls(&s, &world(&s), &computed_tables(), 40_000, 180);
}

#[test]
#[ignore = "requires the owner's identified US ROM; never run in CI"]
fn bob_object_motion_matches_the_decomp_with_rom_tables() {
    use rustario64::import::{collision, engine, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        rom.reader().slice(range.start, range.len()).unwrap(),
        version::MAX_SEGMENT_BYTES,
    )
    .unwrap();
    let start = (version::BOB_COLLISION & 0xFFFFFF) as usize;
    let (mesh, consumed) = collision::decode(&terrain[start..]).unwrap();
    let words: Vec<i16> = terrain[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    let collision = CollisionWorld::load_area_terrain(&mesh).unwrap();
    generated_calls(&words, &collision, &trig, 80_000, 360);
}
