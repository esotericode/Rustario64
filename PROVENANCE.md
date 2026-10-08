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
| Same pinned sm64 | include/surface_terrains.h, src/engine/surface_load.c, include/special_presets.h, include/special_presets.inc.c, src/game/macro_special_objects.c | import/collision.rs, import/special.rs, content.rs | Referenced collision stream/surface semantics; translated preset-to-record-width metadata. Preserves force words including surface 0x0004. Adds bounds, signed-count/index checks and limits. No original collision-query port. | CC0. Authored record/width/error fixtures and exact real collision record comparison pass. |
| Same pinned sm64 | include/level_misc_macros.h, include/macro_presets.h, include/macro_presets.inc.c, src/game/macro_special_objects.c, src/engine/surface_load.c | import/macros.rs, import/level.rs, import/version.rs, content.rs | Translated modern macro record decoding: bias, packed angle, terminators and legacy dispatch; preserves raw params and source order. Bounded reads, table limit, record budget, segment-boundary checks. Defaults, behaviors and respawn decisions are not applied. | CC0. Authored malformed/rotation/termination fixtures; all 88 BOB macro records match pinned source exactly. |
| Same pinned sm64 | levels/bob/areas/1/collision.inc.c, levels/bob/areas/1/macro.inc.c, include/dialog_ids.h, include/special_presets.h, include/surface_terrains.h | tests/import.rs (ignored test), docs/ROM_VALIDATION.md | Validation only: independently expanded source macros into expected raw streams and typed records. Stored only counts/addresses/digests in tests/docs; no source placements or binary content bundled. | CC0 source reference. All 570 vertices, 1,060 ordered triangles, 17 specials and 88 macro placements match; no gameplay validation implied. |
| Same pinned sm64 | src/game/game_init.c | simulation.rs, docs | Reference cadence only: yields to VI twice / 30 FPS. New rational integer accumulator; no movement algorithm translated. | CC0. Synthetic cadence/backlog/long-clock tests. |
| [queueRAM/sm64tools](https://github.com/queueRAM/sm64tools/tree/81de9e5a8f0fa96686a16441d5b9f25742f4d17d) — 81de9e5a8f0fa96686a16441d5b9f25742f4d17d | configs/sm64.u.yaml | import/version.rs | Referenced BOB ROM ranges, segment-7 collision offset, and five RGBA16 texture offsets/dimensions. Other version offsets are not scattered in gameplay. | MIT, Copyright (c) 2015 Q; retained LICENSES/sm64tools-MIT.txt. Owner-ROM decode/export passes. |
| Same pinned sm64tools | libmio0.c : mio0_decode | import/mio0.rs | Rust algorithm adaptation, retaining MSB-first masks, token length/distance and overlapping copies. Adds stream bounds, output cap, checked back-references and strict malformed-token rejection. | MIT notice in source and LICENSES/. Authored literal/overlap/truncation/limit fixtures. |
| Same pinned sm64tools | n64graphics.c | import/texture.rs | Referenced RGBA5551 layout and integer SCALE_5_8 expansion; new safe decoder and preview writer. | MIT. Known independently chosen colors/alpha and bounds fixtures. |
| Same pinned sm64tools | README.md, tools/sm64collision.c, libsm64.c | Research only | Tooling scope and older decoder inspected; not incorporated. Collision force discrepancy resolved using decomp loader. This libsm64.c is tooling, not the engine-integration libsm64 project. | MIT. No runtime/tooling C dependency added. |

Pinned URLs resolve source files by appending the path to the revision above.
The repository-level terms and selected files were inspected. Code licenses do
not license Nintendo's ROM content. The other community projects in the plan
remain candidates; no claim of licensing/reusing their code is made.

## Dependencies and authored code

Newly authored application, safe readers, content types, scheduler, presentation,
trace comparator, exporters, and fixtures use this repository's MIT license.

| Dependency | Exact direct version | Purpose | Declared terms |
| --- | --- | --- | --- |
| sha1 | 0.10.6 | Upstream ROM fingerprint | MIT OR Apache-2.0 |
| serde | 1.0.228 | Typed import/trace serialization | MIT OR Apache-2.0 |
| serde_json | 1.0.145 | JSON reports and exact-integer traces | MIT OR Apache-2.0 |

Cargo.lock pins transitive versions and registry checksums. Dependencies are
downloaded by Cargo rather than copied into repository source. Preserve their
notices when packaging their source or distributable dependencies. The Rust
1.90.0 toolchain is pinned separately; current runtime code is entirely Rust.

## Reference comparison status

Initial target: pinned unmodified US n64decomp/original ROM execution. No oracle
binary/exporter was available, and no movement/collision implementation was
translated yet. The 30/60/120/144 Hz comparison is a synthetic counter/input-edge
fixture only. It must not be described as Mario fidelity coverage.

Original collision and macro asset comparisons now pass using the supplied ROM;
[docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md) records the source expansion,
digests, local checks, and remaining runtime/graphics/oracle gaps.
