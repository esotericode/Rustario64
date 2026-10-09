//! Mario's model: importing his geo layout and display lists from the owner's
//! ROM and posing them from completed ticks. The ignored owner-ROM test checks
//! builds of six switch configurations against triangle counts and digests
//! that tools/check_mario_model_reference.py reproduces from the pinned decomp
//! sources, then poses Mario through a played session.
use rustario64::{
    content::visual::SkinnedModel,
    import::{animation, bob, engine, mario, rom::Rom, sha1_hex},
    play::{Pad, Session},
    presentation::mario::{MarioDrawer, MarioPose},
    simulation::{
        collision::CollisionWorld,
        game::GameEntry,
        mario::{MarioBodyState, constants as c},
    },
};

/// (configuration, triangles, SHA-1 of every triangle vertex's bone,
/// position and color/normal bytes), from tools/check_mario_model_reference.py.
const CONFIGURATIONS: [(&str, usize, &str); 6] = [
    ("standing", 752, "96b96f780903ed14366c11d0f0c8c05ae008fc3f"),
    (
        "moving-near-holding-cap",
        804,
        "59e4ad756541fcf404e921228a45bc24f574ad2d",
    ),
    (
        "moving-medium-cap-off",
        585,
        "320483cde439461fa57bb131f5f8b231c4212d5c",
    ),
    (
        "moving-far-wing-cap",
        284,
        "02960539548f5371cbca8985d6870cb8550cf1db",
    ),
    (
        "metal-standing",
        812,
        "de2746472af5e6ebb24d325d8ba59ad14ad8aeb1",
    ),
    (
        "vanish-flying",
        796,
        "0394142fc8341a04e29ae917be2ba4bda2110dd9",
    ),
];

/// The body state (and blink counter) that selects each configuration through
/// Mario's switch callbacks, and the level-of-detail distance.
fn configuration(name: &str) -> (MarioPose, i16) {
    let mut body = MarioBodyState::default();
    let mut counter = 0;
    let lod = match name {
        "standing" => {
            body.action = c::ACT_IDLE;
            body.eye_state = c::MARIO_EYES_OPEN;
            5000
        }
        "moving-near-holding-cap" => {
            body.action = c::ACT_WALKING;
            // Blinking: ((0 * 32 + 2) >> 1) & 0x1F = 1 selects the closed eyes.
            body.eye_state = c::MARIO_EYES_BLINK;
            counter = 2;
            body.hand_state = c::MARIO_HAND_HOLDING_CAP;
            100
        }
        "moving-medium-cap-off" => {
            body.action = c::ACT_WALKING;
            body.cap_state = c::MARIO_HAS_DEFAULT_CAP_OFF;
            body.eye_state = c::MARIO_EYES_DEAD;
            body.hand_state = c::MARIO_HAND_OPEN;
            1000
        }
        "moving-far-wing-cap" => {
            body.action = c::ACT_WALKING;
            body.cap_state = c::MARIO_HAS_WING_CAP_ON;
            body.eye_state = c::MARIO_EYES_CLOSED;
            body.hand_state = c::MARIO_HAND_PEACE_SIGN;
            2000
        }
        "metal-standing" => {
            body.action = c::ACT_IDLE;
            body.model_state = c::MODEL_STATE_METAL;
            body.eye_state = c::MARIO_EYES_OPEN;
            body.hand_state = c::MARIO_HAND_HOLDING_WING_CAP;
            0
        }
        "vanish-flying" => {
            body.action = c::ACT_FLYING;
            body.model_state = c::MODEL_STATE_NOISE_ALPHA;
            body.cap_state = c::MARIO_HAS_WING_CAP_ON;
            body.eye_state = c::MARIO_EYES_OPEN;
            body.hand_state = c::MARIO_HAND_FISTS;
            100
        }
        _ => unreachable!(),
    };
    let pose = MarioPose {
        visible: true,
        throw_matrix: None,
        position: [0.0; 3],
        angle: [0; 3],
        scale: [1.0; 3],
        animation: None,
        body,
        area_update_counter: counter,
        camera_c_up: false,
        head_rotation: [0; 3],
    };
    (pose, lod)
}

fn digest(built: &SkinnedModel) -> (usize, String) {
    let mut data = vec![];
    let mut vertices = 0;
    for (batch, bones) in built.model.batches.iter().zip(&built.bones) {
        assert_eq!(batch.vertices.len(), bones.len());
        for (vertex, bone) in batch.vertices.iter().zip(bones) {
            data.extend_from_slice(&bone.to_be_bytes());
            for p in vertex.position {
                // Built with the identity, positions are the source integers.
                assert_eq!(p, p.trunc());
                data.extend_from_slice(&(p as i16).to_be_bytes());
            }
            data.extend_from_slice(&vertex.color);
            vertices += 1;
        }
    }
    (vertices / 3, sha1_hex(&data))
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn local_us_rom_mario_model() {
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let source = mario::import(&rom).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
    let mut drawer = MarioDrawer::new(&source, &trig, &anims);
    for (name, triangles, expected) in CONFIGURATIONS {
        let (pose, lod) = configuration(name);
        let (draws, _) = drawer.traverse(&pose, Some(lod));
        let (built, issues) = source.build(&draws).unwrap();
        for issue in &issues {
            println!("{name}: 0x{:08X} {}", issue.address, issue.feature);
        }
        assert_eq!(digest(&built), (triangles, expected.to_owned()), "{name}");
        println!(
            "{name}: {triangles} triangles in {} batches with {} textures match the source",
            built.model.batches.len(),
            built.model.textures.len()
        );
    }

    // Posed through a played session on BOB: the entry is not drawn, then
    // every tick poses a model whose bones stay near Mario.
    let imported = bob::import(&rom).unwrap();
    let world = CollisionWorld::load_area_terrain(&imported.collision).unwrap();
    let camera = imported.visual.as_ref().unwrap().camera.unwrap();
    let entry = GameEntry::script_start(&imported.level, &camera).unwrap();
    let mut session = Session::new(&world, &trig, &anims, entry);
    drawer.update(&session.mario_pose(), None).unwrap();
    assert!(
        drawer.frame(1.0, true).is_none(),
        "the entry state is never drawn"
    );
    let (idle, run, jump) = (
        Pad::default(),
        Pad {
            up: true,
            ..Pad::default()
        },
        Pad {
            up: true,
            a: true,
            ..Pad::default()
        },
    );
    for tick in 1..=90 {
        let pad = match tick {
            ..=10 => &idle,
            40 | 41 => &jump,
            _ => &run,
        };
        assert!(session.step(pad));
        let pose = session.mario_pose();
        drawer.update(&pose, Some(1000)).unwrap();
        for alpha in [0.0, 0.5, 1.0] {
            let frame = drawer.frame(alpha, true).unwrap();
            for v in frame.vertices.iter().flatten() {
                let d: f32 = (0..3)
                    .map(|i| (v.position[i] - pose.position[i]).abs())
                    .sum();
                assert!(d < 500.0, "tick {tick}: a vertex {d} units from Mario");
            }
        }
    }
    println!(
        "90 played ticks posed with {} draw lists built; Mario ended {}",
        drawer.builds(),
        rustario64::play::action_name(session.mario().action)
    );
    assert!(drawer.issues.is_empty());
}
