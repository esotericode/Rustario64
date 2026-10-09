# Provenance ledger

Checked 2026-10-08. Only the exact revisions below supplied format knowledge or
adaptations in this increment. Full notices are retained in LICENSES/.
No ROM, ROM-derived assets, original terrain/animation files, or full C runtime
is tracked or published. Owner-ROM validation exports are local and ignored.
Independently authored fixtures are generated
by src/diagnostics.rs and tests; they are not samples from the game.

## Source use

| Upstream and revision | Source paths | Destination | Use and local changes | Terms / validation |
| --- | --- | --- | --- | --- |
| [n64decomp/sm64](https://github.com/n64decomp/sm64/tree/9921382a68bb0c865e5e45eb594d9c64db59b1af) — 9921382a68bb0c865e5e45eb594d9c64db59b1af | sm64.us.sha1 | import/version.rs, import/rom.rs | Reference metadata: exact normalized full-ROM fingerprint. Strict size and identity checks added. | CC0 in LICENSE.md; retained LICENSES/sm64-CC0.txt. Supplied US ROM matches. |
| Same pinned sm64 | include/level_commands.h, src/engine/level_script.c, levels/bob/script.c, levels/level_defines.h | import/level.rs, import/bob.rs, content.rs | Referenced command layouts, course/level identity, entry load pattern, local call structure, act-mask sentinel, placement and warp fields. New bounded static extractor; no native functions executed or source placements bundled. | CC0. Independent call/field/error fixtures and real-ROM static import pass; runtime gaps explicit. |
| Same pinned sm64 | include/surface_terrains.h, src/engine/surface_load.c, include/special_presets.h, include/special_presets.inc.c, src/game/macro_special_objects.c | import/collision.rs, import/special.rs, content.rs | Referenced collision stream/surface semantics; translated preset-to-record-width metadata. Preserves force words including surface 0x0004. Adds bounds, signed-count/index checks and limits. The loader and queries are ported separately in simulation/collision.rs (below). | CC0. Authored record/width/error fixtures and exact real collision record comparison pass. |
| Same pinned sm64 | include/level_misc_macros.h, include/macro_presets.h, include/macro_presets.inc.c, src/game/macro_special_objects.c, src/engine/surface_load.c | import/macros.rs, import/level.rs, import/version.rs, content.rs | Translated modern macro record decoding: bias, packed angle, terminators and legacy dispatch; preserves raw params and source order. Bounded reads, table limit, record budget, segment-boundary checks. Defaults, behaviors and respawn decisions are not applied. | CC0. Authored malformed/rotation/termination fixtures; all 88 BOB macro records match pinned source exactly. |
| Same pinned sm64 | levels/bob/areas/1/collision.inc.c, levels/bob/areas/1/macro.inc.c, include/dialog_ids.h, include/special_presets.h, include/surface_terrains.h, include/macro_presets.h | tools/check_bob_reference.py, tests/import.rs (ignored test), docs/ROM_VALIDATION.md | Validation only: independently expand source macros into expected raw streams and typed records from a user-provided clean pinned checkout. Stored only counts/addresses/digests in tests/docs; no expanded placements or binary content bundled. | CC0 source reference. All 570 vertices, 1,060 ordered triangles, 17 specials and 88 macro placements match; no gameplay validation implied. |
| Same pinned sm64 | include/level_commands.h, src/engine/level_script.c, src/game/area.c, include/seq_ids.h | import/level.rs, content/mod.rs, tests/import.rs | Area metadata formats and defaults: terrain word OR, two dialog slots, signed music words. Static Rust extraction, without executing gameplay/audio. | CC0 reference. Authored repeated-command/default/bounds fixtures and owner-ROM BOB values. |
| Same pinned sm64 | include/geo_commands.h, src/engine/geo_layout.c, src/engine/geo_layout.h, src/engine/graph_node.h, src/engine/graph_node.c, src/engine/graph_node_manager.c | import/geo.rs | Translated geo command widths, stack/END/RETURN/branch semantics, node depth list and attachment, flag operations, layer-in-flags rule, and the `(deg << 15) / 180` angle conversion into a bounded Rust decoder that records (never runs) callbacks. | CC0. Authored parenting/control-flow/error fixtures; real BOB area and model layouts decode. |
| Same pinned sm64 | include/PR/gbi.h, include/PR/mbi.h (F3D_OLD), src/game/game_init.c (init_rsp/init_rdp), src/game/rendering_graph_node.c (render-mode tables, master lists, transforms), src/engine/math_util.c (mtxf_rotate_zxy_and_translate, mtxf_mul), include/sm64.h (layers), include/geo_commands.h (backgrounds) | import/gfx.rs, import/model.rs, content/visual.rs, render/src/shader.wgsl | Translated Fast3D command encodings, combiner/othermode/geometry-mode bit layouts, default state, layer render modes, fog-position contract and transform order. Render-mode and combiner constants were computed by compiling the gbi.h macros with gcc (not vendored). | CC0. Authored command-word fixtures from the compiled macros; 1,101-triangle BOB comparison passes. |
| Same pinned sm64 | levels/bob/script.c, levels/bob/areas/1/geo.inc.c, levels/bob/areas/1/*/model.inc.c, levels/bob/leveldata.c, levels/bob/texture.inc.c | tests/import.rs (ignored test), docs/ROM_VALIDATION.md | Validation only: a separate script expanded the display-list source in geo order into expected triangle vertices; only counts/digests are stored. | CC0 source reference; no source geometry bundled. |
| Same pinned sm64 | assets.json, extract_assets.py, tools/sm64tools/mio0, tools/sm64tools/n64graphics | docs/ROM_VALIDATION.md | Validation only: built the decomp's vendored tools outside the repository and extracted BOB's textures from the owner ROM to compare pixels. Nothing extracted is tracked. | Decomp tooling terms as shipped in that tree; used locally, not incorporated. |
| Same pinned sm64 | src/engine/surface_load.c, src/engine/surface_load.h, src/engine/surface_collision.c, src/engine/surface_collision.h, include/surface_terrains.h | simulation/collision.rs | Translated static surface loading (normals, offsets, bounds, flags, force, partition cells and ordering) and floor/ceiling/wall/water/gas queries with original widths, casts, f32 order and double thresholds. Globals became explicit parameters; pool overflow became an error. Rooms, dynamic object surfaces and debug counters not yet ported. | CC0. Bitwise differential tests against the same code compiled natively: authored streams in CI, BOB on the owner ROM. |
| Same pinned sm64 | the collision files above plus include/special_presets.h, include/special_presets.inc.c, include/model_ids.h, src/engine/math_util.c/.h, src/game/mario_step.c/.h, include/sm64.h, config.h, macros.h, platform_info.h, object_constants.h, sounds.h, mario_animation_ids.h, mario_geo_switch_case_ids.h, level_table.h, levels/level_defines.h; `spawn_special_objects` from src/game/macro_special_objects.c and the five mario.c items above | oracle/c/decomp/ (byte-identical copies, SHA-1s in oracle/README.md), oracle/c/oracle.c (verbatim functions) | Vendored for the development-only native oracle; authored shim headers and glue (action setters stubbed to abort, sound stubbed). Never linked into the runtime. All copies re-compared byte for byte with the pinned checkout on 2026-10-08. | CC0; notice in oracle/c/decomp/LICENSE-CC0.txt. |
| Same pinned sm64 | src/engine/math_util.c, src/engine/math_util.h, include/trig_tables.inc.c | simulation/math.rs, import/engine.rs, import/version.rs, oracle/c/decomp/ | Translated sins/coss (u16 index, contiguous sine/cosine), atan2_lookup/atan2s/atan2f and approach_s32/f32. The table values were compiled from the decomp source outside the repository only to locate them in the verified ROM (0x102D80, 0x107D80); the repository stores offsets and SHA-1s and loads values from the owner ROM. math_util.c/.h vendored unmodified for the oracle. | CC0. Bitwise differential tests with authored tables (CI) and ROM tables (ignored test). |
| Same pinned sm64 | src/game/mario_step.c, src/game/mario_step.h; from src/game/mario.c: mario_set_forward_vel, resolve_and_return_wall_collisions, vec3f_find_ceil, mario_get_terrain_sound_addend, sTerrainSounds; include/sm64.h, include/surface_terrains.h, include/sounds.h, include/level_table.h, levels/level_defines.h | simulation/mario/mod.rs, simulation/mario/step.rs, simulation/mario/constants.rs | Translated the physics steps (ground/air/stationary steps, ledge grab, gravity, vertical wind, moving sand, windy ground, bonk reflection, velocity helpers) and the four helpers with original f32 order, s16 wraparound and quirks. Globals became explicit `StepWorld` inputs; the water pseudo-floor became a `SurfaceRef` variant; NULL dereferences and out-of-table reads panic. Constants were generated by evaluating the headers with a C compiler. mario_update_quicksand, mario_push_off_steep_floor, bully helpers and sound playback not ported. | CC0. Bitwise differential tests against the same code compiled natively: authored terrain in CI, BOB with ROM trig tables on the owner ROM; a CI test checks every generated constant. |
| Same pinned sm64 | src/game/game_init.c | simulation.rs, docs | Reference cadence only: yields to VI twice / 30 FPS. New rational integer accumulator; no movement algorithm translated. | CC0. Synthetic cadence/backlog/long-clock tests. |
| [queueRAM/sm64tools](https://github.com/queueRAM/sm64tools/tree/81de9e5a8f0fa96686a16441d5b9f25742f4d17d) — 81de9e5a8f0fa96686a16441d5b9f25742f4d17d | configs/sm64.u.yaml | import/version.rs | Referenced BOB ROM ranges, segment-7 collision offset, and five RGBA16 texture offsets/dimensions. Other version offsets are not scattered in gameplay. | MIT, Copyright (c) 2015 Q; retained LICENSES/sm64tools-MIT.txt. Owner-ROM decode/export passes. |
| Same pinned sm64tools | libmio0.c : mio0_decode | import/mio0.rs | Rust algorithm adaptation, retaining MSB-first masks, token length/distance and overlapping copies. Adds stream bounds, output cap, checked back-references and strict malformed-token rejection. | MIT notice in source and LICENSES/. Authored literal/overlap/truncation/limit fixtures. |
| Same pinned sm64tools | n64graphics.c | import/texture.rs | Referenced RGBA16/32, IA4/8/16, I4/8 and CI4/8 layouts and SCALE_5_8/4_8/3_8 expansions; new bounded decoder with strided rows and 16-bit palettes. I4/I8 alpha follows RDP TEXEL alpha instead of the tool's opaque export. | MIT. Authored per-format fixtures; real textures match decomp tool output. |
| Same pinned sm64tools | README.md, tools/sm64collision.c, libsm64.c | Research only | Tooling scope and older decoder inspected; not incorporated. Collision force discrepancy resolved using decomp loader. This libsm64.c is tooling, not the engine-integration libsm64 project. | MIT. No runtime/tooling C dependency added. |
| Same pinned sm64tools | configs/sm64.u.yaml | tests/import.rs (ignored test) | Cross-check only: the eight dependent segment ranges named by BOB's level script equal the config's block boundaries. | MIT. |
| [libsm64/libsm64](https://github.com/libsm64/libsm64/tree/fd11813208272b4271d92bd92feb8f3fdbe61be5) — fd11813208272b4271d92bd92feb8f3fdbe61be5 | src/decomp/engine/surface_collision.c, src/decomp/game/mario*.c | None (research) | Diffed against the pinned decomp to evaluate it as an oracle; rejected because its collision changes first-match ordering. No code incorporated. | CC0 (LICENSE.md). |
| [sm64-port/sm64-port](https://github.com/sm64-port/sm64-port/tree/2b17d081c9798b31b91dc71f37994b0da28cffc9) — 2b17d081c9798b31b91dc71f37994b0da28cffc9 | src/pc/gfx/LICENSE.txt | None | License inspected only. Its renderer terms forbid binary redistribution, so no code was copied, translated, or used as an implementation reference. | Excluded. |

Pinned URLs resolve source files by appending the path to the revision above.
The repository-level terms and selected files were inspected. Code licenses do
not license Nintendo's ROM content. The other community projects in the plan
remain candidates; no claim of licensing/reusing their code is made.

## Dependencies and authored code

Newly authored application, safe readers, content types, geo/Fast3D importers,
scheduler, presentation, renderer, shader, viewer, trace comparator, exporters,
fixtures, and the oracle's shim headers and glue use this repository's MIT
license. Translated collision, math, and Mario step code derives from CC0 sources.

| Dependency | Resolved direct version | Purpose | Declared terms |
| --- | --- | --- | --- |
| sha1 | 0.11.0 | Upstream ROM/table/content fingerprint | MIT OR Apache-2.0 |
| serde | 1.0.229 | Typed import/trace serialization | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | JSON reports and exact-integer traces | MIT OR Apache-2.0 |
| wgpu | 30.0.1 | Optional renderer (render crate only) | MIT OR Apache-2.0 |
| winit | 0.30.13 | Development viewer window/input (render crate only) | Apache-2.0 |
| pollster | 1.0.1 | Blocking on wgpu adapter/device futures | Apache-2.0/MIT |
| png | 0.18.1 | Screenshot encoding | MIT OR Apache-2.0 |
| cc | 1.6.0 | Build-time C compilation for the development oracle only | MIT OR Apache-2.0 |

Cargo.lock pins transitive versions and registry checksums. A metadata scan of
the resolved workspace found permissive license choices (MIT, Apache-2.0,
BSD-2/3-Clause, Zlib, ISC, Unicode-3.0, Unlicense, 0BSD, and dual/tri license
choices including those). The updated 278-package workspace metadata was checked
again on 2026-10-08; a formal per-file notice bundle for binary releases
is still to be produced before distributing builds. Stable direct releases were
checked against crates.io metadata on 2026-10-08. The lockfile is updated to the
latest compatible stable resolution rather than imposing an arbitrary release
age. Upstream dependency constraints still apply, including interlocked
wasm-bindgen/js-sys/web-sys versions. Dependencies are
downloaded by Cargo rather than copied into repository source. Preserve their
notices when packaging their source or distributable dependencies. The Rust
1.99.0 toolchain is pinned separately (official stable release dated 2026-10-01);
current runtime code is entirely Rust. GitHub CI uses SHA-pinned
actions/checkout v7.0.1. Tool updates do not change the original behavioral
reference revision or the simulation cadence.

## Reference comparison status

Initial target: pinned unmodified US n64decomp/original ROM execution. No oracle
binary/exporter for original execution is available yet. Collision loading and
queries, the math utilities, and Mario's physics steps are compared with the
pinned decomp compiled natively (oracle crate), which is a practical oracle, not
original-hardware evidence. Mario's actions, input processing and camera are not
translated yet, so there is no per-tick movement comparison. The
30/60/120/144 Hz comparison is a synthetic counter/input-edge fixture only. It
must not be described as Mario fidelity coverage.

Original collision, macro, visible-triangle, and texture asset comparisons pass
using the supplied ROM; [docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md) records
the source expansions, decomp-tool extraction, digests, local checks, and the
remaining runtime/graphics/oracle gaps. Rendering is visually reviewed only.
