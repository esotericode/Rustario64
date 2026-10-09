//! Complete-frame differential tests with the original camera: the decomp's
//! Mario code and update_camera path compiled natively (c/tick.c with the
//! camera linked) and the Rust port (`simulation::game`) run the same level
//! entry and controller inputs independently; every Mario and camera word
//! must match after every frame. CI uses an authored playground with camera
//! surfaces under BOB's area rules, computed trig tables and authored
//! animations; the ignored test uses BOB and the owner's ROM.
#[path = "support/playground.rs"]
mod playground;
use playground::{Builder, Lcg, authored_animations, computed_tables, world};
use rustario64::{
    content::animation::MarioAnimations,
    simulation::{
        TickInput,
        camera::{D_CBUTTONS, L_CBUTTONS, R_CBUTTONS, U_CBUTTONS, system::GeoCamera},
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, R_TRIG, Z_TRIG},
        game::GameEntry,
        mario::{constants as c, tick::LevelEntry},
        math::TrigTables,
    },
};
use rustario64::{
    play::{Pad, Session, Stop},
    trace::{CameraInput, InputLog},
};
use rustario64_oracle::{
    Oracle,
    camera_trace::{GameScenario, RustStop, capture_game, first_difference},
};
use std::collections::{BTreeMap, BTreeSet};

/// An authored playground for the camera under BOB's area rules: a 9x9 field
/// of 1000-unit tiles with close-camera, boss-fight, camera-rotation and
/// free-roam tiles, tall blocks and a tunnel that obstruct the camera, a
/// hangable ceiling over close-camera floor and over radial floor, a ramp,
/// and a pit with a death plane.
fn playground() -> Vec<i16> {
    let mut b = Builder::default();
    for gz in 0..9 {
        for gx in 0..9 {
            let x = [-4500 + gx * 1000, -3500 + gx * 1000];
            let z = [-4500 + gz * 1000, -3500 + gz * 1000];
            if gz == 8 && gx == 8 {
                continue; // the pit
            }
            let surface = match (gx, gz) {
                (0..=2, 6) | (0, 7) => c::SURFACE_CLOSE_CAMERA,
                (6..=7, 0..=1) => c::SURFACE_BOSS_FIGHT_CAMERA,
                (8, 3) => c::SURFACE_CAMERA_ROTATE_LEFT,
                (8, 4) => c::SURFACE_CAMERA_ROTATE_RIGHT,
                (8, 5) => c::SURFACE_CAMERA_MIDDLE,
                (3, 8) => c::SURFACE_CAMERA_FREE_ROAM,
                (5, 8) => c::SURFACE_NO_CAM_COL_SLIPPERY,
                _ if (gx + gz) % 4 == 0 => c::SURFACE_NOISE_DEFAULT,
                _ => c::SURFACE_DEFAULT,
            };
            b.flat(surface, x, z, 0, true);
        }
    }
    b.flat(
        c::SURFACE_DEATH_PLANE,
        [3400, 4600],
        [3400, 4600],
        -3000,
        true,
    );
    // Obstructions: a tall pillar row and a wide wall.
    b.block(c::SURFACE_DEFAULT, [1200, 1600], [-400, 0], [0, 2500]);
    b.block(c::SURFACE_DEFAULT, [1200, 1600], [600, 1000], [0, 2500]);
    b.block(c::SURFACE_DEFAULT, [-2600, -1800], [-1500, 1500], [0, 1800]);
    // A tunnel: two walls under a ceiling.
    b.block(c::SURFACE_DEFAULT, [-600, -400], [2000, 3200], [0, 400]);
    b.block(c::SURFACE_DEFAULT, [400, 600], [2000, 3200], [0, 400]);
    b.flat(c::SURFACE_DEFAULT, [-600, 600], [2000, 3200], 400, false);
    b.flat(c::SURFACE_DEFAULT, [-600, 600], [2000, 3200], 400, true);
    // Hangable ceilings over close-camera and over radial floor.
    b.flat(
        c::SURFACE_HANGABLE,
        [-4400, -3600],
        [1600, 2400],
        320,
        false,
    );
    b.flat(
        c::SURFACE_HANGABLE,
        [2000, 3000],
        [-3000, -2000],
        320,
        false,
    );
    // Raised close-camera floors: steps and a ramp between the camera and
    // Mario, so the default camera's floor scan finds higher floors.
    b.block(
        c::SURFACE_CLOSE_CAMERA,
        [-4300, -3900],
        [600, 1000],
        [0, 150],
    );
    b.block(
        c::SURFACE_CLOSE_CAMERA,
        [-3500, -3000],
        [400, 1200],
        [0, 300],
    );
    b.block(c::SURFACE_DEFAULT, [-1400, -1000], [1600, 2000], [0, 250]);
    b.ramp(
        c::SURFACE_CLOSE_CAMERA,
        [-1500, -900],
        [-200, 700],
        [0, 350],
    );
    // A ramp up to a ledge.
    b.ramp(c::SURFACE_DEFAULT, [-1000, 0], [-3500, -2500], [0, 500]);
    b.flat(c::SURFACE_DEFAULT, [-1000, 0], [-2500, -2100], 500, true);
    b.stream()
}

const NONE: [i8; 2] = [0, 0];
const UP: [i8; 2] = [0, 80];
const DOWN: [i8; 2] = [0, -80];
const LEFT: [i8; 2] = [-80, 0];
const RIGHT: [i8; 2] = [80, 0];

fn script(segments: &[(usize, u16, [i8; 2])]) -> Vec<TickInput> {
    segments
        .iter()
        .flat_map(|&(n, buttons, stick)| {
            std::iter::repeat_n(
                TickInput {
                    buttons,
                    stick,
                    camera_yaw: 0,
                },
                n,
            )
        })
        .collect()
}

/// (name, spawn yaw in degrees, spawn position, inputs).
fn scripted() -> Vec<(&'static str, i16, [i16; 3], Vec<TickInput>)> {
    let (a, b, z, r) = (A_BUTTON, B_BUTTON, Z_TRIG, R_TRIG);
    let (cu, cd, cl, cr) = (U_CBUTTONS, D_CBUTTONS, L_CBUTTONS, R_CBUTTONS);
    vec![
        (
            "idle-sleep-fov",
            0,
            [0, 0, -1500],
            script(&[(900, 0, NONE), (1, a, NONE), (60, 0, NONE)]),
        ),
        (
            "radial-run-and-c-buttons",
            90,
            [-500, 0, -1500],
            script(&[
                (30, 0, UP),
                (1, cl, UP),
                (40, 0, UP),
                (1, cl, RIGHT),
                (40, 0, RIGHT),
                (1, cl, NONE),
                (30, 0, NONE),
                (1, cr, DOWN),
                (40, 0, DOWN),
                (1, cr, NONE),
                (1, 0, NONE),
                (1, cr, NONE),
                (40, 0, LEFT),
                (1, cd, NONE),
                (40, 0, UP),
                (1, cd, NONE),
                (20, 0, NONE),
                (1, cu, NONE),
                (40, 0, UP),
            ]),
        ),
        (
            "c-up-first-person",
            0,
            [0, 0, -2000],
            script(&[
                (20, 0, NONE),
                (1, cu, NONE),
                (40, 0, NONE),
                (30, 0, [60, 40]),
                (30, 0, [-70, -50]),
                (1, a, NONE),
                (40, 0, NONE),
                (1, cu, NONE),
                (30, 0, NONE),
                (1, cd, NONE),
                (40, 0, NONE),
                (1, cu, NONE),
                (30, 0, NONE),
                (1, b, NONE),
                (40, 0, UP),
            ]),
        ),
        (
            "close-camera-tiles",
            0,
            [-2500, 0, 1000],
            script(&[
                (60, 0, UP),
                (80, 0, NONE),
                (1, cl, NONE),
                (30, 0, NONE),
                (1, cr, NONE),
                (1, cr, NONE),
                (30, 0, NONE),
                (1, cd, NONE),
                (40, 0, LEFT),
                (1, cu, NONE),
                (40, 0, NONE),
                (1, cu, NONE),
                (40, 0, NONE),
                (1, a, NONE),
                (30, 0, DOWN),
                (60, 0, NONE),
            ]),
        ),
        (
            "close-camera-hanging",
            0,
            [-4000, 0, 2000],
            script(&[
                (40, 0, NONE),
                (40, a, NONE),
                (80, a, UP),
                (40, a, RIGHT),
                (20, a, NONE),
                (1, 0, NONE),
                (40, 0, NONE),
            ]),
        ),
        (
            "radial-hanging",
            0,
            [2500, 0, -2500],
            script(&[
                (40, a, NONE),
                (60, a, UP),
                (30, a, DOWN),
                (1, 0, NONE),
                (40, 0, NONE),
            ]),
        ),
        (
            "boss-fight-tiles",
            90,
            [1500, 0, -3000],
            script(&[
                (40, 0, UP),
                (60, 0, NONE),
                (1, cl, NONE),
                (40, 0, NONE),
                (1, cr, NONE),
                (40, 0, RIGHT),
                (1, cd, NONE),
                (40, 0, UP),
                (1, cu, NONE),
                (40, 0, LEFT),
                (60, 0, DOWN),
            ]),
        ),
        (
            "rotation-surfaces",
            0,
            [3500, 0, -1500],
            script(&[
                (40, 0, UP),
                (60, 0, NONE),
                (30, 0, UP),
                (60, 0, NONE),
                (30, 0, UP),
                (60, 0, NONE),
                (1, cl, NONE),
                (40, 0, NONE),
                (40, 0, DOWN),
            ]),
        ),
        (
            "mario-camera-r-button",
            0,
            [0, 0, -1000],
            script(&[
                (20, 0, NONE),
                (1, r, NONE),
                (60, r, UP),
                (1, 0, NONE),
                (1, cd, NONE),
                (40, 0, RIGHT),
                (1, cl, NONE),
                (40, 0, NONE),
                (1, r, NONE),
                (40, 0, UP),
                (1, r, NONE),
                (1, cu, NONE),
                (40, 0, NONE),
            ]),
        ),
        (
            "tunnel-and-walls",
            0,
            [0, 0, 1000],
            script(&[
                (60, 0, UP),
                (40, 0, NONE),
                (30, 0, DOWN),
                (40, 0, LEFT),
                (60, 0, NONE),
                (30, 0, RIGHT),
                (60, 0, UP),
            ]),
        ),
        (
            "jumps-dives-pounds",
            0,
            [-1000, 0, -3800],
            script(&[
                (20, 0, UP),
                (1, a, UP),
                (20, 0, UP),
                (1, b, UP),
                (40, 0, NONE),
                (1, a, NONE),
                (10, 0, NONE),
                (1, z, NONE),
                (40, 0, NONE),
                (30, 0, UP),
                (1, z, UP),
                (1, z | a, UP),
                (40, 0, NONE),
            ]),
        ),
        (
            "close-camera-steps",
            0,
            [-3700, 0, 2000],
            script(&[
                (20, 0, NONE),
                (30, 0, DOWN),
                (40, 0, NONE),
                (30, 0, LEFT),
                (40, 0, NONE),
                (1, a, DOWN),
                (40, 0, DOWN),
                (60, 0, NONE),
                (40, 0, RIGHT),
                (60, 0, NONE),
            ]),
        ),
        (
            "close-camera-c-up-exit-against-wall",
            0,
            [-2200, 0, 1580],
            script(&[
                (60, 0, NONE),
                (1, cu, NONE),
                (40, 0, NONE),
                (20, 0, [40, 0]),
                (1, cd, NONE),
                (40, 0, NONE),
                (1, cu, NONE),
                (40, 0, NONE),
                (1, cl, NONE),
                (40, 0, NONE),
                (20, 0, DOWN),
                (1, cu, NONE),
                (40, 0, NONE),
                (1, cr, NONE),
                (40, 0, NONE),
            ]),
        ),
        (
            "close-camera-ramp",
            0,
            [-1200, 0, 1200],
            script(&[
                (30, 0, DOWN),
                (60, 0, NONE),
                (30, 0, UP),
                (60, 0, NONE),
                (20, 0, LEFT),
                (60, 0, NONE),
            ]),
        ),
        (
            "free-roam-and-slippery-tiles",
            0,
            [-1000, 0, 3000],
            script(&[
                (40, 0, UP),
                (60, 0, NONE),
                (1, cd, NONE),
                (40, 0, RIGHT),
                (40, 0, RIGHT),
                (60, 0, NONE),
            ]),
        ),
    ]
}

/// Random held inputs with C buttons and R: segments of 1-40 ticks.
fn fuzz_inputs(seed: u64, ticks: usize) -> Vec<TickInput> {
    let mut rng = Lcg(seed);
    let mut out = Vec::with_capacity(ticks);
    while out.len() < ticks {
        let n = rng.range(1, 40) as usize;
        let stick = if rng.chance(3) {
            [0, 0]
        } else {
            [rng.range(-128, 127) as i8, rng.range(-128, 127) as i8]
        };
        let mut buttons = 0;
        for (button, odds) in [
            (A_BUTTON, 4),
            (B_BUTTON, 8),
            (Z_TRIG, 9),
            (U_CBUTTONS, 9),
            (D_CBUTTONS, 9),
            (L_CBUTTONS, 6),
            (R_CBUTTONS, 6),
            (R_TRIG, 14),
        ] {
            if rng.chance(odds) {
                buttons |= button;
            }
        }
        let tap = rng.chance(2);
        for i in 0..n {
            out.push(TickInput {
                buttons: if tap && i > 0 { 0 } else { buttons },
                stick,
                camera_yaw: 0,
            });
        }
    }
    out.truncate(ticks);
    out
}

struct Coverage {
    frames: usize,
    modes: BTreeSet<u32>,
    actions: BTreeSet<u32>,
    events: usize,
    stops: Vec<String>,
}

fn compare(
    oracle: &Oracle,
    scenario: &GameScenario<'_>,
    name: &str,
    inputs: &[TickInput],
    coverage: &mut Coverage,
) {
    let (rust, stop) = scenario.rust(inputs);
    let frames = rust.len() - 1;
    let kept = match &stop {
        // The panicking frame has no Rust words; the frames before it compare.
        Some(RustStop::Panic(message)) => {
            coverage
                .stops
                .push(format!("{name} frame {}: {message}", frames + 1));
            frames
        }
        Some(RustStop::Camera(message)) => {
            coverage
                .stops
                .push(format!("{name} frame {frames}: {message}"));
            frames
        }
        None => frames,
    };
    let native = scenario.native(oracle, &inputs[..kept]);
    for (i, (expected, actual)) in native.iter().zip(&rust).enumerate() {
        if let Some(d) = first_difference(expected, actual) {
            let input = if i == 0 { None } else { Some(inputs[i - 1]) };
            panic!("{name}: frame {i} ({input:?}): {d}");
        }
        coverage.modes.insert(actual["camera.c.mode"]);
        coverage.actions.insert(actual["m.action"]);
        coverage.events += actual["events.count"] as usize;
    }
    coverage.frames += kept;
}

fn scenario<'a>(
    world: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    yaw: i16,
    pos: [i16; 3],
    rng_seed: u16,
) -> GameScenario<'a> {
    let mario = LevelEntry::from_level_script(c::LEVEL_BOB, 1, yaw, pos, 0, 1);
    GameScenario {
        collision: world,
        trig,
        anims,
        entry: GameEntry {
            mario,
            camera: GeoCamera {
                mode: 1,
                pos: [0.0, 2000.0, 6000.0],
                focus: [500.0, 0.0, -800.0],
            },
            act_num: 1,
            rng_seed,
        },
    }
}

fn mode_names() -> BTreeMap<u32, &'static str> {
    c::ALL
        .iter()
        .filter(|(name, _)| name.starts_with("CAMERA_MODE_"))
        .map(|(name, value)| (*value as u32, *name))
        .collect()
}

fn report(label: &str, coverage: &Coverage) {
    let names = mode_names();
    let modes: Vec<&str> = coverage
        .modes
        .iter()
        .map(|m| names.get(m).copied().unwrap_or("?"))
        .collect();
    println!(
        "{label}: {} frames identical, {} actions, {} events; camera modes: {}",
        coverage.frames,
        coverage.actions.len(),
        coverage.events,
        modes.join(" ")
    );
    for stop in &coverage.stops {
        println!("{label}: stopped: {stop}");
    }
}

#[test]
fn authored_playground_frames_with_the_camera_match_the_decomp() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0002);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut coverage = Coverage {
        frames: 0,
        modes: BTreeSet::new(),
        actions: BTreeSet::new(),
        events: 0,
        stops: vec![],
    };
    for (i, (name, yaw, pos, inputs)) in scripted().into_iter().enumerate() {
        let s = scenario(
            &world,
            &trig,
            &anims,
            yaw,
            pos,
            0x1234u16.wrapping_mul(i as u16),
        );
        compare(&oracle, &s, name, &inputs, &mut coverage);
    }
    let starts = [
        (0, [0, 0, -1500]),
        (90, [-2500, 0, 1000]),
        (180, [1500, 0, -3000]),
        (-90, [3500, 0, -1500]),
        (45, [0, 0, 1000]),
        (135, [-4000, 0, 1500]),
        (0, [-1000, 0, 3000]),
        (-45, [2500, 0, -2500]),
    ];
    for seed in 0..16u64 {
        let (yaw, pos) = starts[seed as usize % starts.len()];
        let s = scenario(&world, &trig, &anims, yaw, pos, seed as u16 * 0x0F0F);
        compare(
            &oracle,
            &s,
            &format!("fuzz-{seed}"),
            &fuzz_inputs(seed, 900),
            &mut coverage,
        );
    }
    report("authored camera playground", &coverage);
    for mode in [
        c::CAMERA_MODE_RADIAL,
        c::CAMERA_MODE_CLOSE,
        c::CAMERA_MODE_C_UP,
        c::CAMERA_MODE_BOSS_FIGHT,
        c::CAMERA_MODE_FREE_ROAM,
    ] {
        assert!(
            coverage.modes.contains(&(mode as u32)),
            "camera mode {mode} was not exercised"
        );
    }
}

#[test]
fn camera_frames_are_identical_at_every_presentation_rate() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0003);
    let inputs: Vec<TickInput> = scripted()
        .into_iter()
        .find(|(name, ..)| *name == "radial-run-and-c-buttons")
        .unwrap()
        .3;
    let s = scenario(&world, &trig, &anims, 90, [-500, 0, -1500], 7);
    let (reference, stop) = s.rust(&inputs);
    assert_eq!(stop, None);
    for hz in [15, 30, 60, 120, 144] {
        assert!(
            s.rust_at(&inputs, hz) == reference,
            "{hz} Hz presentation changed the frames"
        );
    }
}

/// Held controls as a player gives them: segments of held directions,
/// buttons, C buttons and R, with single-tick taps and releases between.
fn held_controls(seed: u64, ticks: usize) -> Vec<Pad> {
    let mut rng = Lcg(seed ^ 0x9AD);
    let mut pad = Pad::default();
    (0..ticks)
        .map(|i| {
            if i % 12 == 0 {
                pad = Pad {
                    up: rng.chance(2),
                    down: rng.chance(6),
                    left: rng.chance(4),
                    right: rng.chance(4),
                    walk: rng.chance(5),
                    a: rng.chance(4),
                    b: rng.chance(6),
                    z: rng.chance(6),
                    r: rng.chance(12),
                    c_up: rng.chance(10),
                    c_down: rng.chance(10),
                    c_left: rng.chance(6),
                    c_right: rng.chance(6),
                };
            } else if rng.chance(6) {
                match rng.next() % 5 {
                    0 => pad.a ^= true,
                    1 => pad.b ^= true,
                    2 => pad.z ^= true,
                    3 => pad.c_left ^= true,
                    _ => pad.c_right ^= true,
                }
            }
            pad
        })
        .collect()
}

/// The viewer's play path: a `play::Session` driven by held controls logs
/// its inputs (with the camera yaws Mario read); the decomp replaying that
/// log with its own camera reports the session's words after every frame.
/// Re-entering the level and playing the same controls gives the same words.
#[test]
fn played_sessions_with_the_camera_replay_exactly_in_the_decomp() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0004);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let starts = [
        (0, [0, 0, -1500]),
        (90, [-2500, 0, 1000]),
        (45, [1500, 0, -3000]),
        (-90, [3500, 0, -1500]),
        (180, [-4000, 0, 2000]),
        (30, [0, 0, 1000]),
    ];
    let (mut frames, mut yaws, mut modes) = (0, BTreeSet::new(), BTreeSet::new());
    for (seed, (yaw, pos)) in starts.into_iter().enumerate() {
        let s = scenario(&world, &trig, &anims, yaw, pos, seed as u16);
        let mut session = Session::new(&world, &trig, &anims, s.entry);
        let initial = capture_game(session.game());
        let controls = held_controls(seed as u64, 600);
        let mut words = vec![initial];
        for pad in &controls {
            if !session.step(pad) {
                break;
            }
            if matches!(session.stopped(), Some(Stop::Panic(_))) {
                break;
            }
            words.push(capture_game(session.game()));
        }
        let log = session.input_log("test", "synthetic", "playground");
        let log = InputLog::from_json(&log.to_json_pretty()).unwrap();
        assert_eq!(log.camera, CameraInput::Reference);
        let replayed = &log.inputs[..words.len() - 1];
        let (rust, _) = s.rust(replayed);
        assert!(
            rust[..words.len()] == words[..],
            "a replay of the log reaches the same states"
        );
        // The yaw each logged frame records is the one the replay's Mario
        // read: the camera's yaw after the previous frame.
        for (i, input) in replayed.iter().enumerate() {
            assert_eq!(
                rust[i]["world.camera.yaw"] as i32 as i16,
                input.camera_yaw,
                "frame {} read a different camera yaw",
                i + 1
            );
        }
        let native = s.native(&oracle, replayed);
        for (i, (expected, actual)) in native.iter().zip(&words).enumerate() {
            if let Some(d) = first_difference(expected, actual) {
                panic!("played session {seed}: frame {i}: {d}");
            }
            modes.insert(actual["camera.c.mode"]);
        }
        session.reset();
        for (pad, expected) in controls.iter().zip(&words[1..]) {
            assert!(session.step(pad));
            assert!(&capture_game(session.game()) == expected);
        }
        frames += words.len() - 1;
        yaws.extend(log.inputs.iter().map(|i| i.camera_yaw));
    }
    println!(
        "played sessions with the camera: {frames} frames identical, {} camera yaws, modes {:?}",
        yaws.len(),
        modes
    );
    assert!(frames > 1500, "sessions stopped early: {frames} frames");
    assert!(yaws.len() > 100, "the camera barely turned");
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; BOB collision, trig tables and animations from the owner's ROM"]
fn bob_frames_with_the_camera_match_the_decomp_with_rom_data() {
    use rustario64::import::{animation, bob, collision, engine, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
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
    oracle.set_mario_animations(&anims);
    // The area's camera node holds the values the pinned
    // levels/bob/areas/1/geo.inc.c gives GEO_CAMERA(1, 0, 2000, 6000, 3072,
    // 0, -4608, geo_camera_main) inside GEO_CAMERA_FRUSTUM_WITH_FUNC(45, 100,
    // 30000, geo_camera_fov): this aligns the version adapter's addresses.
    let node = imported.visual.as_ref().unwrap().camera.unwrap();
    assert_eq!(
        (node.mode, node.position, node.focus),
        (1, [0, 2000, 6000], [3072, 0, -4608])
    );
    assert_eq!((node.fov_degrees, node.near, node.far), (45, 100, 30000));
    assert_eq!(
        (node.callback, node.perspective_callback),
        (
            version::AREA_CAMERA_CALLBACKS[0].1,
            Some(version::AREA_CAMERA_CALLBACKS[1].1)
        )
    );
    let script_entry = GameEntry::script_start(&imported.level, &node).unwrap();
    let bob = |yaw: i16, pos: [i16; 3], rng_seed: u16| GameScenario {
        collision: &world,
        trig: &trig,
        anims: &anims,
        entry: GameEntry {
            mario: LevelEntry {
                spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                    1,
                    script_entry.mario.spawn.area_index as u8,
                    yaw,
                    pos,
                ),
                ..script_entry.mario
            },
            rng_seed,
            ..script_entry
        },
    };
    let mut coverage = Coverage {
        frames: 0,
        modes: BTreeSet::new(),
        actions: BTreeSet::new(),
        events: 0,
        stops: vec![],
    };
    // The script start, with every scripted input set.
    let (_, script_yaw, script_pos) = imported.level.mario_start.unwrap();
    for (name, _, _, inputs) in scripted() {
        let s = GameScenario {
            entry: script_entry,
            ..bob(script_yaw, script_pos, 0)
        };
        compare(
            &oracle,
            &s,
            &format!("bob-script-start-{name}"),
            &inputs,
            &mut coverage,
        );
    }
    // Starts on BOB's camera surfaces and across the course, fuzzed.
    let mut starts = vec![];
    for surface_type in [
        c::SURFACE_CLOSE_CAMERA,
        c::SURFACE_BOSS_FIGHT_CAMERA,
        c::SURFACE_CAMERA_ROTATE_LEFT,
        c::SURFACE_HANGABLE,
    ] {
        let found: Vec<_> = world
            .surfaces()
            .iter()
            .filter(|s| s.surface_type == surface_type)
            .collect();
        assert!(!found.is_empty(), "BOB has surface type {surface_type:#x}");
        for (i, s) in found.iter().enumerate().step_by((found.len() / 6).max(1)) {
            let center: [i32; 3] = std::array::from_fn(|k| {
                (i32::from(s.vertex1[k]) + i32::from(s.vertex2[k]) + i32::from(s.vertex3[k])) / 3
            });
            let mut flags = Default::default();
            let probe_y = if surface_type == c::SURFACE_HANGABLE {
                center[1] - 50
            } else {
                center[1] + 100
            };
            let (height, floor) = world.find_floor(
                center[0] as f32,
                probe_y as f32,
                center[2] as f32,
                &mut flags,
            );
            if floor.is_some() {
                starts.push((
                    (i as i16 * 47) % 360 - 180,
                    [center[0] as i16, height as i16, center[2] as i16],
                ));
            }
        }
    }
    let mut rng = Lcg(0xCA3E);
    while starts.len() < 40 {
        let (x, z) = (rng.range(-7500, 7500), rng.range(-7500, 7500));
        let mut flags = Default::default();
        let (height, floor) = world.find_floor(x as f32, 9000.0, z as f32, &mut flags);
        if floor.is_some() {
            starts.push((
                rng.range(-180, 179) as i16,
                [x as i16, height as i16, z as i16],
            ));
        }
    }
    for (i, (yaw, pos)) in starts.into_iter().enumerate() {
        let s = bob(yaw, pos, (i as u16).wrapping_mul(0x2F1D));
        compare(
            &oracle,
            &s,
            &format!("bob-fuzz-{i}"),
            &fuzz_inputs(3000 + i as u64, 1800),
            &mut coverage,
        );
    }
    report("BOB with the camera", &coverage);
    for mode in [
        c::CAMERA_MODE_RADIAL,
        c::CAMERA_MODE_CLOSE,
        c::CAMERA_MODE_C_UP,
        c::CAMERA_MODE_BOSS_FIGHT,
    ] {
        assert!(
            coverage.modes.contains(&(mode as u32)),
            "camera mode {mode} was not exercised on BOB"
        );
    }
    let s = GameScenario {
        entry: script_entry,
        ..bob(script_yaw, script_pos, 0)
    };
    let inputs = fuzz_inputs(4243, 900);
    let (reference, stop) = s.rust(&inputs);
    let kept = reference.len() - 1;
    let native = s.native(&oracle, &inputs[..kept]);
    assert!(
        native
            .iter()
            .zip(&reference)
            .all(|(n, r)| first_difference(n, r).is_none())
    );
    if stop.is_none() {
        for hz in [30, 60, 144] {
            assert!(
                s.rust_at(&inputs, hz) == reference,
                "BOB {hz} Hz presentation changed the frames"
            );
        }
        println!("BOB with the camera: identical at 30/60/144 Hz presentation over {kept} frames");
    }
}
