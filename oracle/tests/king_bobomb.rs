//! Complete-frame boss comparisons with the same behavior acceptance policy
//! as runtime play. Seeded trajectories are initial-state fixtures, not an
//! end-to-end three-hit mission completion test.
#[path = "support/playground.rs"]
mod playground;
use playground::{
    ANIMATIONS, Builder, MODELS, SCRIPTS, authored_animations, computed_tables, world,
};
use rustario64::{
    content::dialog::{DialogEntry, DialogTable},
    simulation::{
        TickInput,
        camera::system::GeoCamera,
        controller::{A_BUTTON, B_BUTTON},
        game::GameEntry,
        mario::{
            constants as c,
            tick::{LevelEntry, LevelObjects},
        },
        object::{
            script::Behavior,
            spawn::{AreaObjects, SpawnInfo},
        },
    },
};
use rustario64_oracle::{
    Oracle,
    camera_trace::{GameScenario, first_difference},
};
use std::collections::BTreeSet;

fn field() -> Vec<i16> {
    let mut b = Builder::default();
    for gz in 0..10 {
        for gx in 0..10 {
            let x = [-5000 + gx * 1000, -4000 + gx * 1000];
            let z = [-5000 + gz * 1000, -4000 + gz * 1000];
            b.flat(
                c::SURFACE_DEFAULT,
                x,
                z,
                if (4..6).contains(&gx) && (4..6).contains(&gz) {
                    0
                } else {
                    -400
                },
                true,
            );
        }
    }
    b.stream()
}

fn dialogs() -> DialogTable {
    DialogTable {
        entries: vec![
            DialogEntry {
                unused: 1,
                lines_per_box: 4,
                left_offset: 95,
                width: 200,
                text: vec![0x0A, 0x0B, 0x0C, 0xFE, 0x0D, 0x0E, 0xFF],
            };
            170
        ],
    }
}

fn inputs() -> Vec<TickInput> {
    (0..1200)
        .map(|i| TickInput {
            buttons: if i > 40 && i % 20 == 0 {
                if i % 40 == 0 { A_BUTTON } else { B_BUTTON }
            } else {
                0
            },
            stick: [0, 0],
            camera_yaw: 0,
        })
        .collect()
}

#[test]
fn authored_king_introduction_and_grabs_match_the_decomp() {
    let scripts = SCRIPTS.clone();
    assert!(SCRIPTS.check(SCRIPTS.address(Behavior::KingBobomb)).is_ok());
    scripts
        .check(scripts.address(Behavior::KingBobomb))
        .unwrap();
    let models = models();
    let stream = field();
    let collision = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0xB055);
    let dialogs = dialogs();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let mut actions = BTreeSet::new();
    let mut frame_count = 0;
    for (seed, pos) in [[0, 40, 350], [350, 40, 0], [0, 40, -350]]
        .into_iter()
        .enumerate()
    {
        let s = GameScenario {
            collision: &collision,
            trig: &trig,
            anims: &anims,
            objects: LevelObjects {
                scripts: &scripts,
                models: &models,
                animations: &ANIMATIONS,
                dialogs: &dialogs,
                area: AreaObjects {
                    area_index: 1,
                    macros: vec![],
                    skipped: vec![],
                    spawn_infos: vec![SpawnInfo {
                        source: 0x0E000000,
                        start_pos: [0, 0, 0],
                        start_angle: [0, 0, 0],
                        area_index: 1,
                        active_area_index: 1,
                        behavior_arg: 0,
                        behavior_script: scripts.address(Behavior::KingBobomb),
                        model: c::MODEL_KING_BOBOMB as u8,
                    }],
                },
            },
            entry: GameEntry {
                mario: LevelEntry::from_level_script(c::LEVEL_BOB, 1, 180, pos, 0, 1),
                camera: GeoCamera {
                    mode: 1,
                    pos: [0.0, 1500.0, 3000.0],
                    focus: [0.0, 0.0, -500.0],
                },
                act_num: 1,
                rng_seed: seed as u16 * 977,
            },
        };
        let input = inputs();
        let (rust, stop) = s.rust(&input);
        assert!(stop.is_none(), "scenario {seed}: {stop:?}");
        let native = s.native(&oracle, &input);
        for (frame, (expected, actual)) in native.iter().zip(&rust).enumerate() {
            if let Some(d) = first_difference(expected, actual) {
                panic!("scenario {seed}, frame {frame}: {d}");
            }
            for (key, value) in actual {
                if *value == scripts.address(Behavior::KingBobomb) && key.ends_with("].behavior") {
                    let prefix = key.strip_suffix(".behavior").unwrap();
                    actions.insert(actual[&format!("{prefix}.raw[0x{:02X}]", c::O_ACTION)]);
                }
            }
        }
        frame_count += input.len();
    }
    println!("King Bob-omb: {frame_count} authored frames identical; actions {actions:?}");
    assert!(actions.contains(&0) && actions.contains(&2) && actions.contains(&3));
}

fn models() -> rustario64::simulation::object::render::ObjectModels {
    let mut out = MODELS.clone();
    let graph = out.traversal(c::MODEL_BLACK_BOBOMB as u16).cloned();
    use rustario64::{
        import::geo::{GeoLayout, GeoNode, GeoNodeKind},
        simulation::object::render::{ObjectModel, RenderNode, RenderNodeKind},
    };
    let kinds = [
        GeoNodeKind::Start,
        GeoNodeKind::Scale {
            layer: 0,
            scale: 16384,
            display_list: None,
        },
        GeoNodeKind::AnimatedPart {
            layer: 1,
            translation: [11, 23, 37],
            display_list: None,
        },
        GeoNodeKind::AnimatedPart {
            layer: 1,
            translation: [43, 59, 71],
            display_list: None,
        },
        GeoNodeKind::Generated {
            param: 0,
            callback: 0x80001000,
        },
    ];
    let geo: Vec<GeoNode> = kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| GeoNode {
            source_address: 0x0C001000 + i as u32 * 16,
            flags: 1 | (u16::from(kind.layer()) << 8),
            kind,
            children: if i < 4 { vec![i + 1] } else { vec![] },
            views: vec![],
        })
        .collect();
    let nodes = geo
        .iter()
        .enumerate()
        .map(|(i, g)| RenderNode {
            kind: if i == 4 {
                RenderNodeKind::HeldMarioPos
            } else {
                RenderNodeKind::Plain
            },
            children: g.children.clone(),
        })
        .collect();
    out.insert(
        c::MODEL_KING_BOBOMB as u16,
        Some(ObjectModel {
            nodes,
            root: 0,
            anchor_layout: Some(GeoLayout {
                entry: 0x0C001000,
                root: Some(0),
                nodes: geo,
                detached: vec![],
            }),
        }),
    );
    out.insert(
        c::MODEL_MIST as u16,
        Some(rustario64::simulation::object::render::ObjectModel {
            nodes: vec![rustario64::simulation::object::render::RenderNode {
                kind: rustario64::simulation::object::render::RenderNodeKind::LayerTransparency,
                children: vec![],
            }],
            root: 0,
            anchor_layout: None,
        }),
    );
    out.insert(c::MODEL_DIRT_ANIMATION as u16, graph);
    out
}

#[derive(Default, Debug)]
struct Coverage {
    frames: usize,
    actions: BTreeSet<u32>,
    dialogs: BTreeSet<u32>,
    shakes: usize,
    particles: usize,
    stars: usize,
    held: usize,
    mario_actions: BTreeSet<u32>,
    escapes: usize,
}

/// A single initial-state fixture followed by autonomous complete frames.
/// Patching happens once after the behavior initializes, on both engines;
/// no Rust state is copied into C during the trajectory.
fn trajectory(
    oracle: &Oracle,
    s: &GameScenario<'_>,
    label: &str,
    seed: &[(usize, u32)],
    input: &[TickInput],
    cov: &mut Coverage,
    escape_controls: bool,
) {
    use rustario64::simulation::{game::Game, object::ObjectId};
    use rustario64_oracle::{camera_trace::capture_game, object_trace::NativeObjects};
    let mut game = Game::enter(s.collision, s.trig, s.anims, &s.objects, &s.entry);
    oracle.tick_set_objects(&NativeObjects::new(&s.objects));
    oracle.tick_begin(&s.setup());
    let check = |frame, game: &Game<'_>, cov: &mut Coverage| {
        let native = oracle.tick_snapshot();
        let rust = capture_game(game);
        if let Some(d) = first_difference(&native, &rust) {
            panic!("{label} frame {frame}: {d}");
        }
        let king = s.objects.scripts.address(Behavior::KingBobomb);
        let puff = s.objects.scripts.address(Behavior::WhitePuffExplosion);
        let dirt = s.objects.scripts.address(Behavior::BreakBoxTriangle);
        for (key, value) in &rust {
            if key.ends_with("].behavior") {
                if *value == king {
                    let prefix = key.strip_suffix(".behavior").unwrap();
                    cov.actions
                        .insert(rust[&format!("{prefix}.raw[0x{:02X}]", c::O_ACTION)]);
                    cov.held += usize::from(
                        rust[&format!("{prefix}.raw[0x{:02X}]", c::O_HELD_STATE)]
                            == c::HELD_HELD as u32,
                    );
                    cov.escapes += usize::from(
                        rust[&format!("{prefix}.raw[0x{:02X}]", c::O_KING_BOBOMB_UNK108)] == 35,
                    );
                }
                if *value == puff || *value == dirt {
                    cov.particles += 1;
                }
            }
            if key.ends_with(".kind") && key.starts_with("events[") {
                if *value == 21 {
                    cov.shakes += 1;
                }
                if *value == 13 && rust[&key.replace(".kind", ".a")] == 5 {
                    cov.stars += 1;
                }
            }
        }
        cov.mario_actions.insert(rust["m.action"]);
        if rust["dialog.id"] as i32 >= 0 {
            cov.dialogs.insert(rust["dialog.id"]);
        }
    };
    check(0, &game, cov);
    for (frame, input) in input.iter().enumerate() {
        let input = if escape_controls && game.mario.action == c::ACT_GRABBED {
            TickInput {
                buttons: if frame % 2 == 0 { A_BUTTON } else { 0 },
                stick: if frame % 2 == 1 { [0, 64] } else { [0, 0] },
                ..*input
            }
        } else {
            *input
        };
        let (_, result) = game.frame(input.buttons, input.stick);
        result.camera.expect("supported camera path");
        oracle.tick(input);
        check(frame + 1, &game, cov);
        if frame == 0 && !seed.is_empty() {
            let words = capture_game(&game);
            let key = words
                .iter()
                .find(|(k, v)| {
                    k.ends_with("].behavior")
                        && **v == s.objects.scripts.address(Behavior::KingBobomb)
                })
                .unwrap()
                .0;
            let slot: u32 = key
                .strip_prefix("objects[")
                .unwrap()
                .split(']')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            for (field, word) in seed {
                game.world
                    .objects
                    .slot_mut(ObjectId(slot))
                    .raw
                    .set_u32(*field, *word);
                oracle.tick_seed_boss_field(slot, *field, *word);
            }
            check(1, &game, cov);
        }
        cov.frames += 1;
    }
}

#[test]
fn authored_king_damage_return_and_defeat_match_the_decomp() {
    let scripts = SCRIPTS.clone();
    let models = models();
    let stream = field();
    let collision = world(&stream);
    let trig = computed_tables();
    let anims = authored_animations(0xB055);
    let dialogs = dialogs();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let s = GameScenario {
        collision: &collision,
        trig: &trig,
        anims: &anims,
        objects: LevelObjects {
            scripts: &scripts,
            models: &models,
            animations: &ANIMATIONS,
            dialogs: &dialogs,
            area: AreaObjects {
                area_index: 1,
                macros: vec![],
                skipped: vec![],
                spawn_infos: vec![SpawnInfo {
                    source: 0x0E000000,
                    start_pos: [0, 0, 0],
                    start_angle: [0, 0, 0],
                    area_index: 1,
                    active_area_index: 1,
                    behavior_arg: 0,
                    behavior_script: scripts.address(Behavior::KingBobomb),
                    model: c::MODEL_KING_BOBOMB as u8,
                }],
            },
        },
        entry: GameEntry {
            mario: LevelEntry::from_level_script(c::LEVEL_BOB, 1, 180, [0, 40, 800], 0, 1),
            camera: GeoCamera {
                mode: 1,
                pos: [0.0, 1500.0, 3000.0],
                focus: [0.0, 0.0, -500.0],
            },
            act_num: 1,
            rng_seed: 977,
        },
    };
    let mut cov = Coverage::default();
    for (label, x, y, health) in [
        ("hit", 0.0f32, 140.0f32, 3),
        ("defeat", 0.0, 140.0, 1),
        ("return", 1200.0, -260.0, 3),
    ] {
        let seed = [
            (c::O_ACTION, 4),
            (c::O_PREV_ACTION, 4),
            (c::O_SUB_ACTION, 0),
            (c::O_TIMER, 0),
            (c::O_POS_X, x.to_bits()),
            (c::O_POS_Y, y.to_bits()),
            (c::O_VEL_Y, (-5.0f32).to_bits()),
            (c::O_HEALTH, health),
            (c::O_MOVE_FLAGS, c::OBJ_MOVE_IN_AIR),
        ];
        trajectory(&oracle, &s, label, &seed, &inputs()[..700], &mut cov, false);
    }
    let hold_entry = GameEntry {
        mario: LevelEntry::from_level_script(c::LEVEL_BOB, 1, 0, [0, 40, -120], 0, 1),
        ..s.entry
    };
    let hold = GameScenario {
        entry: hold_entry,
        ..s
    };
    trajectory(
        &oracle,
        &hold,
        "heavy pickup",
        &ready(),
        &heavy_inputs(),
        &mut cov,
        false,
    );
    let escape = GameScenario {
        entry: GameEntry {
            mario: LevelEntry::from_level_script(c::LEVEL_BOB, 1, 180, [0, 40, 350], 0, 1),
            ..hold.entry
        },
        ..hold
    };
    trajectory(
        &oracle,
        &escape,
        "grab escape",
        &[],
        &inputs()[..700],
        &mut cov,
        true,
    );
    println!("Authored boss trajectories: {cov:?}");
    assert!([4, 5, 6, 7, 8].iter().all(|a| cov.actions.contains(a)));
    assert!(cov.shakes > 0 && cov.particles > 0 && cov.stars == 1);
    assert!(cov.held > 0 && cov.mario_actions.contains(&c::ACT_HEAVY_THROW));
    assert!(cov.escapes > 0);
    assert!(cov.dialogs.contains(&116) && cov.dialogs.contains(&128));
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; original BOB summit, models, animations and dialog text"]
fn bob_king_trajectories_match_the_decomp_with_rom_data() {
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
    let start = (version::BOB_COLLISION & 0xFFFFFF) as usize;
    let (mesh, consumed) = collision::decode(&terrain[start..]).unwrap();
    let stream: Vec<i16> = terrain[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    let imported = bob::import(&rom).unwrap();
    let collision =
        rustario64::simulation::collision::CollisionWorld::load_area_terrain(&mesh).unwrap();
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    let node = imported.visual.as_ref().unwrap().camera.unwrap();
    let script_entry = GameEntry::script_start(&imported.level, &node).unwrap();
    let content = objects::bob(&rom, &imported.level, Act::new(1).unwrap()).unwrap();
    let scripts = content.content.scripts.clone();
    let king = scripts.address(Behavior::KingBobomb);
    let placement = content
        .area
        .spawn_infos
        .iter()
        .find(|i| i.behavior_script == king)
        .expect("BOB act 1 boss");
    let home = placement.start_pos;
    let mut cov = Coverage::default();
    for (i, side) in [0.0f32, 90.0, 210.0].into_iter().enumerate() {
        let angle = side.to_radians();
        let pos = [
            home[0] + (350.0 * angle.sin()).round() as i16,
            home[1] + 40,
            home[2] + (350.0 * angle.cos()).round() as i16,
        ];
        let s = GameScenario {
            collision: &collision,
            trig: &trig,
            anims: &anims,
            objects: LevelObjects {
                scripts: &scripts,
                ..content.level_objects()
            },
            entry: GameEntry {
                mario: LevelEntry {
                    spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                        script_entry.mario.spawn.behavior_arg,
                        1,
                        ((side + 180.0) % 360.0) as i16,
                        pos,
                    ),
                    ..script_entry.mario
                },
                rng_seed: i as u16 * 31,
                ..script_entry
            },
        };
        trajectory(
            &oracle,
            &s,
            &format!("BOB intro {side}"),
            &[],
            &inputs()[..900],
            &mut cov,
            false,
        );
    }
    let (return_x, return_y) = [1200.0f32, 1600.0, 2000.0, -1200.0, -1600.0, -2000.0]
        .into_iter()
        .find_map(|dx| {
            let (floor, surface) = collision.find_floor(
                f32::from(home[0]) + dx,
                f32::from(home[1]) + 1000.0,
                f32::from(home[2]),
                &mut Default::default(),
            );
            (surface.is_some()
                && floor < f32::from(home[1]) - 200.0
                && floor > f32::from(home[1]) - 1200.0)
                .then_some((dx, floor + 140.0 - f32::from(home[1])))
        })
        .expect("an off-summit floor for the return fixture");
    for (label, dx, dy, health) in [
        ("BOB hit", 0.0f32, 140.0f32, 3),
        ("BOB defeat", 0.0, 140.0, 1),
        ("BOB return", return_x, return_y, 3),
    ] {
        let s = GameScenario {
            collision: &collision,
            trig: &trig,
            anims: &anims,
            objects: LevelObjects {
                scripts: &scripts,
                ..content.level_objects()
            },
            entry: GameEntry {
                mario: LevelEntry {
                    spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                        script_entry.mario.spawn.behavior_arg,
                        1,
                        180,
                        [home[0], home[1] + 40, home[2] + 650],
                    ),
                    ..script_entry.mario
                },
                rng_seed: 977,
                ..script_entry
            },
        };
        let seed = [
            (c::O_ACTION, 4),
            (c::O_PREV_ACTION, 4),
            (c::O_SUB_ACTION, 0),
            (c::O_TIMER, 0),
            (c::O_POS_X, (f32::from(home[0]) + dx).to_bits()),
            (c::O_POS_Y, (f32::from(home[1]) + dy).to_bits()),
            (c::O_VEL_Y, (-5.0f32).to_bits()),
            (c::O_HEALTH, health),
            (c::O_MOVE_FLAGS, c::OBJ_MOVE_IN_AIR),
        ];
        trajectory(&oracle, &s, label, &seed, &inputs()[..700], &mut cov, false);
    }
    let angle = f32::from(placement.start_angle[1]) * std::f32::consts::PI / 32768.0;
    let behind = [
        home[0] - (120.0 * angle.sin()).round() as i16,
        home[1] + 40,
        home[2] - (120.0 * angle.cos()).round() as i16,
    ];
    let heavy = GameScenario {
        collision: &collision,
        trig: &trig,
        anims: &anims,
        objects: LevelObjects {
            scripts: &scripts,
            ..content.level_objects()
        },
        entry: GameEntry {
            mario: LevelEntry {
                spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                    script_entry.mario.spawn.behavior_arg,
                    1,
                    -147,
                    behind,
                ),
                ..script_entry.mario
            },
            rng_seed: 997,
            ..script_entry
        },
    };
    trajectory(
        &oracle,
        &heavy,
        "BOB heavy pickup",
        &ready(),
        &heavy_inputs(),
        &mut cov,
        false,
    );
    let escape = GameScenario {
        entry: GameEntry {
            mario: LevelEntry {
                spawn: rustario64::simulation::mario::core::SpawnPoint::from_level_script(
                    script_entry.mario.spawn.behavior_arg,
                    1,
                    180,
                    [home[0], home[1] + 40, home[2] + 350],
                ),
                ..script_entry.mario
            },
            ..script_entry
        },
        ..heavy
    };
    trajectory(
        &oracle,
        &escape,
        "BOB escape",
        &[],
        &inputs()[..700],
        &mut cov,
        true,
    );
    println!("BOB boss trajectories: {cov:?}");
    assert!(
        [0, 1, 2, 3, 4, 5, 6, 7, 8]
            .iter()
            .all(|a| cov.actions.contains(a))
    );
    assert!(cov.shakes > 0 && cov.particles > 0 && cov.stars == 1);
    assert!(cov.held > 0 && cov.mario_actions.contains(&c::ACT_HEAVY_THROW));
    assert!(cov.escapes > 0);
    assert!(cov.dialogs.contains(&17) && cov.dialogs.contains(&116) && cov.dialogs.contains(&128));
}

fn ready() -> Vec<(usize, u32)> {
    vec![
        (c::O_ACTION, 2),
        (c::O_PREV_ACTION, 2),
        (c::O_SUB_ACTION, 0),
        (c::O_TIMER, 0),
        (c::O_INTANGIBLE_TIMER, 0),
        (
            c::O_FLAGS,
            c::OBJ_FLAG_COMPUTE_ANGLE_TO_MARIO
                | c::OBJ_FLAG_ACTIVE_FROM_AFAR
                | c::OBJ_FLAG_COMPUTE_DIST_TO_MARIO
                | c::OBJ_FLAG_SET_FACE_YAW_TO_MOVE_YAW
                | c::OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE
                | c::OBJ_FLAG_HOLDABLE,
        ),
    ]
}
fn heavy_inputs() -> Vec<TickInput> {
    (0..700)
        .map(|i| TickInput {
            buttons: if i == 1 || i == 100 { B_BUTTON } else { 0 },
            stick: if (45..90).contains(&i) {
                [0, 50]
            } else {
                [0, 0]
            },
            camera_yaw: 0,
        })
        .collect()
}
