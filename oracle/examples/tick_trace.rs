//! Owner-ROM native/Rust comparison of complete Mario ticks on BOB: the level
//! script's start, BOB's collision, the ROM's trig tables and Mario's
//! animations. The inputs are a built-in 60-second program or a viewer
//! recording (`--inputs`). Writes both traces to a new private directory.
use rustario64::{
    import::{animation, bob, collision, engine, mio0, rom::Rom, version},
    play::BOB_SCRIPT_START,
    presentation::GraphicsOptions,
    simulation::{
        TickInput,
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, Z_TRIG},
        mario::tick::LevelEntry,
    },
    trace::{self, InputLog},
};
use rustario64_oracle::{Oracle, TickSetup, input_trace::world_digest, tick_trace::TickScenario};
use std::{error::Error, fs, io::Write, path::Path};

const USAGE: &str = "usage: cargo run -p rustario64-oracle --example tick_trace -- ROM NEW_PRIVATE_OUTPUT_DIR [--inputs RUN.inputs.json]";

/// Sixty seconds cycling through idle, walking, jump chains, punches,
/// crouch moves, a turning stick and slide kicks under a rotating camera yaw.
fn program() -> Vec<TickInput> {
    (0..1800i32)
        .map(|i| {
            let phase = (i / 150) % 8;
            let up = [0, 80];
            let circle = {
                let angle = f64::from(i) * 0.09;
                [(angle.cos() * 90.0) as i8, (angle.sin() * 90.0) as i8]
            };
            let (buttons, stick) = match phase {
                0 => (0, [0, 0]),
                1 => (0, up),
                2 => (if i % 20 == 0 { A_BUTTON } else { 0 }, up),
                3 => (if i % 25 == 0 { B_BUTTON } else { 0 }, [0, 0]),
                4 => (Z_TRIG | if i % 30 == 0 { A_BUTTON } else { 0 }, up),
                5 => (0, circle),
                6 => (if i % 2 == 0 { A_BUTTON } else { 0 }, up),
                _ => (
                    match i % 40 {
                        0 => Z_TRIG,
                        1 => Z_TRIG | B_BUTTON,
                        _ => 0,
                    },
                    up,
                ),
            };
            TickInput {
                buttons,
                stick,
                camera_yaw: (i * 97) as i16,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let recording = match args.as_slice() {
        [_, _] => None,
        [_, _, flag, log] if flag == "--inputs" => Some(InputLog::from_json(&fs::read(log)?)?),
        _ => return Err(USAGE.into()),
    };
    let rom = Rom::open(Path::new(&args[0]))?;
    let imported = bob::import(&rom)?;
    let trig = engine::trig_tables(&rom)?;
    let anims = animation::mario_animations(&rom)?;
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
    oracle.set_mario_animations(&anims);
    // The viewer's entry: BOB's script start, with the area camera's mode
    // from its GEO_CAMERA node (radial).
    let camera_mode = imported
        .visual
        .as_ref()
        .and_then(|v| v.camera)
        .ok_or("BOB's area has no camera node")?
        .mode;
    let entry = LevelEntry::script_start(&imported.level, camera_mode)?;
    let (inputs, name) = match &recording {
        Some(log) => {
            if log.rom_sha1 != rom.fingerprint() {
                return Err(format!("the recording is from ROM {}", log.rom_sha1).into());
            }
            if log.entry != BOB_SCRIPT_START {
                return Err(format!(
                    "the recording starts from {}, not {BOB_SCRIPT_START}",
                    log.entry
                )
                .into());
            }
            (log.inputs.clone(), "bob-script-start-recording")
        }
        None => (program(), "bob-script-start-full-tick-60s"),
    };
    let scenario = TickScenario {
        collision: &world,
        trig: &trig,
        anims: &anims,
        setup: TickSetup::from_entry(&entry),
        rom_sha1: rom.fingerprint().into(),
        world_digest: world_digest(&stream, &trig),
        scenario: name.into(),
        course: 1,
    };
    // A path the port does not support ends the comparison, as it ends play
    // in the viewer; the ticks before it are compared.
    let (completed, unsupported) = scenario.rust_until_unsupported(&inputs);
    let inputs = &inputs[..completed.len()];
    if inputs.is_empty() {
        return Err("no complete tick to compare".into());
    }
    let reference = scenario.native(&oracle, inputs);
    let candidate = scenario.rust(inputs, 144, GraphicsOptions::default())?;
    trace::compare(&reference, &candidate)?;
    let out = Path::new(&args[1]);
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(out)?; // refuse existing directories, including symlinks
    for (name, trace) in [
        ("native-tick.trace.json", &reference),
        ("rust-tick.trace.json", &candidate),
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(name))?;
        file.write_all(&serde_json::to_vec_pretty(trace)?)?;
    }
    let source = if recording.is_some() {
        "recorded"
    } else {
        "scripted"
    };
    println!(
        "{} {source} BOB Mario ticks compare exactly (native 30 Hz / Rust 144 Hz presentation).",
        inputs.len()
    );
    if let Some(message) = unsupported {
        println!(
            "The port stopped at tick {} on a path it does not support: {message}",
            inputs.len() + 1
        );
    }
    println!(
        "Wrote {}. Private ROM-derived traces; keep them out of Git.",
        out.display()
    );
    println!(
        "Mario's object only: no other objects, reference camera, warps or particles. Comparisons are against the natively compiled decomp, not N64 execution."
    );
    Ok(())
}
