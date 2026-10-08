//! Bitwise differential tests for mario_step.c: Rust port vs the pinned decomp
//! compiled natively. CI uses authored collision and computed (non-ROM) trig
//! tables; the ignored test uses BOB and the ROM's tables.
use rustario64::{
    import::collision,
    simulation::{
        collision::{CollisionFlags, CollisionWorld},
        mario::{MarioState, StepWorld, SurfaceRef, constants as c, step},
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
    },
};
use rustario64_oracle::{MarioCall, Oracle, OracleMario, decomp_constants};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() as f32 / u32::MAX as f32)
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.next() as usize % items.len()]
    }
}

/// Computed approximations of the tables' shape; not the game's values.
fn computed_tables() -> TrigTables {
    let sine = (0..SINE_ENTRIES)
        .map(|i| ((i as f64) * std::f64::consts::TAU / 4096.0).sin() as f32)
        .collect();
    let arctan = (0..ARCTAN_ENTRIES)
        .map(|i| ((i as f64 / 1024.0).atan() * 32768.0 / std::f64::consts::PI).round() as u16)
        .collect();
    TrigTables::new(sine, arctan).unwrap()
}

/// Authored terrain exercising the step code's surface-dependent branches.
fn authored_stream(seed: u64) -> Vec<i16> {
    let mut rng = Lcg(seed);
    let mut vertices: Vec<[i16; 3]> = vec![];
    let mut v = |p: [i32; 3]| {
        vertices.push(p.map(|x| x as i16));
        (vertices.len() - 1) as i16
    };
    let mut groups: Vec<(i16, Vec<[i16; 3]>, Vec<i16>)> = vec![];
    let floor_types = [
        c::SURFACE_DEFAULT,
        c::SURFACE_VERY_SLIPPERY,
        c::SURFACE_SLIPPERY,
        c::SURFACE_NOT_SLIPPERY,
        c::SURFACE_HARD,
        c::SURFACE_NOISE_DEFAULT,
        c::SURFACE_NOISE_SLIPPERY,
        c::SURFACE_ICE,
        c::SURFACE_SHALLOW_QUICKSAND,
        c::SURFACE_VERTICAL_WIND,
    ];
    // A sloped, bumpy floor grid; each tile picks a surface type.
    for gz in 0..8 {
        for gx in 0..8 {
            let x0 = -4000 + gx * 1000;
            let z0 = -4000 + gz * 1000;
            let mut h = || rng.range(-150, 450) + gx * 60;
            let (a, b, cc, d) = (
                v([x0, h(), z0]),
                v([x0, h(), z0 + 1000]),
                v([x0 + 1000, h(), z0]),
                v([x0 + 1000, h(), z0 + 1000]),
            );
            let t = floor_types[((gx + gz * 3) as usize) % floor_types.len()];
            groups.push((t, vec![[a, b, cc], [cc, b, d]], vec![]));
        }
    }
    // Force-bearing floors with valid forces: moving sand speed index 0..3 in
    // the high byte, horizontal wind direction in the low byte.
    for (i, t) in [
        c::SURFACE_DEEP_MOVING_QUICKSAND,
        c::SURFACE_SHALLOW_MOVING_QUICKSAND,
        c::SURFACE_MOVING_QUICKSAND,
        c::SURFACE_INSTANT_MOVING_QUICKSAND,
        c::SURFACE_HORIZONTAL_WIND,
    ]
    .into_iter()
    .enumerate()
    {
        let x0 = 4300 + (i as i32 % 2) * 1300;
        let z0 = -4000 + i as i32 * 1600;
        let (a, b, cc, d) = (
            v([x0, 100, z0]),
            v([x0, 100, z0 + 1200]),
            v([x0 + 1200, 100, z0]),
            v([x0 + 1200, 100, z0 + 1200]),
        );
        let force = |rng: &mut Lcg| ((rng.range(0, 3) << 8) | rng.range(0, 255)) as i16;
        let forces = vec![force(&mut rng), force(&mut rng)];
        groups.push((t, vec![[a, b, cc], [cc, b, d]], forces));
    }
    // Walls of several types and angles: lava (burning) and default, plus low
    // walls with floors on top for ledge grabs.
    for i in 0..24 {
        let cx = rng.range(-3800, 3800);
        let cz = rng.range(-3800, 3800);
        let angle = (i as f64) * 0.37 + rng.f(0.0, 0.2) as f64;
        let (dx, dz) = ((angle.cos() * 500.0) as i32, (angle.sin() * 500.0) as i32);
        let (y0, y1) = (rng.range(-300, 200), rng.range(300, 1200));
        let (a, b, cc, d) = (
            v([cx - dx, y0, cz - dz]),
            v([cx - dx, y1, cz - dz]),
            v([cx + dx, y0, cz + dz]),
            v([cx + dx, y1, cz + dz]),
        );
        let t = if i % 3 == 0 {
            c::SURFACE_BURNING
        } else {
            c::SURFACE_DEFAULT
        };
        groups.push((
            t,
            vec![[a, b, cc], [cc, b, d], [a, cc, b], [cc, d, b]],
            vec![],
        ));
        if i % 2 == 0 {
            // Ledge floor on top of the wall.
            let (e, f, g) = (
                v([cx - dx, y1, cz - dz]),
                v([cx + dx + dz / 4, y1, cz + dz - dx / 4]),
                v([cx + dx, y1, cz + dz]),
            );
            groups.push((c::SURFACE_DEFAULT, vec![[e, g, f], [e, f, g]], vec![]));
        }
    }
    // Low ceilings, some hangable, over parts of the grid.
    for i in 0..10 {
        let x0 = rng.range(-4000, 2500);
        let z0 = rng.range(-4000, 2500);
        let y = rng.range(250, 900);
        let (a, b, cc) = (
            v([x0, y, z0]),
            v([x0 + 1400, y, z0]),
            v([x0, y + rng.range(-60, 60), z0 + 1400]),
        );
        let t = if i % 2 == 0 {
            c::SURFACE_HANGABLE
        } else {
            c::SURFACE_DEFAULT
        };
        groups.push((t, vec![[a, b, cc]], vec![]));
    }
    // A flat tunnel west of the grid: ceilings exactly 160 and 240 units above
    // the floor exercise the step code's >= / > 160 boundaries.
    let mut tunnel = |x0: i32, ceiling: i32| {
        let floor = [
            v([x0, 0, -1000]),
            v([x0, 0, 1000]),
            v([x0 + 600, 0, -1000]),
            v([x0 + 600, 0, 1000]),
        ];
        let top = [
            v([x0, ceiling, -1000]),
            v([x0 + 600, ceiling, -1000]),
            v([x0, ceiling, 1000]),
            v([x0 + 600, ceiling, 1000]),
        ];
        [
            (
                c::SURFACE_DEFAULT,
                vec![
                    [floor[0], floor[1], floor[2]],
                    [floor[2], floor[1], floor[3]],
                ],
            ),
            (
                c::SURFACE_DEFAULT,
                vec![[top[0], top[1], top[2]], [top[1], top[3], top[2]]],
            ),
        ]
    };
    for (x0, ceiling) in [(-6600, 160), (-6000, 240), (-5400, 159)] {
        for (t, tris) in tunnel(x0, ceiling) {
            groups.push((t, tris, vec![]));
        }
    }
    let mut s: Vec<i16> = vec![0x40, vertices.len() as i16];
    for p in &vertices {
        s.extend_from_slice(p);
    }
    for (t, tris, forces) in &groups {
        s.push(*t);
        s.push(tris.len() as i16);
        for (i, tri) in tris.iter().enumerate() {
            s.extend_from_slice(tri);
            if !forces.is_empty() {
                s.push(forces[i]);
            }
        }
    }
    s.push(0x41);
    s.extend_from_slice(&[0x44, 2]);
    s.extend_from_slice(&[0, -4000, -4000, 0, 0, 260]);
    s.extend_from_slice(&[1, 1000, 1000, 4000, 4000, 900]);
    s.push(0x42);
    s
}

fn surface_id(r: Option<SurfaceRef>) -> i32 {
    match r {
        None => -1,
        Some(SurfaceRef::WaterPseudoFloor) => -2,
        Some(SurfaceRef::Collision(i)) => i32::from(i),
    }
}

fn surface_ref(i: i32) -> Option<SurfaceRef> {
    match i {
        -1 => None,
        -2 => Some(SurfaceRef::WaterPseudoFloor),
        i => Some(SurfaceRef::Collision(i as u16)),
    }
}

struct Env {
    global_timer: u32,
    area_terrain_type: u16,
    level_num: i16,
    water_pseudo: f32,
    include_intangible: bool,
}

fn to_oracle(m: &MarioState, e: &Env) -> OracleMario {
    OracleMario {
        input: m.input,
        flags: m.flags,
        action: m.action,
        terrain_sound_addend: m.terrain_sound_addend,
        face_angle: m.face_angle,
        angle_vel: m.angle_vel,
        pos: m.pos,
        vel: m.vel,
        forward_vel: m.forward_vel,
        slide_vel_x: m.slide_vel_x,
        slide_vel_z: m.slide_vel_z,
        wall: surface_id(m.wall),
        ceil: surface_id(m.ceil),
        floor: surface_id(m.floor),
        ceil_height: m.ceil_height,
        floor_height: m.floor_height,
        floor_angle: m.floor_angle,
        water_level: m.water_level,
        peak_height: m.peak_height,
        quicksand_depth: m.quicksand_depth,
        getting_blown_gravity: m.getting_blown_gravity,
        wing_flutter: i8::from(m.wing_flutter),
        gfx_pos: m.gfx_pos,
        gfx_angle: m.gfx_angle,
        global_timer: e.global_timer,
        area_terrain_type: e.area_terrain_type,
        level_num: e.level_num,
        water_pseudo_origin_offset: e.water_pseudo,
        include_intangible: i16::from(e.include_intangible),
    }
}

/// Every field as exact bits, for comparison and readable first-difference output.
fn fields(o: &OracleMario) -> Vec<(&'static str, u64)> {
    let f = |x: f32| u64::from(x.to_bits());
    let mut out = vec![
        ("input", u64::from(o.input)),
        ("flags", u64::from(o.flags)),
        ("action", u64::from(o.action)),
        ("terrain_sound_addend", u64::from(o.terrain_sound_addend)),
        ("forward_vel", f(o.forward_vel)),
        ("slide_vel_x", f(o.slide_vel_x)),
        ("slide_vel_z", f(o.slide_vel_z)),
        ("wall", o.wall as u64),
        ("ceil", o.ceil as u64),
        ("floor", o.floor as u64),
        ("ceil_height", f(o.ceil_height)),
        ("floor_height", f(o.floor_height)),
        ("floor_angle", o.floor_angle as u64),
        ("water_level", o.water_level as u64),
        ("peak_height", f(o.peak_height)),
        ("quicksand_depth", f(o.quicksand_depth)),
        ("getting_blown_gravity", f(o.getting_blown_gravity)),
        ("wing_flutter", o.wing_flutter as u64),
        (
            "water_pseudo_origin_offset",
            f(o.water_pseudo_origin_offset),
        ),
        ("include_intangible", o.include_intangible as u64),
    ];
    for i in 0..3 {
        out.push((
            ["face_angle.x", "face_angle.y", "face_angle.z"][i],
            o.face_angle[i] as u64,
        ));
        out.push((
            ["angle_vel.x", "angle_vel.y", "angle_vel.z"][i],
            o.angle_vel[i] as u64,
        ));
        out.push((["pos.x", "pos.y", "pos.z"][i], f(o.pos[i])));
        out.push((["vel.x", "vel.y", "vel.z"][i], f(o.vel[i])));
        out.push((["gfx_pos.x", "gfx_pos.y", "gfx_pos.z"][i], f(o.gfx_pos[i])));
        out.push((
            ["gfx_angle.x", "gfx_angle.y", "gfx_angle.z"][i],
            o.gfx_angle[i] as u64,
        ));
    }
    out
}

fn rust_call(m: &mut MarioState, w: &mut StepWorld<'_>, call: MarioCall) -> i32 {
    match call {
        MarioCall::GroundStep => step::perform_ground_step(m, w) as i32,
        MarioCall::AirStep(arg) => step::perform_air_step(m, w, arg) as i32,
        MarioCall::StationaryGroundStep => step::stationary_ground_step(m, w) as i32,
        MarioCall::StopAndSetHeightToFloor => {
            step::stop_and_set_height_to_floor(m, w);
            0
        }
        MarioCall::BonkReflection(negate) => {
            step::mario_bonk_reflection(m, w, negate);
            0
        }
        MarioCall::ApplyGravity => {
            step::apply_gravity(m);
            0
        }
        MarioCall::ApplyVerticalWind => {
            step::apply_vertical_wind(m, w);
            0
        }
        MarioCall::VelFromPitchAndYaw => {
            step::set_vel_from_pitch_and_yaw(m, w);
            0
        }
        MarioCall::VelFromYaw => {
            step::set_vel_from_yaw(m, w);
            0
        }
        MarioCall::SetForwardVel(v) => {
            step::mario_set_forward_vel(m, w, v);
            0
        }
        MarioCall::UpdateMovingSand => i32::from(step::mario_update_moving_sand(m, w)),
        MarioCall::UpdateWindyGround => i32::from(step::mario_update_windy_ground(m, w)),
    }
}

#[derive(Default, Debug)]
struct Outcomes {
    checks: usize,
    ground: [usize; 4],
    air: [usize; 7],
    sand: usize,
    wind: usize,
    pseudo_floor: usize,
}

impl Outcomes {
    fn record(&mut self, call: MarioCall, result: i32, after: &MarioState) {
        self.checks += 1;
        match call {
            MarioCall::GroundStep | MarioCall::StationaryGroundStep => {
                self.ground[result as usize] += 1
            }
            MarioCall::AirStep(_) => self.air[result as usize] += 1,
            MarioCall::UpdateMovingSand => self.sand += result as usize,
            MarioCall::UpdateWindyGround => self.wind += result as usize,
            _ => {}
        }
        if after.floor == Some(SurfaceRef::WaterPseudoFloor) {
            self.pseudo_floor += 1;
        }
    }
}

/// The Rust world and the decomp oracle loaded with the same terrain and tables.
struct Sides<'a> {
    world: &'a CollisionWorld,
    trig: &'a TrigTables,
    oracle: &'a Oracle,
}

/// Run one call on both sides from the same state; returns the Rust result state.
fn compare_call(
    sides: &Sides,
    m: &MarioState,
    env: &mut Env,
    call: MarioCall,
    context: &str,
    outcomes: &mut Outcomes,
) -> MarioState {
    let Sides {
        world,
        trig,
        oracle,
    } = *sides;
    let mut theirs = to_oracle(m, env);
    let c_result = oracle.mario_call(&mut theirs, call);
    let mut ours = m.clone();
    let mut w = StepWorld {
        collision: world,
        collision_flags: CollisionFlags {
            checking_for_camera: false,
            find_floor_include_surface_intangible: env.include_intangible,
        },
        trig,
        global_timer: env.global_timer,
        area_terrain_type: env.area_terrain_type,
        level_num: env.level_num,
        water_pseudo_floor_origin_offset: env.water_pseudo,
    };
    let r_result = rust_call(&mut ours, &mut w, call);
    env.water_pseudo = w.water_pseudo_floor_origin_offset;
    env.include_intangible = w.collision_flags.find_floor_include_surface_intangible;
    let ours_flat = to_oracle(&ours, env);
    let (a, b) = (fields(&ours_flat), fields(&theirs));
    if let Some(((name, x), (_, y))) = a.iter().zip(&b).find(|(x, y)| x != y) {
        panic!("{context}: {call:?} first difference in {name}: rust {x:#X} vs decomp {y:#X}");
    }
    assert_eq!(r_result, c_result, "{context}: {call:?} return value");
    outcomes.record(call, r_result, &ours);
    ours
}

const ACTIONS: [u32; 16] = [
    c::ACT_IDLE,
    c::ACT_WALKING,
    c::ACT_JUMP,
    c::ACT_FREEFALL,
    c::ACT_TWIRLING,
    c::ACT_SHOT_FROM_CANNON,
    c::ACT_LONG_JUMP,
    c::ACT_SLIDE_KICK,
    c::ACT_BBH_ENTER_SPIN,
    c::ACT_LAVA_BOOST,
    c::ACT_FALL_AFTER_STAR_GRAB,
    c::ACT_GETTING_BLOWN,
    c::ACT_GROUND_POUND,
    c::ACT_FLYING,
    c::ACT_CRAWLING,
    c::ACT_JUMP | c::ACT_FLAG_RIDING_SHELL | c::ACT_FLAG_METAL_WATER,
];

fn random_state(
    rng: &mut Lcg,
    world: &CollisionWorld,
    span: f32,
    airborne: bool,
) -> Option<MarioState> {
    let x = rng.f(-span, span);
    let z = rng.f(-span, span);
    let probe_y = rng.f(-500.0, 3000.0);
    let mut flags = CollisionFlags::default();
    let (floor_height, floor) = world.find_floor(x, probe_y, z, &mut flags);
    let floor = floor?;
    let y = if airborne {
        floor_height + rng.f(0.0, 1500.0)
    } else {
        floor_height
    };
    let (ceil_height, ceil) = world.find_ceil(x, floor_height + 80.0, z, CollisionFlags::default());
    let mario_flags = [
        0,
        c::MARIO_VANISH_CAP,
        c::MARIO_WING_CAP,
        c::MARIO_UNKNOWN_08,
        c::MARIO_METAL_CAP,
    ]
    .iter()
    .filter(|_| rng.next().is_multiple_of(3))
    .fold(0, |a, b| a | b);
    let mut action = rng.pick(&ACTIONS);
    if rng.next().is_multiple_of(5) {
        action |= c::ACT_FLAG_CONTROL_JUMP_HEIGHT;
    }
    if rng.next().is_multiple_of(6) {
        action |= c::ACT_FLAG_RIDING_SHELL;
    }
    Some(MarioState {
        input: if rng.next().is_multiple_of(2) {
            c::INPUT_A_DOWN
        } else {
            0
        },
        flags: mario_flags,
        action,
        terrain_sound_addend: rng.next(),
        face_angle: [rng.next() as i16, rng.next() as i16, rng.next() as i16],
        angle_vel: [rng.next() as i16, rng.range(-3000, 3000) as i16, 0],
        pos: [x, y, z],
        vel: [rng.f(-90.0, 90.0), rng.f(-80.0, 70.0), rng.f(-90.0, 90.0)],
        forward_vel: rng.f(-40.0, 80.0),
        slide_vel_x: rng.f(-50.0, 50.0),
        slide_vel_z: rng.f(-50.0, 50.0),
        wall: None,
        ceil: ceil.map(SurfaceRef::Collision),
        floor: Some(SurfaceRef::Collision(floor)),
        ceil_height,
        floor_height,
        floor_angle: rng.next() as i16,
        water_level: rng.range(-1500, 1500) as i16,
        peak_height: rng.f(-1000.0, 3000.0),
        quicksand_depth: rng.f(0.0, 30.0),
        getting_blown_gravity: rng.f(0.0, 8.0),
        wing_flutter: rng.next().is_multiple_of(2),
        gfx_pos: [0.0; 3],
        gfx_angle: [0; 3],
    })
}

/// A falling state just outside a wall, 105-145 units below its top, moving
/// into it: the setup that reaches check_ledge_grab.
fn ledge_state(rng: &mut Lcg, world: &CollisionWorld) -> Option<MarioState> {
    let walls: Vec<u16> = (0..world.surfaces().len() as u16)
        .filter(|&i| world.surface(i).normal[1].abs() < 0.01)
        .collect();
    let i = rng.pick(&walls);
    let wall = world.surface(i);
    let t = rng.f(0.2, 0.8);
    let [a, b] = [wall.vertex1, wall.vertex2].map(|v| v.map(f32::from));
    let top = [wall.vertex1[1], wall.vertex2[1], wall.vertex3[1]]
        .into_iter()
        .max()
        .map(f32::from)?;
    let n = wall.normal;
    let out = rng.f(5.0, 45.0);
    let x = a[0] + (b[0] - a[0]) * t + n[0] * out;
    let z = a[2] + (b[2] - a[2]) * t + n[2] * out;
    let y = top - rng.f(105.0, 145.0);
    let mut m = random_state(rng, world, 1.0, true)?;
    let mut flags = CollisionFlags::default();
    let (floor_height, floor) = world.find_floor(x, y, z, &mut flags);
    m.pos = [x, y, z];
    m.floor = Some(SurfaceRef::Collision(floor?));
    m.floor_height = floor_height;
    let speed = rng.f(4.0, 40.0);
    m.vel = [-n[0] * speed, -rng.f(0.0, 30.0), -n[2] * speed];
    m.action = c::ACT_FREEFALL;
    m.flags &= !c::MARIO_VANISH_CAP;
    if rng.next().is_multiple_of(2) {
        // Threshold yaws for the air step's 0x6000 wall check.
        let angle = (f64::from(n[2])).atan2(f64::from(n[0]));
        let units = (angle * 32768.0 / std::f64::consts::PI) as i32 as i16;
        m.face_angle[1] = units.wrapping_sub(rng.pick(&WALL_YAW_OFFSETS));
    }
    Some(m)
}

/// Yaw offsets from a wall's angle at and around the step code's thresholds.
const WALL_YAW_OFFSETS: [i16; 14] = [
    0x2AAA, 0x2AA9, 0x2AAB, 0x5555, 0x5554, 0x5556, -0x2AAA, -0x2AA9, -0x5555, -0x5556, 0x6000,
    0x6001, -0x6000, -0x6001,
];

/// Mario on the floor beside a wall, facing it at a threshold yaw offset.
fn wall_ground_state(
    rng: &mut Lcg,
    world: &CollisionWorld,
    trig: &TrigTables,
) -> Option<MarioState> {
    let mut m = ledge_state(rng, world)?;
    let wall = world.surface(
        rng.pick(
            &(0..world.surfaces().len() as u16)
                .filter(|&i| world.surface(i).normal[1].abs() < 0.01)
                .collect::<Vec<_>>(),
        ),
    );
    let n = wall.normal;
    let [a, b] = [wall.vertex1, wall.vertex2].map(|v| v.map(f32::from));
    let t = rng.f(0.2, 0.8);
    let out = rng.f(10.0, 45.0);
    let (x, z) = (
        a[0] + (b[0] - a[0]) * t + n[0] * out,
        a[2] + (b[2] - a[2]) * t + n[2] * out,
    );
    let mut flags = CollisionFlags::default();
    let (floor_height, floor) = world.find_floor(x, 3000.0, z, &mut flags);
    m.pos = [x, floor_height, z];
    m.floor = Some(SurfaceRef::Collision(floor?));
    m.floor_height = floor_height;
    let speed = rng.f(4.0, 60.0);
    m.vel = [-n[0] * speed, 0.0, -n[2] * speed];
    m.action = c::ACT_WALKING;
    let wall_angle = trig.atan2s(n[2], n[0]);
    m.face_angle[1] = wall_angle.wrapping_sub(rng.pick(&WALL_YAW_OFFSETS));
    Some(m)
}

/// Mario in the flat tunnel, on its floor or falling onto it.
fn tunnel_state(rng: &mut Lcg, world: &CollisionWorld) -> Option<MarioState> {
    let mut m = random_state(rng, world, 1.0, true)?;
    let (x, z) = (rng.f(-6550.0, -4850.0), rng.f(-950.0, 950.0));
    let mut flags = CollisionFlags::default();
    let (floor_height, floor) = world.find_floor(x, 50.0, z, &mut flags);
    m.pos = [x, floor_height + rng.pick(&[0.0, 0.0, 5.0, 30.0, 120.0]), z];
    m.floor = Some(SurfaceRef::Collision(floor?));
    m.floor_height = floor_height;
    m.vel = [rng.f(-60.0, 60.0), rng.f(-40.0, 10.0), rng.f(-60.0, 60.0)];
    m.flags &= !c::MARIO_VANISH_CAP;
    Some(m)
}

/// Single calls from random states plus multi-tick step sequences.
fn exercise(sides: &Sides, seed: u64, span: f32, states: usize, outcomes: &mut Outcomes) {
    let Sides { world, trig, .. } = *sides;
    let mut rng = Lcg(seed);
    let mut produced = 0;
    while produced < states {
        let airborne = rng.next().is_multiple_of(2);
        let state = match rng.next() % 10 {
            0 => ledge_state(&mut rng, world),
            1 => wall_ground_state(&mut rng, world, trig),
            2 if span < 6000.0 => tunnel_state(&mut rng, world),
            _ => random_state(&mut rng, world, span, airborne),
        };
        let Some(m) = state else {
            continue;
        };
        let airborne = (airborne || m.action == c::ACT_FREEFALL) && m.action != c::ACT_WALKING;
        produced += 1;
        let mut env = Env {
            global_timer: rng.next(),
            area_terrain_type: rng.range(0, 6) as u16,
            level_num: rng.pick(&[c::LEVEL_BOB, c::LEVEL_LLL]),
            water_pseudo: rng.f(-100.0, 100.0),
            include_intangible: rng.next().is_multiple_of(4),
        };
        let context = format!("seed {seed} state {produced}");
        let calls = [
            MarioCall::GroundStep,
            MarioCall::AirStep(0),
            MarioCall::AirStep(c::AIR_STEP_CHECK_LEDGE_GRAB),
            MarioCall::AirStep(c::AIR_STEP_CHECK_LEDGE_GRAB | c::AIR_STEP_CHECK_HANG),
            MarioCall::StationaryGroundStep,
            MarioCall::StopAndSetHeightToFloor,
            MarioCall::BonkReflection(rng.next().is_multiple_of(2)),
            MarioCall::ApplyGravity,
            MarioCall::ApplyVerticalWind,
            MarioCall::VelFromPitchAndYaw,
            MarioCall::VelFromYaw,
            MarioCall::SetForwardVel(rng.f(-50.0, 50.0)),
            MarioCall::UpdateMovingSand,
            MarioCall::UpdateWindyGround,
        ];
        for call in calls {
            let mut start = m.clone();
            if matches!(call, MarioCall::BonkReflection(_)) && rng.next().is_multiple_of(2) {
                start.wall = Some(SurfaceRef::Collision(
                    (rng.next() % world.surfaces().len() as u32) as u16,
                ));
            }
            compare_call(sides, &start, &mut env, call, &context, outcomes);
        }
        // A 45-tick sequence: air steps until landing, then ground steps.
        let mut s = m.clone();
        let mut on_ground = !airborne;
        for tick in 0..45 {
            let call = if on_ground {
                MarioCall::GroundStep
            } else {
                MarioCall::AirStep(c::AIR_STEP_CHECK_LEDGE_GRAB)
            };
            let before = s.clone();
            s = compare_call(
                sides,
                &before,
                &mut env,
                call,
                &format!("{context} tick {tick}"),
                outcomes,
            );
            if s.floor.is_none() {
                break;
            }
            on_ground = if on_ground {
                s.pos[1] <= s.floor_height + 0.5
            } else {
                s.pos[1] <= s.floor_height
            };
        }
    }
}

#[test]
fn generated_constants_match_the_decomp_headers() {
    let decomp = decomp_constants();
    assert_eq!(decomp.len(), c::ALL.len());
    for ((name, value), (rust_name, rust_value)) in decomp.iter().zip(c::ALL) {
        assert_eq!(name, rust_name);
        assert_eq!(*value, rust_value, "{name}");
    }
}

#[test]
fn authored_terrain_steps_match_the_decomp() {
    let trig = computed_tables();
    let mut outcomes = Outcomes::default();
    for seed in [11u64, 12, 13] {
        let stream = authored_stream(seed);
        let bytes: Vec<u8> = stream.iter().flat_map(|w| w.to_be_bytes()).collect();
        let (mesh, _) = collision::decode(&bytes).unwrap();
        let world = CollisionWorld::load_area_terrain(&mesh).unwrap();
        let oracle = Oracle::load(&stream);
        oracle.set_trig(trig.sine_table(), trig.arctan_table());
        let sides = Sides {
            world: &world,
            trig: &trig,
            oracle: &oracle,
        };
        exercise(&sides, seed, 5000.0, 700, &mut outcomes);
    }
    println!("mario_step comparisons identical: {outcomes:?}");
    assert!(outcomes.checks > 30_000, "{outcomes:?}");
    // Every step outcome the authored terrain is built to reach must occur.
    let g = outcomes.ground;
    let a = outcomes.air;
    assert!(g[0] > 0 && g[1] > 0 && g[2] > 0, "{outcomes:?}");
    assert!(
        a[0] > 0 && a[1] > 0 && a[2] > 0 && a[3] > 0 && a[4] > 0 && a[6] > 0,
        "{outcomes:?}"
    );
    assert!(
        outcomes.sand > 0 && outcomes.wind > 0 && outcomes.pseudo_floor > 0,
        "{outcomes:?}"
    );
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn bob_steps_match_the_decomp_with_rom_tables() {
    use rustario64::import::{bob, engine, mio0, rom::Rom, version};
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
    assert_eq!(mesh, bob::import(&rom).unwrap().collision);
    let words: Vec<i16> = terrain[start..start + consumed]
        .chunks_exact(2)
        .map(|b| i16::from_be_bytes([b[0], b[1]]))
        .collect();
    let world = CollisionWorld::load_area_terrain(&mesh).unwrap();
    let oracle = Oracle::load(&words);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let mut outcomes = Outcomes::default();
    let sides = Sides {
        world: &world,
        trig: &trig,
        oracle: &oracle,
    };
    exercise(&sides, 99, 8000.0, 20_000, &mut outcomes);
    println!("BOB mario_step comparisons identical: {outcomes:?}");
    assert!(outcomes.checks > 500_000, "{outcomes:?}");
}

#[test]
fn surface_refs_round_trip() {
    for r in [
        None,
        Some(SurfaceRef::WaterPseudoFloor),
        Some(SurfaceRef::Collision(7)),
    ] {
        assert_eq!(surface_ref(surface_id(r)), r);
    }
}
