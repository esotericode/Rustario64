# Mario 64 Rust Engine Project Plan

Last updated: 2026-10-09

Status: M0 complete. M1 imported level works: BOB's original terrain and textures import from the ROM and render through an optional wgpu viewer with collision/placement overlays. M2 in progress: Mario's complete tick (inputs, non-object actions, his object update and animation frame advance, with the ROM's animations) matches the natively compiled decomp per tick, and BOB's original camera (radial, R/close, C-Up, boss-fight modes, shakes, FOV and graph camera) runs with it in the original frame order, matching the decomp word for word per frame. The viewer's Mario mode drives that frame from the keyboard and draws from the reference camera; recorded runs replay exactly against the decomp with its own camera. Mario's model and display lists import from the ROM and are posed from each tick as the original render pass does. D1 local ROM launcher and D2 development pause/settings are implemented; blink, LOD and animation/action switches preserve Mario's pose interpolation. Mario's shadow, objects, cutscenes, other areas' camera modes and triggers, cutscene/water actions, warps and missions remain missing.
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

### Desktop builds and first GUI (added 2026-10-09)

Treat Windows and Linux builds as an early playtesting requirement, alongside
M2. Native runtime build jobs and private-content-free CI bundles begin now;
compile success does not replace human GPU/driver/control testing.

| Increment | Acceptance / priority |
| --- | --- |
| D0 Native runtime builds | Linux x86_64 and Windows x86_64 compile and run authored core tests in CI; bundle only Rust runtime binaries and required notices. Check the run before calling a target verified. |
| D1 Launcher | Before wider human playtests: simple, elegant local ROM-selection window, supported-ROM validation and clear recovery errors, optional remembered path, basic presentation settings; no ROM upload. |
| D2 In-game settings | Accessible pause/settings menu; pause ticks and discard menu elapsed time on resume; clear held inputs; presentation settings separate from reference camera/input and optional gameplay modes; behavior-changing options included in replay metadata. |
| D3 Human testing | Owner-ROM smoke on Windows and Linux physical GPUs; document supported OS/drivers and control mapping. Collect reviewable input logs, without bundling game content. |

Keep the GPU-free core platform-neutral. The C oracle is development-only and
must not become a desktop runtime dependency. Saves/settings belong outside
asset caches. Gamepad input/remapping remains early M2 work. See
[docs/PLAYTEST.md](docs/PLAYTEST.md) for current bundle use and limitations.

## 8. Reference projects and reuse

References were checked on 2026-10-08. Links below identify upstream projects; pin the actual revision used before copying or deriving code. Listed uses are recommendations for this project, not promises of ready-made compatibility.

| Project | Useful material | Reuse notes |
| --- | --- | --- |
| [n64decomp/sm64](https://github.com/n64decomp/sm64) | Full decompilation, behavior definitions, formats, level definitions, extraction metadata, and original gameplay implementations. | Repository publishes CC0 terms. Primary behavioral reference; inspect the selected files and provenance. |
| [queueRAM/sm64tools](https://github.com/queueRAM/sm64tools) | ROM splitting, MIO0, Fast3D and geometry decoding, texture tools, and version configurations. | MIT declaration. Strong candidate for adapting importer algorithms; this project's `libsm64.c` is different from the libsm64 engine-integration project below. |
| [libsm64/libsm64](https://github.com/libsm64/libsm64) | A library interface for original Mario movement and rendering data, with ROM-based texture and animation extraction. | CC0 declaration. Useful temporary integration or oracle; it does not implement the entire game for us. |
| [nickmass/libsm64-rust](https://github.com/nickmass/libsm64-rust) | Rust bindings and an example supplying collision and input and retrieving Mario's animated geometry. | MIT declaration. Calls the C library; bindings are not a full Rust translation. |
| [sm64-port/sm64-port](https://github.com/sm64-port/sm64-port) and [sm64pc/sm64ex](https://github.com/sm64pc/sm64ex) | Native platform integration; sm64ex adds remapping, camera options, external assets, and save handling. | Inspect file and inherited terms before copying; use as references and comparison builds as appropriate. The `src/pc/gfx` renderer license forbids binary redistribution, so this project does not copy, translate, or derive from it. |
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
| Implementation | Headless core (ROM inspection, BOB static + visual import/export, Mario's animation table, original collision loader and queries, math utilities, Mario's physics steps, inputs, core update, non-object actions and per-frame tick, the original RNG, the reference camera for areas without camera triggers and the game frame that runs both in the original order, the play session that drives it from held controls with input logs, Mario's model import and render-pass posing, synthetic diagnostics, exact trace comparison), an optional wgpu renderer crate (offscreen PNGs, windowed inspection viewer, collision/placement overlays, Mario mode drawn from the interpolated reference camera with his skinned model, local ROM launcher and development pause/settings), and a development-only native decomp oracle (collision, math, Mario components, complete Mario ticks, camera components, complete frames with the original camera linked, recorded-run replay) |
| Initial platform and Rust stack | Linux x86_64; Rust 1.99.0 / edition 2024; Cargo workspace: `rustario64` (GPU-free core; sha1 0.11.0, serde 1.0.229, serde_json 1.0.151), `rustario64-render` (wgpu 30.0.1, winit 0.30.13, pollster 1.0.1, png 0.18.1, egui/egui-winit/egui-wgpu 0.36.2, rfd 0.17.2), and `rustario64-oracle` (dev-only; cc 1.6.0 builds vendored CC0 C). Current stable direct dependencies, compatible SemVer requirements, committed Cargo.lock; CI reads the toolchain file |
| Supported ROM revision | US v1.0, exactly 8 MiB, normalized SHA-1 `9bef1128717f958171a4afac3ed78ee2bb4e86ce`; Z64/V64/N64 normalization; supplied Z64 positive path passes |
| Comparison implementation | Target: pinned unmodified US n64decomp/original ROM execution at `9921382a68bb0c865e5e45eb594d9c64db59b1af`; no original-execution per-tick exporter yet. Native oracle: the same decomp's collision, math_util, mario.c, mario_step.c and the five non-cutscene action files (whole files) plus verbatim excerpts, compiled natively; `oracle/c/tick.c` runs complete frames of Mario's object, optionally with camera.c's update path for areas without triggers linked (verbatim excerpts, aborting stubs for unreachable modes) |
| Bob-omb Battlefield | Imported level: 1,101 visible area triangles (24 batches, 18 textures) from eight script-named dependent segments, plus the gate/seesaw/grate geo models; collision (570 vertices, 1,060 triangles), 17 specials, 30 script placements, 88 macros, seven warps. Every visible triangle and texture matches independent decomp-derived references. Renders in the viewer with collision and placement overlays. Collision loads into the ported original partition and answers queries identically to the decomp. Mario's complete ticks run identically to the decomp on it with the ROM's animations (64,158 compared ticks). Its camera runs from the area's camera node (validated callbacks) through BOB's surface rules, radial/close/C-Up/boss-fight modes and R/C-button controls identically to the decomp (77,112 compared frames). Mario can be moved around it in the viewer with his ROM model and animations, seen through the reference camera; recorded runs replay exactly. No skybox, objects, Mario shadow, cutscenes, warps or missions |
| Fidelity coverage | Component checks against the natively compiled decomp, all bitwise-identical. Collision: loader and floor/ceiling/wall/water/gas queries (857k authored comparisons in CI; 4.08M on BOB). Math: ROM trig tables, sins/coss/atan2s/atan2f/approach (3.99M). Mario steps: ground/air/stationary steps, ledge grabs, gravity, wind, moving sand, bonk, velocity helpers from generated states (124k authored in CI; 1.18M on BOB with ROM tables). Exact trace comparator tested; 300 synthetic counter/input ticks identical at 30/60/120/144 Hz. Input stage: 196,608 controller/intent cases on authored and again on ROM tables; 10,009 authored and 20,000 BOB geometry cases; 1,200 chained ticks at multiple presentation rates. **Complete Mario ticks** (Mario's object only, against the native decomp): 28,158 authored ticks (69 actions) and 64,158 BOB ticks with ROM animations (60 actions), all identical, also at 15–144 Hz presentation. **Played sessions:** 3,600 CI ticks of held-control sessions replay identically in the decomp; two recorded BOB viewer runs (233 ticks) replay identically. Camera helpers/radial goals and persistent Lakitu/transition updates have exact authored and owner-ROM native comparisons. **Complete frames with the original camera** (Mario's object plus every camera.c word, the RNG seed and the graph camera, per frame): 19,512 authored frames (45 actions; radial, close, free-roam, C-Up and boss-fight modes) and 77,112 BOB frames with ROM data (62 actions; radial, close, C-Up, boss-fight), identical, also at 15–144 Hz presentation; 3,600 CI frames of played sessions with C buttons and R; a 1,800-frame built-in BOB camera program and two windowed reference-camera recordings (904 and 124 frames) replay identically. No object, cutscene, other-area camera mode, cutscene/submerged or original-N64 coverage |
| Optional enhancements | Graphics-only options: higher resolution, 4x MSAA, culling and fog toggles, interpolation toggle, free inspection camera. Mario's skinned model and the reference camera's view interpolate between ticks at any frame rate (the view snaps across cuts). Local launcher and pause/settings offer interpolation, fog, VSync and fullscreen; optional remembered path and window size. No enhanced lighting/shadows, no optional gameplay camera |
| Immediate next task | D3 physical-GPU Windows/Linux playtests of D1/D2; then Mario's original shadow and the first BOB objects (object list processor, coins, the first mission's actors). D1 local ROM selection and D2 development pause/settings work; original pause-camera behavior (`zoom_out_if_paused_and_outside`) remains unreached. The camera's cutscene paths (star dance, death, dialog, doors) are needed with the first mission. D0 native Windows/Linux build/CI bundles passed again at 32e79e1. |

### Implementation session 1 — 2026-10-08 (M0 and early M1)

- Built the headless foundation: bounded readers, full-ROM identity, three byte orders, MIO0, segmented references, collision/special/environment records, five RGBA16 textures, local script calls, act masks, script/macro placements and warps, typed IDs, behavior registry, static/dynamic collision boundaries, rational 30 Hz scheduler, input edges, snapshot interpolation, and exact per-tick trace comparison.
- Owner-ROM checks: collision and macro records match independently expanded pinned source exactly ([docs/ROM_VALIDATION.md](docs/ROM_VALIDATION.md)). No ROM executable code runs.

### Implementation session 2 — 2026-10-08 (M1 imported level)

- **Inspection:** Continued from `codex/rom-import-foundation` (fast-forwarded onto `claude/gracious-bell-jhqg4y`). All prior checks passed before changes, including the owner-ROM test.
- **Actual increment:** Dependent segments load from the ROM ranges named by BOB's own level script (bounded MIO0/raw). New bounded geo-layout decoder reproduces original parenting, stack, branch and flag semantics. New Fast3D (F3D_OLD) interpreter converts display lists into engine-owned `VisualModel` batches: vertices, triangles, G_DL calls/branches, tiles, LOADBLOCK/LOADTILE/TLUT, combiner, geometry/othermode, lights, fog, layer render modes. Texture decoding covers RGBA16/32, IA4/8/16, I4/8, CI4/8. Native geo callbacks are reported, never executed; unsupported Fast3D commands are reported with addresses (none occur in BOB area 1).
- **Renderer:** New `rustario64-render` crate keeps the core GPU-free. It draws batches by original layer order with a generic RDP color-combiner shader, one-light Fast3D shading, `gsSPFogPosition` fog, cutout/translucent blending, decal depth bias, and imported wrap/filter modes. `rustario64-viewer screenshot` renders offscreen PNGs; `rustario64-viewer view` opens a window with a presentation-only free camera and drives the fixed 30 Hz clock (no gameplay systems yet).
- **Validation:** Every one of the 1,101 area triangles matches an independent expansion of the pinned decomp display-list source in geo order (positions, color/normal bytes, order, layer, texture address). All 18 textures match PNGs produced by the decomp's own `mio0`/`n64graphics` tools. Dependent ROM ranges match sm64tools' pinned US configuration. Model triangle counts match source. Digests are pinned in the ignored owner-ROM test.
- **Checks run:** `cargo fmt --all --check`; `cargo test --locked --workspace --all-targets` (58 core, 4 render, and 3 oracle tests; GPU tests ran on Mesa lavapipe with `RUSTARIO64_REQUIRE_GPU=1`); `cargo clippy --locked --workspace --all-targets -- -D warnings`; `cargo build --locked --workspace --release`; `demo`; owner-ROM `local_us_rom_import`; `import-bob`; `rustario64-viewer screenshot` for four views; `rustario64-viewer view --frames 90` under Xvfb; owner-ROM oracle tests `bob_collision_matches_the_decomp_on_a_dense_grid` and `rom_trig_tables_match_decomp_math_util`; viewer `--collision`/`--placements` screenshots; mutation checks of the collision harness.
- **Environment notes:** No GPU here; Mesa's lavapipe (`mesa-vulkan-drivers`) provided Vulkan, Xvfb provided X11, and `libxkbcommon-x11-0` was installed for winit. These are local test-environment installs, not engine dependencies.
- **Overlays:** Viewer collision overlay (floor/wall/ceiling tint from surface_load.c's normal thresholds) and placement markers (Mario start, script, macro, special objects). Special-object markers sit exactly in BOB's painted tree shadows.
- **Collision port (M2 groundwork):** `simulation/collision.rs` translates the original static loader and floor/ceiling/wall/water/gas queries with original widths, casts, operation order, ordering and quirks. The new dev-only `rustario64-oracle` crate compiles byte-identical CC0 decomp collision files natively and compares bit for bit: 795,000 comparisons on authored streams (CI) and 4,084,680 on BOB (owner ROM), all identical, including every surface field and partition list. Behavior mutations (tie order, 78-unit buffer, wall push sign) are caught. Dynamic/object surfaces and rooms are not ported.
- **Math utilities:** `simulation/math.rs` ports sins/coss/atan2s/atan2f/approach. The original tables load from the owner ROM (offsets located by content matching, SHA-1 verified; values never committed). Bitwise-identical to the natively compiled decomp on authored and ROM tables.
- **Oracle selection:** Audited pinned libsm64; its collision scan keeps highest floors/lowest ceilings instead of the original first match, so it is rejected as a fidelity oracle. The Mario oracle will compile unmodified decomp Mario sources natively on the verified decomp collision (DECISIONS.md).
- **Dependencies:** An initial age-based dependency freeze was superseded by deliberate stable-release updates in session 4; it is not a current requirement.
- **Not done / gaps:** Skybox (native `geo_skybox_main` callback), objects and their models from global segments (trees, coins, enemies), `geo_envfx_main` and cannon-circle callbacks, billboards/animated parts at runtime, TEXEL1 sampling, texture-gen accuracy, N64 3-point filtering, and lighting-space verification against original output. The free camera is not the original camera. No gameplay, Mario, or missions.
- **Fidelity:** Unchanged: zero validated gameplay coverage. Rendering correctness is visual, not a simulation claim.

### Implementation session 3 — 2026-10-08 (M2 groundwork: Mario physics steps)

- **Inspection:** Continued on `claude/gracious-bell-jhqg4y` from the libsm64 audit commit; the plan's next task (Mario oracle) was still open.
- **Actual increment:** `simulation/mario/` adds a `MarioState` holding only verified original fields, a `StepWorld` with the step code's globals made explicit, a `SurfaceRef` covering the water pseudo-floor, and `step.rs`: a direct translation of mario_step.c (ground, air, stationary steps; ledge grab; gravity incl. twirl and jump-ascent rules; vertical wind; moving sand; windy ground; bonk reflection; velocity helpers) with mario.c's forward-velocity, wall-resolution, ceiling and terrain-sound helpers. Constants are generated from the decomp headers through a C compiler.
- **Oracle:** The oracle now also compiles unmodified mario_step.c (plus the headers it needs) and verbatim copies of the mario.c helpers, with action setters stubbed to abort. Every vendored file was re-compared byte for byte with the pinned checkout.
- **Validation:** Each check runs one function from the same state on both sides and compares every field and the return value. Authored terrain (CI): 123,900 comparisons, all identical, reaching every reachable ground/air step result, moving sand, wind, and the water pseudo-floor. BOB collision with the ROM's trig tables (owner ROM): 1,180,000 comparisons, all identical. Mutations of the ledge probe distance, the ground quarter-step floor normal factor, the wall-angle bound, and the 160-unit ceiling-gap rule each fail CI. A CI test compares all 85 generated constants with the decomp headers.
- **Test fixes:** The oracle tests' random generator returned 31 bits, so float ranges sampled only their lower half. It is fixed in all three oracle tests; collision and math still match, and collision tests now also probe either side of the 78-unit floor/ceiling buffers (857,400 authored comparisons; the ceiling-buffer mutation was previously undetected and is now caught).
- **Checks run:** `cargo fmt --all --check`; `cargo clippy --locked --workspace --all-targets -- -D warnings`; `cargo test --locked --workspace --all-targets` with `RUSTARIO64_REQUIRE_GPU=1` under Xvfb/lavapipe (58 core, 6 oracle, 4 render tests pass); `cargo build --locked --workspace --release`; owner-ROM oracle tests `bob_collision_matches_the_decomp_on_a_dense_grid`, `rom_trig_tables_match_decomp_math_util`, and `bob_steps_match_the_decomp_with_rom_tables`; the mutation checks above.
- **Not done / gaps:** mario_update_quicksand, mario_push_off_steep_floor, and bully helpers (they set actions); no action code, input/stick processing, spawn, camera, animation, or per-tick Mario trace. The step checks start from generated states; they do not show that Mario reaches those states as the original does. Paths where the original dereferences NULL or reads past a table panic and are outside coverage.
- **Fidelity:** Component coverage only. No per-tick movement claim.

### Implementation session 4 — 2026-10-08 (tooling and focused cleanup)

- **Base:** Latest main/`claude/gracious-bell-jhqg4y` at `79d1dc1`; work on `codex/modernize-and-cleanup` preserves the renderer, collision/math ports, and Mario step oracle.
- **Tooling:** Rust/rustfmt/Clippy 1.99.0, sha1 0.11.0, serde 1.0.229, serde_json 1.0.151, cc 1.6.0, and SHA-pinned actions/checkout v7.0.1. Graphics dependencies already match current stable releases. Removed exact manifest constraints and the unsupported age-based lockfile freeze; Cargo.lock/`--locked` still pin builds. CI reads `rust-toolchain.toml`. SHA-1's new byte-array API uses explicit hex encoding; new Clippy guidance uses fixed-size slice chunks with identical iteration/order. Removed the oracle's unused SHA-1 dependency.
- **Focused fixes:** Area terrain/dialog/music metadata follows original defaults, OR/update semantics, and signed words; importer schema 4. BOB entry signature derives its range from the version adapter. Exports stage complete directories with cleanup on write failure, an OS lock for concurrent publishers, and rejection of existing files/directories/symlinks. Independently sourced collision/macro records are checked with canonical keys and the now-tracked `tools/check_bob_reference.py`. Stale statements about absent physics/oracles/overlays and obsolete version requirements are corrected throughout docs.
- **Checks:** Rust 1.99 formatting, Clippy with `-D warnings`, locked workspace/all-target tests (63 core, 6 oracle, 4 render tests), and optimized workspace build pass. Required offscreen GPU tests ran on Mesa lavapipe, not skipped. Authored oracle tests pass in both debug and release. Supplied-ROM inspect/import/schema-4 export and the ignored asset integration test pass; the independent checker also passes and rejects an altered vertex while accepting reordered JSON keys. Export tests cover failed-write cleanup/retry, existing destinations, dangling symlinks, and concurrent publishers. The synthetic 30/60/120/144 Hz demo passes. Four BOB screenshots and a combined MSAA/fog/culling/collision/placement-options screenshot render successfully at 1280×960 on software Vulkan; start/options images were visually inspected. Dependency metadata still offers permissive license choices.
- **Owner-ROM optimized comparisons:** 4,084,680 collision queries, 3,993,600 math checks, and 1,180,000 Mario step checks are bitwise-identical to the same pinned native C (9,258,280 total). The previous collision count in the docs omitted points in the existing test; updated to the observed count without changing its generator or simulation algorithms.
- **Environment/check limits:** Native tool components were installed locally from checksum-verified official archives. This restricted environment has no `/proc/self/exe`, so direct rustfmt and Clippy-driver invocations plus local compiler/runner environment wrappers provided the equivalent checks; normal commands remain in README/CI. Windowed Xvfb smoke could not run here because virtual-keyboard initialization requires `/usr/bin/xkbcomp`, unavailable in this sandbox. No physical GPU, Windows/macOS checks, original-rendered-frame comparisons, or original-execution gameplay traces were run. Offscreen rendering and component comparisons passed; those limits do not establish full gameplay fidelity.
- **Next:** Resume the existing M2 task: extend the native oracle to per-tick Mario update/input/action dispatch, then port spawn and first stationary/walking actions against those traces. Skybox and object models remain M1 presentation work.

### Implementation session 5 — 2026-10-08–09 (M2 pre-action inputs)

- **Base:** Continued from the latest remote development commit `01d6f37888e553308a1334c51b166b3a565fa29d` (`codex/modernize-and-cleanup`, PR #2), ahead of main. Rust 1.99.0 confirmed with the official stable manifest dated 2026-10-01; no version downgrade or dependency refresh.
- **Increment:** Ported `adjust_analog_stick`, connected-controller edges, the full `update_mario_inputs` stage, and floor classification/slipperiness. Preserves original f32 order, dead zones, radial clamp, wrapped yaw, squish gating, button ages, wall-query order, graphical-position floor fallback, camera flags, object-status flags and timers. Globals become explicit `InputContext`; a missing-floor death is a must-use request, not a fake implemented warp.
- **Oracle:** Seven verbatim CC0 functions from the pinned decomp, byte-checked against the checkout. Debug output is disabled and death-warp requests are recorded at an explicit boundary. Action setters still abort. The C remains development-only; the game runtime remains Rust.
- **Traces:** Schema-1 producers record every exposed Mario/controller/world field, exact float bits, initial controller history and an ordered terrain/table digest. A runnable owner-ROM `input_trace` example imports actual BOB content and exports both traces privately. Raw `MARIO_POS` degrees convert using the original level-script formula. Initial Mario state is a fixture, not `init_mario` or an original spawn warp.
- **Evidence before workspace reset:** 196,608 exhaustive controller/button/intent comparisons on authored tables and again on ROM tables; 10,009 authored geometry/threshold cases; 20,000 BOB states. A 1,200-tick input-stage sequence matched native C at 15/30/60/120/144 Hz with presentation settings toggled on authored terrain and at 30/60/120/144 Hz on BOB. Owner-ROM example export and exact trace CLI comparison passed. This is input-stage component coverage, not full movement fidelity.
- **Recovery:** The execution workspace reset during the session, removing the unpushed checkout and ROM mount. Reconstructed this increment from session history, re-extracted the same seven reference functions (same hashes), checkpointed to GitHub, and reran authored checks. Fresh owner-ROM checks on the final reconstructed commit require the ROM to be supplied again. No ROM, decoded assets or private traces were published.
- **Fresh recovered-source checks:** Workspace/all-target tests pass (77 tests, five owner-ROM cases explicitly ignored); required offscreen GPU tests pass. All ten authored oracle tests also pass in release, including exhaustive input and chained trace comparisons. Direct rustfmt and Clippy workspace checks pass with warnings denied. The dead-zone offset, camera-yaw sign, and strict off-floor threshold mutations each fail the intended test; original source is restored and the input suite passes. Local `/proc` limitations require direct rustfmt/Clippy and the system BFD linker; repository build settings are unchanged.
- **Local build limit:** The full workspace release build encountered zero-length object files while archiving `wgpu-hal`/`wgpu-core`, including on retry. No graphics or compiler settings were changed in the repository to work around it. The branch's standard GitHub CI includes the full release build; use that run for its result. Optimized oracle tests and the release foundation demo pass locally.
- **Missing:** Spawn initialization, action dispatch/actions, reference camera logic, animation, objects, warp execution and original-N64 traces. Dynamic surface loading and its squish-input branch remain unvalidated.
- **Next:** Add spawn/action initialization plus stationary/walking/stopping actions, including animation state used by transitions, and extend the trace to complete Mario ticks before viewer integration.

### Implementation session 6 — 2026-10-09 (M2 complete Mario ticks)

- **Base:** Continued from the latest code: main after PR #3 (`ba4c303`, the
  pre-action input stage), on `claude/gifted-goldberg-5bz4sb`. The Rust 1.99.0
  update was rechecked first: formatting, Clippy, all workspace tests, the
  release build and every owner-ROM test pass; session 5's local release-build
  failure did not reproduce.
- **Animation import:** Mario's 209-entry DMA animation table loads from the ROM
  into engine-owned records with bounded frame lookups. A new checker rebuilds the
  table from the pinned `assets/anims` sources and matches the ROM byte for byte.
- **Oracle on original headers:** The oracle now compiles the real vendored
  headers and whole `mario.c` and action files, with excerpts generated by a
  tracked tool and per-item hashes, recorded boundaries for sound, camera and
  warps, and aborting stubs for objects. Constants (2,246) are generated from the
  headers by a tracked tool and checked against the compiled values.
- **Mario port:** `simulation/mario` now translates mario.c (action setters and
  transitions, health, caps, `execute_mario_action`, `init_mario`), the stationary,
  moving, airborne, object (punching) and automatic (hanging, ledges) action
  files, interaction.c's floor and wall hooks, the animation helpers with the
  render pass's authoritative frame advance, and the per-frame update of Mario's
  object. Globals are `StepWorld` fields; sound/camera/warp calls are recorded
  events; object paths panic. See DECISIONS.md.
- **Full-tick oracle:** `oracle/c/tick.c` runs complete frames of the decomp's
  Mario code with its own persistent state; both sides report 260 named words
  per frame plus three per event. CI: 15 scripted move sets and 24 fuzzed runs
  on an authored playground, 28,158 ticks identical, 69 distinct actions, 10,092 events, plus a
  presentation check at 15/30/60/120/144 Hz with interpolation toggled. Owner
  ROM: 64,158 BOB ticks identical with the ROM's animations (60 actions, 19,048
  events) and a 30/60/144 Hz check; a private `tick_trace` export compares
  exactly through the CLI. Three deliberate translation mutations each failed at
  the first affected tick.
- **Bugs found:** The harness exposed implicit C declarations in two excerpts
  (`vec3f_set`, `absf`) that silently corrupted float arguments under `-w`; the
  includes are fixed and implicit declarations are now build errors. Authored
  ramps initially sorted behind the field under them (the original's first-vertex
  floor ordering), reproduced identically on both sides; the fixture now winds
  ramps high edge first.
- **Checks run:** `cargo fmt --all --check`; `cargo clippy --locked --workspace
  --all-targets -- -D warnings`; `cargo test --locked --workspace --all-targets`;
  all ignored owner-ROM tests in release (import, animations, collision, math,
  steps, input stage, full ticks); the `tick_trace` export and CLI comparison;
  the excerpt checker and byte comparison of all 65 vendored files; the animation
  reference checker; `cargo build --locked --workspace --release`.
- **Not done / gaps:** Objects (and every action that holds, rides or uses one),
  the reference camera (yaw and mode are inputs), cutscene and submerged action
  groups (recorded as unsupported), warp execution and the level runtime,
  particles, the painting spawn, viewer integration, and original-N64 traces.
- **Next:** Drive and draw Mario in the viewer from the compared tick, then the
  reference camera.

### Implementation session 7 — 2026-10-09 (M2 viewer play)

- **Base:** Continued on `claude/gifted-goldberg-5bz4sb` from session 6's
  pushed commit (`578d263`).
- **Shared level entry:** `tick::LevelEntry` and `enter_level` moved into the
  core; `LevelEntry::script_start` builds BOB's entry from the import (script
  start, area terrain type, GEO_CAMERA mode), and the oracle's native setup
  converts from the same value.
- **Play session:** `rustario64::play` (GPU-free) maps held controls to
  `TickInput` (raw stick bytes past the original dead zone, walk modifier,
  A/B/Z), supplies the camera yaw from a follow camera that turns once per tick
  after Mario's update, keeps each tick's input, interpolates Mario's pose from
  completed snapshots, and stops on unsupported paths, unsupported action groups
  and warp requests. `trace::InputLog` stores a run.
- **Viewer:** `view --mario` (or M) drives Mario from the keyboard on the fixed
  30 Hz clock with a follow camera and a hitbox-sized placeholder box, R
  re-enters, M returns to the free camera; taps shorter than a tick are latched
  for the next tick; `--record DIR` writes one input log per run;
  `screenshot --mario-ticks N` renders Mario offscreen. The renderer gained a
  per-model translation/yaw transform.
- **Replay:** `tick_trace --inputs` replays a recorded run in the native decomp
  and compares every tick.
- **Checks run:** `cargo fmt --all --check`; `cargo clippy --locked --workspace
  --all-targets -- -D warnings`; `cargo test --locked --workspace --all-targets`
  (new: 5 core play tests, 2 render play tests, the GPU transform test, and the
  oracle's played-session replay (3,600 ticks) and entry-conversion tests);
  `RUSTARIO64_REQUIRE_GPU=1` render tests on lavapipe; owner-ROM `bob_ticks`
  (64,158 ticks, now also checking the viewer entry);
  `cargo build --locked --workspace --release`; viewer screenshots at 0 and 90
  ticks (visually inspected: Mario at the start, then climbing out of the open
  cannon hole); a windowed Mario-mode session under Xvfb driven by xdotool key
  events (run, jump, camera turn, reset, dive) with two recorded runs (154 and
  79 ticks) replayed exactly by `tick_trace --inputs` and the CLI comparator;
  the built-in 1,800-tick `tick_trace` program.
- **Environment:** `libxkbcommon-x11-0` and `xdotool` were installed locally for
  the windowed check (not engine dependencies).
- **Mario's model (same session):** `import::mario` reads the main level
  scripts' group0 loads and MODEL_MARIO's layout from the ROM (ranges checked
  against sm64tools) and resolves Mario's 13 native geo callbacks through
  addresses that tools/check_mario_model_reference.py locates by walking
  `mario_geo` alongside the pinned geo.inc.c (2,322 commands, 122 display lists
  at their commented addresses). `presentation::mario` follows the render pass:
  object placement (or the floor-alignment matrix the tick's render stage now
  reports), animated parts from the tick's animation, Mario's switch, rotation
  and scale callbacks from his body state (without their body-state writes),
  and the original's levels of detail. Display lists are built once per
  distinct draw list with each vertex's bone, skinned per tick and
  interpolated per frame; the viewer and `screenshot --mario-ticks` draw it.
  The geo decoder's switch parameter is now named `num_cases` (it is stored as
  numCases; switches start at case 0), and the static model builder draws case
  0 accordingly; BOB's import digests are unchanged.
- **Model checks:** six configurations (all four bodies, three levels of
  detail, eyes, hands, cap, wings) match triangle counts and vertex digests the
  checker rebuilds from the pinned source; authored-skeleton CI tests check
  selection, animation translation modes, matrix order, normals and
  interpolation; a 90-tick played session poses bounded models; screenshots and
  Xvfb window captures (idle, run, jump) were inspected.
- **Not done / gaps:** the original camera, Mario's shadow, objects (the cannon
  lid is an object, so its hole is open), cutscene and water actions, warps
  (play stops at them), gamepad input, sound; the callbacks' render-pass writes
  into the body state are approximated in presentation (DECISIONS.md).
- **Next:** Port the reference camera with per-tick comparisons; Mario's shadow.

### Implementation session 8 — 2026-10-09 (camera foundation and desktop builds)

- **Base:** Latest remote main and `claude/gifted-goldberg-5bz4sb`,
  `15368f8dd944fa7648f54451e08fff9a801f446d` (Mario ROM model); work on
  `codex/reference-camera-foundation`. No older development branch reused.
- **Camera increment:** 33 original camera.c functions translated into the
  GPU-free `simulation::camera`: approaches, button precedence, geometry/angle
  helpers, strict trigger bounds, camera collisions and radial goal construction.
  Globals and collision flags are explicit; the complete camera update remains
  pending. Generated constants now include camera status flags and area IDs.
- **Native reference:** Generated verbatim camera excerpts from the same pinned
  CC0 source, with hashes and original headers. Differential cases cover integer
  promotions/wrap, float bits, vector aliases, filtered contact types/heights,
  wall reuse, water and radial goals; persistent helper sequences run at
  15/30/60/120/144 Hz with interpolation on/off.
- **Source boundary found:** With an intangible floor and no floor beneath it,
  the collision query retains a height but returns NULL; camera.c dereferences
  that contact. The Rust camera panics explicitly. Random valid fixtures include
  a lower floor. BOB's named trigger table is unused and remains disabled.
- **Desktop:** Native Windows/Linux runtime build/test jobs plus ZIP packaging
  with an explicit allowlist and dependency notices. The C oracle and private
  content are excluded. Recorded D1 ROM-selection GUI and D2 pause/settings
  requirements before broader playtesting; neither GUI is implemented yet.
- **Checks:** Workspace/all-target tests pass on authored fixtures; updated camera
  suite passes eight tests in debug and release (one owner-ROM test ignored).
  Required offscreen GPU tests pass. Direct rustfmt and warnings-denied Clippy
  workspace checks pass; release runtime binaries and the runnable demo pass.
  Verbatim excerpt regeneration matches the clean pinned source. Inclusive-trigger
  and first-wall mutations each fail the intended comparison; source restored.
  Three packaging checks pass; locked Linux/Windows runtime notice graphs resolve,
  and an actual Linux ZIP builds with only the allowlisted files.
- **Blocked then:** No ROM attachment or original-execution comparison setup in that
  workspace. Owner-ROM camera integration and visual play smoke not run here.
  Windows and physical-GPU testing depend on CI/human checks.
- **Next:** Port complete BOB camera mode/transition/Lakitu state and compare each
  camera-and-Mario tick; then integrate it into the viewer. In parallel with M2
  milestones, deliver D1/D2 before wider human testing; then Mario's shadow and
  first-mission objects.

### Implementation session 9 — 2026-10-09 (owner ROM and persistent Lakitu ticks)

- **Base:** Continued latest `codex/reference-camera-foundation` at
  `be65225a94aac20df2778cf4624a87ccfb5f8176`; main is still at 15368f8.
  The same draft PR remains the review point.
- **ROM:** The newly attached 8 MiB Z64 validates as supported US v1.0. Existing
  owner-ROM import, model/animation, collision, math, input, Mario-step and
  complete-Mario-tick checks pass. Animation/model source-rebuild checks pass.
  Fresh 0/90-tick BOB screenshots render on llvmpipe; the 90-tick image was
  inspected. No ROM, extracted content, screenshots or state records are staged.
- **Increment:** `simulation::camera::lakitu` translates the persistent
  `update_lakitu`/`next_lakitu_state` stage, level-oriented transitions and
  deterministic hit/shake/FOV setup. Explicit mode-controller goals precede the
  stage; the outer `last_frame_action` assignment remains afterward. Source
  float-angle conversion, phase order, smoothing, floor/filter quirks and the
  paused final clamp are preserved. Active RNG requests return an unsupported
  result before mutation. No runtime dependency added.
- **Comparison:** 94 modeled state words plus collision flags after every tick;
  verbatim pinned C functions run with independent persistent state. 40,000
  authored and 80,000 BOB randomized updates pass; 36,000 persistent ticks per
  terrain pass at 15/30/60/120/144 Hz with interpolation on/off. Transition and
  damage-priority tests pass. Shared layout generation only transports fields.
  Mario's existing camera-event boundary stays unchanged in its separate oracle.
- **Desktop evidence:** All jobs at be65225 passed, including Windows/Linux
  runtime tests, native release builds and artifact upload:
  [workflow run 37959530642](https://github.com/esotericode/Rustario64/actions/runs/37959530642).
  These establish compile/package support, not human physical-GPU/control tests.
- **Checks:** Release workspace/all-target suite with every ignored check enabled
  and offscreen GPU required passes: 121 tests, zero failures/ignored. New stage
  tests pass in debug/release, with BOB integration in release. Warnings-denied
  Clippy and direct rustfmt checks pass. Verbatim excerpt regeneration and the
  generated record layout match. Truncating the shake increment first and
  clearing the camera-floor flag each fail their intended test; both reverted,
  and restored authored/owner-ROM camera suites pass.
- **Still missing:** Full update_camera dispatch, BOB initialization and radial/
  free-roam controllers, obstruction rotation and surface mode selection;
  original-execution traces/RNG; GUI/settings, shadow, objects and missions.
  The viewer continues using its approximate follow camera.
- **Next:** Port wall-avoidance/rotation and BOB mode controllers/init, compare
  complete camera-and-Mario ticks, then replace the viewer camera. D1 ROM-picker
  and D2 pause/settings remain priorities before wider human playtesting.

### Implementation session 10 — 2026-10-09 (camera obstruction and radial movement)

- **Base:** Continued the latest remote `codex/reference-camera-foundation` at
  `0a627845fa540dc89a8e83a435263d0d6c3296ef`; the existing draft PR is the
  review point. Main remains 15368f8.
- **ROM:** The new private attachment validates as the same supported 8 MiB
  US v1.0 ROM. Full owner-ROM checks pass; no private content is tracked.
- **Increment:** Original eight-probe wall obstruction scan, vertex/sector
  helpers, radial surface/first/second rotation, outward offsets and zoom.
  Preserve integer cross products, strict height bounds, last-wall selection,
  coarse/fine query order, conflicting flags, signed narrowing and promoted
  angle comparisons. Shared globals remain in Rig; area center and second-turn
  flags live in RadialMovement. No runtime dependency added.
- **Comparison:** Nine additional verbatim pinned native functions. 655,360
  angle cases; 20,000 randomized vertex/bounds cases and targeted boundaries;
  40,000 authored/80,000 BOB obstruction scans; 20,000 authored/40,000 BOB
  rotation/offset/zoom states. Persistent radial movement → zoom → radial goals
  → Lakitu compositions compare every shared word after each stage for 18,000
  ticks per terrain, at 15–144 Hz with interpolation off/on. Native state is
  never repaired from Rust output. Mario paths/floor inputs remain authored.
- **Checks:** Full release workspace/all-target suite, every ignored test
  enabled with the ROM and offscreen GPU required: 129 passed, zero failures or
  ignored. New authored suite passes in debug and release. Warnings-denied
  Clippy, formatting, excerpt/layout regeneration and packaging checks pass.
  Native release runtime binaries and the headless demo run.
  Selecting the first wall, using an inclusive low-wall cutoff, or narrowing
  the promoted radial angle condition each fails its intended comparison;
  all three mutations are reverted and the restored stage checks pass.
- **Desktop evidence:** Latest preceding Windows/Linux test/build/package jobs
  and artifacts passed at 0a62784:
  [workflow 37963513367](https://github.com/esotericode/Rustario64/actions/runs/37963513367).
  These remain compile/package evidence; physical GPU/human control checks need
  testers. Updated CI also runs optimized radial comparisons.
- **Still missing:** Complete radial input/height/pan, free-roam controller,
  surface-mode selection, BOB initialization and full camera dispatch;
  combined Mario/camera comparisons and original-execution traces. The viewer
  continues using its approximate follow camera. GUI/settings, gamepad,
  shadows, objects and missions remain pending.
- **Next:** Finish the remaining BOB mode logic and init, compare combined ticks
  before viewer integration. D1 ROM-picker and D2 pause/settings remain early
  priorities before wider human playtesting; then shadow and first-mission actors.

### Implementation session 11 — 2026-10-09 (small look-ahead pan increment)

- **Base:** Continued latest remote camera branch at `1d5030b`, keeping this
  session limited to one original function: `pan_ahead_of_player`.
- **Result:** Rig owns the original persistent focus-pan update. Preserve the
  two rotations and operation order, long-jump/pole reversal (except the top),
  sleeping decay and the fixed 0.025 approach per tick, including snap mode.
  Camera eye and authoritative yaw remain unchanged. No new dependency.
- **Comparison:** Verbatim pinned CC0 native excerpt; 40,000 authored cases
  and 40,000 with ROM trig tables. Coincident/vertical eyes, signed angles,
  signed-zero inputs and smooth/snap/sleeping combinations are covered. Pan
  is now compared between radial goals and Lakitu in the existing 18,000-tick
  stage compositions per terrain at 15–144 Hz with interpolation off/on.
- **Checks:** Optimized radial/Lakitu suites including owner-ROM checks,
  debug authored pan, warnings-denied workspace lint, formatting and generated
  excerpt/layout checks. ROM and assets remain private.
- **Not done / next:** Viewer still uses its approximate follow camera. Next
  small camera step: original height adjustment; then mode input, free roam,
  initialization and combined Mario/camera comparisons before integration.
  ROM-picker/settings GUI remain early usability priorities.

### Implementation session 12 — 2026-10-09 (complete BOB reference camera)

- **Base:** Main at 7cde5a0 (session 11 merged); work on
  `claude/keen-maxwell-bqppaj`. The owner's supplied ROM validates as the
  supported US v1.0; it and every trace stay under ignored `private/` paths.
- **Increment:** `simulation::camera::system` translates camera.c's frame for
  areas without camera triggers: the area's camera creation and level entry
  (`create_camera`, `select_mario_cam_mode`, `reset_camera`, `init_camera`),
  `update_camera`, BOB's course processing and surface rules, the radial mode
  (C-button input, height, pan), R/Mario (close) and Lakitu free-roam modes,
  the boss-fight mode, C-Up (enter, head look, exit search), mode transitions,
  cutscene detection (reported, not run), the HUD camera status, and the render
  pass's FOV and graph-camera callbacks. `simulation::rng` is the original
  shared generator, used by the handheld and shock shakes. `simulation::game`
  runs one original frame: Mario's update, his camera requests in call order,
  `update_camera`, then the render pass's camera and animation stages. Modes,
  cutscenes and trigger levels the port lacks end play with a typed
  `Unsupported` after the frame.
- **Viewer:** `play::Session` now runs the game frame. Mario reads the yaw
  the original camera produced last frame; the viewer draws from Lakitu's
  position, focus, roll and field of view, interpolated between frames and
  snapped across cuts. Arrow keys are the C buttons and E is R. The follow
  camera is gone. Input logs are schema 2 with a `camera` field. The area
  camera node's callbacks are imported and checked against the version
  adapter's `geo_camera_main`/`geo_camera_fov` addresses.
- **Comparison:** The oracle compiles the same path from verbatim pinned
  camera.c and behavior_script.c excerpts (no cutscene, spline or trigger
  data; aborting stubs for unreachable modes) and links it into the tick
  harness. Per frame, Mario's words plus every camera word match: 19,512
  authored frames (45 actions, five camera modes), 77,112 BOB frames with ROM
  data (62 actions; radial, close, C-Up, boss-fight), 3,600 played-session
  frames with C buttons and R, the 1,800-frame `tick_trace --camera` program,
  and two windowed reference-camera recordings (904 and 124 frames). Frames
  are identical at 15–144 Hz presentation. Ten of 14 seeded one-line
  mutations fail the authored suite (the floor-scan step, handheld spline and
  C-Up exit search only after strengthening its scenarios); the four that
  survive are not distinguished by the reached states (docs/FIDELITY.md).
- **Checks:** `cargo fmt --all --check`; warnings-denied Clippy on all
  targets; full release workspace suite with every ignored test enabled, the
  ROM attached and an offscreen GPU required: 133 passed, none failed or
  ignored; excerpt and layout regeneration checks against a clean pinned
  checkout (all 67 vendored files byte-identical); windowed viewer smoke under
  Xvfb with synthetic keys (C buttons, R, C-Up) and recordings replayed. CI
  (headless, Windows and Linux runtime builds and packages) passed at c2c8416
  and 32e79e1
  ([workflow 37990489308](https://github.com/esotericode/Rustario64/actions/runs/37990489308));
  the optimized CI camera step now also runs `camera_tick`.
- **Still missing:** Cutscenes (star, death, dialog, doors, intro), the
  camera modes and triggers of other areas, water camera, pause/menus,
  Mario's shadow, objects, warps, missions and original-N64 traces.
- **Next:** D1 ROM launcher and D2 pause/settings for playtesting, then the
  shadow and the first BOB objects.

### Implementation session 13 — 2026-10-09 (animation continuity and desktop UI)

- **Base:** Latest remote main `8598367`; work on
  `codex/animation-continuity-desktop`, continuing session 12 rather than an older branch.
- **Animation report:** Found that each blink draw-list switch disabled the
  entire skinned model's interpolation. The new mesh now draws from both poses,
  with the previous skeleton evaluated under current switches/LOD. Authored
  material/geometry regressions and a 256-frame owner-ROM blink/LOD regression
  cover 32 switches. Animation changes initially snapped (superseded by session
  14); level re-entry still snaps. Authoritative
  animation advancement remains at 30 Hz.
- **D1:** Local ROM-selection window (Browse, path entry or drag/drop), supported
  identity validation/import before play, recoverable errors, opt-in remembered
  path, basic graphics and window-size settings. One winit loop/window/device
  transitions into BOB. Settings live outside content caches. CLI diagnostics
  and private input recording remain available.
- **D2:** Esc pause/settings and explicit resume after focus loss. Held controls,
  taps, elapsed menu/focus/inspection time and backlog are cleared. Interpolation,
  fog, VSync and borderless fullscreen are presentation-only. Snapshot history
  snaps without changing game state; the held-control replay suite checks it
  every 17 frames. This freezes the complete frame; original pause-camera behavior
  remains unimplemented.
- **Dependencies/notices:** Optional render crate adds egui integrations 0.36.2
  (same wgpu 30) and rfd 0.17.2; lockfile and runtime notices include all UI font
  licenses. GPU-free core and the development-only native oracle boundaries remain.
- **Checks:** 127 optimized core/oracle tests with every owner-ROM test enabled
  pass, plus 10 desktop/render tests with offscreen GPU required (137 total,
  zero failures or ignored). The ROM validates as supported US v1.0. Disabling
  the mesh-switch re-skinning fails the regression at the first blink; restored
  owner-ROM tests pass. Direct rustfmt and warnings-denied Clippy-driver workspace
  checks pass; generated camera layout and three packaging checks pass. Both
  native desktop dependency notice graphs resolve, including font licenses.
  Windowed launcher, invalid-ROM recovery, pause/resume, input clearing and live
  interpolation/fog/VSync changes pass under Xvfb; launcher/game close cleanly.
  A fresh 68-frame recorded ROM-backed play run replays every Mario/camera word
  exactly against native C. Linux checks use Xvfb and software Vulkan, not a
  physical GPU; no human Windows/GPU validation is claimed.
- **Still missing:** Physical-GPU/human Windows/Linux checks, input remapping and
  gamepad, original pause behavior, shadow, objects, cutscenes, warps, missions,
  audio, saves and original-N64 execution traces.
- **Next:** D3 playtesting; Mario's original shadow, then object-list processing
  and the first BOB actors with independent per-tick comparisons.

### Session 14 — animation changes retain their intermediate poses (2026-10-09)

- **Report/reproduction:** Landing, resuming running and other action changes
  still stuttered after the blink fix. The remaining clip-ID equality guard
  snapped the complete model, including its origin, while the camera kept
  interpolating. An owner-ROM script reproduces 17 affected clip changes,
  including five landing transitions, across three 180-frame BOB sessions.
- **Change:** Interpolate the completed model endpoints regardless of clip ID
  when they share the same geometry and epoch. The mesh-switch path continues
  to pose today's geometry under the earlier skeleton. No extra simulation
  ticks, animation advancement, delayed crossfade or input changes are added.
- **Regression:** Authored different-clip translations/joint poses and combined
  clip/material/geometry changes check endpoints and intermediate frames. The
  540-frame owner-ROM test covers landing → run, landing → turn and landing →
  stop/restart at five render fractions. Both tests fail against the previous
  guard and pass with the fix. Reset and interpolation-off behavior remain.
- **Checks:** 128 optimized core/oracle tests with all owner-ROM checks enabled
  and 10 desktop/render tests with offscreen GPU required pass (138 total).
  All 540 recorded transition-script frames replay every Mario and camera word
  exactly against native C. Warnings-denied Clippy-driver workspace checks and
  formatting pass. Linux runtime checks use Xvfb/software Vulkan; human hardware
  playtesting remains necessary.
- **Next:** Hardware playtesting of transitions at 60 Hz and higher, then the
  previously planned Mario shadow and BOB actors. Other milestone gaps remain.

### Bob-omb Battlefield acceptance tracker

| Capability / act | Actual state |
| --- | --- |
| M0 bounded ROM foundation | Complete: authored fixtures and positive owner-ROM integration pass |
| M1 imported original visible level | Met for terrain: original terrain and textures import and render from the ROM (independently validated); collision inspectable in the viewer overlay and as OBJ; placements inspectable. Skybox and object models are presentation gaps |
| M2 playable exploration | Mostly met: Mario's complete tick (inputs, non-object actions, object update, animations from the ROM) and BOB's reference camera match the native decomp per frame on BOB, and Mario runs, jumps and climbs through the original level in the viewer seen through the original camera (C buttons, R, C-Up); recorded runs replay exactly. Mario is drawn with his ROM model and animations, posed as the original render pass does. Not yet: Mario's shadow, objects, cutscene/water actions, camera cutscenes |
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
- 2026-10-08: Rust headless foundation, pinned MIT/CC0 references and US ROM metadata, MIT authored code, and an unmodified US comparison target selected. Toolchain/dependency policy updated to Rust 1.99.0 with compatible requirements and a committed lockfile (session 4).
- 2026-10-08: Bounded modern macro placement decoding and independent source-derived collision/macro digests. Import data does not apply unimplemented gameplay defaults or respawn rules.
- 2026-10-08: Workspace split: GPU-free core crate plus optional `rustario64-render` (wgpu 30.0.1 + winit 0.30.13). Fast3D is interpreted at import into engine-owned meshes rather than emulated at draw time. Importer schema 3. sm64-port's gfx code is excluded for license reasons. See [docs/DECISIONS.md](docs/DECISIONS.md).
- 2026-10-08: Original trig tables load from the ROM at verified offsets; no table values are committed.
- 2026-10-08: Collision is a direct translation of the decomp's loader/queries with globals made explicit. A development-only native decomp oracle (vendored CC0 C, never linked into the runtime) provides bitwise component checks until original-execution traces exist.
- 2026-10-08: libsm64 rejected as a fidelity oracle (changed collision ordering); the Mario oracle compiles unmodified decomp Mario sources natively.
- 2026-10-08: Mario's physics steps are a direct translation of mario_step.c. `MarioState` holds only fields verified so far (superseded 2026-10-09: it now mirrors the whole struct); globals live in `StepWorld`; original NULL dereferences and out-of-table reads panic rather than invent values. See [docs/DECISIONS.md](docs/DECISIONS.md).
- 2026-10-08: Importer schema 4 preserves area terrain/dialog/music metadata. Exports stage all writes before publishing with error cleanup and concurrent-export locking. Collision/macro content expectations use canonical JSON keys and a reproducible pinned-source checker rather than struct serialization order.
- 2026-10-09: Mario's model is imported from the main scripts' group0 segments and drawn by a presentation translation of the render pass and his geo callbacks: builds per draw list, CPU skinning per tick, interpolation per frame; callbacks resolve through a version-adapter address table located by a reproducible checker. See [docs/DECISIONS.md](docs/DECISIONS.md).
- 2026-10-09: The viewer drives Mario through the core's `play` session: same level entry and tick as the oracle, a follow camera's yaw as a logged tick input (superseded by the reference camera in session 12), taps latched to the next tick, play stopping at unsupported paths and warps, and runs recorded as input logs that replay against the decomp. See [docs/DECISIONS.md](docs/DECISIONS.md).
- 2026-10-09: Mario's animations load from the ROM; the oracle compiles the real decomp headers and whole Mario sources; Mario's update, non-object actions and per-frame tick are translated with globals in `StepWorld`, outside calls as recorded events, and object paths panicking; the render pass's animation frame advance is a simulation stage. Complete ticks compare against the native decomp by named words. See [docs/DECISIONS.md](docs/DECISIONS.md).
- 2026-10-09: The reference camera is a direct translation of camera.c's frame for areas without triggers, with camera.c's globals in one `CameraSystem` and the RNG seed owned by the game frame. Mario's camera requests apply after his update in call order (his own mode reads mirror them), unsupported modes/cutscenes/trigger levels end play after the frame, and the viewer draws from the authoritative camera instead of a follow camera. No cutscene, spline or trigger data is copied. See [docs/DECISIONS.md](docs/DECISIONS.md).
