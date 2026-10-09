# Owner-ROM validation — 2026-10-08

The supplied 8 MiB Z64 ROM matches the supported US v1.0 normalized SHA-1
`9bef1128717f958171a4afac3ed78ee2bb4e86ce`. ROM inspection, the real BOB import,
private export, the strengthened local integration test, and viewer screenshots
have passed on Linux x86_64. Build instructions now select Rust 1.99.0 and
schema 4; current-toolchain rechecks are recorded below. The ROM and exported assets
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
| Level entry at 0x0E000264 | One area, 30 script placements, seven warps; start yaw 135 at (-6558, 0, 6464); grass terrain 0, dialog slots [0, 255], music words [0, 3] | Unique reference segment-load pattern derived from the adapter; source placement/warp/start and area metadata fields; no runtime execution |
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

`tools/check_bob_reference.py` resolves the upstream constants, expands each
stream into big-endian shorts, and compares it with independently decompressed
ROM bytes. It also constructs expected typed records from source invocations
and compares every field with exported Rust JSON. Reference expectations do not
come from the Rust parser's output. It checks the ROM/export identity and requires
the exact clean upstream revision. Reproduce with Python 3's standard library:

```sh
git clone https://github.com/n64decomp/sm64.git /path/to/sm64-reference
git -C /path/to/sm64-reference checkout 9921382a68bb0c865e5e45eb594d9c64db59b1af
python3 -I tools/check_bob_reference.py --rom /path/to/sm64.z64 \
  --reference /path/to/sm64-reference \
  --export private/imports/9bef1128717f958171a4afac3ed78ee2bb4e86ce-schema4/bob
```

The checker accepts all three supported byte orders, reads reference data locally,
and writes no assets. It covers collision/macros; visual references below remain
separate.

| Reference data | SHA-1 |
| --- | --- |
| Expanded collision stream, 9,972 bytes | `fec575bc5aab234abf17da8ff02b49ff90c7c78e` |
| Expanded macro stream, 882 bytes | `d8ba1fc16dd1a82b72a8008a8bfc8cec17ce48d2` |
| Collision records, canonical compact JSON | `dfe37da1b39dada6ebf6c31cab9af3ca16c6e269` |
| Macro placements, canonical compact JSON | `ea3910a778a11c1f1bdd5c6d6e23af177b93a7c8` |

The ignored test compares the decoded collision and macro records with the last
two digests. JSON sorts object keys, preserves all array/source order, uses no
whitespace, signed
integer coordinates/angles, unsigned packed words/parameters, and `null` for
absent force words. Field declaration/key order is irrelevant; content changes
require fresh independent expectations. Import-format changes still require a
schema update. The ordinary fixture tests exercise malformed
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
every partition list, and 4,084,680 query results bit for bit (dense 97-unit grid
at six heights plus randomized and edge points). All are identical. This is a
component check against compiled decomp source, not a trace from original
execution.

## Rust 1.99 recheck — 2026-10-08

The reuploaded owner ROM passes the current asset integration test, inspection,
fresh schema-4 export, and the reproducible source checker. All 1,101 visible
triangles and 18 texture digests still match. Optimized native-C comparisons pass:
4,084,680 collision queries, 3,993,600 math checks, and 1,180,000 Mario step checks,
all bitwise-identical. The previously recorded collision count omitted points
already in the test; its generator was not changed in this update.

Four offscreen BOB views and a screenshot with 4× MSAA, fog/culling disabled,
collision and placement overlays render successfully on software Vulkan. Start
and options screenshots were visually reviewed. The 73 authored workspace tests
pass with the offscreen GPU tests required, as do optimized authored oracle tests,
formatting, Clippy, release builds, and the synthetic presentation-rate demo.
Windowed Xvfb smoke is blocked in this environment by unavailable
`/usr/bin/xkbcomp` during keyboard initialization. Hardware GPU, Windows/macOS,
original-frame rendering, and original-execution per-tick gameplay remain
unchecked. Normal run/check commands are in README; sandbox-specific compiler
wrapper details are in PROJECT_PLAN's session 4.

## Remaining checks

Static script parsing reports global calls and missing dependent segments.
Original spawn ordering, preset defaults, respawn mutation, executable behaviors,
native callbacks, and object interactions remain unimplemented. No behavior
address has been promoted to an implemented runtime behavior.

Skybox import, object models from globally loaded segments, dynamic object
collision, objects, the camera, cutscene/submerged actions, audio, and missions
are missing. No movement fidelity claim follows from matching the asset streams.
Complete Mario ticks are compared with the natively compiled decomp (below); no
original-execution per-tick exporter exists. Next: drive and draw Mario in the
viewer from the compared tick, then the reference camera.

## Input-stage integration — 2026-10-08

The supplied US ROM passed the new pre-action input suite before a workspace
reset: 196,608 controller/button/intent checks using actual ROM tables, 20,000
BOB geometry-input cases, and 1,200 chained input-stage ticks at multiple render
rates. Both native/Rust trace files exported and the exact trace CLI comparison
passed. The fixture includes imported BOB collision and script-start data;
script yaw is converted from degrees. It is not a spawn or action replay.

The reset removed the ROM mount and unpushed files. The change was reconstructed
and fresh authored checks pass; the owner-ROM input suite passed again on the
reuploaded ROM on 2026-10-09 (below). No ROM, decoded assets, or traces were
uploaded to the repository.

## Mario animation table — 2026-10-09

`RUSTARIO64_ROM=... cargo test --test animation local_us_rom_mario_animations
-- --ignored` decodes Mario's DMA animation table (0x4EC000, 209 entries) from
the owner ROM. Independently, `tools/check_mario_anims_reference.py --reference
<clean pinned checkout> --rom <ROM>` parses the pinned `assets/anims/*.inc.c`
with the rules of the decomp's `tools/mario_anims_converter.py`, rebuilds the
big-endian N64 table (0x8DC16 bytes, SHA-1
`27efba64f6698b8f87ec39a68cce3b0f5895cfb2`) and finds it equal to ROM bytes
0x4EC000..0x579C16. Its canonical record digest
`919843c7438f964a888830607c04e37e433fb99b` (every header field and array) is
the value the Rust test pins, so the Rust decoder and the source rebuild agree on
all 209 animations. Neither the table nor decoded values are stored.

## Owner-ROM recheck and complete Mario ticks — 2026-10-09

With the reuploaded US ROM, every ignored owner-ROM test passes in release mode:
the asset import digests, the animation table, BOB collision (4,084,680
queries), trig tables (3,993,600 checks), Mario steps (1,180,000 calls), the
input stage, and the new complete-tick suite. The tick suite runs BOB's
collision with the ROM's trig tables and Mario's real animations from the level
script's start and from 31 random floor positions: all 64,158 ticks of 47 runs
are identical between the Rust port and the natively compiled decomp,
reaching 60 distinct actions and 19,048 boundary events, and a 900-tick run is
identical at 30, 60 and 144 Hz presentation. The `tick_trace` example exports a
1,800-tick native/Rust trace pair privately; the CLI comparator reports an exact
match. These compare against native host C, not N64 execution; see
[FIDELITY.md](FIDELITY.md) for the limits.

## Viewer play and recorded replays — 2026-10-09

With the same ROM: `rustario64-viewer screenshot --mario-ticks 0` and `90`
rendered Mario's placeholder at the script start (ACT_IDLE at (-6558, 0, 6464),
facing 0x6000) and, after 90 ticks of holding the stick up, climbing out of the
open cannon hole (ACT_LEDGE_CLIMB_SLOW_2 at (-5583, 128, 5548)); both images
were inspected. A windowed `view --mario --record` session under Xvfb, driven
by xdotool key events, ran, jumped, turned the camera, re-entered the level and
dove into a stomach slide; its two recorded runs (154 and 79 ticks) replayed
exactly in the native decomp with `tick_trace --inputs`, and the CLI comparator
agreed. The ignored `bob_ticks` test passes again (64,158 ticks) and now checks
that the viewer's entry equals the script-start scenarios'. The recordings and
traces stayed in `private/`.
