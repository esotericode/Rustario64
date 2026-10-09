//! Owner-ROM native/Rust comparison of the pre-action input stage only.
use rustario64::{
    import::{bob, collision, engine, mio0, rom::Rom, version},
    presentation::GraphicsOptions,
    simulation::{
        TickInput,
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, Controller, Z_TRIG},
        mario::{MarioState, constants as c},
    },
    trace,
};
use rustario64_oracle::{
    Oracle,
    input_trace::{InputContext, Replay, world_digest},
};
use std::{error::Error, fs, io::Write, path::Path};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: cargo run -p rustario64-oracle --example input_trace -- ROM NEW_PRIVATE_OUTPUT_DIR".into());
    }
    let rom = Rom::open(Path::new(&args[0]))?;
    let imported = bob::import(&rom)?;
    let trig = engine::trig_tables(&rom)?;
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        rom.reader().slice(range.start, range.len())?,
        version::MAX_SEGMENT_BYTES,
    )?;
    let start = (version::BOB_COLLISION & 0xffffff) as usize;
    let (mesh, consumed) = collision::decode(&terrain[start..])?;
    if mesh != imported.collision {
        return Err("raw and normal import collision differ".into());
    }
    let stream: Vec<i16> = terrain[start..start + consumed]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    let world = CollisionWorld::load_area_terrain(&mesh)?;
    let oracle = Oracle::load(&stream);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    let (area, yaw_degrees, position) = imported
        .level
        .mario_start
        .ok_or("missing Mario script start")?;
    let terrain_type = imported
        .level
        .areas
        .iter()
        .find(|a| a.id == area)
        .ok_or("missing start area")?
        .terrain_type;
    let replay = Replay {
        collision: &world,
        trig: &trig,
        initial: {
            let mut m = MarioState {
                action: c::ACT_IDLE,
                pos: position.map(f32::from),
                // level_cmd_set_mario_start_pos converts raw script degrees.
                face_angle: [0, (i32::from(yaw_degrees) * 0x8000 / 180) as i16, 0],
                frames_since_a: 255,
                frames_since_b: 255,
                ..Default::default()
            };
            m.obj.gfx.pos = position.map(f32::from);
            m
        },
        controller: Controller::default(),
        context: InputContext::default(),
        area_terrain_type: terrain_type,
        level_num: c::LEVEL_BOB,
        rom_sha1: rom.fingerprint().into(),
        world_digest: world_digest(&stream, &trig),
        scenario: "bob-script-start-input-stage-40s-no-spawn-or-actions".into(),
    };
    let samples: Vec<TickInput> = (0..1200)
        .map(|i| TickInput {
            buttons: match i % 300 {
                0..=19 => A_BUTTON | B_BUTTON | Z_TRIG,
                20..=22 => 0,
                23..=45 => A_BUTTON,
                _ => 0,
            },
            stick: if i % 137 < 12 {
                [0, 0]
            } else {
                [(i * 17) as i8, (i * 37) as i8]
            },
            camera_yaw: (i * 419) as i16,
        })
        .collect();
    let reference = replay.run(&samples, 30, GraphicsOptions::default(), Some(&oracle))?;
    let candidate = replay.run(&samples, 144, GraphicsOptions::default(), None)?;
    trace::compare(&reference, &candidate)?;
    let out = Path::new(&args[1]);
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(out)?; // refuse existing directories, including symlinks
    for (name, trace) in [
        ("native-input.trace.json", &reference),
        ("rust-input.trace.json", &candidate),
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(name))?;
        file.write_all(&serde_json::to_vec_pretty(trace)?)?;
    }
    println!(
        "1,200 BOB input-stage ticks compare exactly (native 30 Hz / Rust 144 Hz presentation)."
    );
    println!(
        "Wrote {}. Private ROM-derived traces; keep them out of Git.",
        out.display()
    );
    println!(
        "Input-stage component only: no spawn initializer, actions, camera update, objects, animation, or warp execution. Mario is not playable."
    );
    Ok(())
}
