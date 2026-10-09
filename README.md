# Rustario64

A new Rust engine that imports Super Mario 64 content from a user-supplied ROM.
Bob-omb Battlefield is the first playable **target**; today it is an imported,
viewable level, not yet a playable one. Read [PROJECT_PLAN.md](PROJECT_PLAN.md)
for scope, milestones, and the live status.

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
- A fixed 30 Hz scheduler and exact trace comparison, now with exportable
  native-C/Rust **input-stage** trace pairs.

Mario is not playable yet: the physics steps exist and are tested on their own,
but there are no actions, spawn initializer, camera logic, animation, objects,
or missions. Input-stage traces do not validate complete movement ticks. The imported level is independently validated against the
pinned decompilation; see [docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md).

## Layout

| Crate | Path | Purpose |
| --- | --- | --- |
| `rustario64` | `.` | GPU-free core: import, content, simulation scaffolding, traces, headless CLI |
| `rustario64-render` | `render/` | Optional wgpu renderer and the `rustario64-viewer` development binary |
| `rustario64-oracle` | `oracle/` | Development-only: pinned CC0 decomp collision, math, Mario step and input C compiled natively for bitwise differential tests (needs a C compiler); never a runtime dependency |

The core never depends on the renderer, so simulation and replay comparisons run
without a GPU or window.

## Build and test

Rust 1.99.0 (current stable as of 2026-10-08) with rustfmt and Clippy is selected
by `rust-toolchain.toml`. Linux x86_64 is the checked platform. Dependencies use
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
with the Rust collision, math, Mario step and input ports on authored data; see
[oracle/README.md](oracle/README.md).
Render tests draw small authored models offscreen. Without a GPU adapter they
skip; set `RUSTARIO64_REQUIRE_GPU=1` to make a missing adapter fail (CI does this
with Mesa's software Vulkan, `mesa-vulkan-drivers`). The viewer needs a Vulkan,
Metal, DX12, or GL driver; on Linux the windowed mode also needs X11 or Wayland
libraries (for example `libxkbcommon-x11-0` on X11).

`demo` decodes an independently authored MIO0/BOB-shaped fixture and runs a
ten-second synthetic counter/input replay at 30, 60, 120, and 144 Hz presentation
schedules with identical tick records. This tests scaffolding, not Mario.

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
```

The oracle tests compare BOB's real collision (about four million queries), the
ROM's trig tables (about four million lookups), and Mario's physics steps on BOB
(1.18 million step calls) between the Rust port and the decomp C.

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

Graphics-only flags: `--msaa 4`, `--no-fog`, `--no-cull`, `--size WxH`;
inspection overlays: `--collision` (floors blue, walls red, ceilings yellow) and
`--placements` (Mario start, script objects, macro objects, specials). In the
window, C, P, and F toggle collision, placements, and fog. The
viewer launches straight into BOB area 1 as a development entry point. The camera
is a presentation-only inspection camera, not the original game camera. The sky
is a placeholder color until the skybox is imported; trees, coins, enemies, and
other objects are not drawn yet (their painted ground shadows are terrain).

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

Extend the input-stage oracle to spawn initialization, action setters/dispatch,
and idle/walking/stopping actions. Represent animation state where it controls
transitions and declare camera/object/sound boundaries. Compare complete ticks
on BOB before connecting validated actions to the viewer. Original-execution
traces remain the eventual authority. Skybox and placement models remain M1 work.
