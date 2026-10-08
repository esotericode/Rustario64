# Owner-ROM validation — 2026-10-08

The supplied 8 MiB Z64 ROM matches the supported US v1.0 normalized SHA-1
`9bef1128717f958171a4afac3ed78ee2bb4e86ce`. ROM inspection, the real BOB import,
private schema-3 export, the strengthened local integration test, and the viewer
screenshots passed on Linux x86_64 with Rust 1.90.0. The ROM and exported assets
are excluded from git.

```sh
cargo run --locked -- inspect-rom /path/to/your/sm64.z64
cargo run --locked -- import-bob /path/to/your/sm64.z64 --out private/imports
RUSTARIO64_ROM=/path/to/your/sm64.z64 cargo test --locked --test import local_us_rom_import -- --ignored --exact
cargo run --locked -p rustario64-render --bin rustario64-viewer -- screenshot /path/to/your/sm64.z64 --out private/bob.png
```

| Imported component | Observed result | Validation |
| --- | --- | --- |
| Segment 7 | 71,618 decompressed bytes | Real MIO0 block and independently decoded source-stream comparisons |
| Collision at 0x0700E958 | 570 vertices; 1,060 ordered triangles; 17 specials; no environment regions | Every decoded field/order and all 9,972 source bytes match the pinned collision macros |
| Macros at 0x0701104C | 88 records; 882 bytes including terminator | Every packed word, preset, signed coordinate, angle, raw parameter, address, and record order matches the pinned macro list |
| Level entry at 0x0E000264 | One area, 30 script placements, seven warps; start yaw 135 at (-6558, 0, 6464) | Unique reference segment-load pattern, source placement/warp counts and start fields; no runtime execution |
| Segment-7 textures | Five RGBA16 textures, 32×32 | Pixel-identical to the decomp toolchain's extraction (below) |
| Dependent segments | 0x09, 0x0A, 0x05, 0x0C, 0x06, 0x0D, 0x08, 0x0F from the script's own LOAD commands | ROM ranges equal sm64tools' pinned US block boundaries |
| Area geo layout 0x0E000488 | Ocean skybox background, camera node, six display lists (layers 1, 1, 6, 4, 1, 1) | Same nodes and order as the pinned `areas/1/geo.inc.c` |
| Visible area geometry | 1,101 triangles, 24 material batches, 18 textures (17 RGBA16, 1 IA16); no unsupported Fast3D commands | Every triangle matches the independently expanded display-list source; every texture matches decomp-tool PNGs |
| Geo models | Chain-chomp gate 2, seesaw 12, grate 3 triangles; bubbly tree reported (segment 0x16 not loaded) | Triangle counts match `levels/bob/*/model.inc.c` |

## Independent ground truth

Used [n64decomp/sm64 at 9921382a68bb0c865e5e45eb594d9c64db59b1af](https://github.com/n64decomp/sm64/tree/9921382a68bb0c865e5e45eb594d9c64db59b1af):
`levels/bob/areas/1/collision.inc.c`, `levels/bob/areas/1/macro.inc.c`,
`include/surface_terrains.h`, `include/special_presets.h`,
`include/macro_presets.h`, `include/level_misc_macros.h`, and
`include/dialog_ids.h`. These source placements were inspected outside the
tracked repository; none are included in authored fixtures.

A separate one-off Python source-macro expander resolved the upstream constants,
expanded each stream into big-endian shorts, and compared it with independently
decompressed ROM bytes. It also constructed expected typed records from those
source invocations and compared every field with exported Rust JSON. The
reference expectations did not come from the Rust parser's output.

| Reference data | SHA-1 |
| --- | --- |
| Expanded collision stream, 9,972 bytes | `fec575bc5aab234abf17da8ff02b49ff90c7c78e` |
| Expanded macro stream, 882 bytes | `d8ba1fc16dd1a82b72a8008a8bfc8cec17ce48d2` |
| Collision records, schema-2 compact JSON | `0b5c611a87b3e0afb76baeedfcba7352e96134e8` |
| Macro placements, schema-2 compact JSON | `b3bb06ce2e341e7b47fae1653f586464a76686b9` |

The ignored test compares the decoded collision and macro records with the last
two digests. JSON uses the declared struct-field order, no whitespace, signed
integer coordinates/angles, unsigned packed words/parameters, and `null` for
absent force words. Deliberate serialization changes require a schema update and
fresh independent expectations. The ordinary fixture tests exercise malformed
records, bounds, all 128 packed yaw values, legacy dispatch, and terminators.

## Visible geometry ground truth

A second one-off script (`python3 -I`, outside the repository) parsed the pinned
`levels/bob/areas/1/*/model.inc.c` vertex arrays and display lists, followed the
`GEO_DISPLAY_LIST` order from `areas/1/geo.inc.c`, and expanded `gsSPVertex`,
`gsSP1Triangle`, `gsSP2Triangles`, `gsSPDisplayList`, `gsSPBranchList` and
`gsSPEndDisplayList` into a 16-slot vertex buffer and a drawn-triangle stream. It
compared every triangle vertex (integer position and the four color/normal bytes),
the draw order, the drawing layer, and the address of the last
`gsDPSetTextureImage` with Rust's exported `visual.json`. All 1,101 match.

Textures were checked against a different implementation: the decomp's own
`tools/sm64tools/mio0` and `n64graphics`, built from the pinned tree, extracted
each of the 18 bound textures from the owner ROM using the offsets and formats in
the pinned `assets.json`, as `extract_assets.py` does. Every RGBA8 pixel matches
Rust's decode. Asset names that alias identical ROM bytes were accepted when their
format and dimensions agree.

| Reference data | SHA-1 |
| --- | --- |
| Drawn triangle stream `[[x,y,z,r,g,b,a] x3]...`, compact JSON | `f6ac0b00e30b5bb0583fb4f3dfc1670f411b3f74` |
| RGBA8 of the 18 bound textures, concatenated by source address | `cc0c962ef7fa0a8ae9f2d6ec1f7fb0ec8ee1cf14` |

The ignored owner-ROM test pins both digests, the dependent-segment ranges, the
batch/triangle/model counts, and the background. Fixture command words in
`tests/visual.rs` come from compiling the pinned `gbi.h` macros (F3D_OLD) with gcc,
for example `gsSPVertex(v, 15, 0)` = `04E000F0`, `gsSPFogPosition(980, 1000)` =
`BC000008 1900E800`, and `G_RM_AA_ZB_XLU_SURF | G_RM_AA_ZB_XLU_SURF2` = `005049D8`.

What this does not establish: UV normalization, combiner results, lighting
space, fog curve, and blending are checked only by authored fixtures, GPU pixel
tests on authored models, and visual review of screenshots. No comparison with
original rendered frames has been made.

## Collision behavior comparison

`RUSTARIO64_ROM=... cargo test --release -p rustario64-oracle --test collision
bob_collision -- --ignored` decodes BOB's collision stream from the owner ROM,
checks the Rust decoder agrees with `bob::import`, loads it into both the Rust
port and the natively compiled decomp loader, and compares all 1,060 surfaces,
every partition list, and 4,064,920 query results bit for bit (dense 97-unit grid
at six heights plus randomized and edge points). All are identical. This is a
component check against compiled decomp source, not a trace from original
execution.

## Remaining checks

Static script parsing reports global calls and missing dependent segments.
Original spawn ordering, preset defaults, respawn mutation, executable behaviors,
native callbacks, and object interactions remain unimplemented. No behavior
address has been promoted to an implemented runtime behavior.

Skybox import, object models from globally loaded segments, animation import,
dynamic object collision, Mario actions, camera, audio, and missions are missing. No
movement or collision fidelity claim follows from matching the asset streams.
There is still no matching oracle build/emulator exporter or genuine gameplay
trace. The next content tasks are viewer overlays for collision and placements,
the skybox, and object models; the next fidelity tasks are an exact differential
harness for collision loading/queries and a per-tick original-game spawn trace.
