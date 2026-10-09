//! Full-tick differential tests: the decomp's Mario code compiled natively
//! (c/tick.c) and the Rust port run the same level entry and inputs
//! independently, and every compared word must match after every tick. CI
//! uses an authored playground, computed trig tables and an authored
//! animation table; the ignored test uses BOB and the owner's ROM.
#[path = "support/playground.rs"]
mod playground;
use playground::{Builder, Lcg, authored_animations, computed_tables, world};
use rustario64::{
    content::animation::MarioAnimations,
    import::collision,
    play::{Pad, Session, Stop},
    presentation::GraphicsOptions,
    simulation::{
        TickInput,
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, Z_TRIG},
        mario::{constants as c, tick::LevelEntry},
        math::TrigTables,
    },
    trace::{self, InputLog},
};
use rustario64_oracle::{
    Oracle, TickSetup,
    input_trace::world_digest,
    tick_trace::{TickScenario, capture},
};
use std::collections::{BTreeMap, BTreeSet};

/// An authored playground: an 8x8 field of 1000-unit tiles (special floors
/// on the north row, a pit with a death plane in the north-east corner), a
/// low block for ledge grabs, a taller one for climbing, a high wall for
/// wall kicks, a hangable ceiling, a gentle ramp and a steep slope.
fn playground() -> Vec<i16> {
    let mut b = Builder::default();
    let north_row = [
        c::SURFACE_BURNING,
        c::SURFACE_SHALLOW_QUICKSAND,
        c::SURFACE_VERY_SLIPPERY,
        c::SURFACE_SLIPPERY,
        c::SURFACE_NOT_SLIPPERY,
        c::SURFACE_HORIZONTAL_WIND,
        c::SURFACE_QUICKSAND,
    ];
    for gz in 0..8 {
        for gx in 0..8 {
            let x = [-4000 + gx * 1000, -3000 + gx * 1000];
            let z = [-4000 + gz * 1000, -3000 + gz * 1000];
            if gz == 7 && gx == 7 {
                continue; // the pit
            }
            if gz == 7 {
                let surface = north_row[gx as usize];
                if surface == c::SURFACE_HORIZONTAL_WIND {
                    let p = [
                        [x[0], 0, z[0]],
                        [x[0], 0, z[1]],
                        [x[1], 0, z[0]],
                        [x[1], 0, z[1]],
                    ];
                    // Wind pushing toward +x (force high byte: angle 0x40xx).
                    b.quad(surface, p, [0, 1, 0], Some(0x40));
                } else {
                    b.flat(surface, x, z, 0, true);
                }
            } else {
                let surface = if (gx + gz) % 5 == 0 {
                    c::SURFACE_NOISE_DEFAULT
                } else if (gx * gz) % 7 == 3 {
                    c::SURFACE_HARD
                } else {
                    c::SURFACE_DEFAULT
                };
                b.flat(surface, x, z, 0, true);
            }
        }
    }
    b.flat(
        c::SURFACE_DEATH_PLANE,
        [2900, 4100],
        [2900, 4100],
        -3000,
        true,
    );
    b.block(c::SURFACE_DEFAULT, [1000, 1600], [-300, 300], [0, 180]);
    b.block(c::SURFACE_DEFAULT, [1000, 1600], [1200, 1800], [0, 400]);
    b.block(c::SURFACE_DEFAULT, [-2400, -1600], [-1200, 1200], [0, 1400]);
    b.flat(c::SURFACE_HANGABLE, [-1000, 0], [-3000, -2000], 320, false);
    b.ramp(c::SURFACE_DEFAULT, [-500, 500], [1500, 2500], [0, 400]);
    b.flat(c::SURFACE_DEFAULT, [-500, 500], [2500, 2900], 400, true);
    b.ramp(c::SURFACE_DEFAULT, [600, 1200], [1500, 2500], [0, 1300]);
    b.stream()
}

/// Camera yaws that make "stick up" move Mario along +x, -x and +z.
const TOWARD_POS_X: i16 = -0x4000;
const TOWARD_NEG_X: i16 = 0x4000;
const TOWARD_POS_Z: i16 = i16::MIN;
const UP: [i8; 2] = [0, 80];
const DOWN: [i8; 2] = [0, -80];
const HALF_UP: [i8; 2] = [0, 40];
const NONE: [i8; 2] = [0, 0];

/// An input script: (ticks, buttons, stick) segments with a fixed camera yaw.
fn script(camera_yaw: i16, segments: &[(usize, u16, [i8; 2])]) -> Vec<TickInput> {
    segments
        .iter()
        .flat_map(|&(n, buttons, stick)| {
            std::iter::repeat_n(
                TickInput {
                    buttons,
                    stick,
                    camera_yaw,
                },
                n,
            )
        })
        .collect()
}

/// Presses `button` every `period` ticks while holding the stick.
fn tapping(
    camera_yaw: i16,
    ticks: usize,
    period: usize,
    button: u16,
    stick: [i8; 2],
) -> Vec<TickInput> {
    (0..ticks)
        .map(|i| TickInput {
            buttons: if i % period == 0 { button } else { 0 },
            stick,
            camera_yaw,
        })
        .collect()
}

/// (name, spawn yaw in degrees, spawn position, inputs).
fn scripted_scenarios() -> Vec<(&'static str, i16, [i16; 3], Vec<TickInput>)> {
    let (a, b, z) = (A_BUTTON, B_BUTTON, Z_TRIG);
    let mut out = vec![
        ("idle-and-sleep", 0, [0, 0, 0], script(0, &[(900, 0, NONE)])),
        (
            "walk-run-turn-brake",
            0,
            [0, 0, 0],
            script(
                TOWARD_POS_Z,
                &[
                    (10, 0, NONE),
                    (60, 0, UP),
                    (20, 0, NONE),
                    (30, 0, HALF_UP),
                    (25, 0, DOWN),
                    (30, 0, NONE),
                    (40, 0, [60, 60]),
                    (12, 0, [-60, -60]),
                    (40, 0, NONE),
                ],
            ),
        ),
        (
            "jump-chains",
            0,
            [-3000, 0, -3500],
            [18usize, 20, 22, 25, 30]
                .iter()
                .flat_map(|&period| tapping(TOWARD_POS_Z, 120, period, a, UP))
                .collect(),
        ),
        (
            "long-jump-backflip-side-flip",
            0,
            [0, 0, -3000],
            script(
                TOWARD_POS_Z,
                &[
                    (30, 0, UP),
                    (2, z, UP),
                    (1, z | a, UP),
                    (40, 0, NONE),
                    (12, z, NONE),
                    (1, z | a, NONE),
                    (50, 0, NONE),
                    (30, 0, UP),
                    (2, 0, DOWN),
                    (1, a, DOWN),
                    (50, 0, NONE),
                ],
            ),
        ),
        (
            "crouch-crawl-slide-kick-dive",
            0,
            [-2500, 0, -3500],
            script(
                TOWARD_POS_Z,
                &[
                    (10, z, NONE),
                    (40, z, UP),
                    (10, 0, NONE),
                    (30, 0, UP),
                    (1, z, UP),
                    (1, z | b, UP),
                    (40, 0, NONE),
                    (30, 0, UP),
                    (1, b, UP),
                    (20, 0, UP),
                    (1, a, UP),
                    (40, 0, NONE),
                    (30, 0, UP),
                    (1, b, UP),
                    (30, 0, NONE),
                ],
            ),
        ),
        (
            "punches-kicks-ground-pound",
            0,
            [-3500, 0, 0],
            script(
                0,
                &[
                    (1, b, NONE),
                    (6, 0, NONE),
                    (1, b, NONE),
                    (6, 0, NONE),
                    (1, b, NONE),
                    (30, 0, NONE),
                    (1, a, NONE),
                    (6, 0, NONE),
                    (1, b, NONE),
                    (40, 0, NONE),
                    (1, a, NONE),
                    (8, 0, NONE),
                    (1, z, NONE),
                    (60, 0, NONE),
                    (10, z, NONE),
                    (1, z | b, NONE),
                    (40, 0, NONE),
                ],
            ),
        ),
    ];
    out.push((
        "triple-jumps",
        0,
        [-3500, 0, -3500],
        (0..3)
            .flat_map(|_| {
                let mut v = script(TOWARD_POS_Z, &[(25, 0, UP)]);
                v.extend(tapping(TOWARD_POS_Z, 110, 2, a, UP));
                v.extend(script(TOWARD_POS_X, &[(40, 0, UP), (30, 0, NONE)]));
                v
            })
            .collect(),
    ));
    out.push((
        "wall-kicks",
        -90,
        [-1350, 0, 0],
        (0..6)
            .flat_map(|i| {
                let mut v = tapping(TOWARD_NEG_X, 40, 2 + i % 3, a, UP);
                v.extend(script(TOWARD_POS_X, &[(25, 0, UP), (20, 0, NONE)]));
                v
            })
            .collect(),
    ));
    out.push((
        "ledge-grab-climb-drop",
        90,
        [600, 0, 0],
        (0..5)
            .flat_map(|i| {
                let mut v = script(TOWARD_POS_X, &[(4 + i * 3, 0, UP), (1, a, UP), (30, 0, UP)]);
                v.extend(script(
                    TOWARD_POS_X,
                    &[(15, 0, NONE), (1, a, NONE), (40, 0, NONE)],
                ));
                v.extend(script(TOWARD_NEG_X, &[(30, 0, UP), (20, 0, NONE)]));
                v
            })
            .collect(),
    ));
    out.push((
        "hanging",
        0,
        [-500, 0, -2500],
        script(
            TOWARD_POS_X,
            &[
                (40, a, NONE),
                (60, a, UP),
                (30, a, DOWN),
                (20, a, NONE),
                (1, a | z, NONE),
                (40, 0, NONE),
                (60, a, NONE),
                (10, 0, NONE),
                (40, 0, NONE),
            ],
        ),
    ));
    out.push((
        "ramps-and-steep-slopes",
        0,
        [0, 0, 1200],
        script(
            TOWARD_POS_Z,
            &[
                (90, 0, UP),
                (1, a, UP),
                (40, 0, UP),
                (20, 0, NONE),
                (40, 0, DOWN),
                (60, 0, NONE),
            ],
        )
        .into_iter()
        .chain(script(
            TOWARD_POS_X,
            &[(30, 0, UP), (90, 0, NONE), (1, a, NONE), (60, 0, NONE)],
        ))
        .collect(),
    ));
    out.push((
        "steep-slope-climbs",
        0,
        [900, 0, 1200],
        (0..4)
            .flat_map(|i| {
                let mut v = script(TOWARD_POS_Z, &[(20 + i * 10, 0, UP)]);
                v.extend(tapping(TOWARD_POS_Z, 30, 7, a, UP));
                v.extend(script(TOWARD_POS_Z, &[(60, 0, NONE), (20, 0, DOWN)]));
                v
            })
            .collect(),
    ));
    out.push((
        "butt-slides",
        180,
        [900, 680, 2000],
        script(TOWARD_POS_X, &[(40, 0, NONE)])
            .into_iter()
            .chain(tapping(TOWARD_POS_X, 60, 15, a, UP))
            .chain(script(TOWARD_POS_X, &[(60, 0, NONE)]))
            .collect(),
    ));
    out.push((
        "special-floors",
        0,
        [-3500, 0, 2300],
        script(TOWARD_POS_Z, &[(40, 0, UP), (60, 0, NONE), (40, 0, UP)])
            .into_iter()
            .chain(script(TOWARD_POS_X, &[(200, 0, UP), (60, 0, NONE)]))
            .chain(tapping(TOWARD_NEG_X, 120, 9, a, UP))
            .collect(),
    ));
    out.push((
        "pit-and-death-plane",
        0,
        [2500, 0, 2500],
        script(TOWARD_POS_X, &[(30, 0, UP)])
            .into_iter()
            .chain(script(TOWARD_POS_Z, &[(40, 0, UP), (150, 0, NONE)]))
            .collect(),
    ));
    out
}

/// Random held inputs: segments of 1-40 ticks with random sticks and
/// buttons, and a drifting camera yaw.
fn fuzz_inputs(seed: u64, ticks: usize) -> Vec<TickInput> {
    let mut rng = Lcg(seed);
    let mut camera_yaw = rng.next() as i16;
    let mut out = Vec::with_capacity(ticks);
    while out.len() < ticks {
        let n = rng.range(1, 40) as usize;
        let stick = if rng.chance(4) {
            [0, 0]
        } else {
            [rng.range(-128, 127) as i8, rng.range(-128, 127) as i8]
        };
        let mut buttons = 0;
        for (button, odds) in [(A_BUTTON, 3), (B_BUTTON, 6), (Z_TRIG, 7)] {
            if rng.chance(odds) {
                buttons |= button;
            }
        }
        let tap = rng.chance(2);
        let drift = rng.range(-0x300, 0x300) as i16;
        for i in 0..n {
            camera_yaw = camera_yaw.wrapping_add(drift);
            out.push(TickInput {
                buttons: if tap && i > 0 { 0 } else { buttons },
                stick,
                camera_yaw,
            });
        }
    }
    out.truncate(ticks);
    out
}

struct Coverage {
    ticks: usize,
    actions: BTreeSet<u32>,
    events: usize,
    unsupported: Vec<String>,
}

/// Run one scenario on both sides; compare every tick. When the Rust port
/// reaches a path it does not support, the comparison covers the ticks
/// before it.
fn compare(
    oracle: &Oracle,
    scenario: &TickScenario<'_>,
    inputs: &[TickInput],
    coverage: &mut Coverage,
) {
    let (rust, unsupported) = scenario.rust_until_unsupported(inputs);
    if let Some(message) = unsupported {
        coverage.unsupported.push(format!(
            "{} tick {}: {message}",
            scenario.scenario,
            rust.len() + 1
        ));
    }
    if rust.is_empty() {
        return;
    }
    let native = scenario.native(oracle, &inputs[..rust.len()]);
    if let Some(d) = diff(
        &native.metadata.initial_state.fields,
        &scenario.rust_initial(),
    ) {
        panic!("{}: level entry: {d}", scenario.scenario);
    }
    for (frame, words) in native.frames.iter().zip(&rust) {
        if let Some(d) = diff(&frame.state.fields, words) {
            panic!(
                "{}: tick {} ({:?}): {d}",
                scenario.scenario, frame.tick, frame.input
            );
        }
        coverage.actions.insert(words["m.action"]);
        coverage.events += words["events.count"] as usize;
    }
    coverage.ticks += rust.len();
}

/// The first differing word, as "name: native X vs Rust Y".
fn diff(native: &BTreeMap<String, u32>, rust: &BTreeMap<String, u32>) -> Option<String> {
    for (name, value) in native {
        match rust.get(name) {
            None => return Some(format!("{name}: missing on the Rust side")),
            Some(other) if other != value => {
                return Some(format!("{name}: native {value:#X} vs Rust {other:#X}"));
            }
            _ => {}
        }
    }
    rust.keys()
        .find(|name| !native.contains_key(*name))
        .map(|name| format!("{name}: missing on the native side"))
}

fn scenario<'a>(
    world: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    stream: &[i16],
    name: &str,
    yaw: i16,
    pos: [i16; 3],
) -> TickScenario<'a> {
    TickScenario {
        collision: world,
        trig,
        anims,
        setup: TickSetup::from_level_script(c::LEVEL_BOB, 1, yaw, pos, 0, 1),
        rom_sha1: "synthetic".into(),
        world_digest: world_digest(stream, trig),
        scenario: name.into(),
        course: 1,
    }
}

fn new_coverage() -> Coverage {
    Coverage {
        ticks: 0,
        actions: BTreeSet::new(),
        events: 0,
        unsupported: vec![],
    }
}

fn action_names() -> BTreeMap<u32, &'static str> {
    c::ALL
        .iter()
        .filter(|(name, _)| {
            name.starts_with("ACT_")
                && !name.starts_with("ACT_FLAG_")
                && !name.starts_with("ACT_GROUP_")
        })
        .map(|(name, value)| (*value as u32, *name))
        .collect()
}

fn report(label: &str, coverage: &Coverage) {
    let names = action_names();
    let actions: Vec<&str> = coverage
        .actions
        .iter()
        .map(|a| names.get(a).copied().unwrap_or("?"))
        .collect();
    println!(
        "{label}: {} ticks identical, {} distinct actions, {} events; actions: {}",
        coverage.ticks,
        coverage.actions.len(),
        coverage.events,
        actions.join(" ")
    );
    for u in &coverage.unsupported {
        println!("{label}: stopped at an unsupported path: {u}");
    }
}

#[test]
fn authored_playground_ticks_match_the_decomp() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0001);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut coverage = new_coverage();
    for (name, yaw, pos, inputs) in scripted_scenarios() {
        let s = scenario(&world, &trig, &anims, &stream, name, yaw, pos);
        compare(&oracle, &s, &inputs, &mut coverage);
    }
    let starts = [
        (0, [0, 0, 0]),
        (90, [700, 0, 0]),
        (-90, [-1350, 0, 0]),
        (45, [-500, 0, -2500]),
        (0, [0, 0, 1200]),
        (180, [900, 0, 1400]),
        (0, [-3500, 0, 2400]),
        (30, [2500, 0, 2500]),
        (135, [1300, 180, 0]),
        (-45, [1300, 400, 1500]),
        (60, [-2000, 1400, 0]),
        (0, [0, 400, 2700]),
    ];
    for seed in 0..24u64 {
        let (yaw, pos) = starts[seed as usize % starts.len()];
        let name = format!("fuzz-{seed}");
        let s = scenario(&world, &trig, &anims, &stream, &name, yaw, pos);
        compare(&oracle, &s, &fuzz_inputs(seed, 900), &mut coverage);
    }
    report("authored playground", &coverage);
    for action in [
        c::ACT_IDLE,
        c::ACT_WALKING,
        c::ACT_DECELERATING,
        c::ACT_BRAKING,
        c::ACT_TURNING_AROUND,
        c::ACT_JUMP,
        c::ACT_JUMP_LAND,
        c::ACT_DOUBLE_JUMP,
        c::ACT_TRIPLE_JUMP,
        c::ACT_LONG_JUMP,
        c::ACT_BACKFLIP,
        c::ACT_SIDE_FLIP,
        c::ACT_FREEFALL,
        c::ACT_CROUCHING,
        c::ACT_CRAWLING,
        c::ACT_CROUCH_SLIDE,
        c::ACT_SLIDE_KICK,
        c::ACT_DIVE,
        c::ACT_DIVE_SLIDE,
        c::ACT_PUNCHING,
        c::ACT_JUMP_KICK,
        c::ACT_GROUND_POUND,
        c::ACT_GROUND_POUND_LAND,
        c::ACT_AIR_HIT_WALL,
        c::ACT_LEDGE_GRAB,
        c::ACT_START_HANGING,
        c::ACT_BUTT_SLIDE,
        c::ACT_LAVA_BOOST,
    ] {
        assert!(
            coverage.actions.contains(&action),
            "authored scenarios no longer reach {}",
            action_names()[&action]
        );
    }
}

#[test]
fn presentation_rates_and_settings_do_not_change_ticks() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0002);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let inputs = fuzz_inputs(77, 600);
    let s = scenario(&world, &trig, &anims, &stream, "render-rates", 0, [0, 0, 0]);
    let native = s.native(&oracle, &inputs);
    for render_hz in [15, 30, 60, 120, 144] {
        for interpolation in [false, true] {
            let graphics = GraphicsOptions {
                interpolation,
                enhanced_lighting: interpolation,
                dynamic_shadows: interpolation,
            };
            let rust = s.rust(&inputs, render_hz, graphics).unwrap();
            trace::compare(&native, &rust)
                .unwrap_or_else(|e| panic!("{render_hz} Hz, interpolation {interpolation}: {e}"));
        }
    }
    let mut changed = native.clone();
    *changed.frames[300]
        .state
        .fields
        .get_mut("m.forwardVel")
        .unwrap() ^= 1;
    let error = trace::compare(&native, &changed).unwrap_err();
    assert_eq!(error.tick, Some(301));
}

/// Held controls as a player gives them: segments of held directions,
/// buttons and camera turns, with single-tick taps and releases between.
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
                    camera_left: rng.chance(4),
                    camera_right: rng.chance(5),
                };
            } else if rng.chance(6) {
                match rng.next() % 3 {
                    0 => pad.a ^= true,
                    1 => pad.b ^= true,
                    _ => pad.z ^= true,
                }
            }
            pad
        })
        .collect()
}

/// The viewer's play path: a `play::Session` driven by held controls, with
/// the follow camera turning, logs its inputs; the decomp replaying that log
/// reports the session's own words after every tick. Re-entering the level
/// and playing the same controls gives the same words.
#[test]
fn played_sessions_replay_exactly_in_the_decomp() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0003);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let starts = [
        (0, [0, 0, 0]),
        (90, [700, 0, 0]),
        (45, [-500, 0, -2500]),
        (0, [0, 400, 2700]),
        (-90, [-1350, 0, 0]),
        (30, [2500, 0, 2500]),
    ];
    let (mut ticks, mut camera_yaws, mut actions) = (0, BTreeSet::new(), BTreeSet::new());
    for (seed, (yaw, pos)) in starts.into_iter().enumerate() {
        let s = scenario(&world, &trig, &anims, &stream, "played", yaw, pos);
        let mut session = Session::new(&world, &trig, &anims, s.setup.entry());
        let initial = capture(session.mario(), session.world());
        let controls = held_controls(seed as u64, 600);
        let mut words = vec![];
        for pad in &controls {
            if !session.step(pad) || matches!(session.stopped(), Some(Stop::Panic(_))) {
                break;
            }
            words.push(capture(session.mario(), session.world()));
        }
        let log = session.input_log("test", "synthetic", "playground");
        let log = InputLog::from_json(&log.to_json_pretty()).unwrap();
        let (rust, _) = s.rust_until_unsupported(&log.inputs);
        assert_eq!(rust, words, "a replay of the log reaches the same states");
        let native = s.native(&oracle, &log.inputs[..words.len()]);
        if let Some(d) = diff(&native.metadata.initial_state.fields, &initial) {
            panic!("played session {seed}: level entry: {d}");
        }
        for (frame, words) in native.frames.iter().zip(&words) {
            if let Some(d) = diff(&frame.state.fields, words) {
                panic!(
                    "played session {seed}: tick {} ({:?}): {d}",
                    frame.tick, frame.input
                );
            }
            actions.insert(words["m.action"]);
        }
        session.reset();
        for (pad, expected) in controls.iter().zip(&words) {
            assert!(session.step(pad));
            assert_eq!(&capture(session.mario(), session.world()), expected);
        }
        ticks += words.len();
        camera_yaws.extend(log.inputs.iter().map(|i| i.camera_yaw));
    }
    println!(
        "played sessions: {ticks} ticks identical, {} actions, {} camera yaws",
        actions.len(),
        camera_yaws.len()
    );
    assert!(ticks > 1500, "sessions stopped early: {ticks} ticks");
    assert!(camera_yaws.len() > 30, "the follow camera barely turned");
    assert!(actions.len() > 15, "few actions reached: {}", actions.len());
}

/// The level entry the viewer builds from an import equals the oracle's.
#[test]
fn level_script_entries_match_the_native_setup() {
    let entry = LevelEntry::from_level_script(c::LEVEL_BOB, 1, 135, [-6558, 0, 6464], 0, 1);
    assert_eq!(TickSetup::from_entry(&entry).entry(), entry);
    assert_eq!(entry.spawn.start_angle, [0, 0x6000, 0]);
    assert_eq!(entry.camera_def_mode, entry.camera_mode);
}

#[test]
#[ignore = "diagnostic: prints action timelines"]
fn print_action_timelines() {
    let stream = playground();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0001);
    let names = action_names();
    for (name, yaw, pos, inputs) in scripted_scenarios() {
        if !std::env::var("SCENARIO").is_ok_and(|s| name.contains(&s)) {
            continue;
        }
        let s = scenario(&world, &trig, &anims, &stream, name, yaw, pos);
        let (ticks, _) = s.rust_until_unsupported(&inputs);
        let mut last = 0;
        for (i, w) in ticks.iter().enumerate() {
            if w["m.action"] != last {
                last = w["m.action"];
                println!(
                    "{name} tick {}: {} fv {} pos ({}, {}, {}) yaw {:#X}",
                    i + 1,
                    names.get(&last).unwrap_or(&"?"),
                    f32::from_bits(w["m.forwardVel"]),
                    f32::from_bits(w["m.pos[0]"]),
                    f32::from_bits(w["m.pos[1]"]),
                    f32::from_bits(w["m.pos[2]"]),
                    w["m.faceAngle[1]"] as u16
                );
            }
        }
    }
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn bob_ticks_match_the_decomp_with_rom_data() {
    use rustario64::import::{animation, bob, engine, mio0, rom::Rom, version};
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
    let (area, script_yaw, script_pos) = imported.level.mario_start.unwrap();
    let terrain_type = imported
        .level
        .areas
        .iter()
        .find(|a| a.id == area)
        .unwrap()
        .terrain_type;
    // create_camera takes the mode from the area's GEO_CAMERA node: radial.
    let camera_mode = imported.visual.as_ref().unwrap().camera.unwrap().mode;
    assert_eq!(camera_mode, c::CAMERA_MODE_RADIAL);
    let digest = world_digest(&stream, &trig);
    let bob_scenario = |name: String, yaw: i16, pos: [i16; 3]| TickScenario {
        collision: &world,
        trig: &trig,
        anims: &anims,
        setup: TickSetup::from_level_script(
            c::LEVEL_BOB,
            area.0,
            yaw,
            pos,
            terrain_type,
            camera_mode as u8,
        ),
        rom_sha1: rom.fingerprint().into(),
        world_digest: digest.clone(),
        scenario: name,
        course: 1,
    };
    // The viewer enters BOB with the same setup the script-start scenarios use.
    let viewer_entry = LevelEntry::script_start(&imported.level, camera_mode).unwrap();
    assert_eq!(
        bob_scenario(String::new(), script_yaw, script_pos)
            .setup
            .entry(),
        viewer_entry
    );
    let mut coverage = new_coverage();
    // Every scripted move set from the script's start.
    for (name, _, _, inputs) in scripted_scenarios() {
        let s = bob_scenario(format!("bob-script-start-{name}"), script_yaw, script_pos);
        compare(&oracle, &s, &inputs, &mut coverage);
    }
    // Fuzzed inputs from the script start and from floors across the course.
    let mut rng = Lcg(0xB0B);
    let mut starts = vec![(script_yaw, script_pos)];
    while starts.len() < 32 {
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
        let s = bob_scenario(format!("bob-fuzz-{i}"), yaw, pos);
        compare(
            &oracle,
            &s,
            &fuzz_inputs(1000 + i as u64, 1800),
            &mut coverage,
        );
    }
    report("BOB", &coverage);
    let reference = bob_scenario("bob-render-rates".into(), script_yaw, script_pos);
    let inputs = fuzz_inputs(4242, 900);
    let native = reference.native(&oracle, &inputs);
    for render_hz in [30, 60, 144] {
        let rust = reference
            .rust(&inputs, render_hz, GraphicsOptions::default())
            .unwrap();
        trace::compare(&native, &rust).unwrap_or_else(|e| panic!("BOB {render_hz} Hz: {e}"));
    }
    println!("BOB: render-rate check identical at 30/60/144 Hz over 900 ticks");
}
