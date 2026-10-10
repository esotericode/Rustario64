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
- **The original camera on Bob-omb Battlefield**: `camera.c`'s frame for areas
  without camera triggers, ported to Rust: Lakitu's radial camera with C-button
  turns and zoom, the R-button Mario camera, C-Up first person, the boss-fight
  camera, BOB's surface rules, shakes (with the original random generator), the
  field of view and the graph camera. Mario and the camera run in the original
  frame order; **complete frames match the decomp word for word** (19,512
  authored frames in CI, 77,112 on Bob-omb Battlefield with your ROM).
- **Mario mode in the viewer**: keyboard or a mapped controller drives that same frame at 30 Hz,
  the window draws through the original camera (interpolated between frames),
  and every run can be recorded and replayed exactly against the decomp.
- **Mario's model from your ROM**: his geo layout and display lists, posed each
  tick from the animation the tick advanced and his body state (eyes, hands,
  cap, torso tilt, level of detail) as the original render pass does, skinned
  on the CPU and interpolated between ticks at any frame rate. Six switch
  configurations match triangle digests rebuilt from the pinned decomp source.
- A local ROM-selection launcher, remembered-path opt-in, and Esc pause/settings
  (interpolation, fog, VSync and fullscreen), with paused and unfocused time discarded.
- **Mario's original blob shadow**: the ROM's quarter-circle texture, nine
  vertices, slope/ledge clipping, height-based size and opacity, ledge-animation
  fades, water/ice layers and lateral animation offsets. Shadow presentation
  interpolates with Mario while simulation stays at 30 Hz. Original vertex
  coordinates, alpha and layers match the native decomp on authored cases and BOB.
- **The original object system with BOB's coins**: the 240-slot object pool
  and its lists, `update_objects` in the original list order, the ROM's own
  behavior scripts (segment 0x13) run by a port of the behavior interpreter,
  object collision, and the coin behaviors: yellow coins, every coin formation
  type (lines, rings, the arrow, flying variants) with their respawn bits,
  sparkles, and coin collection. **Complete frames with objects match the decomp
  word for word**, including every object's fields, the lists and the free list
  (5,400 authored frames in CI, 9,600 on BOB's coins with your ROM). In the
  viewer, Mario sees and collects BOB's act-1 coins with their ROM models,
  animated sparkles and a coin counter.
- A fixed 30 Hz scheduler and exact trace comparison, with exportable native-C/Rust
  **full-tick** and input-stage trace pairs.
- The original enemy `object_step` physics, ready for actor integration:
  wall reflection, slopes/friction, bouncing, water motion and terrain alignment.
  63,174 authored and 123,200 local BOB component calls match native C bit for bit.
  This does not yet spawn Bob-ombs or run their behaviors.

This is level exploration with Mario's movement and camera, not mission
support: of the objects only the coins are simulated. Enemies, trees, signs,
red coins, the cannon lid and every other placement are recorded as unported
and not spawned. There are no camera cutscenes, original pause behavior, cutscene or water
actions, warps, deaths, or missions. Play stops where the port stops
(unsupported paths, falling off the course); R re-enters.
The comparisons are against the natively compiled decomp, not N64 execution.
The imported level is independently validated against the pinned
decompilation; see [docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md).

## Layout

| Crate | Path | Purpose |
| --- | --- | --- |
| `rustario64` | `.` | GPU-free core: import, content, simulation (Mario, the reference camera, the game frame), the play session that drives it from held controls, traces, headless CLI |
| `rustario64-render` | `render/` | Optional wgpu renderer, controller input, `rustario64-viewer` diagnostics and `rustario64-desktop` launcher |
| `rustario64-oracle` | `oracle/` | Development-only: the pinned CC0 decomp's collision, math, Mario, camera and object code compiled natively for bitwise component and full-tick differential tests (needs a C compiler); never a runtime dependency |

The core never depends on the renderer, so simulation and replay comparisons run
without a GPU or window.

## Build and test

Rust 1.99.0 (current stable as of 2026-10-08) with rustfmt and Clippy is selected
by `rust-toolchain.toml`. Linux x86_64 is the locally checked platform. CI also builds/tests the Rust
runtime on Windows x86_64. Both targets passed build/test/package CI at
`e0365fd`; consult each new run before treating that revision as verified. Dependencies use
compatible SemVer requirements; committed Cargo.lock and `--locked` make builds
reproducible. Upgrade the toolchain and lockfile together and rerun the component
oracles; compiler age is not a requirement for original gameplay behavior.

```sh
# Linux desktop builds need pkg-config and libudev development files:
# sudo apt-get install pkg-config libudev-dev
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

The `Desktop runtime` jobs build the three Rust runtime binaries on Linux and
Windows and upload `rustario64-linux-x86_64` / `rustario64-windows-x86_64` ZIPs
as workflow artifacts. The C oracle, ROMs, caches and extracted content never
ship. Open `rustario64-desktop.exe` on Windows for the ROM-selection window
without a console. Open `rustario64-desktop` on Linux (file-manager execution
depends on the desktop). Browse, type a path or drop your ROM, then select Play.
Pause/settings includes controller selection and Choose another ROM.
`rustario64-viewer` without arguments also opens the launcher; its console
entry points remain available for diagnostics and recording.
See [docs/PLAYTEST.md](docs/PLAYTEST.md) for commands, controls and missing features.

Settings live in `%APPDATA%/rustario64/settings.json` on Windows and
`$XDG_CONFIG_HOME/rustario64/settings.json` (or `~/.config/rustario64/settings.json`)
on Linux, outside imported content. A ROM path is saved only when the launcher’s
remember checkbox is selected and import succeeds. A stale path or invalid settings
file leaves a usable launcher with an error. Linux’s native Browse dialog needs a
desktop file portal; entering a path or dropping a file also works.

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
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release --test objects -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test objects -- --include-ignored --nocapture
```

With a clean pinned decomp checkout, two checkers re-derive the object
content the importer relies on: `python3 -I tools/check_behavior_reference.py
--rom /path/to/sm64.z64 --reference /path/to/sm64` lays the pinned
`behavior_data.c` over the ROM's segment 0x13 and checks every word, the
macro preset table and the version adapter's script and native addresses;
`tools/check_object_model_reference.py` (same arguments) checks the coin and
sparkle geo layouts and the object geo callback address.

The oracle tests compare BOB's real collision (about four million queries), the
ROM's trig tables (about four million lookups), Mario's physics steps on BOB
(1.18 million step calls), and 64,158 complete Mario ticks on BOB with the ROM's
animations between the Rust port and the decomp C.

### View Bob-omb Battlefield

```sh
# Local ROM launcher (no terminal arguments required in desktop bundles)
cargo run --locked --release -p rustario64-render --bin rustario64-viewer
# Offscreen PNG (works headless): views start, overview, summit, top
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  screenshot /path/to/sm64.z64 --out private/bob.png --view start --size 1280x960
# Window: WASD move, Q/E down/up, Shift faster, hold right mouse or arrows to look,
# 1-4 presets, Esc pauses/settings
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  view /path/to/sm64.z64
```

Graphics-only flags: `--msaa 4`, `--no-fog`, `--no-cull`, `--size WxH`,
`--no-interpolation`, `--fullscreen`, `--no-vsync`; inspection overlays: `--collision` (floors blue, walls
red, ceilings yellow) and `--placements` (Mario start, script objects, macro
objects, specials). In the window, C, P, and F toggle collision, placements, and
fog. The viewer launches straight into BOB area 1 as a development entry point.
The free camera is a presentation-only inspection camera, not the original game
camera. The sky is a placeholder color until the skybox is imported; trees,
enemies, and other unported objects are not drawn yet (their painted ground
shadows are terrain). In Mario mode BOB's act-1 coins and collection sparkles
are drawn from their ROM models, facing the displayed camera. Coin positions
interpolate between completed ticks; the original texture-frame switches stay
at 30 Hz. The window shows the original HUD coin value in a development text
overlay. Coin shadows and original HUD glyphs remain pending. Offscreen
`--mario-ticks` images include coins/sparkles; the text overlay is window-only.

#### Move Mario

Basic controllers: left stick moves with a 10% circular dead zone, A/Cross jumps,
X/Square (left face button) attacks, LT/LB/RT crouches, RB changes camera,
right stick or D-pad supplies C buttons, and Start pauses/resumes. Choose a
controller in the launcher or pause menu.
Disconnecting it releases its inputs and play continues. Center sticks and
release buttons after a pause, focus change or restart before controller input
resumes. Keyboard remains usable.
Remapping, calibration, rumble and menu navigation are future work; see the
mapping and hardware checklist in [docs/PLAYTEST.md](docs/PLAYTEST.md).

```sh
# Window: Mario mode from the start, recording each run's inputs privately
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  view /path/to/sm64.z64 --mario --record private/runs
# Offscreen: Mario after 90 frames of holding the stick up, from the original camera
cargo run --locked --release -p rustario64-render --bin rustario64-viewer -- \
  screenshot /path/to/sm64.z64 --out private/mario.png --mario-ticks 90
```

Mario mode (`--mario`, or M in the window): WASD is the stick (hold Shift to
walk), Space is A (jump), J is B (punch, dive), K is Z (crouch, ground pound),
the arrow keys are the C buttons (Left/Right rotate Lakitu, Down zooms out and
Up back in, then Up enters first person; A, B or another C button leaves it), E
is the R button (Lakitu or Mario camera), R re-enters the level, and M returns
to the free camera (Mario pauses). Esc opens the pause/settings menu; Esc again
resumes, and the menu has Restart and Quit. Losing focus also pauses and releases
held inputs; resume explicitly after returning. Menu/unfocused time and pending
catch-up ticks are discarded. This is a development pause: it freezes the entire
frame, rather than implementing the original pause-camera adjustment. Mario starts at the level script's start; he
and the original camera run the frame that is compared with the decomp, at the
original 30 Hz. The window draws from the camera's position, focus, roll and
field of view, interpolated between frames and snapped across cuts; Mario's
controls are relative to the yaw the camera produced the frame before, as in
the original. A key tapped between two frames counts as held for the next
frame. Play stops on paths the port does not support and on warps (falling off
the course); the window title and terminal say why, and R re-enters. `--record
DIR` writes `run-NNN.inputs.json` for each run (see Trace comparison to replay
one against the decomp).

Mario is drawn with his model from the ROM, posed from each completed tick (he
appears from his first tick on, as in the original, whose first frame renders
after his first update). The original's levels of detail apply: moving Mario
switches to the medium and low-detail bodies with his distance from the camera.
Blinks, material switches and detail changes keep interpolating the skeleton using
the newly selected mesh at both endpoints. Jumping, landing, turning, stopping
and other animation changes also interpolate between completed poses. Level
re-entry and pause/resume clear pose history. Interpolation presents completed
frames about one 30 Hz tick behind real time;
it does not change input processing or simulation cadence.
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
scripted moves from BOB's script start; Rust runs at 144 Hz presentation), the
same with the original camera linked on both sides (`--camera`, adding the
camera buttons), or replay a run recorded in the viewer with `--inputs` (runs
recorded with the reference camera replay with the camera linked):

```sh
cargo test --locked -p rustario64-oracle --test mario_tick
cargo run --locked --release -p rustario64-oracle --example tick_trace -- \
  /path/to/sm64.z64 private/tick-traces
cargo run --locked -- compare-traces \
  private/tick-traces/native-tick.trace.json private/tick-traces/rust-tick.trace.json
cargo run --locked --release -p rustario64-oracle --example tick_trace -- \
  /path/to/sm64.z64 private/camera-traces --camera
cargo run --locked --release -p rustario64-oracle --example tick_trace -- \
  /path/to/sm64.z64 private/replay-1 --inputs private/runs/run-001.inputs.json
```

Each frame records about 260 named words: all of MarioState, Mario's object, his
body and camera-status state, world globals, controller 1 and the frame's sound,
camera and warp events. The entry is a fresh-boot level entry at the script's
start, not a painting spawn. Without the camera, Mario's object is the only
object and the yaw is part of each tick's input; with it, BOB's act-1 coins
are spawned on both sides and each frame adds every camera word (the area's
`struct Camera`, Lakitu, transitions, C-Up, shakes, the random seed, FOV and
graph camera) and every object word (the lists, the free list and each
object's fields).

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

The object system lives in `simulation::object` (pool, lists, interpreter,
collision, processor, render-pass writes, coin behaviors) and
`import::objects` (behavior segment, macro presets, model registrations and
traversals, area placements). A placement whose script the port does not run
is never spawned; it is listed in `AreaObjects::skipped` with the reason.

## Next increment

Physical-GPU Windows/Linux playtesting continues. Coin/sparkle drawing and the
coin counter now work. Port the first mission's actors (King Bob-omb, Bob-ombs,
the star) with
the camera cutscenes that mission needs. Original-execution traces remain the
eventual authority. Skybox and placement models remain M1 work.

Camera checks without a ROM: `cargo test --locked -p rustario64-oracle --test camera --test lakitu --test radial --test camera_tick`;
repeat with `--release` for optimized comparisons. The owner-ROM camera tests
are ignored in ordinary CI; see [docs/FIDELITY.md](docs/FIDELITY.md).

Shadow checks: `cargo test --locked -p rustario64-oracle --test shadow`.
With your ROM, set `RUSTARIO64_ROM` and add `-- --include-ignored`.
`tools/check_shadow_reference.py --rom ROM --reference SM64_SOURCE`
independently validates the texture digest and original triangle order using
hash-checked files from the pinned source. Special lava-level and flying-carpet
shadow adjustments remain outside the BOB implementation; enhanced shadow maps
remain optional future work.

With your supported ROM, run the whole integration suite locally:

```sh
RUSTARIO64_ROM=/path/to/sm64.z64 RUSTARIO64_REQUIRE_GPU=1 \
  cargo test --locked --release --workspace --all-targets -- --include-ignored
```
