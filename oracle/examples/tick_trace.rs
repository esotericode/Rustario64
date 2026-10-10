//! Owner-ROM native/Rust comparison of complete frames on BOB: the level
//! script's start, BOB's collision, the ROM's trig tables and Mario's
//! animations. The inputs are a built-in 60-second program (Mario's object
//! with a recorded camera yaw, or with `--camera` the original camera too) or
//! a viewer recording (`--inputs`; recordings with the reference camera
//! replay with the camera linked). Writes both traces to a new private
//! directory.
use rustario64::{
    content::Act,
    import::{animation, bob, collision, engine, mio0, objects, rom::Rom, version},
    play::BOB_SCRIPT_START,
    presentation::GraphicsOptions,
    simulation::{
        TickInput,
        camera::{D_CBUTTONS, L_CBUTTONS, R_CBUTTONS, U_CBUTTONS},
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, R_TRIG, Z_TRIG},
        game::GameEntry,
    },
    trace::{self, CameraInput, InputLog, Trace},
};
use rustario64_oracle::{
    Oracle, TickSetup,
    camera_trace::{GameScenario, RustStop, game_trace},
    input_trace::world_digest,
    tick_trace::TickScenario,
};
use std::{error::Error, fs, io::Write, path::Path};

const USAGE: &str = "usage: cargo run -p rustario64-oracle --example tick_trace -- ROM NEW_PRIVATE_OUTPUT_DIR [--inputs RUN.inputs.json | --camera]";

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

/// The built-in program with the camera's buttons: C-Left/Right turns,
/// C-Down zooms, C-Up first person (left with A), and the R camera.
fn camera_program() -> Vec<TickInput> {
    program()
        .into_iter()
        .enumerate()
        .map(|(i, mut input)| {
            input.camera_yaw = 0;
            input.buttons |= match i % 300 {
                40 | 46 => L_CBUTTONS,
                90 => R_CBUTTONS,
                130 | 135 => D_CBUTTONS,
                160 => U_CBUTTONS,
                200 => U_CBUTTONS,
                250 => A_BUTTON,
                _ if (600..660).contains(&i) => R_TRIG,
                _ => 0,
            };
            input
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (recording, camera) = match args.as_slice() {
        [_, _] => (None, false),
        [_, _, flag] if flag == "--camera" => (None, true),
        [_, _, flag, log] if flag == "--inputs" => {
            let log = InputLog::from_json(&fs::read(log)?)?;
            let camera = log.camera == CameraInput::Reference;
            (Some(log), camera)
        }
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
    // The viewer's entry: BOB's script start, with the area's camera node.
    let node = imported
        .visual
        .as_ref()
        .and_then(|v| v.camera)
        .ok_or("BOB's area has no camera node")?;
    let game_entry = GameEntry::script_start(&imported.level, &node)?;
    let entry = game_entry.mario;
    // The ROM's scripts and models; with the camera, BOB's act-1 placements
    // as the viewer enters them.
    let content = objects::bob(&rom, &imported.level, Act::new(1).ok_or("act")?)?;
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
        None if camera => (camera_program(), "bob-script-start-camera-60s"),
        None => (program(), "bob-script-start-full-tick-60s"),
    };
    let out = Path::new(&args[1]);
    if camera {
        return compare_with_camera(
            &oracle,
            GameScenario {
                collision: &world,
                trig: &trig,
                anims: &anims,
                objects: content.level_objects(),
                entry: game_entry,
            },
            &inputs,
            recording.is_some(),
            name,
            rom.fingerprint(),
            &world_digest(&stream, &trig),
            out,
        );
    }
    let scenario = TickScenario {
        collision: &world,
        trig: &trig,
        anims: &anims,
        objects: content.mario_only(),
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
    write_traces(out, &reference, &candidate)?;
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

fn write_traces(out: &Path, native: &Trace, rust: &Trace) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(out)?; // refuse existing directories, including symlinks
    for (name, trace) in [
        ("native-tick.trace.json", native),
        ("rust-tick.trace.json", rust),
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(name))?;
        file.write_all(&serde_json::to_vec_pretty(trace)?)?;
    }
    Ok(())
}

/// Frames with the original camera linked on both sides. A recording's
/// camera yaws must be the ones the replay's Mario reads.
#[allow(clippy::too_many_arguments)]
fn compare_with_camera(
    oracle: &Oracle,
    scenario: GameScenario<'_>,
    inputs: &[TickInput],
    recorded: bool,
    name: &str,
    rom_sha1: &str,
    digest: &str,
    out: &Path,
) -> Result<(), Box<dyn Error>> {
    let (rust, stop) = scenario.rust(inputs);
    let frames = rust.len() - 1;
    // A panicking frame has no words; a camera stop's frame completed.
    let inputs = &inputs[..frames];
    if inputs.is_empty() {
        return Err("no complete frame to compare".into());
    }
    if recorded {
        for (i, input) in inputs.iter().enumerate() {
            let read = rust[i]["world.camera.yaw"] as i32 as i16;
            if read != input.camera_yaw {
                return Err(format!(
                    "frame {}: the recording's camera yaw {} differs from the replay's {read}",
                    i + 1,
                    input.camera_yaw
                )
                .into());
            }
        }
    }
    let native = scenario.native(oracle, inputs);
    let native = game_trace(
        "native-decomp frame with camera",
        rom_sha1,
        digest,
        name,
        &scenario.entry,
        inputs,
        &native,
    );
    let candidate = game_trace(
        "Rust frame with camera",
        rom_sha1,
        digest,
        name,
        &scenario.entry,
        inputs,
        &rust,
    );
    trace::compare(&native, &candidate)?;
    write_traces(out, &native, &candidate)?;
    let source = if recorded { "recorded" } else { "scripted" };
    println!(
        "{frames} {source} BOB frames with the original camera compare exactly (Mario and every camera word)."
    );
    match stop {
        Some(RustStop::Panic(message)) => println!(
            "The port stopped at frame {} on a path it does not support: {message}",
            frames + 1
        ),
        Some(RustStop::Camera(message)) => {
            println!("The camera reached a path it does not support at frame {frames}: {message}")
        }
        None => {}
    }
    println!(
        "Wrote {}. Private ROM-derived traces; keep them out of Git.",
        out.display()
    );
    println!(
        "Mario's object and the area camera only: no other objects, cutscenes, warps or particles. Comparisons are against the natively compiled decomp, not N64 execution."
    );
    Ok(())
}
