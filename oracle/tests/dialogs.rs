//! Dialog differential tests: talking to a Bob-omb Buddy. The decomp's
//! verbatim Buddy behavior, Mario's text interaction (interact_text,
//! check_npc_talk) and NPC dialog actions, the camera's dialog cutscene
//! (start_object_cutscene, cutscene_object_with_dialog and sCutsceneDialog's
//! shots through an authored partial play_cutscene), time stop and the dialog
//! box (ingame_menu.c's state and page breaks, with drawing discarded), all
//! compiled natively, against the Rust port through `simulation::game`. Every
//! word of Mario, the camera (with the cutscene variables), every object and
//! the dialog box must match after every frame. CI uses an authored field
//! and an authored dialog table; the ignored test talks to BOB's two act-1
//! Buddies with the owner's ROM, its text and its Buddy model.
#[path = "support/playground.rs"]
mod playground;
use playground::{
    ANIMATIONS, Builder, Lcg, MODELS, SCRIPTS, authored_animations, computed_tables, world,
};
use rustario64::{
    content::dialog::{DialogEntry, DialogTable},
    simulation::{
        TickInput,
        camera::system::GeoCamera,
        controller::{A_BUTTON, B_BUTTON, Z_TRIG},
        game::GameEntry,
        mario::{
            constants as c,
            core::SpawnPoint,
            tick::{LevelEntry, LevelObjects},
        },
        object::{
            script::{Behavior, BehaviorScripts},
            spawn::{AreaObjects, MacroEntry, SpawnInfo, convert_rotation},
        },
    },
};
use rustario64_oracle::{
    Oracle,
    camera_trace::{GameScenario, RustStop, first_difference},
};
use std::collections::BTreeMap;

/// A flat field at y = 0.
fn field() -> Vec<i16> {
    let mut b = Builder::default();
    for gz in 0..10 {
        for gx in 0..10 {
            let x = [-5000 + gx * 1000, -4000 + gx * 1000];
            let z = [-5000 + gz * 1000, -4000 + gz * 1000];
            b.flat(c::SURFACE_DEFAULT, x, z, 0, true);
        }
    }
    b.stream()
}

/// Authored text (not the game's): letters, spaces and the multi-character
/// and star-count codes, with each entry's line count chosen so its pages
/// end on and between box boundaries.
fn authored_dialogs() -> DialogTable {
    let line = |n: usize, seed: u8| -> Vec<u8> {
        (0..n)
            .map(|i| match (i + usize::from(seed)) % 7 {
                0 => 0x9E,
                3 => 0xD1,
                5 if seed == 3 => 0xE0,
                k => 0x0A + ((k as u8 * 5 + seed) % 52),
            })
            .collect()
    };
    let entry = |lines_per_box: i8, lines: usize, seed: u8| {
        let mut text = vec![];
        for i in 0..lines {
            if i > 0 {
                text.push(0xFE);
            }
            text.extend(line(8 + (i * 3 + usize::from(seed)) % 13, seed));
        }
        text.push(0xFF);
        DialogEntry {
            unused: 1,
            lines_per_box,
            left_offset: 95,
            width: 200,
            text,
        }
    };
    DialogTable {
        entries: vec![
            entry(4, 3, 0),
            entry(4, 4, 1),
            entry(4, 9, 2),
            entry(5, 12, 3),
            entry(3, 7, 4),
        ],
    }
}

fn buddy(index: u32, pos: [i16; 3], yaw_degrees: i32, dialog: u32, model: i32) -> SpawnInfo {
    SpawnInfo {
        source: 0x0E00_0000 + 0x18 * index,
        start_pos: pos,
        start_angle: [0, (yaw_degrees * 0x8000 / 180) as i16, 0],
        area_index: 1,
        active_area_index: 1,
        behavior_arg: dialog << 16,
        behavior_script: SCRIPTS.address(Behavior::BobombBuddy),
        model: model as u8,
    }
}

/// A Bob-omb (macro_bobomb's preset) and a yellow coin that time stop must
/// freeze while a dialog plays.
fn bystanders() -> Vec<MacroEntry> {
    let entry = |index: u32, preset: u16, behavior, model: i32, pos| {
        let packed = preset + 31;
        MacroEntry {
            source: 0x0701_0000 + 10 * index,
            packed,
            preset,
            behavior: SCRIPTS.address(behavior),
            model: model as i16,
            preset_param: 0,
            pos,
            yaw: convert_rotation((((packed as i16) >> 9) & 0x7F) << 1),
            params: 0,
        }
    };
    vec![
        entry(
            0,
            20,
            Behavior::Bobomb,
            c::MODEL_BLACK_BOBOMB,
            [1500, 0, 900],
        ),
        entry(
            1,
            0,
            Behavior::YellowCoin,
            c::MODEL_YELLOW_COIN,
            [-700, 0, 300],
        ),
    ]
}

/// Buddies at the origin and 1000 units away (dialogs 2 and 3), drawn with
/// the authored Bob-omb model so their walk animation advances.
fn placements() -> AreaObjects {
    AreaObjects {
        area_index: 1,
        macros: bystanders(),
        spawn_infos: vec![
            buddy(0, [0, 0, 0], 0, 2, c::MODEL_BLACK_BOBOMB),
            buddy(1, [-1000, 0, -1000], 90, 3, c::MODEL_BLACK_BOBOMB),
            buddy(2, [1000, 0, -1500], 0, 4, c::MODEL_NONE),
        ],
        skipped: vec![],
    }
}

fn push(out: &mut Vec<TickInput>, n: usize, buttons: u16, stick: [i8; 2]) {
    out.extend(std::iter::repeat_n(
        TickInput {
            buttons,
            stick,
            camera_yaw: 0,
        },
        n,
    ));
}

/// Talk with B (or A), wait for the box, advance pages with presses every
/// `page` frames (alternating A and B), then wait for the cutscene to end.
fn talk(out: &mut Vec<TickInput>, talk_button: u16, page: usize, pages: usize) {
    push(out, 1, talk_button, [0, 0]);
    push(out, 45, 0, [0, 0]);
    for i in 0..pages {
        push(out, 1, if i % 2 == 0 { A_BUTTON } else { B_BUTTON }, [0, 0]);
        push(out, page - 1, 0, [0, 0]);
    }
    push(out, 50, 0, [0, 0]);
}

/// Scripted conversations and random play between them.
fn inputs(seed: u64) -> Vec<TickInput> {
    let mut rng = Lcg(seed);
    let mut out = vec![];
    push(&mut out, 4, 0, [0, 0]);
    talk(&mut out, B_BUTTON, 20, 6);
    // Again at once, faster presses (presses during scrolling are ignored),
    // and A held through a page.
    talk(&mut out, A_BUTTON, 9, 9);
    push(&mut out, 30, A_BUTTON, [0, 0]);
    push(&mut out, 10, 0, [0, 0]);
    talk(&mut out, B_BUTTON, 3, 25);
    // Random play: walks, jumps, punches, crouches.
    for _ in 0..12 {
        let n = rng.range(10, 60) as usize;
        let stick = if rng.chance(3) {
            [0, 0]
        } else {
            [rng.range(-128, 127) as i8, rng.range(-128, 127) as i8]
        };
        let mut buttons = 0;
        for (button, odds) in [(A_BUTTON, 4), (B_BUTTON, 3), (Z_TRIG, 12)] {
            if rng.chance(odds) {
                buttons |= button;
            }
        }
        for i in 0..n {
            let b = if i % 9 == 0 {
                buttons
            } else {
                buttons & Z_TRIG
            };
            push(&mut out, 1, b, stick);
        }
    }
    push(&mut out, 20, 0, [0, 0]);
    talk(&mut out, B_BUTTON, 15, 12);
    out
}

#[derive(Default)]
struct Coverage {
    frames: usize,
    cutscene_frames: usize,
    box_frames: usize,
    pages_scrolled: usize,
    dialogs_closed: usize,
    time_stop_frames: usize,
    reading_frames: usize,
    conversations: usize,
    dialogs: std::collections::BTreeSet<u32>,
    stops: Vec<String>,
}

fn tally(
    scripts: &BehaviorScripts,
    words: &BTreeMap<String, u32>,
    previous: Option<&BTreeMap<String, u32>>,
    cov: &mut Coverage,
) {
    cov.cutscene_frames += usize::from(words["camera.c.cutscene"] == u32::from(c::CUTSCENE_DIALOG));
    let id = words["dialog.id"] as i32;
    if id != -1 {
        cov.box_frames += 1;
        cov.dialogs.insert(id as u32);
    }
    cov.time_stop_frames += usize::from(words["world.timeStopState"] & c::TIME_STOP_ENABLED != 0);
    cov.reading_frames += usize::from(words["m.action"] == c::ACT_READING_NPC_DIALOG);
    if let Some(p) = previous {
        cov.pages_scrolled +=
            usize::from(words["dialog.menuState"] == 2 && p["dialog.menuState"] != 2);
        cov.dialogs_closed += usize::from(id == -1 && p["dialog.id"] as i32 != -1);
        let buddy = scripts.address(Behavior::BobombBuddy);
        for (name, value) in words {
            let Some(slot) = name
                .strip_prefix("objects[")
                .and_then(|r| r.strip_suffix("].behavior"))
            else {
                continue;
            };
            if *value != buddy {
                continue;
            }
            let action = format!("objects[{slot}].raw[0x{:02X}]", c::O_ACTION);
            if words[&action] as i32 == c::BOBOMB_BUDDY_ACT_IDLE
                && p.get(&action)
                    .is_some_and(|a| *a as i32 == c::BOBOMB_BUDDY_ACT_TALK)
            {
                cov.conversations += 1;
            }
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
        "{label}: {} frames identical; {} conversations finished, dialogs {:?}; {} dialog \
         cutscene frames, {} dialog box frames, {} pages scrolled, {} boxes closed, {} time-stop \
         frames, {} frames of Mario reading",
        cov.frames,
        cov.conversations,
        cov.dialogs,
        cov.cutscene_frames,
        cov.box_frames,
        cov.pages_scrolled,
        cov.dialogs_closed,
        cov.time_stop_frames,
        cov.reading_frames
    );
    for stop in &cov.stops {
        println!("{label}: stopped: {stop}");
    }
}

/// Mario 120 units from a Buddy on its `side` (degrees around it), facing
/// it.
fn beside(pos: [i16; 3], side: i32) -> ([i16; 3], i16) {
    let angle = (side as f32).to_radians();
    let offset = [120.0 * angle.sin(), 120.0 * angle.cos()];
    (
        [
            pos[0] + offset[0].round() as i16,
            pos[1] + 40,
            pos[2] + offset[1].round() as i16,
        ],
        ((side + 180) % 360) as i16,
    )
}

#[test]
fn authored_buddy_conversations_match_the_decomp() {
    let stream = field();
    let world = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_0005);
    let dialogs = authored_dialogs();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut cov = Coverage::default();
    for (seed, (target, side, mode)) in [
        ([0, 0, 0], 0, 1),
        ([0, 0, 0], 120, 1),
        ([0, 0, 0], 250, 4),
        ([-1000, 0, -1000], 45, 1),
        ([1000, 0, -1500], 300, 1),
        ([-1000, 0, -1000], 200, 4),
    ]
    .into_iter()
    .enumerate()
    {
        let (pos, yaw) = beside(target, side);
        let s = GameScenario {
            collision: &world,
            trig: &trig,
            anims: &anims,
            objects: LevelObjects {
                scripts: &SCRIPTS,
                models: &MODELS,
                animations: &ANIMATIONS,
                area: placements(),
                dialogs: &dialogs,
            },
            entry: GameEntry {
                mario: LevelEntry::from_level_script(c::LEVEL_BOB, 1, yaw, pos, 0, mode),
                camera: GeoCamera {
                    mode,
                    pos: [0.0, 1500.0, 3000.0],
                    focus: [0.0, 0.0, -500.0],
                },
                act_num: 1,
                rng_seed: seed as u16 * 977,
            },
        };
        compare(
            &oracle,
            &s,
            &format!("buddy-{seed}"),
            &inputs(0xD1A + seed as u64),
            &mut cov,
        );
    }
    report("authored Buddies", &cov);
    assert!(cov.stops.is_empty(), "a run stopped: {:?}", cov.stops);
    assert!(cov.frames >= 6000, "too few compared frames");
    assert!(cov.conversations >= 12, "too few finished conversations");
    assert!(cov.dialogs.len() == 3, "not every Buddy talked");
    assert!(cov.pages_scrolled > 0 && cov.dialogs_closed > 0);
    assert!(cov.time_stop_frames > 0 && cov.reading_frames > 0);
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; BOB's Bob-omb Buddies with the owner's ROM"]
fn bob_buddy_conversations_match_the_decomp_with_rom_data() {
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
    let buddy = scripts.address(Behavior::BobombBuddy);
    let buddies: Vec<[i16; 3]> = content
        .area
        .spawn_infos
        .iter()
        .filter(|i| i.behavior_script == buddy)
        .map(|i| i.start_pos)
        .collect();
    assert_eq!(buddies.len(), 2, "BOB act 1 places two Bob-omb Buddies");
    let mut cov = Coverage::default();
    for (i, target) in buddies.iter().enumerate() {
        for (j, side) in [0, 90, 200, 300].into_iter().enumerate() {
            let (pos, yaw) = beside(*target, side);
            let s = GameScenario {
                collision: &world,
                trig: &trig,
                anims: &anims,
                objects: content.level_objects(),
                entry: GameEntry {
                    mario: LevelEntry {
                        spawn: SpawnPoint::from_level_script(
                            1,
                            script_entry.mario.spawn.area_index as u8,
                            yaw,
                            pos,
                        ),
                        ..script_entry.mario
                    },
                    rng_seed: (i * 4 + j) as u16 * 31,
                    ..script_entry
                },
            };
            compare(
                &oracle,
                &s,
                &format!("bob-buddy-{i}-{side}"),
                &inputs(0xB0BD + (i * 4 + j) as u64),
                &mut cov,
            );
        }
    }
    report("BOB Buddies", &cov);
    assert!(cov.stops.is_empty(), "a run stopped: {:?}", cov.stops);
    assert!(cov.frames >= 8000, "too few compared frames");
    assert_eq!(
        cov.dialogs,
        [c::DIALOG_001 as u32, c::DIALOG_002 as u32].into(),
        "both Buddies' dialogs played"
    );
    assert!(cov.conversations >= 16, "too few finished conversations");
    assert!(cov.pages_scrolled > 0 && cov.dialogs_closed > 0);
}
