//! Exact standard-motion component comparisons, not a working boss/mission.
#[path = "support/playground.rs"]
mod playground;

use playground::{Builder, Lcg, computed_tables, world};
use rustario64::{
    content::animation::MarioAnimations,
    simulation::{
        collision::{CollisionFlags, CollisionWorld},
        mario::{StepWorld, constants::*},
        math::TrigTables,
        object::{Object, standard_motion::*},
    },
};
use rustario64_oracle::{Oracle, standard_motion::StandardMotionCall as Call};

static NO_ANIMATIONS: MarioAnimations = MarioAnimations {
    animations: Vec::new(),
};

fn stream() -> Vec<i16> {
    let mut b = Builder::default();
    b.flat(SURFACE_DEFAULT, [-7000, 7000], [-7000, 7000], -300, true);
    b.flat(SURFACE_DEFAULT, [-3000, 0], [-3000, 3000], 0, true);
    b.ramp(SURFACE_DEFAULT, [1000, 2000], [-2000, 2000], [0, 1000]);
    b.ramp(SURFACE_DEFAULT, [3000, 4000], [-1000, 1000], [0, 4000]);
    b.ramp(SURFACE_DEFAULT, [4500, 5500], [-500, 500], [0, 7000]);
    b.flat(SURFACE_BURNING, [-6000, -5000], [-3000, 3000], 200, true);
    b.flat(
        SURFACE_DEATH_PLANE,
        [-5000, -4000],
        [-3000, 3000],
        200,
        true,
    );
    b.flat(SURFACE_INTANGIBLE, [-1500, -500], [1000, 2000], 300, true);
    // A submerged platform and one above the water test the floor/water order.
    b.flat(SURFACE_DEFAULT, [-2000, -1000], [3500, 4500], 900, true);
    b.flat(SURFACE_DEFAULT, [-1000, 0], [3500, 4500], 1100, true);
    for (x, kind) in [
        (600, SURFACE_VANISH_CAP_WALLS),
        (650, SURFACE_DEFAULT),
        (-2500, SURFACE_NO_CAM_COLLISION),
    ] {
        b.quad(
            kind,
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
    b.quad(
        SURFACE_DEFAULT,
        [
            [400, -300, 100],
            [900, -300, 100],
            [400, 3000, 100],
            [900, 3000, 100],
        ],
        [0, 0, -1],
        None,
    );
    let mut s = b.stream();
    s.pop();
    s.extend([0x44, 1, 0, -3000, -3000, 0, 5000, 1000, 0x42]);
    s
}

fn object(pos: [f32; 3]) -> Object {
    let mut o = Object {
        active_flags: ACTIVE_FLAG_ACTIVE,
        ..Default::default()
    };
    for (field, value) in [
        (O_POS_X, pos[0]),
        (O_POS_Y, pos[1]),
        (O_POS_Z, pos[2]),
        (O_GRAVITY, -4.0),
        (O_BOUNCINESS, -0.5),
        (O_BUOYANCY, 2.0),
        (O_DRAG_STRENGTH, 10.0),
        (O_WALL_HITBOX_RADIUS, 100.0),
    ] {
        o.raw.set_f32(field, value);
    }
    o.raw.set_s32(O_ROOM, -1);
    o
}

fn rust_call(o: &mut Object, w: &mut StepWorld<'_>, call: Call) -> i32 {
    match call {
        Call::UpdateFloorAndWalls => cur_obj_update_floor_and_walls(o, w),
        Call::MoveStandard(angle) => cur_obj_move_standard(o, w, angle),
        Call::MoveY {
            gravity,
            bounciness,
            buoyancy,
        } => cur_obj_move_y(o, w, gravity, bounciness, buoyancy),
        Call::MoveUsingFvelAndGravity => cur_obj_move_using_fvel_and_gravity(o, w.trig),
        Call::ResolveWalls => return i32::from(cur_obj_resolve_wall_collisions(o, w)),
        Call::ApplyDrag(drag) => cur_obj_apply_drag_xz(o, drag),
        Call::FloorHeight => {
            return cur_obj_update_floor_height_and_get_floor(o, w).map_or(-1, i32::from);
        }
        Call::AngleDiff(a, b) => return i32::from(abs_angle_diff(a, b)),
        Call::Release {
            forward_vel,
            vel_y,
            mario_pos,
        } => {
            let mario = object(mario_pos);
            rustario64::simulation::object::held::cur_obj_move_after_thrown_or_dropped(
                o,
                w,
                &mario,
                forward_vel,
                vel_y,
            );
        }
    }
    0
}

fn compare_pair(
    oracle: &Oracle,
    rust: &mut Object,
    native: &mut Object,
    w: &mut StepWorld<'_>,
    native_flags: &mut CollisionFlags,
    call: Call,
    case: usize,
) {
    let before = rust.clone();
    let expected = oracle.standard_motion(native, native_flags, call);
    let actual = rust_call(rust, w, call);
    assert_eq!(actual, expected, "case {case}: {call:?} return value");
    for field in 0..0x50 {
        assert_eq!(
            rust.raw.u32(field),
            native.raw.u32(field),
            "case {case}: {call:?} raw[{field:#x}], before {before:?}"
        );
    }
    assert_eq!(rust, native, "case {case}: {call:?}, non-raw fields");
    assert_eq!(
        w.collision_flags, *native_flags,
        "case {case}: {call:?} query flags"
    );
}

fn compare(oracle: &Oracle, o: &mut Object, w: &mut StepWorld<'_>, call: Call, case: usize) {
    let mut native = o.clone();
    let mut flags = w.collision_flags;
    compare_pair(oracle, o, &mut native, w, &mut flags, call, case);
}

#[test]
fn signed_angle_differences_match_exhaustively() {
    let s = stream();
    let collision = world(&s);
    let trig = computed_tables();
    let oracle = Oracle::load(&s);
    let mut w = StepWorld::new(&collision, &trig, &NO_ANIMATIONS);
    let mut o = object([0.0; 3]);
    for a in [i16::MIN, -12345, 0, 12345, i16::MAX] {
        for b in i16::MIN..=i16::MAX {
            compare(
                &oracle,
                &mut o,
                &mut w,
                Call::AngleDiff(a, b),
                b as u16 as usize,
            );
        }
    }
    assert_eq!(abs_angle_diff(0, i16::MIN), i16::MAX);
}

#[test]
fn landing_water_drag_and_partial_update_boundaries_match() {
    let s = stream();
    let collision = world(&s);
    let trig = computed_tables();
    let oracle = Oracle::load(&s);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut w = StepWorld::new(&collision, &trig, &NO_ANIMATIONS);
    let mut cases = 0;
    let mut observed = 0;
    // At exactly floor height, zero vertical velocity must stay in air.
    // This distinguishes the original strict floor test from <=.
    let mut exact_floor = object([200.0, -300.0, 4000.0]);
    exact_floor.raw.set_f32(O_FLOOR_HEIGHT, -300.0);
    compare(
        &oracle,
        &mut exact_floor,
        &mut w,
        Call::MoveY {
            gravity: 0.0,
            bounciness: -0.5,
            buoyancy: 2.0,
        },
        cases,
    );
    assert_eq!(exact_floor.raw.u32(O_MOVE_FLAGS), OBJ_MOVE_IN_AIR);
    cases += 1;
    for pos in [
        [200.0, 0.0, 4000.0],
        [-2000.0, 0.0, 0.0],
        [-1500.0, 900.0, 4000.0],
        [-500.0, 1100.0, 4000.0],
    ] {
        for offset in [-0.001, 0.0, 0.001, 1.0, 999.0, 1000.0, 1001.0] {
            for vy in [-200.0, -78.0, -5.0, -0.0, 0.0, 5.0, 5.000001, 6.0, 200.0] {
                for flags in [
                    0,
                    OBJ_MOVE_LANDED,
                    OBJ_MOVE_ON_GROUND,
                    OBJ_MOVE_ENTERED_WATER,
                    OBJ_MOVE_AT_WATER_SURFACE,
                    OBJ_MOVE_UNDERWATER_ON_GROUND,
                    OBJ_MOVE_UNDERWATER_OFF_GROUND,
                    0xFFFF_FFFF,
                ] {
                    for disable_water in [false, true] {
                        let mut o = object([pos[0], pos[1] + offset, pos[2]]);
                        o.raw.set_f32(O_FLOOR_HEIGHT, pos[1]);
                        o.raw.set_f32(O_VEL_Y, vy);
                        o.raw.set_u32(O_MOVE_FLAGS, flags);
                        if disable_water {
                            o.active_flags |= ACTIVE_FLAG_UNK10;
                        }
                        compare(
                            &oracle,
                            &mut o,
                            &mut w,
                            Call::MoveY {
                                gravity: -4.0,
                                bounciness: -0.5,
                                buoyancy: 2.0,
                            },
                            cases,
                        );
                        if flags != u32::MAX {
                            observed |= o.raw.u32(O_MOVE_FLAGS);
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    for flag in [
        OBJ_MOVE_LANDED,
        OBJ_MOVE_ON_GROUND,
        OBJ_MOVE_LEFT_GROUND,
        OBJ_MOVE_ENTERED_WATER,
        OBJ_MOVE_AT_WATER_SURFACE,
        OBJ_MOVE_UNDERWATER_ON_GROUND,
        OBJ_MOVE_UNDERWATER_OFF_GROUND,
        OBJ_MOVE_LEAVING_WATER,
        OBJ_MOVE_BOUNCE,
    ] {
        assert_ne!(
            observed & flag,
            0,
            "movement flag {flag:#x} must be reached"
        );
    }
    for value in [
        -100000.0, -10000.0, -100.0, -0.001, -0.0, 0.0, 0.001, 100.0, 10000.0, 100000.0,
    ] {
        for drag in [-100.0, -0.0, 0.0, 0.01, 10.0, 100.0, 10000.0] {
            let mut o = object([0.0; 3]);
            o.raw.set_f32(O_VEL_X, value);
            o.raw.set_f32(O_VEL_Z, -value);
            compare(&oracle, &mut o, &mut w, Call::ApplyDrag(drag), cases);
            cases += 1;
        }
    }
    for height in [-249.99998, -250.0, -250.00002] {
        for degrees in [-78, 78] {
            let mut o = object([200.0, -300.0, 4000.0]);
            o.raw.set_f32(O_FLOOR_HEIGHT, height);
            o.raw.set_f32(O_FORWARD_VEL, 3.0);
            o.raw.set_u32(O_MOVE_FLAGS, OBJ_MOVE_ON_GROUND);
            compare(&oracle, &mut o, &mut w, Call::MoveStandard(degrees), cases);
            assert_eq!(
                o.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_HIT_EDGE != 0,
                degrees < 0 && -300.0 - height < -50.0
            );
            cases += 1;
        }
    }
    for pos in [
        [-5500.0, 201.0, 0.0],
        [-4500.0, 201.0, 0.0],
        [650.25, 80.75, 0.5],
        [10000.0, 500.0, 0.0],
    ] {
        for active in [
            ACTIVE_FLAG_ACTIVE,
            ACTIVE_FLAG_ACTIVE | ACTIVE_FLAG_FAR_AWAY,
            ACTIVE_FLAG_ACTIVE | ACTIVE_FLAG_IN_DIFFERENT_ROOM,
        ] {
            let mut o = object(pos);
            o.active_flags = active;
            o.raw.set_u32(
                O_MOVE_FLAGS,
                OBJ_MOVE_ABOVE_LAVA
                    | OBJ_MOVE_ABOVE_DEATH_BARRIER
                    | OBJ_MOVE_HIT_WALL
                    | OBJ_MOVE_MASK_IN_WATER,
            );
            o.raw.set_f32(O_FORWARD_VEL, 3.0);
            o.raw.set_f32(O_VEL_X, 3.0);
            compare(&oracle, &mut o, &mut w, Call::UpdateFloorAndWalls, cases);
            if pos[0] == -5500.0 {
                assert_eq!(o.raw.s16(O_FLOOR_TYPE, 0), SURFACE_BURNING);
                assert!(o.floor().is_some());
            }
            if pos[0] == -4500.0 {
                assert_eq!(o.raw.s16(O_FLOOR_TYPE, 0), SURFACE_DEATH_PLANE);
            }
            if pos[0] == 10000.0 {
                assert_eq!(o.floor(), None);
                assert_eq!(o.raw.u32(O_FLOOR_TYPE), 0);
            }
            let before = o.clone();
            compare(&oracle, &mut o, &mut w, Call::MoveStandard(-78), cases);
            if active != ACTIVE_FLAG_ACTIVE {
                assert_eq!(o, before, "partial-update objects must not move");
            }
            cases += 1;
        }
    }
    // Strict wall radius/angle bounds; original s16 coordinate truncation.
    for x in [549.9, 599.9, 649.9, 650.0, 650.9, 700.1, 66186.75] {
        for radius in [
            0.0,
            f32::from_bits(0.1f32.to_bits() - 1),
            0.1,
            50.0,
            100.0,
            300.0,
        ] {
            for yaw in [-0x8000, -0x4001, -0x4000, 0, 0x3FFF, 0x4000, 0x4001, 0x7FFF] {
                let mut o = object([x, 80.75, 0.5]);
                o.raw.set_f32(O_WALL_HITBOX_RADIUS, radius);
                o.raw.set_s32(O_MOVE_ANGLE_YAW, yaw);
                compare(&oracle, &mut o, &mut w, Call::ResolveWalls, cases);
                cases += 1;
            }
        }
    }
    // Arc movement computes X/Z velocities even when outside its bounds.
    for axis in 0..3 {
        for value in [-12000.1, -12000.0, -11999.9, 11999.9, 12000.0, 12000.1] {
            let mut pos = [0.0; 3];
            pos[axis] = value;
            let mut o = object(pos);
            o.raw.set_f32(O_VEL_Y, -200.0);
            o.raw.set_f32(O_FORWARD_VEL, 20.0);
            o.raw.set_s32(O_MOVE_ANGLE_YAW, 0x2000);
            compare(
                &oracle,
                &mut o,
                &mut w,
                Call::MoveUsingFvelAndGravity,
                cases,
            );
            if value.abs() > 12000.0 {
                assert_eq!(o.pos(), pos);
            } else {
                assert_eq!(o.raw.f32(O_VEL_Y), -204.0, "arc has no terminal speed");
            }
            cases += 1;
        }
    }
    eprintln!("{cases} standard-motion boundary calls matched");
}

#[test]
fn thrown_release_floor_correction_and_motion_match() {
    let s = stream();
    let collision = world(&s);
    let trig = computed_tables();
    let oracle = Oracle::load(&s);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut w = StepWorld::new(&collision, &trig, &NO_ANIMATIONS);
    let mario_pos = [200.0, -200.0, 4000.0];
    let mut cases = 0;
    for pos in [
        [200.0, -301.0, 4000.0],
        [200.0, -300.0, 4000.0],
        [200.0, 300.0, 4000.0],
        [-2000.0, 1001.0, 0.0],
        [9000.0, 200.0, 9000.0],
    ] {
        for forward_vel in [-20.0, -0.0, 0.0, 20.0] {
            for vel_y in [-200.0, -1.0, 0.0, 50.0] {
                let mut o = object(pos);
                o.raw.set_u32(O_MOVE_FLAGS, u32::MAX);
                compare(
                    &oracle,
                    &mut o,
                    &mut w,
                    Call::Release {
                        forward_vel,
                        vel_y,
                        mario_pos,
                    },
                    cases,
                );
                if pos[0] == 9000.0 {
                    assert_eq!(o.pos()[0], mario_pos[0]);
                    assert_eq!(o.pos()[2], mario_pos[2]);
                }
                if forward_vel == 0.0 {
                    assert_eq!(o.raw.f32(O_VEL_Y).to_bits(), vel_y.to_bits());
                    assert_eq!(
                        o.raw.u32(O_MOVE_FLAGS),
                        0,
                        "stationary release skips vertical motion"
                    );
                }
                cases += 1;
            }
        }
    }
    eprintln!("{cases} thrown-release calls matched");
}

fn generated_suite(stream: &[i16], collision: &CollisionWorld, trig: &TrigTables, count: usize) {
    let oracle = Oracle::load(stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut w = StepWorld::new(collision, trig, &NO_ANIMATIONS);
    let mut rng = Lcg(0x4B1A_6024_712A);
    let mut calls = 0;
    for case in 0..count {
        let mut o = object([
            rng.range(-8000, 8000) as f32 + 0.75,
            rng.range(-300, 5500) as f32 + 0.25,
            rng.range(-8000, 8000) as f32 - 0.5,
        ]);
        // Unused words are sentinels, so unintended writes are detectable.
        for field in [
            O_ACTION,
            O_SUB_ACTION,
            O_TIMER,
            O_HEALTH,
            O_INTERACT_STATUS,
            O_FLAGS,
        ] {
            o.raw.set_u32(field, rng.next());
        }
        o.raw.set_u32(O_FLOOR_TYPE, rng.next());
        o.raw.set_s32(O_MOVE_ANGLE_YAW, rng.next() as i32);
        o.raw
            .set_f32(O_FORWARD_VEL, rng.range(-15000, 15000) as f32 / 100.0);
        o.raw
            .set_f32(O_VEL_X, rng.range(-10000, 10000) as f32 / 100.0);
        o.raw
            .set_f32(O_VEL_Z, rng.range(-10000, 10000) as f32 / 100.0);
        o.raw
            .set_f32(O_VEL_Y, rng.range(-20000, 20000) as f32 / 100.0);
        o.raw.set_f32(O_GRAVITY, rng.range(-200, 100) as f32 / 10.0);
        o.raw
            .set_f32(O_BOUNCINESS, rng.range(-100, 50) as f32 / 100.0);
        o.raw.set_f32(O_BUOYANCY, rng.range(-50, 200) as f32 / 10.0);
        o.raw
            .set_f32(O_DRAG_STRENGTH, rng.range(0, 2000) as f32 / 10.0);
        o.raw
            .set_f32(O_WALL_HITBOX_RADIUS, rng.range(0, 30000) as f32 / 100.0);
        o.raw.set_u32(O_MOVE_FLAGS, rng.next() & 0xFFFF);
        o.active_flags |= match case % 6 {
            1 => ACTIVE_FLAG_FAR_AWAY,
            2 => ACTIVE_FLAG_IN_DIFFERENT_ROOM,
            3 => ACTIVE_FLAG_UNK10,
            4 => ACTIVE_FLAG_MOVE_THROUGH_GRATE,
            _ => 0,
        };
        w.collision_flags.checking_for_camera = rng.chance(7);
        w.collision_flags.find_floor_include_surface_intangible = rng.chance(7);
        compare(&oracle, &mut o, &mut w, Call::FloorHeight, calls);
        calls += 1;
        compare(&oracle, &mut o, &mut w, Call::UpdateFloorAndWalls, calls);
        calls += 1;
        let angle = [-78, 78, -60, 60, -45, 45, 0, i16::MIN, i16::MAX][case % 9];
        compare(&oracle, &mut o, &mut w, Call::MoveStandard(angle), calls);
        calls += 1;
    }
    // Two independently evolving objects, never reseeded from one another.
    // Alternate King Bob-omb's normal and return-home movement families.
    for trajectory in 0..120 {
        let mut rust = object([
            rng.range(-6000, 6000) as f32,
            rng.range(0, 4000) as f32,
            rng.range(-6000, 6000) as f32,
        ]);
        rust.raw.set_f32(O_FORWARD_VEL, 3.0);
        rust.raw.set_s32(O_MOVE_ANGLE_YAW, rng.next() as i32);
        let mut native = rust.clone();
        w.collision_flags = Default::default();
        let mut native_flags = w.collision_flags;
        for tick in 0..240 {
            if tick % 45 == 0 {
                let yaw = rng.next() as i32;
                let vy = rng.range(10, 100) as f32;
                for o in [&mut rust, &mut native] {
                    o.raw.set_s32(O_MOVE_ANGLE_YAW, yaw);
                    o.raw
                        .set_f32(O_FORWARD_VEL, if trajectory % 2 == 0 { 3.0 } else { -20.0 });
                    o.raw.set_f32(O_VEL_Y, vy);
                }
            }
            for call in [
                Call::UpdateFloorAndWalls,
                if trajectory % 3 == 0 && tick % 60 < 20 {
                    Call::MoveUsingFvelAndGravity
                } else {
                    Call::MoveStandard(-78)
                },
            ] {
                compare_pair(
                    &oracle,
                    &mut rust,
                    &mut native,
                    &mut w,
                    &mut native_flags,
                    call,
                    calls,
                );
                calls += 1;
            }
        }
    }
    eprintln!("{calls} generated/chained standard-motion calls matched");
}

#[test]
fn authored_standard_motion_matches_generated_and_chained_states() {
    let s = stream();
    generated_suite(&s, &world(&s), &computed_tables(), 10000);
}

#[test]
#[ignore = "requires the owner's supported US ROM"]
fn bob_standard_motion_matches_with_rom_collision_and_trig() {
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
    generated_suite(&words, &collision, &trig, 20000);
}
