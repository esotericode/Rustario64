//! Object differential tests: the decomp's object system compiled natively
//! (c/tick.c with the camera linked: the verbatim pool, update_objects,
//! cur_obj_update over the verbatim scripts, object collision, interact_coin
//! and the coin behaviors, plus the render pass's oAnimState writes) and the
//! Rust port (`simulation::object` through `simulation::game`) enter the same
//! level with the same placements and inputs; every word of Mario, the
//! camera and every object (lists, free list, respawn records) must match
//! after every frame. CI uses an authored playground with every coin
//! formation type; the ignored test uses BOB's coins with the owner's ROM.
#[path = "support/playground.rs"]
mod playground;
use playground::{Builder, Lcg, MODELS, SCRIPTS, authored_animations, computed_tables, world};
use rustario64::simulation::{
    TickInput,
    camera::{D_CBUTTONS, L_CBUTTONS, R_CBUTTONS, U_CBUTTONS, system::GeoCamera},
    controller::{A_BUTTON, B_BUTTON, R_TRIG, Z_TRIG},
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

/// A flat 10000-unit field at y = 0 with a gap (no floor) beyond x = 5000,
/// and a raised block for coins well above its floor.
fn field() -> Vec<i16> {
    let mut b = Builder::default();
    for gz in 0..10 {
        for gx in 0..10 {
            let x = [-5000 + gx * 1000, -4000 + gx * 1000];
            let z = [-5000 + gz * 1000, -4000 + gz * 1000];
            b.flat(c::SURFACE_DEFAULT, x, z, 0, true);
        }
    }
    b.block(c::SURFACE_DEFAULT, [2000, 2600], [-3000, -2400], [0, 400]);
    b.stream()
}

/// One sMacroObjectPresets row: preset index, behavior, model, parameter
/// (the pinned include/macro_presets.inc.c's values for these presets).
type Preset = (u16, Behavior, i32, i32);

/// A macro-list entry with its preset resolved as sMacroObjectPresets would.
fn entry(index: usize, preset: Preset, pos: [i16; 3], yaw_bits: u16, params: u16) -> MacroEntry {
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
        params,
    }
}

/// Yellow coins (one far above the floor, one over the void) and every coin
/// formation type around the start, and one formation 2600 units away for
/// spawning and respawning as Mario comes and goes.
fn placements() -> AreaObjects {
    let coin = |i, pos| {
        entry(
            i,
            (0, Behavior::YellowCoin, c::MODEL_YELLOW_COIN, 0),
            pos,
            0,
            0,
        )
    };
    let formation = |i, preset, param, pos, yaw, params| {
        let preset = (preset, Behavior::CoinFormation, c::MODEL_NONE, param);
        entry(i, preset, pos, yaw, params)
    };
    let (line, ring, arrow) = (
        c::COIN_FORMATION_BP_LINE_HORIZONTAL,
        c::COIN_FORMATION_BP_RING_HORIZONTAL,
        c::COIN_FORMATION_BP_ARROW,
    );
    let flying = c::COIN_FORMATION_BP_FLAG_FLYING;
    let macros = vec![
        coin(0, [0, 0, 400]),
        coin(1, [300, 0, 650]),
        coin(2, [-300, 0, 650]),
        coin(3, [0, 0, -450]),
        coin(4, [500, 0, 0]),
        coin(5, [-500, 60, 0]),
        coin(6, [0, 700, 900]),
        coin(7, [5600, 100, 0]),
        // The list's parameter selects the type when the preset has none.
        formation(8, 6, line, [0, 0, 1300], 0, 0),
        formation(
            9,
            6,
            line,
            [900, 0, 300],
            0,
            c::COIN_FORMATION_BP_LINE_VERTICAL as u16,
        ),
        formation(10, 7, ring, [-1100, 0, 0], 0, 0),
        formation(
            11,
            6,
            line,
            [0, 0, -1300],
            32,
            c::COIN_FORMATION_BP_RING_VERTICAL as u16,
        ),
        formation(12, 8, arrow, [1000, 0, 1100], 64, 0),
        formation(13, 9, line | flying, [-1000, 250, 1100], 96, 0),
        formation(14, 11, ring | flying, [-1200, 300, -1200], 16, 0),
        formation(
            15,
            12,
            c::COIN_FORMATION_BP_RING_VERTICAL | flying,
            [1200, 0, -1200],
            48,
            0,
        ),
        // Collected-coin respawn bits (params high byte) skip coins 0 and 2.
        formation(16, 6, line, [2600, 0, 0], 0, 0x0500),
        formation(17, 7, ring, [2300, 400, -2700], 0, 0),
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
    let mario = LevelEntry::from_level_script(c::LEVEL_BOB, 1, yaw, pos, 0, 1);
    GameScenario {
        collision: world,
        trig,
        anims,
        objects: LevelObjects {
            scripts: &SCRIPTS,
            models: &MODELS,
            area: placements(),
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

/// Held sticks in random directions for 10-60 ticks, with jumps, dives and
/// camera buttons.
fn inputs(seed: u64, ticks: usize) -> Vec<TickInput> {
    let mut rng = Lcg(seed);
    let mut out = Vec::with_capacity(ticks);
    while out.len() < ticks {
        let n = rng.range(10, 60) as usize;
        let stick = if rng.chance(6) {
            [0, 0]
        } else {
            [rng.range(-128, 127) as i8, rng.range(-128, 127) as i8]
        };
        let mut buttons = 0;
        for (button, odds) in [
            (A_BUTTON, 4),
            (B_BUTTON, 10),
            (Z_TRIG, 12),
            (U_CBUTTONS, 14),
            (D_CBUTTONS, 14),
            (L_CBUTTONS, 8),
            (R_CBUTTONS, 8),
            (R_TRIG, 16),
        ] {
            if rng.chance(odds) {
                buttons |= button;
            }
        }
        for i in 0..n {
            out.push(TickInput {
                buttons: if i % 7 == 0 {
                    buttons
                } else {
                    buttons & !A_BUTTON
                },
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
    coins: u32,
    max_objects: u32,
    golden_sparkles: usize,
    coin_sparkles: usize,
    respawns: usize,
    unloads: usize,
    shadowless: usize,
    /// The HUD counter's highest value and its coin sounds.
    hud_coins: u32,
    coin_sounds: usize,
    stops: Vec<String>,
}

/// Object facts from one frame's words.
fn tally(
    scripts: &BehaviorScripts,
    words: &BTreeMap<String, u32>,
    previous: Option<&BTreeMap<String, u32>>,
    cov: &mut Coverage,
) {
    let golden = scripts.address(Behavior::GoldenCoinSparkles);
    let sparkles = scripts.address(Behavior::CoinSparkles);
    let formation = scripts.address(Behavior::CoinFormation);
    cov.coins = cov.coins.max(words["m.numCoins"]);
    cov.hud_coins = cov.hud_coins.max(words["hud.coins"]);
    for i in 0..words["events.count"] {
        cov.coin_sounds += usize::from(
            words[&format!("events[{i}].kind")] == 1
                && words[&format!("events[{i}].a")] == c::SOUND_GENERAL_COIN,
        );
    }
    let mut objects = 0;
    for (name, value) in words {
        if let Some(rest) = name.strip_prefix("objects[") {
            if rest.ends_with("].behavior") {
                objects += 1;
                cov.golden_sparkles += usize::from(*value == golden);
                cov.coin_sparkles += usize::from(*value == sparkles);
                if *value == formation {
                    let slot = &rest[..rest.find(']').unwrap()];
                    let action = format!("objects[{slot}].raw[0x{:02X}]", c::O_ACTION);
                    cov.respawns +=
                        usize::from(words[&action] == c::COIN_FORMATION_ACT_RESPAWN_COINS as u32);
                }
            }
            if rest.ends_with("].gfx.sharedChild") {
                cov.shadowless += usize::from(*value == c::MODEL_YELLOW_COIN_NO_SHADOW as u32);
            }
        }
    }
    cov.max_objects = cov.max_objects.max(objects);
    if let Some(previous) = previous
        && words["free.count"] > previous["free.count"]
    {
        cov.unloads += 1;
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
        cov.stops
            .push(format!("{name} frame {}: {stop:?}", frames + 1));
    }
    let kept = match stop {
        Some(RustStop::Panic(_)) | Some(RustStop::Camera(_)) | None => frames,
    };
    let native = s.native(oracle, &inputs[..kept]);
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
    cov.frames += kept;
}

fn report(label: &str, cov: &Coverage) {
    println!(
        "{label}: {} frames identical; {} coins collected at most, up to {} objects, \
         {} golden-sparkle and {} coin-sparkle object-frames, {} formation respawn frames, \
         {} frames with unloads, {} shadowless-coin object-frames; the HUD counted to {} \
         with {} coin sounds",
        cov.frames,
        cov.coins,
        cov.max_objects,
        cov.golden_sparkles,
        cov.coin_sparkles,
        cov.respawns,
        cov.unloads,
        cov.shadowless,
        cov.hud_coins,
        cov.coin_sounds
    );
    for stop in &cov.stops {
        println!("{label}: stopped: {stop}");
    }
}

#[test]
fn authored_coins_and_formations_match_the_decomp() {
    let stream = field();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0003);
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut cov = Coverage::default();
    for (seed, (yaw, pos)) in [
        (0, [0, 0, 0]),
        (90, [0, 0, 0]),
        (180, [200, 0, 200]),
        (-90, [-200, 0, -200]),
        (45, [1500, 0, 0]),
        (0, [2600, 0, -600]),
    ]
    .into_iter()
    .enumerate()
    {
        let s = scenario(&world, &trig, &anims, yaw, pos, seed as u16 * 977);
        let inputs = inputs(0xC015 + seed as u64, 900);
        compare(&oracle, &s, &format!("coins-{seed}"), &inputs, &mut cov);
    }
    report("authored coins", &cov);
    assert!(cov.frames >= 4000, "too few compared frames");
    assert!(cov.coins >= 10, "too few coins collected");
    assert!(
        cov.golden_sparkles > 0 && cov.coin_sparkles > 0,
        "no sparkles"
    );
    assert!(cov.respawns > 0, "no formation respawned");
    assert!(cov.unloads > 0, "no object was unloaded");
    assert!(cov.shadowless > 0, "no coin used the shadowless model");
    assert!(
        cov.hud_coins >= 10 && cov.coin_sounds >= 10,
        "the HUD counter did not count the coins"
    );
    let _ = (
        U_CBUTTONS, D_CBUTTONS, L_CBUTTONS, R_CBUTTONS, R_TRIG, Z_TRIG, B_BUTTON,
    );
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; BOB's coins with the owner's ROM"]
fn bob_coins_match_the_decomp_with_rom_data() {
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
    // The ground coin formations Mario can reach on foot, each a start.
    let scripts = &content.content.scripts;
    let formation = scripts.address(Behavior::CoinFormation);
    let starts: Vec<[i16; 3]> = content
        .area
        .macros
        .iter()
        .filter(|e| {
            e.behavior == formation && e.preset_param & c::COIN_FORMATION_BP_FLAG_FLYING as i16 == 0
        })
        .map(|e| e.pos)
        .collect();
    assert!(!starts.is_empty());
    let mut cov = Coverage::default();
    for (i, pos) in starts.iter().enumerate() {
        for (j, yaw) in [0i16, 90, 180, 270].into_iter().enumerate() {
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
                            [pos[0], pos[1] + 200, pos[2]],
                        ),
                        ..script_entry.mario
                    },
                    rng_seed: (i * 4 + j) as u16,
                    ..script_entry
                },
            };
            let inputs = inputs(0xB0B0 + (i * 4 + j) as u64, 600);
            compare(
                &oracle,
                &s,
                &format!("bob-coins-{i}-{yaw}"),
                &inputs,
                &mut cov,
            );
        }
    }
    report("BOB coins", &cov);
    assert!(cov.coins >= 5, "too few BOB coins collected");
    assert!(
        cov.hud_coins >= 5 && cov.coin_sounds >= 5,
        "the HUD counter did not count BOB's coins"
    );
    assert!(cov.golden_sparkles > 0 && cov.coin_sparkles > 0);
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; drawing cannot change any compared frame word"]
fn drawing_bob_coins_and_sparkles_preserves_every_authoritative_word() {
    use rustario64::{
        content::Act,
        import::{animation, bob, engine, objects, rom::Rom},
        play::{Pad, Session},
        presentation::objects::{BillboardBasis, ObjectDrawer},
        simulation::object::render::visible_objects,
    };
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let imported = bob::import(&rom).unwrap();
    let world =
        rustario64::simulation::collision::CollisionWorld::load_area_terrain(&imported.collision)
            .unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
    let content = objects::bob(&rom, &imported.level, Act::new(1).unwrap()).unwrap();
    let node = imported.visual.as_ref().unwrap().camera.unwrap();
    let mut entry = GameEntry::script_start(&imported.level, &node).unwrap();
    let coin = content
        .area
        .macros
        .iter()
        .find(|e| e.behavior == content.content.scripts.address(Behavior::YellowCoin))
        .unwrap();
    entry.mario.spawn =
        rustario64::simulation::mario::core::SpawnPoint::from_level_script(1, 1, 0, coin.pos);
    let mut session = Session::new(&world, &trig, &anims, content.level_objects(), entry);
    let mut drawer = ObjectDrawer::new(&content.content, &trig);
    let basis = BillboardBasis {
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        toward: [0.0, 0.0, 1.0],
    };
    let mut visible = 0;
    for tick in 0..120 {
        assert!(session.step(&Pad::default()), "{:?}", session.stopped());
        let before = rustario64_oracle::camera_trace::capture_game(session.game());
        let objects = visible_objects(session.world(), &session.game().camera.graph);
        visible += objects.len();
        drawer.update(objects).unwrap();
        for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for interpolation in [false, true] {
                let _ = drawer.frame(alpha, interpolation, basis);
            }
        }
        if tick % 17 == 0 {
            drawer.snap();
        }
        assert_eq!(
            before,
            rustario64_oracle::camera_trace::capture_game(session.game())
        );
    }
    assert!(visible > 0, "no visible objects drawn");
    assert!(session.world().hud.coins >= 1);
    assert_eq!(session.world().hud.coins, session.mario().num_coins);
}
