use rustario64::{
    diagnostics,
    import::{bob, collision, rom::Rom, version},
    presentation::GraphicsOptions,
    trace::{self, Trace},
};
use serde::Serialize;
use std::{
    error::Error,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

type AppResult<T> = Result<T, Box<dyn Error>>;

const HELP: &str = "Rustario64 foundation (headless; no playable course yet)\n\n\
  cargo run --locked -- demo\n\
  cargo run --locked -- demo --trace private/foundation.trace.json\n\
  cargo run --locked -- inspect-rom /path/to/sm64.z64\n\
  cargo run --locked -- import-bob /path/to/sm64.z64 --out private/imports\n\
  cargo run --locked -- compare-traces reference.trace.json candidate.trace.json\n";

fn write_new(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn json_new<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    write_new(path, &serde_json::to_vec_pretty(value)?)
}

fn read_trace(path: &Path) -> AppResult<Trace> {
    const MAX: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX {
        return Err("trace exceeds 64 MiB limit".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn demo(trace_path: Option<&Path>) -> AppResult<()> {
    let fixture = diagnostics::parser_smoke()?;
    println!(
        "Independently authored import fixture: {} collision vertices, {} triangles, {} area(s), {} placements, {} RGBA16 textures",
        fixture.collision.vertices.len(),
        fixture.collision.triangles.len(),
        fixture.level.areas.len(),
        fixture
            .level
            .areas
            .iter()
            .map(|a| a.spawns.len())
            .sum::<usize>(),
        fixture.textures.len()
    );
    let baseline = diagnostics::counter_replay(30, GraphicsOptions::default())?;
    for cap in [30, 60, 120, 144] {
        for interpolation in [false, true] {
            let candidate = diagnostics::counter_replay(
                cap,
                GraphicsOptions {
                    interpolation,
                    enhanced_lighting: true,
                    dynamic_shadows: true,
                },
            )?;
            trace::compare(&baseline, &candidate)?;
        }
        println!(
            "{cap} Hz presentation schedule: 300 ticks, exact synthetic state/input comparison passed"
        );
    }
    if let Some(path) = trace_path {
        json_new(path, &baseline)?;
        println!("Wrote {}", path.display());
    }
    println!(
        "This validates parser/scheduler scaffolding, not Mario physics or original ROM integration."
    );
    Ok(())
}

fn import_bob(rom_path: &Path, output_root: &Path) -> AppResult<()> {
    let rom = Rom::open(rom_path)?;
    let imported = bob::import(&rom)?;
    let output = output_root
        .join(format!(
            "{}-schema{}",
            rom.fingerprint(),
            version::IMPORT_SCHEMA
        ))
        .join("bob");
    // Reserve the destination: exports never overwrite existing files.
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&output)?;
    write_new(
        &output.join(".gitignore"),
        b"# Private ROM-derived export\n*\n",
    )?;
    let manifest = serde_json::json!({
        "schema": version::IMPORT_SCHEMA, "rom_sha1": rom.fingerprint(),
        "input_byte_order": rom.input_order, "reference_revision": version::REFERENCE_REVISION,
        "course": 1, "level": 9, "terrain_segment_bytes": imported.terrain_bytes,
        "collision_vertices": imported.collision.vertices.len(), "collision_triangles": imported.collision.triangles.len(),
        "special_placements": imported.collision.specials.len(), "textures": imported.textures.len(),
        "macro_placements": imported.level.areas.iter().map(|a| a.macro_spawns.len()).sum::<usize>(),
        "import_status": "partial", "visible_terrain_decoded": false, "playable": false,
        "unsupported": &imported.level.issues,
    });
    json_new(&output.join("manifest.json"), &manifest)?;
    json_new(&output.join("level.json"), &imported.level)?;
    json_new(&output.join("collision.json"), &imported.collision)?;
    write_new(
        &output.join("collision.obj"),
        collision::to_obj(&imported.collision).as_bytes(),
    )?;
    for (i, texture) in imported.textures.iter().enumerate() {
        write_new(
            &output.join(format!("terrain-{i}-32x32.rgba")),
            &texture.rgba,
        )?;
        write_new(&output.join(format!("terrain-{i}.ppm")), &texture.to_ppm())?;
    }
    println!(
        "Imported BOB collision, script/macro placements, warps, and five segment-7 textures into {}",
        output.display()
    );
    println!(
        "{} coverage issues recorded in manifest.json. Visible terrain, Mario, and missions remain unimplemented.",
        imported.level.issues.len()
    );
    Ok(())
}

fn run(args: &[OsString]) -> AppResult<()> {
    match args {
        [] => print!("{HELP}"),
        [cmd] if cmd == "--help" || cmd == "-h" || cmd == "help" => print!("{HELP}"),
        [cmd] if cmd == "demo" => demo(None)?,
        [cmd, flag, path] if cmd == "demo" && flag == "--trace" => demo(Some(Path::new(path)))?,
        [cmd, path] if cmd == "inspect-rom" => {
            let rom = Rom::open(Path::new(path))?;
            println!(
                "Supported SM64 US v1.0; normalized SHA-1 {}; input {:?}; 30 simulation ticks/s",
                rom.fingerprint(),
                rom.input_order
            );
        }
        [cmd, path, flag, output] if cmd == "import-bob" && flag == "--out" => {
            import_bob(Path::new(path), Path::new(output))?
        }
        [cmd, reference, candidate] if cmd == "compare-traces" => {
            let reference = read_trace(Path::new(reference))?;
            let candidate = read_trace(Path::new(candidate))?;
            trace::compare(&reference, &candidate)?;
            println!(
                "Exact comparison passed for {} recorded ticks. ROM: {}. No broader fidelity claim.",
                reference.frames.len(),
                reference.metadata.rom_sha1
            );
        }
        _ => return Err(format!("invalid arguments\n\n{HELP}").into()),
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Err(error) = run(&args) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
