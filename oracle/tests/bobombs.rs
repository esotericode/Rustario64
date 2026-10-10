//! Bob-omb differential tests: the decomp's verbatim Bob-omb, explosion,
//! respawner, smoke, moving-coin and sound-spawner behaviors, Mario's
//! grabbable and damage interactions and the environmental camera shake,
//! compiled natively (c/obj_behaviors_unit.c and the tick harness), against
//! the Rust port through `simulation::game`. Every word of Mario, the camera
//! and every object must match after every frame. CI uses an authored field
//! with lava, a death-plane pit, a ramp and a walled block; the ignored test
//! starts Mario beside each of BOB's Bob-ombs with the owner's ROM.
//!
//! Picking a Bob-omb up runs Mario's holding actions and the render pass's
//! hand position (HOLP, written by geo_switch_mario_hand_grab_pos through
//! the original matrix stack), which throws and drops read. A run that
//! reaches a path the port does not run stops; the comparison covers the
//! frames before it and the stop is reported.
#[path = "support/playground.rs"]
mod playground;
use playground::{
    ANIMATIONS, Builder, Lcg, MODELS, SCRIPTS, authored_animations, computed_tables, world,
};
use rustario64::simulation::{
    TickInput,
    camera::{L_CBUTTONS, R_CBUTTONS, system::GeoCamera},
    controller::{A_BUTTON, B_BUTTON, Z_TRIG},
    game::GameEntry,
    mario::{
        constants as c,
        tick::{LevelEntry, LevelObjects},
    },
    object::{
        script::{Behavior, BehaviorScripts},
        spawn::{AreaObjects, MacroEntry, convert_rotation},
    },
};
use rustario64_oracle::{
    Oracle,
    camera_trace::{GameScenario, RustStop, first_difference},
};
use std::collections::BTreeMap;

/// A flat field at y = 0 with a lava tile, a death-plane pit 500 units deep
/// (open to the field), a ramp and a walled block.
fn field() -> Vec<i16> {
    let mut b = Builder::default();
    for gz in 0..10 {
        for gx in 0..10 {
            let x = [-5000 + gx * 1000, -4000 + gx * 1000];
            let z = [-5000 + gz * 1000, -4000 + gz * 1000];
            let surface = match (gx, gz) {
                (6, 3) => c::SURFACE_BURNING,
                (2, 7) => continue,
                _ => c::SURFACE_DEFAULT,
            };
            b.flat(surface, x, z, 0, true);
        }
    }
    // The pit under tile (2, 7): x -3000..-2000, z 2000..3000.
    b.flat(
        c::SURFACE_DEATH_PLANE,
        [-3000, -2000],
        [2000, 3000],
        -500,
        true,
    );
    b.ramp(c::SURFACE_DEFAULT, [-1000, 0], [-3600, -2600], [0, 500]);
    b.block(c::SURFACE_DEFAULT, [2600, 3200], [1400, 2000], [0, 300]);
    b.stream()
}

/// sMacroObjectPresets rows for the placements (preset index, behavior,
/// model, parameter): macro_bobomb and macro_bobomb_stationary's values in
/// the pinned include/macro_presets.inc.c, at authored indices.
type Preset = (u16, Behavior, i32, i32);
const BOBOMB: Preset = (20, Behavior::Bobomb, c::MODEL_BLACK_BOBOMB, 0);
const STATIONARY: Preset = (
    21,
    Behavior::Bobomb,
    c::MODEL_BLACK_BOBOMB,
    c::BOBOMB_BP_STYPE_STATIONARY,
);

fn entry(index: usize, preset: Preset, pos: [i16; 3], yaw_bits: u16) -> MacroEntry {
    let (preset, behavior, model, preset_param) = preset;
    let packed = (yaw_bits << 9) | (preset + 31);
    MacroEntry {
        source: 0x0701_0000 + 10 * index as u32,
        packed,
        preset,
        behavior: SCRIPTS.address(behavior),
        model: model as i16,
        preset_param: preset_param as i16,
        pos,
        yaw: convert_rotation((((packed as i16) >> 9) & 0x7F) << 1),
        params: 0,
    }
}

/// Bob-ombs around the start, by the lava, at the pit's edge, on the ramp,
/// against the block, and two pairs close enough to touch each other.
fn placements() -> AreaObjects {
    let macros = vec![
        entry(0, BOBOMB, [300, 0, 600], 0),
        entry(1, BOBOMB, [-600, 0, 300], 32),
        entry(2, STATIONARY, [0, 0, -700], 64),
        entry(3, BOBOMB, [1500, 0, -1200], 16),
        entry(4, BOBOMB, [1300, 0, -1800], 80),
        entry(5, BOBOMB, [-1900, 0, 2400], 96),
        entry(6, BOBOMB, [-500, 250, -3100], 0),
        entry(7, BOBOMB, [2500, 0, 1700], 32),
        entry(8, BOBOMB, [800, 0, 900], 0),
        entry(9, BOBOMB, [800, 0, 1020], 64),
        entry(10, STATIONARY, [-1200, 0, -400], 0),
        entry(11, BOBOMB, [-1290, 0, -400], 96),
    ];
    AreaObjects {
        area_index: 1,
        macros,
        spawn_infos: vec![],
        skipped: vec![],
    }
}

fn scenario<'a>(
    world: &'a rustario64::simulation::collision::CollisionWorld,
    trig: &'a rustario64::simulation::math::TrigTables,
    anims: &'a rustario64::content::animation::MarioAnimations,
    yaw: i16,
    pos: [i16; 3],
    rng_seed: u16,
) -> GameScenario<'a> {
    scenario_in(world, trig, anims, yaw, pos, rng_seed, placements())
}

fn scenario_in<'a>(
    world: &'a rustario64::simulation::collision::CollisionWorld,
    trig: &'a rustario64::simulation::math::TrigTables,
    anims: &'a rustario64::content::animation::MarioAnimations,
    yaw: i16,
    pos: [i16; 3],
    rng_seed: u16,
    area: AreaObjects,
) -> GameScenario<'a> {
    let mario = LevelEntry::from_level_script(c::LEVEL_BOB, 1, yaw, pos, 0, 1);
    GameScenario {
        collision: world,
        trig,
        anims,
        objects: LevelObjects {
            scripts: &SCRIPTS,
            models: &MODELS,
            animations: &ANIMATIONS,
            area,
        },
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

/// Held sticks or long waits (so Bob-ombs light their fuses and reach
/// Mario), with jumps and occasional B presses: jump kicks launch Bob-ombs,
/// ground punches try to pick them up.
fn inputs(seed: u64, ticks: usize, b_odds: u32) -> Vec<TickInput> {
    let mut rng = Lcg(seed);
    let mut out = Vec::with_capacity(ticks);
    while out.len() < ticks {
        let n = rng.range(10, 90) as usize;
        let stick = if rng.chance(3) {
            [0, 0]
        } else {
            [rng.range(-128, 127) as i8, rng.range(-128, 127) as i8]
        };
        let mut buttons = 0;
        for (button, odds) in [
            (A_BUTTON, 3),
            (B_BUTTON, b_odds),
            (Z_TRIG, 20),
            (L_CBUTTONS, 10),
            (R_CBUTTONS, 10),
        ] {
            if rng.chance(odds) {
                buttons |= button;
            }
        }
        for i in 0..n {
            // A on the first frame of a run, B a few frames into the jump.
            let mut b = buttons & !(A_BUTTON | B_BUTTON);
            if i % 11 == 0 {
                b |= buttons & A_BUTTON;
            }
            if i % 11 == 5 {
                b |= buttons & B_BUTTON;
            }
            out.push(TickInput {
                buttons: b,
                stick,
                camera_yaw: 0,
            });
        }
    }
    out.truncate(ticks);
    out
}

#[derive(Default)]
struct Coverage {
    frames: usize,
    fuse_frames: usize,
    chase_frames: usize,
    launched_frames: usize,
    explosions: usize,
    lava_deaths: usize,
    plane_deaths: usize,
    respawned: usize,
    loot_collected: usize,
    knockbacks: usize,
    env_shakes: usize,
    /// Bob-omb frames in Mario's hands, and its releases: thrown (it
    /// leaves the hand launched) or dropped (the held state's one frame is
    /// cleared by the Bob-omb's own loop in the same frame).
    held_frames: usize,
    throws: usize,
    drops: usize,
    /// Frames on which the render pass moved the HOLP.
    holp_updates: usize,
    /// Mario frames in a holding action, and the distinct ones seen.
    hold_action_frames: usize,
    hold_actions: std::collections::BTreeSet<u32>,
    stops: Vec<String>,
}

fn tally(
    scripts: &BehaviorScripts,
    words: &BTreeMap<String, u32>,
    previous: Option<&BTreeMap<String, u32>>,
    cov: &mut Coverage,
) {
    let bobomb = scripts.address(Behavior::Bobomb);
    let explosion = scripts.address(Behavior::Explosion);
    for i in 0..words["events.count"] {
        cov.env_shakes += usize::from(words[&format!("events[{i}].kind")] == 14);
    }
    let action_of = |slot: &str| words[&format!("objects[{slot}].raw[0x{:02X}]", c::O_ACTION)];
    for (name, value) in words {
        let Some(rest) = name.strip_prefix("objects[") else {
            continue;
        };
        let slot = &rest[..rest.find(']').unwrap()];
        if rest.ends_with("].behavior") && *value == bobomb {
            let held_key = format!("objects[{slot}].raw[0x{:02X}]", c::O_HELD_STATE);
            let held = words[&held_key] as i32;
            cov.held_frames += usize::from(held == c::HELD_HELD);
            let was_held = previous.is_some_and(|p| {
                p.get(&format!("objects[{slot}].behavior")) == Some(&bobomb)
                    && p[&held_key] as i32 == c::HELD_HELD
            });
            if was_held && held != c::HELD_HELD {
                if action_of(slot) as i32 == c::BOBOMB_ACT_LAUNCHED {
                    cov.throws += 1;
                } else {
                    cov.drops += 1;
                }
            }
            let fuse = words[&format!("objects[{slot}].raw[0x{:02X}]", c::O_BOBOMB_FUSE_LIT)];
            cov.fuse_frames += usize::from(fuse == 1);
            let action = action_of(slot) as i32;
            cov.chase_frames += usize::from(action == c::BOBOMB_ACT_CHASE_MARIO);
            cov.launched_frames += usize::from(action == c::BOBOMB_ACT_LAUNCHED);
            if words[&format!("objects[{slot}].raw[0x{:02X}]", c::O_TIMER)] == 0 {
                cov.lava_deaths += usize::from(action == c::BOBOMB_ACT_LAVA_DEATH);
                cov.plane_deaths += usize::from(action == c::BOBOMB_ACT_DEATH_PLANE_DEATH);
            }
            // A Bob-omb a respawner made: its parent is not itself.
            if words[&format!("objects[{slot}].parentObj")].to_string() != slot
                && words[&format!("objects[{slot}].raw[0x{:02X}]", c::O_TIMER)] == 1
            {
                cov.respawned += 1;
            }
        }
        if rest.ends_with("].behavior") && *value == explosion {
            cov.explosions +=
                usize::from(words[&format!("objects[{slot}].raw[0x{:02X}]", c::O_TIMER)] == 1);
        }
    }
    let action = words["m.action"];
    if matches!(
        action,
        c::ACT_PICKING_UP
            | c::ACT_DIVE_PICKING_UP
            | c::ACT_PLACING_DOWN
            | c::ACT_THROWING
            | c::ACT_AIR_THROW
            | c::ACT_AIR_THROW_LAND
            | c::ACT_HOLD_IDLE
            | c::ACT_HOLD_WALKING
            | c::ACT_HOLD_DECELERATING
            | c::ACT_HOLD_JUMP
            | c::ACT_HOLD_FREEFALL
            | c::ACT_HOLD_JUMP_LAND
            | c::ACT_HOLD_FREEFALL_LAND
            | c::ACT_HOLD_JUMP_LAND_STOP
            | c::ACT_HOLD_FREEFALL_LAND_STOP
            | c::ACT_HOLD_BEGIN_SLIDING
            | c::ACT_HOLD_BUTT_SLIDE
            | c::ACT_HOLD_BUTT_SLIDE_AIR
            | c::ACT_HOLD_BUTT_SLIDE_STOP
    ) {
        cov.hold_action_frames += 1;
        cov.hold_actions.insert(action);
    }
    if let Some(previous) = previous {
        let holp = |w: &BTreeMap<String, u32>| {
            (0..3)
                .map(|i| w[&format!("body.heldObjLastPosition[{i}]")])
                .collect::<Vec<_>>()
        };
        cov.holp_updates += usize::from(holp(words) != holp(previous));
        if action != previous["m.action"]
            && matches!(
                action,
                c::ACT_SOFT_BACKWARD_GROUND_KB
                    | c::ACT_BACKWARD_GROUND_KB
                    | c::ACT_HARD_BACKWARD_GROUND_KB
                    | c::ACT_SOFT_FORWARD_GROUND_KB
                    | c::ACT_FORWARD_GROUND_KB
                    | c::ACT_HARD_FORWARD_GROUND_KB
                    | c::ACT_BACKWARD_AIR_KB
                    | c::ACT_FORWARD_AIR_KB
                    | c::ACT_HARD_BACKWARD_AIR_KB
                    | c::ACT_HARD_FORWARD_AIR_KB
            )
        {
            cov.knockbacks += 1;
        }
        if words["m.numCoins"] > previous["m.numCoins"] {
            cov.loot_collected += 1;
        }
    }
}

fn compare(
    oracle: &Oracle,
    s: &GameScenario<'_>,
    name: &str,
    inputs: &[TickInput],
    cov: &mut Coverage,
) {
    let (rust, stop) = s.rust(inputs);
    let frames = rust.len() - 1;
    if let Some(stop) = &stop {
        let what = match stop {
            RustStop::Panic(message) | RustStop::Camera(message) => message,
        };
        cov.stops
            .push(format!("{name} frame {}: {what}", frames + 1));
    }
    let native = s.native(oracle, &inputs[..frames]);
    for (i, (expected, actual)) in native.iter().zip(&rust).enumerate() {
        if let Some(d) = first_difference(expected, actual) {
            panic!("{name}: frame {i}: {d}");
        }
        tally(
            s.objects.scripts,
            actual,
            i.checked_sub(1).map(|p| &rust[p]),
            cov,
        );
    }
    cov.frames += frames;
}

fn report(label: &str, cov: &Coverage) {
    println!(
        "{label}: {} frames identical; Bob-omb frames with a lit fuse {}, chasing {}, launched {}, \
         held {} ({} throws, {} drops); Mario frames holding {} in {} actions, {} HOLP updates; \
         {} explosions ({} environmental shakes), {} lava and {} death-plane deaths, {} respawns, \
         {} coins collected, {} knockbacks",
        cov.frames,
        cov.fuse_frames,
        cov.chase_frames,
        cov.launched_frames,
        cov.held_frames,
        cov.throws,
        cov.drops,
        cov.hold_action_frames,
        cov.hold_actions.len(),
        cov.holp_updates,
        cov.explosions,
        cov.env_shakes,
        cov.lava_deaths,
        cov.plane_deaths,
        cov.respawned,
        cov.loot_collected,
        cov.knockbacks
    );
    for stop in &cov.stops {
        println!("{label}: stopped: {stop}");
    }
}

#[test]
fn authored_bobombs_match_the_decomp() {
    let stream = field();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0004);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut cov = Coverage::default();
    for (seed, (yaw, pos, b_odds)) in [
        (0, [0, 0, 0], 1000),
        (90, [0, 0, 0], 1000),
        (180, [1500, 0, -800], 1000),
        (-90, [-2100, 0, 1900], 1000),
        (0, [-500, 300, -2900], 1000),
        (45, [2300, 0, 1300], 1000),
        (0, [800, 0, 600], 1000),
        (135, [0, 0, 0], 4),
        (-45, [-1000, 0, -200], 4),
        (0, [600, 0, 300], 6),
    ]
    .into_iter()
    .enumerate()
    {
        let s = scenario(&world, &trig, &anims, yaw, pos, seed as u16 * 1013);
        let inputs = inputs(0xB0B + seed as u64, 900, b_odds);
        compare(&oracle, &s, &format!("bobombs-{seed}"), &inputs, &mut cov);
    }
    report("authored Bob-ombs", &cov);
    assert!(cov.frames >= 4000, "too few compared frames");
    assert!(
        cov.fuse_frames > 0 && cov.chase_frames > 0,
        "no Bob-omb chased Mario"
    );
    assert!(cov.explosions > 0 && cov.env_shakes > 0, "no explosion");
    assert!(cov.respawned > 0, "no Bob-omb respawned");
    assert!(cov.knockbacks > 0, "no explosion knocked Mario back");
    assert!(
        cov.lava_deaths > 0 && cov.plane_deaths > 0,
        "no floor death"
    );
}

/// Scripted encounters with the stationary Bob-omb at (0, 0, -700): jump
/// kicks launch it into an explosion; punches pick it up, then Mario carries
/// it, throws it on the ground and in the air, drops it, lets its fuse run
/// out in his hands, and dives into it from a run.
#[test]
fn authored_bobomb_kicks_and_holding_match_the_decomp() {
    let stream = field();
    let world = world(&stream);
    let trig = computed_tables();
    // A seed whose punch, pick-up, carry, throw, drop and dive animations
    // play forward (the invented set can freeze or reverse any of them,
    // which ends a pick-up never, identically on both sides).
    let anims = authored_animations(0x5EED_000B);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut cov = Coverage::default();
    let push = |out: &mut Vec<TickInput>, n: usize, buttons: u16, stick: [i8; 2]| {
        out.extend(std::iter::repeat_n(
            TickInput {
                buttons,
                stick,
                camera_yaw: 0,
            },
            n,
        ));
    };
    // Mario faces the stationary Bob-omb inside its reach (the hitboxes
    // touch below 102 units; yaw 180 faces -z), then jumps and kicks it
    // without moving the stick.
    for (i, (wait, kick_at)) in [(10, 4), (12, 5), (20, 6), (8, 3)].into_iter().enumerate() {
        let s = scenario(
            &world,
            &trig,
            &anims,
            180,
            [0, 0, -612 + 3 * i as i16],
            77 + i as u16,
        );
        let mut inputs = vec![];
        push(&mut inputs, wait, 0, [0, 0]);
        push(&mut inputs, kick_at, A_BUTTON, [0, 0]);
        push(&mut inputs, 1, B_BUTTON, [0, 0]);
        push(&mut inputs, 300, 0, [0, 0]);
        compare(&oracle, &s, &format!("kick-{i}"), &inputs, &mut cov);
    }
    let kicked = cov.launched_frames;
    // A ground punch picks it up; then each script, from the hold.
    let scripts = holding_scripts();
    for (i, (name, script)) in scripts.into_iter().enumerate() {
        let s = scenario(&world, &trig, &anims, 180, [0, 0, -610], 91 + i as u16);
        let mut inputs = vec![];
        push(&mut inputs, 10, 0, [0, 0]);
        push(&mut inputs, 1, B_BUTTON, [0, 0]);
        script(&mut inputs);
        compare(&oracle, &s, name, &inputs, &mut cov);
    }
    // A dive from a short run.
    let s = scenario(&world, &trig, &anims, 180, [0, 0, -330], 101);
    let mut inputs = vec![];
    push(&mut inputs, 8, 0, [0, 70]);
    push(&mut inputs, 1, B_BUTTON, [0, 70]);
    push(&mut inputs, 60, 0, [0, 0]);
    push(&mut inputs, 1, B_BUTTON, [0, 0]);
    push(&mut inputs, 150, 0, [0, 0]);
    compare(&oracle, &s, "dive-grab", &inputs, &mut cov);
    report("authored Bob-omb encounters", &cov);
    assert!(kicked > 0, "no kick launched the Bob-omb");
    assert!(cov.explosions > 0);
    assert!(cov.held_frames > 0, "no Bob-omb was held");
    assert!(cov.throws > 0 && cov.drops > 0, "no throw or no drop");
    assert!(cov.holp_updates > 0, "the render pass never moved the HOLP");
    for action in [
        c::ACT_PICKING_UP,
        c::ACT_HOLD_IDLE,
        c::ACT_HOLD_WALKING,
        c::ACT_THROWING,
        c::ACT_AIR_THROW,
        c::ACT_HOLD_JUMP,
    ] {
        assert!(
            cov.hold_actions.contains(&action),
            "no frame in holding action {action:#X}"
        );
    }
    assert!(cov.stops.is_empty(), "a run stopped: {:?}", cov.stops);
}

/// What Mario does once a punch has picked a Bob-omb up: carry and throw
/// it, hold it until its fuse runs out, drop it (Z), throw it from a jump,
/// jump and land with it, walk and turn with it, and drop and pick it up
/// again.
type Script = fn(&mut Vec<TickInput>);
fn holding_scripts() -> [(&'static str, Script); 7] {
    [
        ("carry-and-throw", |out| {
            push_inputs(out, 40, 0, [0, 0]);
            push_inputs(out, 35, 0, [0, 70]);
            push_inputs(out, 1, B_BUTTON, [0, 70]);
            push_inputs(out, 200, 0, [0, 0]);
        }),
        ("fuse-in-hand", |out| {
            push_inputs(out, 230, 0, [0, 0]);
        }),
        ("drop", |out| {
            push_inputs(out, 40, 0, [0, 0]);
            push_inputs(out, 3, Z_TRIG, [0, 0]);
            push_inputs(out, 150, 0, [0, 0]);
        }),
        ("air-throw", |out| {
            push_inputs(out, 40, 0, [0, 0]);
            push_inputs(out, 6, A_BUTTON, [0, 0]);
            push_inputs(out, 1, A_BUTTON | B_BUTTON, [0, 0]);
            push_inputs(out, 200, 0, [0, 0]);
        }),
        ("jump-and-land", |out| {
            push_inputs(out, 40, 0, [0, 0]);
            push_inputs(out, 12, A_BUTTON, [40, 60]);
            push_inputs(out, 30, 0, [0, 0]);
            push_inputs(out, 1, B_BUTTON, [0, 0]);
            push_inputs(out, 150, 0, [0, 0]);
        }),
        ("walk-turn-and-stop", |out| {
            push_inputs(out, 40, 0, [0, 0]);
            push_inputs(out, 30, 0, [127, 0]);
            push_inputs(out, 30, 0, [0, -127]);
            push_inputs(out, 20, 0, [0, 0]);
            push_inputs(out, 1, B_BUTTON, [0, 0]);
            push_inputs(out, 150, 0, [0, 0]);
        }),
        ("regrab", |out| {
            push_inputs(out, 40, 0, [0, 0]);
            push_inputs(out, 3, Z_TRIG, [0, 0]);
            push_inputs(out, 20, 0, [0, 0]);
            push_inputs(out, 1, B_BUTTON, [0, 0]);
            push_inputs(out, 120, 0, [0, 0]);
        }),
    ]
}

/// A plateau at y = 500 above a slippery slope down to the ground: carried
/// down the slope, a Bob-omb rides along in a held butt slide (the torso
/// tilt the render pass keeps), then Mario stands and throws it.
#[test]
fn authored_bobomb_carried_into_a_held_butt_slide_matches_the_decomp() {
    let mut b = Builder::default();
    b.flat(c::SURFACE_DEFAULT, [-2000, 2000], [-3000, 0], 0, true);
    b.ramp(c::SURFACE_SLIPPERY, [-2000, 2000], [0, 1000], [0, 500]);
    b.flat(c::SURFACE_DEFAULT, [-2000, 2000], [1000, 2500], 500, true);
    let stream = b.stream();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_000B);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let area = AreaObjects {
        area_index: 1,
        macros: vec![entry(0, STATIONARY, [0, 500, 1300], 0)],
        spawn_infos: vec![],
        skipped: vec![],
    };
    let mut cov = Coverage::default();
    for (i, run) in [30, 45, 60].into_iter().enumerate() {
        let s = scenario_in(
            &world,
            &trig,
            &anims,
            180,
            [0, 500, 1390],
            300 + i as u16,
            area.clone(),
        );
        let mut inputs = vec![];
        push_inputs(&mut inputs, 10, 0, [0, 0]);
        push_inputs(&mut inputs, 1, B_BUTTON, [0, 0]);
        push_inputs(&mut inputs, 35, 0, [0, 0]);
        push_inputs(&mut inputs, run, 0, [0, 80]);
        push_inputs(&mut inputs, 40, 0, [0, 0]);
        push_inputs(&mut inputs, 1, B_BUTTON, [0, 0]);
        push_inputs(&mut inputs, 120, 0, [0, 0]);
        compare(&oracle, &s, &format!("slope-{run}"), &inputs, &mut cov);
    }
    report("authored held slide", &cov);
    assert!(
        cov.hold_actions.contains(&c::ACT_HOLD_BUTT_SLIDE),
        "no held butt slide"
    );
    assert!(cov.held_frames > 0 && cov.holp_updates > 0);
    assert!(cov.stops.is_empty(), "a run stopped: {:?}", cov.stops);
}

fn push_inputs(out: &mut Vec<TickInput>, n: usize, buttons: u16, stick: [i8; 2]) {
    out.extend(std::iter::repeat_n(
        TickInput {
            buttons,
            stick,
            camera_yaw: 0,
        },
        n,
    ));
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; BOB's Bob-ombs with the owner's ROM"]
fn bob_bobombs_match_the_decomp_with_rom_data() {
    use rustario64::{
        content::Act,
        import::{animation, bob, collision, engine, mio0, objects, rom::Rom, version},
    };
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
    let stream: Vec<i16> = terrain[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    let imported = bob::import(&rom).unwrap();
    let world =
        rustario64::simulation::collision::CollisionWorld::load_area_terrain(&mesh).unwrap();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let node = imported.visual.as_ref().unwrap().camera.unwrap();
    let script_entry = GameEntry::script_start(&imported.level, &node).unwrap();
    let content = objects::bob(&rom, &imported.level, Act::new(1).unwrap()).unwrap();
    let scripts = &content.content.scripts;
    let bobomb = scripts.address(Behavior::Bobomb);
    let starts: Vec<[i16; 3]> = content
        .area
        .macros
        .iter()
        .filter(|e| e.behavior == bobomb)
        .map(|e| e.pos)
        .collect();
    assert_eq!(starts.len(), 12);
    let mut cov = Coverage::default();
    for (i, pos) in starts.iter().enumerate() {
        for (j, (yaw, b_odds)) in [(0i16, 1000), (90, 1000), (180, 5), (270, 1000)]
            .into_iter()
            .enumerate()
        {
            // Beside the Bob-omb, 350 units toward its home's +z side.
            let s = GameScenario {
                collision: &world,
                trig: &trig,
                anims: &anims,
                objects: content.level_objects(),
                entry: GameEntry {
                    mario: LevelEntry {
                        spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                            1,
                            script_entry.mario.spawn.area_index as u8,
                            yaw,
                            [pos[0], pos[1] + 150, pos[2] + 350],
                        ),
                        ..script_entry.mario
                    },
                    rng_seed: (i * 4 + j) as u16,
                    ..script_entry
                },
            };
            let inputs = inputs(0xB0B0B + (i * 4 + j) as u64, 600, b_odds);
            compare(
                &oracle,
                &s,
                &format!("bob-bobomb-{i}-{yaw}"),
                &inputs,
                &mut cov,
            );
        }
    }
    // The holding scripts beside BOB's stationary Bob-omb: Mario 88 units
    // to its +z side, facing it, punches it up first.
    let stationary = content
        .area
        .macros
        .iter()
        .find(|e| {
            e.behavior == bobomb && i32::from(e.preset_param) == c::BOBOMB_BP_STYPE_STATIONARY
        })
        .expect("BOB act 1 has a stationary Bob-omb")
        .pos;
    for (i, (name, script)) in holding_scripts().into_iter().enumerate() {
        let s = GameScenario {
            collision: &world,
            trig: &trig,
            anims: &anims,
            objects: content.level_objects(),
            entry: GameEntry {
                mario: LevelEntry {
                    spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                        1,
                        script_entry.mario.spawn.area_index as u8,
                        180,
                        [stationary[0], stationary[1], stationary[2] + 88],
                    ),
                    ..script_entry.mario
                },
                rng_seed: 200 + i as u16,
                ..script_entry
            },
        };
        let mut inputs = vec![];
        push_inputs(&mut inputs, 10, 0, [0, 0]);
        push_inputs(&mut inputs, 1, B_BUTTON, [0, 0]);
        script(&mut inputs);
        compare(&oracle, &s, &format!("bob-{name}"), &inputs, &mut cov);
    }
    report("BOB Bob-ombs", &cov);
    assert!(cov.frames >= 10_000, "too few compared frames");
    assert!(cov.fuse_frames > 0 && cov.chase_frames > 0);
    assert!(cov.explosions > 0 && cov.env_shakes > 0);
    assert!(cov.knockbacks > 0);
    assert!(
        cov.held_frames > 0 && cov.holp_updates > 0,
        "no Bob-omb was carried"
    );
    assert!(cov.throws > 0 && cov.drops > 0, "no throw or no drop");
}
