# Rustario64

A new Rust engine foundation for importing Super Mario 64 content from a
user-supplied ROM. Bob-omb Battlefield is the first playable **target**, not yet a
playable result. Read [PROJECT_PLAN.md](PROJECT_PLAN.md) for scope and status.

The executable is headless: ROM validation, a BOB static import path, independent
parser fixtures, and exact per-tick trace comparison. There is no window, visible
terrain renderer, Mario movement, or mission implementation. The real-ROM import
path passes the local owner-ROM check for the supported US revision. Collision
and macro records were checked exactly against the pinned decompilation source;
see [docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md) for evidence and limits.

## Build and run

Rust 1.90.0, Cargo, rustfmt, Clippy, and a system linker are required.
`rust-toolchain.toml` selects the toolchain with rustup. Linux x86_64 is the
checked platform. Direct crates and transitive dependencies are pinned.

```sh
rustup toolchain install 1.90.0 --profile minimal --component rustfmt --component clippy
cargo run --locked -- demo
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

`demo` decodes an independently authored MIO0/BOB-shaped fixture and runs the
same ten-second synthetic counter/input replay at 30, 60, 120, and 144 Hz
presentation schedules. Each produces 300 identical tick records. Interpolation
and graphics flags are toggled. This tests scaffolding, not Mario movement
fidelity or original level import.

## Use your ROM locally

Only the original 8 MiB US v1.0 revision is accepted, with normalized SHA-1
`9bef1128717f958171a4afac3ed78ee2bb4e86ce`. Z64, V64, and N64 byte orders are
normalized before fingerprinting. Regions, hacks, padding, truncation, and edited
headers are rejected. Identity comes from the pinned decompilation's full hash,
not the filename or N64 header CRCs.

```sh
cargo run --locked -- inspect-rom /path/to/your/sm64.z64
cargo run --locked -- import-bob /path/to/your/sm64.z64 --out private/imports
RUSTARIO64_ROM=/path/to/your/sm64.z64 cargo test --locked --test import local_us_rom_import -- --ignored --exact
```

Output goes to `private/imports/<normalized-sha1>-schema2/bob/`:

| File | Contents |
| --- | --- |
| `manifest.json` | Identity, partial-import status, counts, and explicit unsupported-content diagnostics |
| `level.json` | Course/area IDs, act masks, script and macro placements, warps, Mario start, segment loads, and model references |
| `collision.json`, `collision.obj` | Original integer collision vertices, ordered surfaces/force words, specials, and environment regions |
| `terrain-N-32x32.rgba`, `terrain-N.ppm` | Five known segment-7 RGBA5551 textures, with RGBA bytes and alpha-free previews |

The OBJ inspects **collision**, not visible terrain. Textures are not yet attached
to Fast3D materials. The verified import contains 570 vertices, 1,060 collision
triangles, 17 special placements, 30 script placements, 88 macro placements,
seven warps, and five textures. Macro records preserve packed yaw, source order,
and raw parameters; preset defaults and respawn behavior are still missing.
Global scripts, dependent segments, geometry, callbacks, and behaviors remain
reported gaps. No ROM code is executed.

The ignored integration test checks exact counts/addresses and source-derived
digests for every collision and macro field. Ordinary CI uses authored fixtures
without a ROM. Schema 2 adds macro records and replaces the old untested-status
manifest field with an explicit partial-import status; schema-1 exports are not
reused or upgraded automatically.

Exports reserve a new destination and never overwrite files. Remove an old
private export deliberately or choose another output root to reimport. Hash and
schema keys are ready for caching; cache loading/reuse is not implemented. Saves
will be separate. Exports include an ignore rule even under a custom output root.
Keep ROMs and all ROM-derived output private and out of Git.

## Trace comparison

```sh
cargo run --locked -- demo --trace private/foundation.trace.json
cargo run --locked -- compare-traces private/foundation.trace.json private/foundation.trace.json
```

Existing trace files are not overwritten. Use an oracle and Rust replay once
those gameplay producers exist. A mismatch fails with the first tick/field.
Authoritative floats use `f32::to_bits()` integer values, including signed zero,
with no tolerance. ROM, reference configuration, initial state/world, inputs,
and gameplay options must match. Empty traces and missing fields fail.

See [docs/FIDELITY.md](docs/FIDELITY.md) for the comparison contract and blockers,
[docs/DECISIONS.md](docs/DECISIONS.md) for module/timing choices, and
[PROVENANCE.md](PROVENANCE.md) for exact sources and reuse notices.

## Next increment

Decode BOB geometry layouts and Fast3D materials, including required dependent
segments, for a Rust renderer. Establish
the first genuine per-tick oracle trace before porting and claiming faithful
movement. Imported level, exploration, and mission completion remain distinct
milestones in the live plan.
