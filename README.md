# Rustario64

A new Rust engine that imports Super Mario 64 content from a user-supplied ROM.
Bob-omb Battlefield is the first playable **target**; today it is an imported,
viewable level where Mario, with his model and animations from your ROM, can be
moved around by the compared simulation; it is not yet a playable course. Read
[PROJECT_PLAN.md](PROJECT_PLAN.md) for scope, milestones, and the live status.

What works now:

- ROM identification (US v1.0 only) and a bounded import of Bob-omb Battlefield:
  collision, script and macro placements, warps, and the **visible terrain and
  textures**, decoded from the original geo layouts and Fast3D display lists.
- An optional wgpu viewer that renders the imported level offscreen to PNG or in a
  window with a free inspection camera, with collision and placement overlays.
- The original static collision loader and floor/ceiling/wall/water queries, the
  trig/approach math utilities (tables loaded from your ROM), and Mario's physics
  steps from `mario_step.c` (ground, air, and stationary steps, ledge grabs,
  gravity, wind), ported to Rust and verified bit for bit against the pinned
  decompilation's C.
- Original controller normalization and Mario's pre-action input update: button
  edges, intended magnitude/yaw, geometry flags, floor fallback, and input timers.
- Mario's animation table imported from your ROM, and Mario's complete update
  ported from `mario.c` and the action files: level entry (`init_mario`), the
  stationary, moving, airborne, punching and hanging/ledge actions, health, caps,
  special floors, and the per-frame update of Mario's object including the
  animation frame advance. **Complete ticks match the decomp bit for bit**:
  28,158 ticks on an authored playground (CI) and 64,158 on Bob-omb Battlefield
  with your ROM's collision, tables and animations, at several presentation rates.
- **Mario mode in the viewer**: the keyboard drives that same tick at 30 Hz with
  a simple follow camera supplying the camera yaw, and every run can be recorded
  and replayed exactly against the decomp.
- **Mario's model from your ROM**: his geo layout and display lists, posed each
  tick from the animation the tick advanced and his body state (eyes, hands,
  cap, torso tilt, level of detail) as the original render pass does, skinned
  on the CPU and interpolated between ticks at any frame rate. Six switch
  configurations match triangle digests rebuilt from the pinned decomp source.
- Reference-camera helpers, camera collision/geometry, radial goals, and the
  persistent Lakitu/transition stage with exact native comparisons on authored
  fixtures and BOB's ROM data. Full mode dispatch is pending; the viewer keeps
  its labeled follow camera.
- A fixed 30 Hz scheduler and exact trace comparison, with exportable native-C/Rust
  **full-tick** and input-stage trace pairs.

This is level exploration with Mario's movement, not mission support: the
follow camera is not the original camera, Mario has no shadow yet, and there
are no objects (coins, enemies, trees, the cannon lid), cutscene or water
actions, warps, deaths, or missions. Play stops
where the port stops (unsupported paths, falling off the course); R re-enters.
The comparisons are against the natively compiled decomp, not N64 execution.
The imported level is independently validated against the pinned
decompilation; see [docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md).

## Layout

| Crate | Path | Purpose |
| --- | --- | --- |
| `rustario64` | `.` | GPU-free core: import, content, simulation, the play session that drives it from held controls, traces, headless CLI |
| `rustario64-render` | `render/` | Optional wgpu renderer and the `rustario64-viewer` development binary |
| `rustario64-oracle` | `oracle/` | Development-only: the pinned CC0 decomp's collision, math and Mario code compiled natively for bitwise component and full-tick differential tests (needs a C compiler); never a runtime dependency |

The core never depends on the renderer, so simulation and replay comparisons run
without a GPU or window.

## Build and test

Rust 1.99.0 (current stable as of 2026-10-08) with rustfmt and Clippy is selected
by `rust-toolchain.toml`. Linux x86_64 is the locally checked platform. CI also builds/tests the Rust
runtime on Windows x86_64. Both targets passed build/test/package CI at
`be65225`; consult each new run before treating that revision as verified. Dependencies use
compatible SemVer requirements; committed Cargo.lock and `--locked` make builds
reproducible. Upgrade the toolchain and lockfile together and rerun the component
oracles; compiler age is not a requirement for original gameplay behavior.

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
cargo fmt --all --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --release
cargo run --locked -- demo
```

Oracle tests compile vendored decomp C with the system C compiler and compare it
with the Rust collision, math, Mario step, input and full-tick ports on authored
data; see [oracle/README.md](oracle/README.md).
Render tests draw small authored models offscreen. Without a GPU adapter they
skip; set `RUSTARIO64_REQUIRE_GPU=1` to make a missing adapter fail (CI does this
with Mesa's software Vulkan, `mesa-vulkan-drivers`). The viewer needs a Vulkan,
Metal, DX12, or GL driver; on Linux the windowed mode also needs X11 or Wayland
libraries (for example `libxkbcommon-x11-0` on X11).

`demo` decodes an independently authored MIO0/BOB-shaped fixture and runs a
ten-second synthetic counter/input replay at 30, 60, 120, and 144 Hz presentation
schedules with identical tick records. This tests scaffolding, not Mario.

### Desktop test bundles

The `Desktop runtime` jobs build the two Rust runtime binaries on Linux and
Windows and upload `rustario64-linux-x86_64` / `rustario64-windows-x86_64` ZIPs
as workflow artifacts. The C oracle, ROMs, caches and extracted content never
ship. These are early terminal-launched test builds; a local ROM-selection GUI
and in-game pause/settings menu are explicit early priorities and remain pending.
See [docs/PLAYTEST.md](docs/PLAYTEST.md) for commands, controls and missing features.

Local runtime-only packaging (use `windows-x86_64` on Windows):

```sh
cargo build --locked --release -p rustario64 -p rustario64-render --bins
python tools/package_desktop.py --target linux-x86_64
python tools/test_package_desktop.py
```

The packager includes the commit/target/compiler in `BUILD_INFO.txt` and notices
from the target-filtered Cargo dependency graph. It refuses an existing ZIP;
use a fresh `--output` directory for another build.

## Use your ROM locally

Only the original 8 MiB US v1.0 revision is accepted, identified by normalized
SHA-1 `9bef1128717f958171a4afac3ed78ee2bb4e86ce`. Z64, V64, and N64 byte orders
are normalized first. Other regions, hacks, padding, truncation, and edited
headers are rejected. Keep ROMs and everything derived from them private; the
`private/` directory is git-ignored.

```sh
cargo run --locked -- inspect-rom /path/to/sm64.z64
cargo run --locked -- import-bob /path/to/sm64.z64 --out private/imports
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --test import local_us_rom_import -- --ignored --exact
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test collision bob_collision -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked -p rustario64-oracle --test math rom_trig -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_step bob_steps -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_input bob_input -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --test animation local_us_rom_mario_animations -- --ignored --exact
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release --test mario_model -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_tick bob_ticks -- --ignored --nocapture
```

The oracle tests compare BOB's real collision (about four million queries), the
ROM's trig tables (about four million lookups), Mario's physics steps on BOB
(1.18 million step calls), and 64,158 complete Mario ticks on BOB with the ROM's
animations between the Rust port and the decomp C.

### View Bob-omb Battlefield

```sh
# Offscreen PNG (works headless): views start, overview, summit, top
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  screenshot /path/to/sm64.z64 --out private/bob.png --view start --size 1280x960
# Window: WASD move, Q/E down/up, Shift faster, hold right mouse or arrows to look,
# 1-4 presets, Esc quits
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  view /path/to/sm64.z64
```

Graphics-only flags: `--msaa 4`, `--no-fog`, `--no-cull`, `--size WxH`,
`--no-interpolation`; inspection overlays: `--collision` (floors blue, walls
red, ceilings yellow) and `--placements` (Mario start, script objects, macro
objects, specials). In the window, C, P, and F toggle collision, placements, and
fog. The viewer launches straight into BOB area 1 as a development entry point.
The free camera is a presentation-only inspection camera, not the original game
camera. The sky is a placeholder color until the skybox is imported; trees,
coins, enemies, and other objects are not drawn yet (their painted ground
shadows are terrain).

#### Move Mario

```sh
# Window: Mario mode from the start, recording each run's inputs privately
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  view /path/to/sm64.z64 --mario --record private/runs
# Offscreen: Mario after 90 ticks of holding the stick up, from the follow camera
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  screenshot /path/to/sm64.z64 --out private/mario.png --mario-ticks 90
```

Mario mode (`--mario`, or M in the window): WASD is the stick (hold Shift to
walk), Space is A (jump), J is B (punch, dive), K is Z (crouch, ground pound),
the Left/Right arrows turn the camera, R re-enters the level, and M returns to
the free camera (Mario pauses). Mario starts at the level script's start and
runs the tick that is compared with the decomp, at the original 30 Hz; the
window interpolates his pose between ticks. The follow camera turns only when
asked; its yaw is the camera input Mario's controls are relative to (stick up
moves away from the camera), as the original camera's is. A key tapped between
two ticks counts as held for the next tick. Play stops on paths the port does
not support and on warps (falling off the course); the window title and terminal
say why, and R re-enters. `--record DIR` writes `run-NNN.inputs.json` for each
run (see Trace comparison to replay one against the decomp).

Mario is drawn with his model from the ROM, posed from each completed tick (he
appears from his first tick on, as in the original, whose first frame renders
after his first update). The original's levels of detail apply: moving Mario
switches to the medium and low-detail bodies with his distance from the camera.
If his model cannot be imported, a red placeholder box is drawn instead. With
a clean pinned decomp checkout, `python3 -I tools/check_mario_model_reference.py
--rom /path/to/sm64.z64 --reference /path/to/sm64` re-derives the geo
callbacks' addresses and the configuration digests that the ROM test pins.

### Import export

`import-bob` writes `private/imports/<normalized-sha1>-schema4/bob/`:

| File | Contents |
| --- | --- |
| `manifest.json` | Identity, dependent segments, visible-geometry counts, partial-import status, and every unsupported-content diagnostic |
| `level.json` | Course/area IDs, terrain type, dialog slots, music words, act masks, script and macro placements, warps, Mario start, segment loads, model references |
| `collision.json`, `collision.obj` | Original integer collision vertices, ordered surfaces/force words, specials, environment regions |
| `visual.json` | Area visual model: batches with decoded materials and vertices, background and camera nodes |
| `models.json` | Geo models from `LOAD_MODEL_FROM_GEO` that resolve in loaded segments |
| `visual-NN-<address>-WxH.rgba` | Decoded RGBA8 texture data for the visual model |
| `terrain-N-32x32.rgba`, `terrain-N.ppm` | The five segment-7 RGBA16 textures, with alpha-free previews |

The OBJ shows **collision**, not visible terrain. Exports are staged and published
after every file has been written; a write failure cleans up staging so a retry
can succeed. Concurrent exporters share an OS file lock, and existing exports
are rejected. Remove an old export or choose another output root to reimport.
An interrupted process may leave a private `.rustario64-import-*` staging
directory; it is safe to remove once no import is running. Schema 4 retains area
terrain/dialog/music metadata; older exports are not reused. Hash/schema
keys are ready for caching, but cache reuse is not implemented. No ROM code is
executed: native callbacks referenced by scripts and geo layouts are reported.

## Trace comparison

```sh
cargo run --locked -- demo --trace private/foundation.trace.json
cargo run --locked -- compare-traces private/foundation.trace.json private/foundation.trace.json
```

Export a native-vs-Rust **full-tick** trace pair with your ROM (60 seconds of
scripted moves from BOB's script start; Rust runs at 144 Hz presentation), or
replay a run recorded in the viewer with `--inputs`:

```sh
cargo test --locked -p rustario64-oracle --test mario_tick
cargo run --locked --release -p rustario64-oracle --example tick_trace -- \
  /path/to/sm64.z64 private/tick-traces
cargo run --locked -- compare-traces \
  private/tick-traces/native-tick.trace.json private/tick-traces/rust-tick.trace.json
cargo run --locked --release -p rustario64-oracle --example tick_trace -- \
  /path/to/sm64.z64 private/replay-1 --inputs private/runs/run-001.inputs.json
```

Each frame records about 260 named words: all of MarioState, Mario's object, his
body and camera-status state, world globals, controller 1 and the frame's sound,
camera and warp events. The entry is a fresh-boot level entry at the script's
start, not a painting spawn. Mario's object is the only object, and the camera
yaw is part of each tick's input.

Export a native-vs-Rust **input-stage** trace pair with your ROM:

```sh
cargo test --locked -p rustario64-oracle --test mario_input
cargo run --locked --release -p rustario64-oracle --example input_trace -- \
  /path/to/sm64.z64 private/input-traces
cargo run --locked -- compare-traces \
  private/input-traces/native-input.trace.json private/input-traces/rust-input.trace.json
```

Use a new output directory. Both files are private ROM-derived traces. The
40-second fixture uses BOB's imported script position/yaw, original collision,
and ROM trig tables. It does **not** run `init_mario`, a spawn warp, actions,
reference camera logic, objects, or animation. Missing-floor death-warp requests
are recorded, not executed. Mario does not run or jump. Scope metadata,
controller history, a terrain/table digest, input fields and exact float bits
make these input-stage comparisons reviewable.

A mismatch fails with the first tick and field. Authoritative floats compare as
`f32::to_bits()` integers with no tolerance. See [docs/FIDELITY.md](docs/FIDELITY.md).

## Further reading

[docs/DECISIONS.md](docs/DECISIONS.md) records module, timing, import, and
rendering decisions; [PROVENANCE.md](PROVENANCE.md) lists exact upstream sources,
reuse, and dependency terms; [docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md)
holds the owner-ROM evidence.

## Next increment

Connect the compared reference-camera helpers and persistent Lakitu/transition
stage to full BOB radial and free-roam mode controllers, with per-tick comparisons
so camera yaw and mode stop being inputs (the source's named BOB trigger table
is unused); draw Mario's shadow; then begin objects for the first
mission. Original-execution traces remain the eventual authority. Skybox and
placement models remain M1 work.

Camera checks without a ROM: `cargo test --locked -p rustario64-oracle --test camera --test lakitu`;
repeat with `--release` for optimized comparisons. The owner-ROM camera test
is ignored in ordinary CI; see [docs/FIDELITY.md](docs/FIDELITY.md).

With your supported ROM, run the whole integration suite locally:

```sh
RUSTARIO64_ROM=/path/to/sm64.z64 RUSTARIO64_REQUIRE_GPU=1 \
  cargo test --locked --release --workspace --all-targets -- --include-ignored
```
