# Mario 64 Rust Engine Project Plan

Last updated: 2026-10-08  
Status: M0 headless foundation implemented and fixture-tested; owner-ROM import and gameplay fidelity checks remain blocked.
Initial content target: Bob-omb Battlefield from a supported Super Mario 64 ROM.  
Long-term intent: Support the complete original game through the same engine.

## 1. Purpose and project identity

Build a new game engine and Mario 64 runtime in Rust. The user supplies a ROM, and the application imports the original content into our own asset, world, simulation, and rendering systems. Preserve Mario 64's physics and movement while offering modern presentation and optional controls and quality-of-life features.

We can reference, adapt, or translate existing community code when its terms permit. Replacing the engine architecture does not require inventing every gameplay algorithm again. The decompilation is a valuable behavioral specification and source of implementations; existing ports are useful references and comparison tools.

The Rust engine must own the application loop, imported world representation, rendering, and gameplay integration. A wrapper around the original game's renderer is outside this project's intended identity. A permanent full-game C runtime is also outside the intended end state. Small C integrations are acceptable as documented development scaffolding or comparison tools, with explicit boundaries and a replacement plan.

This is a game-specific engine replacement. Supporting arbitrary N64 games or arbitrary ROM hacks is not an initial goal.

## 2. Requirements and priorities

1. **Faithful simulation.** Preserve movement, collision, action transitions, interactions, and gameplay timing for the supported reference version. Exact fidelity is a target that requires evidence, not a claim implied by similar-looking play.
2. **Real ROM content.** Import the original terrain, collision, character geometry, textures, animation data, object placements, and other needed content from the user's ROM. Synthetic fixtures help development but do not count as a working Mario 64 level.
3. **A Rust implementation.** Own the engine and progressively implement the runtime in Rust. Reuse libraries and existing algorithms where useful.
4. **Modern presentation.** Support higher resolution and smooth rendering independently of simulation. Enhanced lighting and shadows should be optional.
5. **Room for the whole game.** Treat Bob-omb Battlefield as the first course. Preserve concepts for other courses, areas, acts, behaviors, warps, save progression, and special mechanics.
6. **Honest progress.** Keep this document current. State what works, what is approximated, what is missing, and what was actually tested.

Faithful movement takes priority over attractive graphics. A small, working implementation takes priority over speculative framework design. Choose routine technical details autonomously within these goals; discuss proposed changes to the goals before adopting them.

## 3. Initial scope and what working means

Start with one documented ROM revision, preferably the original US release, in big-endian Z64 format. Confirm its identity against upstream reference metadata. Additional byte orders and regions can follow through version adapters. Pick one available desktop platform for development and record it; keep platform-specific code contained so Windows, Linux, and macOS remain feasible.

The first playable result may launch directly into Bob-omb Battlefield through a development entry point. The title screen, intro, castle hub, and original menus can come later. Document this entry point as development behavior.

Use distinct completion levels:

| Milestone | Required result |
| --- | --- |
| Imported level | Original terrain and textures render from the ROM; the collision mesh can be inspected. |
| Playable exploration | Mario runs and jumps through the original level with imported animations, a usable camera, and validated movement and collision for the implemented actions. |
| First mission | At least one original mission works through its actual interactions, star collection, and recorded completion. A manually spawned star is insufficient. |
| Complete Bob-omb Battlefield | All six selected-act stars and the 100-coin star are obtainable through their required mechanics; act selection, death, re-entry, and saving work. |

Complete course support includes King Bob-omb, Koopa the Quick, Chain Chomp and its gate, cannons, red coins, coin rings, the wing cap, rolling balls, relevant enemies, platforms, pickups, and mission-dependent placements. Build this incrementally; keep a per-act checklist rather than labeling the entire course complete after exploration works.

Wing-cap access normally depends on a campaign unlock. A documented development override may make the intended flight mechanics testable before the castle progression exists. That override does not count as implementing the original unlock. Course completion and full campaign completion are separate milestones.

## 4. Simulation fidelity and smooth rendering

### Preserve the original simulation cadence

For the initial US reference, use **30 simulation ticks per second**, independently of display refresh. The original source's [display timing code](https://github.com/n64decomp/sm64/blob/master/src/game/game_init.c) documents its 30 FPS cadence. Preserve the order and internal steps of movement and collision within each tick. Do not multiply per-frame movement constants by a variable delta time or double the simulation frequency to obtain 60 FPS.

Keep authoritative simulation state separate from presentation snapshots. A renderer can display interpolated transforms and animation poses at 60, 120, 144 FPS, or another selected rate. It must never write those interpolated values back into collision, AI, gameplay timers, RNG, or save state.

Interpolation needs explicit handling for angle wrapping, animation changes, spawn and despawn, teleportation, death, and area transitions. Snap across discontinuities rather than blending Mario through the world. Interpolating between completed snapshots can add presentation delay; measure and document it. Smoother images do not automatically provide more frequent gameplay input processing.

Use a fixed-step accumulator or an equivalent deterministic scheduler. Wall-clock time schedules ticks; it does not enter the gameplay equations. On stalls, pause, or loss of focus, use a documented catch-up policy without inventing variable-size physics steps or silently omitting gameplay updates.

### Preserve numerical and ordering behavior

Port behavior with attention to float precision and operation order, integer widths, casts, signed angles, wrapping arithmetic, original lookup tables, RNG state, object update order, surface ordering, and collision-query tie breaking. Follow the source's declared precision, including `f32` where used; do not uniformly promote calculations to `f64`. Do not assume Rust casts, generic math libraries, an ECS's iteration order, or a modern physics engine reproduce the original semantics. Avoid arithmetic transformations such as fused operations where they change reference results.

Keep original collision data distinct from visible meshes. Enhanced meshes, extra rendering distance, shadows, or camera interpolation must not alter gameplay collision or the distances that control gameplay updates.

Preserve defined, reproducible gameplay quirks in the faithful baseline. Track discrepancies and any behavior that cannot be reproduced safely or deterministically. Put intentional gameplay fixes in explicit optional modes; never silently merge them into the fidelity baseline.

### Make exactness measurable

Select and pin a comparison implementation before claiming faithful physics. An unmodified, suitably configured decompilation-based build can provide a practical oracle. Check whether the chosen port or libsm64 version has already changed relevant behavior; use traces from the original ROM execution to settle differences where necessary.

Record initial state, ROM identity, reference revision, configuration, per-tick buttons and stick values, camera state used by movement, RNG seed, and relevant world state. Compare the Rust and reference results at every simulation tick: actions, position, velocity, angles, timers, collision contacts, interaction outcomes, and relevant object state. Report the first divergent field and tick.

Require exact equality for discrete state and target bit-for-bit equality for authoritative floating-point results in the declared compatibility target. A temporary tolerance must be labeled as a discrepancy with a reason and plan; it does not satisfy the perfect-preservation requirement. Record which platforms and cases have actually passed. A finite replay suite demonstrates coverage, not universal proof.

Use the same replay to confirm identical authoritative states at multiple render caps, with interpolation and graphics options enabled and disabled. Test representative running, turning, acceleration, stopping, jump variants, wall kicks, slopes, ceilings, ledges, landing, and dynamic surfaces as they are implemented. Include long sequences that can expose accumulated drift.

Camera-relative movement deserves particular care. Exact comparisons require the same normalized tick input and the same authoritative camera orientation. Preserve reference stick processing and button-edge semantics, consuming gameplay input at simulation ticks rather than render frames. Optional modern camera or dead-zone settings can change the player's intended direction even when the underlying physics is unchanged. Preserve a reference camera and input profile for comparisons.

## 5. ROM import and asset handling

The normal user flow should eventually be: select a supported ROM once, import or cache its assets, and play. A CLI path is sufficient for the first milestone. Do not require users to compile the whole original C game to run the final Rust implementation.

Implement a bounded reader and explicit version description:

1. Identify byte order and revision using a known fingerprint. Reject unsupported or truncated inputs clearly; file extensions and ROM-header checksums alone are insufficient identity checks.
2. Normalize supported byte orders into an internal big-endian view. Supporting only Z64 at first is acceptable if stated accurately.
3. Read version-specific offsets and segment mappings from an identified source. Decompress MIO0 blocks with bounds checks and output limits.
4. Decode the needed level scripts, geometry layouts, Fast3D display lists, textures, collision records, animation data, and placements into engine-owned structures. Preserve material state, hierarchy, surface types, behavior parameters, and act masks.
5. Resolve known behavior and callback addresses through a version-specific mapping to our own implementations. A script can reference executable routines; parsing the script alone does not recreate those routines.
6. Cache decoded content locally, keyed by ROM fingerprint and importer schema version. Keep saves and user settings outside disposable caches.

Use the existing extraction manifests, parsers, and format descriptions listed below. Avoid guessing offsets or recreating formats from memory. Version-dependent addresses belong in the import adapter, not scattered through gameplay code. Rust simulation should use stable IDs and typed handles rather than raw ROM pointers.

For unsupported commands or behaviors, report their identity and location. A viewer may skip them visibly, but a playable course must not silently treat them as successful no-ops. Keep a coverage list for commands, callbacks, and behaviors needed by each area and act.

Audio needs its own decoding and playback work: sequences and instrument banks do not automatically become ordinary audio files. It may follow the earliest visual prototype, but original course music and effects belong in the complete-course milestone.

The repository and distributable builds must exclude the user's ROM and ROM-derived assets. Ignore private ROM, extraction, cache, and local trace directories. Use small independently authored fixtures for ordinary CI; document how owners of a supported ROM run the integration and fidelity checks locally.

## 6. Architecture that can grow

Keep a small number of clear modules initially. These are responsibility boundaries, not a demand for one crate each:

| Responsibility | Boundary |
| --- | --- |
| ROM and import | Validation, decompression, segment resolution, decoding, and version metadata. |
| Assets and content | Meshes, materials, animations, collision, course and area definitions, spawns, acts, and warps. |
| Simulation | Deterministic Mario and object state, movement, collision, behaviors, interactions, RNG, and timers. |
| Presentation | Read-only render snapshots, animation and camera interpolation, lighting, shadows, HUD, and audio events. |
| Application | Window, devices, fixed-step scheduling, settings, saves, diagnostics, and development entry points. |

Begin with course, area, and act identifiers; a behavior registry; separate static and dynamic collision; and explicit transition events. Reusable definitions should hold placements and mission conditions. Level-specific behaviors may remain level-specific, but the application loop should not contain hardcoded Bob-omb Battlefield rules.

The reference camera's logic and movement-facing orientation belong to authoritative tick state; the displayed camera can interpolate separately. Audit imported geometry callbacks for gameplay effects and schedule those effects during simulation, never once per rendered frame.

Generalizing a system should follow demonstrated needs. Do not build a plugin framework, a universal script VM, networking, or an editor before they serve a playable milestone. It is acceptable to implement only the script commands encountered so far through an extensible dispatcher.

Use Rust libraries rather than recreating windowing or GPU access. A reasonable starting candidate is [wgpu](https://github.com/gfx-rs/wgpu) with [winit](https://github.com/rust-windowing/winit). [Bevy](https://github.com/bevyengine/bevy), or selected Bevy crates, is another option if it reduces practical work. Either choice must leave us in control of deterministic simulation and the content adapter. Pin compatible versions and briefly record the choice; this brief does not mandate a framework.

Keep simulation usable without a GPU or window so replay comparisons are easy. Add a development way to load another area or course through the same interfaces. Before a major graphics expansion, exercise those interfaces with a second real course to catch assumptions unique to the first one.

## 7. Improvements and options

Start with a faithful profile. Keep presentation settings independent of gameplay settings and include behavior-affecting configuration in replay metadata.

| Option | Intended behavior | Priority |
| --- | --- | --- |
| Render rate and interpolation | Reference-rate display or smooth higher-rate display, without changing simulation ticks. | Early |
| Resolution and render scale | Adjustable window and internal resolution, fullscreen, frame cap, and VSync. | Early |
| Aspect ratio | Original 4:3 baseline; optional widescreen with corrected projection and HUD placement. | Early |
| Antialiasing and texture filtering | Optional cleaner edges and filtering; retain a profile that respects the original appearance. | After the level renders |
| Shadows | Original-style character shadow baseline; optional dynamic shadow maps and quality settings. | After movement is validated |
| Lighting | Optional enhanced lighting and later ambient occlusion; keep original-style materials available. | Later |
| Input mapping | Gamepad and keyboard remapping; optional stick calibration, inversion, and dead zones. | Early |
| Camera | Reference behavior plus an optional right-stick or mouse camera with adjustable sensitivity. | After the reference path works |
| Quality of life | Optional stay-in-course after a star, faster transitions, and additional HUD choices. | After original progression works |
| Replacement assets | Optional user-provided texture or model packs, with original gameplay collision retained. | Later |

Do not add new jump physics, automatic wall kicks, extra moves, coyote time, or input buffering to the faithful profile. If desired later, keep them in clearly named gameplay modes with separate validation. Ray tracing, multiplayer, and ROM-hack compatibility are future possibilities, not initial dependencies.

Enhanced lighting may require authored light and material metadata because the original content does not supply a complete modern lighting setup. Shadows and filters should respect the game's visual style rather than require replacement assets.

## 8. Reference projects and reuse

References were checked on 2026-10-08. Links below identify upstream projects; pin the actual revision used before copying or deriving code. Listed uses are recommendations for this project, not promises of ready-made compatibility.

| Project | Useful material | Reuse notes |
| --- | --- | --- |
| [n64decomp/sm64](https://github.com/n64decomp/sm64) | Full decompilation, behavior definitions, formats, level definitions, extraction metadata, and original gameplay implementations. | Repository publishes CC0 terms. Primary behavioral reference; inspect the selected files and provenance. |
| [queueRAM/sm64tools](https://github.com/queueRAM/sm64tools) | ROM splitting, MIO0, Fast3D and geometry decoding, texture tools, and version configurations. | MIT declaration. Strong candidate for adapting importer algorithms; this project's `libsm64.c` is different from the libsm64 engine-integration project below. |
| [libsm64/libsm64](https://github.com/libsm64/libsm64) | A library interface for original Mario movement and rendering data, with ROM-based texture and animation extraction. | CC0 declaration. Useful temporary integration or oracle; it does not implement the entire game for us. |
| [nickmass/libsm64-rust](https://github.com/nickmass/libsm64-rust) | Rust bindings and an example supplying collision and input and retrieving Mario's animated geometry. | MIT declaration. Calls the C library; bindings are not a full Rust translation. |
| [sm64-port/sm64-port](https://github.com/sm64-port/sm64-port) and [sm64pc/sm64ex](https://github.com/sm64pc/sm64ex) | Native platform integration; sm64ex adds remapping, camera options, external assets, and save handling. | Inspect file and inherited terms before copying; use as references and comparison builds as appropriate. |
| [MorsGames/sm64plus](https://github.com/MorsGames/sm64plus) | Camera, interpolation, control changes, and progression options. | Inspect terms before copying. Its gameplay changes make it unsuitable as an unquestioned fidelity oracle. |
| [coop-deluxe/sm64coopdx](https://github.com/coop-deluxe/sm64coopdx) | Mature gameplay integration, customization, behavior APIs, and multiplayer implementation. | Inspect terms before copying; networking is outside the first milestones. |
| [Fast-64/fast64](https://github.com/Fast-64/fast64) | Blender tooling and knowledge of N64 geometry and materials. | GPL-3.0 declaration. Useful tooling reference; treat direct code incorporation as a license decision. |
| [rt64/rt64](https://github.com/rt64/rt64) | Modern N64 rendering, graphics decoding, interpolation, and material/texture handling. | MIT declaration; C++. Its public README currently says ray tracing and the emulator plugin are unavailable in that repository. Full adoption is optional. |
| [DarioSamo/sm64rt](https://github.com/DarioSamo/sm64rt) | A demonstration of ray-traced Mario 64 lighting and shadows. | Research reference; do not assume it supplies a maintained Rust graphics backend. |

Repository declarations are starting information, not a blanket clearance of every dependency or third-party right. Preserve required notices. A Rust translation still needs the relevant source terms considered; changing language does not erase provenance. Project licenses do not license Nintendo's ROM assets.

Maintain a provenance ledger as code is brought in: upstream URL, exact commit, source paths, destination paths, license and notices, whether copied/translated/referenced, local changes, and comparison coverage. Check exact file terms, including bundled dependencies. Choose the repository's license after identifying the code actually incorporated.

### Where to begin reading

In the pinned `n64decomp/sm64` tree, start with:

- [Bob-omb Battlefield's level script](https://github.com/n64decomp/sm64/blob/master/levels/bob/script.c) for loading, act-dependent placements, behavior parameters, warps, and collision references.
- [assets.json](https://github.com/n64decomp/sm64/blob/master/assets.json) and [extract_assets.py](https://github.com/n64decomp/sm64/blob/master/extract_assets.py) for extraction metadata and existing workflows.
- [mario.c](https://github.com/n64decomp/sm64/blob/master/src/game/mario.c), its action modules in `src/game`, and the movement-step code for action transitions and motion.
- [surface_collision.c](https://github.com/n64decomp/sm64/blob/master/src/engine/surface_collision.c) for exact floor, wall, and ceiling query behavior; inspect collision loading alongside it.
- [behavior_script.c](https://github.com/n64decomp/sm64/blob/master/src/engine/behavior_script.c) for the behavior interpreter and native-function calls; follow those calls into the relevant object behavior modules.
- The camera, interaction, animation, math, and save modules as the first playable course requires them.

For ROM tooling, begin with [sm64tools' README](https://github.com/queueRAM/sm64tools/blob/master/README.md) and its `libmio0.c`, `f3d.c`, `sm64geo.c`, and `n64split.c`. Reuse the needed decoding knowledge without importing a disassembler or ROM-rebuilding system into the runtime unnecessarily.

## 9. Delivery sequence

Each milestone should leave an executable result or a meaningful tested component. Later presentation work may proceed once its prerequisites exist, but it must not delay establishing a comparison baseline.

| Stage | Deliverable and acceptance |
| --- | --- |
| M0 Foundation | Inspect the repository, choose and pin the initial stack and references, establish the ROM revision, build a minimal Rust app, and implement ROM identification plus a tested bounded reader/MIO0 path. Document working commands. |
| M1 Original level | Load and render actual Bob-omb Battlefield geometry and textures from the ROM. Inspect original collision and placements; report unsupported content. |
| M2 Faithful Mario | Run imported Mario animations with the implemented reference movement/actions, collision, and camera/input profile. Add per-tick comparisons early and test render-rate independence. Clearly list unsupported actions. |
| M3 First mission | Add the real actors and interactions needed to complete one original mission, with death/restart, HUD feedback, star collection, and saved completion. |
| M4 Complete course | Finish the six acts and 100-coin star, audio, relevant cap/cannon mechanics, mission conditions, and persistence. Test every mission and record remaining campaign prerequisites. |
| M5 Expansion proof | Load a second real course through the same importer and world interfaces. Extend behaviors and transitions without adding course logic to the application loop. |
| M6 Modern presentation | Finish optional higher-rate interpolation, widescreen, antialiasing, shadows, and camera options incrementally; prove the graphics-only settings preserve authoritative state. Early options can be implemented alongside M2 through M4. |
| M7 Campaign expansion | Add castle progression and remaining courses, secrets, bosses, caps, menus, cutscenes, endings, and save compatibility in recorded increments. Track completeness per feature, area, and act. |

Full-game completion is the direction of the architecture, not a promise attached to the first prototype. Do not defer every shared system until M7, and do not attempt every remaining system during M0.

## 10. Working agreement and living status

Before each implementation session, read this file and the repository's actual state. Choose the smallest useful next increment, implement it, run relevant checks, and update the status below. Keep README build/run instructions and the provenance ledger consistent with reality.

When a dependency choice, data representation, fidelity assumption, ROM mapping, or milestone boundary changes, record what changed and why. Replace stale decisions; do not append contradictory plans indefinitely. Use short decision records if explanations grow too long for this document. Ask the owner about material scope or fidelity changes; routine implementation choices are yours to make.

For Rust changes, run formatting, relevant tests, and appropriate build/lint checks. Add tests that detect actual parser errors, movement divergence, or gameplay failures. Keep ROM-dependent checks distinct from ordinary CI. A screenshot supports visual review but cannot establish simulation fidelity. If no ROM, comparison build, or graphics device is available, continue useful independent work and state exactly which checks could not run.

Every handoff should report the working result, commands actually run, missing features or fidelity discrepancies, and the next useful task. Do not claim a course, action family, or platform complete on the strength of a stub or an untested path.

### Current status

| Item | Status |
| --- | --- |
| Implementation | Runnable headless Rust app: ROM inspection, static BOB import/export path, synthetic diagnostics, exact trace comparison |
| Initial platform and Rust stack | Linux x86_64; Rust 1.90.0; one package with import/content/simulation/presentation/application boundaries; exact sha1/serde/serde_json dependencies and Cargo.lock |
| Supported ROM revision | US v1.0, exactly 8 MiB, normalized SHA-1 `9bef1128717f958171a4afac3ed78ee2bb4e86ce`; Z64/V64/N64 normalization; owner-ROM positive path not yet tested |
| Comparison implementation | Pinned unmodified US n64decomp/original ROM execution target at `9921382a68bb0c865e5e45eb594d9c64db59b1af`; exporter/reference build unavailable |
| Bob-omb Battlefield | Segment-7 MIO0/collision, local static placements/warps/start, and five RGBA16 texture decoding paths implemented; real-ROM integration pending; no visible terrain renderer or playable exploration |
| Fidelity coverage | Exact trace comparator tested; 300 synthetic counter/input ticks identical at 30/60/120/144 Hz; no original Mario movement/collision/camera coverage |
| Optional enhancements | Snapshot interpolation with angle wrapping/discontinuity handling; lighting/shadow settings are scaffolding, without rendered effects |
| Immediate next task | Run owner-ROM smoke/import validation, then BOB geometry/Fast3D/material decoding and renderer; establish genuine per-tick reference traces before movement fidelity claims |

### Implementation session — 2026-10-08

- **Inspection:** Repository initially had this brief, AGENTS.md, and a title-only README; no engine, ROM, or comparison setup. AGENTS.md is unchanged.
- **Actual increment:** M0 runnable headless foundation plus early M1 static-import work. Bounded readers, full-ROM identity, three byte orders, MIO0, segmented references, collision/special/environment records, five RGBA16 textures, local script calls, act masks, placements and warps. Unknown/malformed commands fail; unsupported dependencies/geometry/behaviors/callbacks are reported. No ROM executable code runs.
- **Architecture:** Typed course/level/area/act IDs, empty implementation-only behavior registry, static/dynamic collision boundaries, explicit transitions, immutable presentation snapshots, rational 30 Hz scheduler with retained catch-up backlog, and input edges consumed per tick. No approximate Mario equations or C game runtime introduced.
- **Decisions and reuse:** [docs/DECISIONS.md](docs/DECISIONS.md), [PROVENANCE.md](PROVENANCE.md), and retained MIT/CC0 notices. US offsets are centralized. wgpu/winit are selected for the future renderer boundary; exact versions will be pinned when M1 introduces them. Newly authored code uses MIT.
- **Run instructions:** [README.md](README.md) documents `demo`, `inspect-rom`, `import-bob`, `compare-traces`, build/lint/test commands, private output, and the ignored owner-ROM check. Exports have ROM/schema keys; cache reuse and saves remain unimplemented.
- **Checks:** `cargo fmt --check`, `cargo check --locked --all-targets`, 38 tests via `cargo test --locked --all-targets`, pinned Clippy-driver checks with warnings denied, `cargo build --locked --release`, and `cargo run --locked -- demo --trace private/foundation.trace.json` passed. One ROM-dependent test is ignored. CI runs authored fixtures only and never requests/uploads a ROM.
- **Environment:** Standard cargo-clippy launcher cannot resolve its executable without `/proc/self/exe` in this sandbox. The same pinned Clippy driver is invoked through `RUSTC_WORKSPACE_WRAPPER` with deny-warnings arguments. Local linking uses `RUSTFLAGS='-C linker-features=-lld'` to avoid that environment's lld-wrapper restriction; these are local environment adjustments, not engine dependencies or gameplay flags.
- **Fidelity:** [docs/FIDELITY.md](docs/FIDELITY.md) defines exact per-tick state/input metadata and first-divergence reporting. Tests include float-bit drift and signed zero. The synthetic counter is not an original-game oracle. No movement, original collision queries, camera, animations, RNG behavior, objects/interactions, or mission has passed a gameplay comparison.
- **Blocked integration:** No supplied supported ROM, matching reference build, trace exporter/emulator setup, or GPU presentation check. The owner-ROM test is documented and explicitly ignored; no actual BOB import or gameplay fidelity is claimed.
- **Next task:** Validate real BOB blobs/counts and static traversal, decode geometry layouts and Fast3D/material state (including required dependent segments), and add a Rust renderer. Capture a stationary original-game oracle trace before porting action families.

### Bob-omb Battlefield acceptance tracker

| Capability / act | Actual state |
| --- | --- |
| M0 bounded ROM foundation | Authored fixtures pass; positive owner-ROM integration pending |
| M1 imported original visible level | Static collision/placement/texture path implemented; real ROM and geometry/Fast3D/rendering pending |
| M2 playable exploration | Not implemented; no Mario movement/camera/animations or collision queries |
| Act 1 — King Bob-omb | Not implemented |
| Act 2 — Koopa the Quick | Not implemented |
| Act 3 — Shoot to the Island | Not implemented |
| Act 4 — Eight Red Coins | Not implemented |
| Act 5 — Mario Wings to the Sky | Not implemented; wing-cap campaign unlock also missing |
| Act 6 — Chain Chomp gate | Not implemented |
| 100-coin star, death/re-entry/save, audio | Not implemented |

### Session update template

- Date and change:
- Milestone and actual working result:
- References or decisions changed:
- Build/run/test commands and results:
- Fidelity coverage and known divergences:
- Missing content and blockers:
- Next useful task:

### Initial decisions

- 2026-10-08: New Rust engine with selectively adapted gameplay code and a ROM content adapter.
- 2026-10-08: Bob-omb Battlefield first; shared course/area/act/behavior/warp concepts support eventual full-game expansion.
- 2026-10-08: Original simulation cadence and measured movement fidelity take priority over visual enhancements.
- 2026-10-08: Rust 1.90.0 headless foundation, exact serde/serde_json/sha1 dependencies, pinned MIT/CC0 references and US ROM metadata, MIT authored code, and an unmodified US comparison target selected. Actual ROM/oracle checks and future wgpu/winit version selection remain pending.
