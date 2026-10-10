# Comparison contract and coverage

Mario's movement now has **per-tick coverage against the natively compiled
decomp**: complete frames of Mario alone (input stage, every non-object action
group, Mario's object update and the animation frame advance) match bit for bit
on an authored playground and on Bob-omb Battlefield with the owner ROM's data.
That is not yet coverage against original N64 execution. Since session 17
the object system (pool, lists, behavior scripts, object collision) with the
coin behaviors and coin collection also has per-frame coverage (Objects and
coins below); every other object behavior, interactions with other objects
and cutscene/submerged actions have **zero validated coverage**. Collision, math, physics steps and
pre-action inputs also keep their component suites. Camera helpers and radial
goal construction have native component checks; the persistent Lakitu/transition
stage also has per-tick native comparisons (Camera sections below). The viewer's Mario mode
runs the same tick, and its recorded runs replay exactly in the decomp (Played
sessions below). Mario's drawn model is presentation: it reads completed ticks
and is checked as imported content (ROM_VALIDATION.md), not compared per tick;
the render pass's own writes into Mario's body state (torso/head angle resets,
the punch-scale countdown) are not simulated on either side. The
`demo` command's diagnostic marker visualizes a tick counter; it is not a Mario
approximation.

Target US v1.0 at n64decomp/sm64 revision
9921382a68bb0c865e5e45eb594d9c64db59b1af, ROM SHA-1
9bef1128717f958171a4afac3ed78ee2bb4e86ce, 30 ticks/second, reference camera/input
profile. Prefer unmodified matching N64/original execution; native-port float
differences need separate evidence. Physics-modified ports are not unquestioned
oracles. Static asset checks and every owner-ROM comparison below were rerun
with the supplied ROM on 2026-10-09 (see [ROM_VALIDATION.md](ROM_VALIDATION.md)).
A matching N64 build, emulator trace setup and original-execution exporter are
still unavailable, so comparisons against original execution have not run.

## Schema 1

src/trace.rs defines the required JSON fields:

| Part | Data |
| --- | --- |
| Envelope | Schema 1, producer, metadata, nonempty consecutive frames |
| Metadata | ROM/revision/configuration, scenario, 30 Hz cadence, course/area/act, initial world identity, gameplay options, initial state |
| Tick input | Buttons, signed stick bytes, authoritative camera yaw consumed for movement |
| Tick state | Action/state/timer, position/velocity bits, face angles, camera yaw, RNG, contacts, interaction outcomes, ordered objects, extra fields |
| Object | Stable ID/behavior ID, action/timer, position/velocity bits, extra fields in original update order |

Initial state is before tick 1. Frame N records input consumed at tick N and
complete state immediately after that tick. Reference and Rust advance once with
identical input. Original stick normalization and button edges are ported and compared below;
initial controller history is included in input-stage traces.

Authoritative floats serialize with f32::to_bits() into u32 values. Discrete
state and bit patterns compare exactly, including signed zero. No tolerance,
rounding, promotion, sorting of objects, or dropped contact/event ordering.
Stable IDs must map to the same original surfaces/objects in both producers.
Add required hidden fields as action families need them; unrecorded state is a
coverage gap even if recorded fields match.

The comparator checks initial conditions, then the first differing tick/field.
Producer names may differ. Missing/unknown fields, empty traces, wrong reference
identity/cadence, duplicate per-tick object IDs, and tick gaps fail. The CLI caps
files at 64 MiB; longer traces need a future streaming format. Real producers
must establish a canonical initial-world digest; the synthetic replay uses an
explicit authored-world label.

## Checks today

```sh
cargo run --locked -- demo --trace private/foundation.trace.json
cargo run --locked -- compare-traces private/foundation.trace.json private/foundation.trace.json
cargo test --locked --test timing_and_traces
```

Tests cover 300 synthetic counter/input ticks at 30/60/120/144 Hz and toggle
interpolation/graphics flags. They test wrapped angles, discontinuities,
retained backlog, clock drift, one-bit float changes, signed zero, metadata
differences, ordered objects, and invalid traces. The optional wgpu viewer
renders imported BOB terrain and, in Mario mode, runs Mario's compared tick on
the same fixed 30 Hz clock; real gameplay's render-rate independence is covered
by the full-tick presentation checks below. Enhanced lighting/shadows are not
implemented; the renderer's options (MSAA, fog/culling toggles, interpolation,
free and follow camera placement) live in the render crate or presentation and
have no path into simulation state.

The ignored owner-ROM test is documented in README. It checks original collision
and macro records against canonical source-derived digests, script counts/area
metadata, and independently derived visible-triangle and texture digests. The
collision/macro checker is reproducible (ROM_VALIDATION.md). These asset checks
do not validate final rendered images, all assets, or gameplay.

## Collision component coverage

`rustario64-oracle` compiles the pinned decomp's surface_load.c and
surface_collision.c natively (`-fwrapv`, `-ffp-contract=off`) and compares the
Rust port bit for bit:

| Check | Authored streams (CI) | BOB, owner ROM (ignored test) |
| --- | --- | --- |
| Loaded surfaces (every field, f32 as bits) and node count | 5 seeded streams, ~196 surfaces each | 1,060 surfaces |
| All 16x16 cells x floor/ceiling/wall x static/dynamic lists, in order | Identical | Identical |
| find_floor (camera and intangible-flag variants, including the flag's clearing) | Identical | Identical |
| find_ceil, find_wall_collisions (3 offsets, radius 0–260, vanish-wall and camera variants), water and gas levels | Identical | Identical |
| Comparisons / non-trivial hits | 857,400; 58,638 floors, 14,560 ceilings, 27,576 wall pushes, 1,321 intangible-affected floors, 5,862 water and 9,681 gas hits | 4,084,680; 404,380 floors, 18,638 ceilings, 63,140 wall pushes |

Query points include random positions, every vertex with small offsets, points
0.5 units either side of the 78-unit floor and ceiling buffers at each surface's
centroid, all cell borders with offsets around the 50-unit overlap, and large
coordinates that wrap through (s16) casts. Mutation checks: changing the cell
insertion tie order, the floor or ceiling 78-unit buffer, or the wall push sign
each fails the CI test. Rounding instead of truncating the wall-query position
was not detected, because the original's 50-unit cell overlap makes it
unobservable for these fixtures.

The test generator's random numbers were corrected on 2026-10-08: it had
returned 31 bits, so float ranges were sampled only in their lower half. The
counts above are from the corrected generator; the same tests also passed
before the fix.

Math utilities: `sins`/`coss` for 196,608 integer inputs (three wraps), `atan2s`
and `atan2f` on 1.6 million points (signed zeros, infinities, equal magnitudes,
tiny and huge ratios), and `approach_s32`/`approach_f32` on 200,000 random cases
each match the natively compiled decomp bit for bit, both with authored tables
(CI) and with the owner ROM's tables (3,993,600 comparisons). NaN inputs to
atan2s are outside coverage.

## Mario step component coverage

The oracle also compiles the pinned, unmodified mario_step.c with mario.c's
`mario_set_forward_vel`, `resolve_and_return_wall_collisions`, `vec3f_find_ceil`,
`mario_get_terrain_sound_addend`, and `sTerrainSounds` copied verbatim. Each
check copies one Rust `MarioState` into the C struct, runs the same function on
both sides, and compares every field afterwards (f32 as bits, floor/wall/ceiling
references by index) and the return value. The first difference fails with the
call and field name.

| Check | Authored terrain (CI) | BOB, owner ROM (ignored test) |
| --- | --- | --- |
| Terrain and tables | 3 seeded streams: typed floor grids, force floors (moving sand, wind), walls including burning walls, ledges, low and hangable ceilings, a flat tunnel with 159/160/240-unit gaps, water boxes; computed trig tables | BOB collision; ROM trig tables |
| Calls | 14 single calls per state (ground step, air step with each ledge/hang flag set, stationary step, stop-and-set-height, bonk reflection, gravity, vertical wind, both velocity-from-angle helpers, set-forward-vel, moving sand, windy ground) plus a 45-tick air-then-ground sequence | Same |
| States | 2,100: random actions, flags, caps, angles, speeds; ledge approaches, wall-facing ground states at the wall-angle thresholds, tunnel states | 20,000 |
| Comparisons, all identical | 123,900 | 1,180,000 |
| Ground step results (left ground / none / hit wall) | 1,173 / 27,537 / 44,513 | 11,518 / 538,080 / 166,256 |
| Air step results (none / landed / wall / ledge / ceiling / lava wall) | 17,304 / 4,556 / 8,403 / 5 / 24 / 1,485 | 238,574 / 28,724 / 13,164 / 3,684 / 0 / 0 |
| Moving sand, windy ground, water pseudo-floor | 38 / 19 / 3,186 | none on BOB |

The CI test asserts that every reachable step result, moving sand, wind, and the
pseudo-floor occur. Mutation checks: the ledge probe distance 60 to 59,
dropping the floor normal's Y factor from the ground quarter-step displacement,
the wall-angle bound 0x2AAA to 0x2AAB, and the 160-unit ceiling gap `>` to `>=`
each fail the CI test.

Not covered here: the paths where the original dereferences NULL or reads past
a table (the port panics), object (dynamic) surfaces, and the camera. Single
calls from generated states show the step functions match; the complete-tick
comparisons below show Mario reaching those states through his actions.

All component suites were rechecked on Rust 1.99.0 after the dependency update,
including optimized authored and owner-ROM tests. Zero component divergences were
observed. The old BOB collision count omitted query points already in the suite;
the table now reports its observed count. No gameplay algorithms or query
generators changed in that update. See PROJECT_PLAN session 4 for check limits.

Limits: dynamic (object) surfaces, rooms, and float-to-int casts of values
beyond the s32 range are not covered. Native IEEE single precision is assumed to
match the N64 for these operations until original-execution traces confirm it.

## Pre-action input coverage

Seven verbatim native functions cover controller normalization, buttons,
intended magnitude/yaw, floor classification, and the input update. Camera yaw
and object status are supplied context. Debug output is disabled and death-warp
requests recorded without executing the level runtime. No actions run.

| Check | Authored data | Owner ROM (before workspace reset) |
| --- | --- | --- |
| Every raw stick pair; three squish/wrapped-camera contexts; button edges/ages | 196,608 identical cases | 196,608 with ROM tables |
| Complete input stage, all exposed fields | 10,000 generated + 9 exact threshold cases | 20,000 BOB states |
| Geometry outcomes | 5,152 moved positions, including 5,148 graphical fallbacks; 1,309 death requests; 5,223 ceiling contacts | 6,851 moved, including 6,715 fallbacks; 8,451 death requests; 475 ceiling contacts |
| Chained 40-second replay | 1,200 ticks each at 15/30/60/120/144 Hz; interpolation/options toggled | 1,200 ticks each at 30/60/120/144 Hz |

Comparisons use exact float bits and discrete state. Traces record controller
history, normalized stick, intended direction, flags, timers, collision contacts,
positions, velocities, graphic fields and world inputs. Metadata hashes ordered
terrain words and actual trig tables. A one-bit intended-magnitude mutation
reports its exact tick/field. Strict floor/water/gas threshold cases guard the
original inequalities. Deliberately changing the negative-axis dead-zone
offset, reversing the camera-yaw addition, or changing the off-floor `>` to
`>=` each fails the corresponding authored test; source was restored afterwards.

```sh
cargo test --locked -p rustario64-oracle --test mario_input -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_input bob_input -- --ignored --nocapture
cargo run --locked --release -p rustario64-oracle --example input_trace -- /path/to/sm64.z64 private/input-traces
cargo run --locked -- compare-traces private/input-traces/native-input.trace.json private/input-traces/rust-input.trace.json
```

The fixture starts at the imported script position/yaw, not an original spawn.
Camera yaw is scripted. Action timers/RNG and empty object lists are explicitly
excluded placeholders. Dynamic surfaces and the dynamic-gap squish-input path
remain unvalidated; original undefined paths remain outside coverage. Graphics
options here test scheduler/snapshot isolation, not GPU gameplay. The input
suite passed again on the supplied ROM on 2026-10-09 with the action port's
state model (the oracle's `InputContext` adapter maps the schema-1 context onto
`StepWorld` and Mario's object).

## Complete Mario tick coverage

`oracle/c/tick.c` runs complete frames of the vendored decomp's Mario code with
Mario's object as the only object, and `simulation::mario::tick` runs the same
frames in Rust. Both start from the same level entry (`init_mario_from_save_file`,
Mario's spawn from `gMarioSpawnInfo`, `init_mario`, `ACT_IDLE`) and then run
independently: nothing is copied between them, so a divergence carries forward
as it would in play. After the entry and after every frame, both report every
compared value as a named 32-bit word (260 per frame, plus three per event): all MarioState
fields (surfaces by index), Mario's object (graph-node flags, area, angles,
position, scale, the whole animation state, throw matrix, collision fields, all
0x50 raw object words, hitboxes, platform, behavior-script position), body and
camera-status state, world globals and interaction statics, both floor-align
matrices, the animation in the DMA buffer, controller 1, and the frame's boundary
events in call order (sounds, camera requests, warps, level text, wind
particles, unsupported groups). Schema-1 traces carry the words, so the CLI
comparator reports the first differing tick and word.

| Check | Authored playground (CI) | BOB, owner ROM (ignored test) |
| --- | --- | --- |
| World | 8x8 tiled field with typed floors (burning, quicksand, slippery, wind), a pit over a death plane, low and high blocks, a 1,400-unit wall, a hangable ceiling, a gentle ramp and a 52° slope; computed trig tables; an authored 209-entry animation table | BOB collision, ROM trig tables, Mario's 209 ROM animations, the level script's start, radial camera mode |
| Scenarios | 15 scripted move sets (idle to sleep, run/turn/brake, jump chains to triple jumps, long jump, backflip, side flip, crouch/crawl, slide kick, dive and rollouts, punch/kick combos, ground pound, wall kicks, ledge grab/climb/drop, hanging, ramps, steep jumps, stomach and butt slides, lava, quicksand, wind, the pit) and 24 fuzzed 900-tick runs from 12 starts | The same 15 scripts from the script start and 32 fuzzed 1,800-tick runs from the start and random floors |
| Ticks compared, all identical | 28,158 | 64,158 |
| Distinct end-of-tick actions / boundary events | 69 / 10,092 | 60 / 19,048 |
| Presentation | 600 ticks, Rust at 15/30/60/120/144 Hz with interpolation and graphics options on and off: identical to native | 900 ticks at 30/60/144 Hz: identical |

The CI test fails if any of 28 key actions (from walking to lava boost) stops
being reached, so script drift cannot silently shrink coverage. A Rust panic (a
path the port does not support) ends a run's comparison at the previous tick
and is reported; none occurred in these suites. Deliberately changing the
steep-jump speed factor, the walking acceleration or the render stage's area
check each failed the CI test at the first affected tick (sources restored).
Building this harness exposed C implicit-declaration bugs in two excerpts'
includes; the oracle now treats implicit declarations as errors (oracle/README.md).

```sh
cargo test --locked -p rustario64-oracle --test mario_tick -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_tick bob_ticks -- --ignored --nocapture
cargo run --locked --release -p rustario64-oracle --example tick_trace -- /path/to/sm64.z64 private/tick-traces
cargo run --locked -- compare-traces private/tick-traces/native-tick.trace.json private/tick-traces/rust-tick.trace.json
```

Limits of this evidence:

- Native host C, not N64 execution. IEEE single precision with `-fwrapv`,
  `-ffp-contract=off` and `AVOID_UB` is assumed to match the N64 for these
  operations; original-execution traces remain the authority.
- Mario alone on static terrain. No objects, so object interactions, held or
  ridden objects, platforms, dynamic surfaces and particles are absent; those
  paths panic in Rust and abort in C. Cutscene and submerged action groups are
  recorded as unsupported events and freeze the action (deaths, star dances,
  water). Warps are recorded but not executed: the original's
  `level_trigger_warp` would also set `invincTimer` and start a transition, so a
  tick that records a warp ends faithful comparison with the original game.
- In these Mario-only suites the camera is a recorded input: its yaw comes
  with each tick, and camera requests are events that do not change the mode.
  The complete-frame suites below link the original camera instead.
- The level entry is the no-warp branch with a fresh boot, not a painting entry
  (which uses cutscene spawn actions).
- Authored animations make action timing differ from the game; the BOB suite
  uses the real table. The harness's object-pool and bhvMario steps are authored
  mirrors on both sides (oracle/README.md).
- The render pass's presentation-only writes (torso/head angle resets when Mario
  is in view, the hand-scale counter) are not modelled on either side; they
  never feed back into gameplay.

## Played sessions

The viewer's Mario mode runs `play::Session`, which since session 12 runs
`simulation::game`: the same level entry and frame as the complete-frame
suites below, with the original camera. Each logged frame keeps the buttons,
stick bytes and the camera yaw Mario read (the yaw the camera produced the
frame before); input log schema 2 marks such logs `"camera": "reference"`.
Two checks cover this path:

| Check | Evidence |
| --- | --- |
| CI: `played_sessions_with_the_camera_replay_exactly_in_the_decomp` | Six sessions on the authored camera playground driven by held controls (directions, walking, held buttons, C buttons, R, single-frame taps): 3,600 frames, 2,250 distinct camera yaws, radial, close, C-Up and boss-fight modes. The decomp replaying each session's input log with its own camera reports the session's own words after every frame; each logged yaw equals the one the replay's Mario reads; the log survives a JSON round trip; re-entering the level with the same controls gives the same words. `level_script_entries_match_the_native_setup` (mario_tick) checks the entry conversion |
| Owner ROM: viewer recordings | `rustario64-viewer view ROM --mario --record DIR` writes one input log per run; `tick_trace ROM NEW_DIR --inputs RUN` replays it in the native decomp, with the camera linked for reference-camera logs. Two windowed BOB runs with the original camera, driven by synthetic key events under Xvfb (904 frames: running, jumps, C-Left/Right turns, C-Down zoom, C-Up and its exit, R; 124 frames after a reset), compared exactly with every camera word. Session 7's two follow-camera recordings (schema 1, 233 ticks) predate the reference camera |

These show the port matches the decomp for the inputs given, with the camera
the original game would have shown. Play stops at the same boundaries as the
suites (unsupported paths, unsupported camera modes or cutscenes, and warps).

## Next reference work

1. Done for BOB in session 12 (below). Next: the submerged and cutscene groups
   BOB needs (water is absent from BOB, but deaths, star dances and spawn
   actions are not), with the camera's cutscenes, and the camera modes and
   trigger tables of further areas as they are imported.
2. Objects for the first mission, with object state added to the tick snapshot.
3. Obtain original-execution traces from a matching US build and emulator. Those
   remain unavailable and are the eventual authority over native-host results.
   libsm64 remains excluded as a fidelity oracle because it changes collision
   ordering. A finite exact suite covers its cases and platforms, not all
   behavior.

## Camera components (2026-10-09)

`simulation::camera` is a direct translation of 33 camera.c helpers, including
radial goal construction. The native oracle compiles verbatim excerpts generated
by `oracle/tools/extract_excerpts.py`; item hashes are in oracle/README.md.
All equality checks use original integer widths and exact float bits.

The authored suite covers every initial s16 value with ten signed increments/
divisors, two smooth/snap states and five approach operations; signed extremes
exercise C promotions. Float cases include signed zero, subnormals, extreme
finite values, negative increments and multipliers above one. All 4,096
C-button history/pressed/held combinations also run with unrelated high bits.
Vector/angle/distance, in-place rotation, pitch reconstruction, strict trigger
faces and all four area clamps compare independently against native C.
An authored terrain covers overlapping walls, camera-only/ignored surfaces,
slopes, tight floor/ceiling gaps, water and missing contacts. Camera geometry,
wall correction, vertical resolution and radial goals compare for four incoming
collision-flag combinations. The retained-height NULL-floor boundary panics;
the C side is not invoked on the original crashing path.

A persistent 3,600-step helper sequence runs at 15/30/60/120/144 Hz with
presentation interpolation on and off, comparing its position, yaw and C-button
state after each step. This is an authored component sequence, **not**
`update_camera`, `mode_radial_camera` or `update_lakitu`, and does not validate
original camera-relative Mario movement (see the complete-frame section below).

```sh
cargo test --locked -p rustario64-oracle --test camera -- --nocapture
cargo test --locked --release -p rustario64-oracle --test camera -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test camera bob_camera_components -- --ignored --nocapture
```

The owner-ROM camera check imports BOB's raw collision and original trig tables.
The ROM attached on 2026-10-09 validates as supported US v1.0 and passes all
20,000 positions x four collision-flag combinations. Original-N64 execution
remains unvalidated.

Fresh authored checks pass in debug and release (eight tests, one owner-ROM
test ignored). Deliberately changing strict trigger bounds to inclusive bounds
or last-wall reuse to first-wall reuse fails the intended native comparison.
Both mutations are reverted; the restored suite passes.

## Persistent Lakitu stage (2026-10-09)

`simulation::camera::lakitu` translates `update_lakitu`, `next_lakitu_state`,
level-oriented transition setup, deterministic pitch/yaw/roll shakes, damage
shake priority and FOV shake setup. Its mode goals are explicit inputs. The
native adapter compiles verbatim originals; after initialization, native and
Rust state persist independently. Exact comparison covers 94 modeled words
(camera goals/yaw, current/render Lakitu state, speeds, shakes, transitions,
mode/status fields and helper globals) and the two collision flags after every
tick. The shared generated layout only transports fields; it contains no camera
algorithm or expected state.

Checks pass for 10,000 randomized states x four collision-flag combinations on
authored terrain and 20,000 x four on BOB's real collision/original ROM tables.
Each terrain also runs a 3,600-tick persistent sequence at 15/30/60/120/144 Hz,
with interpolation on and off (36,000 compared ticks per terrain). Authored
mode goals and Mario paths exercise moving transition origins, mode changes,
dive edges, interrupted/repeated transitions, signed shake phases and speed
recovery. Mode-change tests cover init-frame suppression, mode restoration,
flag clearing and damage-shake priority. A separate assertion covers retained
camera-filter flags when the rendered camera is floor-corrected or finds no
floor. Random requests are rejected before state changes.

```sh
cargo test --locked -p rustario64-oracle --test lakitu -- --nocapture
cargo test --locked --release -p rustario64-oracle --test lakitu -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test lakitu -- --include-ignored --nocapture
python3 oracle/tools/lakitu_layout.py --check
```

This stage alone is **not** `update_camera` coverage; the complete-frame
section below covers the dispatcher. Since session 12 the stage takes the
shared RNG: `shock_shakes_draw_from_the_shared_random_sequence` compares the
shock shake's two draws and the seed, and the handheld shake runs in the
complete-frame suites.
FOV setup is compared, not the perspective node's FOV animation or rendering.
The outer update must write `last_frame_action` after this stage; tests compare
that assignment separately.

Deliberately truncating a float shake increment before adding it to the angle
fails the native comparison; unconditionally clearing the camera-floor flag
fails the independent flag assertion. Both changes are reverted, and the
restored authored/owner-ROM stage suites pass.

## Camera obstruction and radial stages (2026-10-09)

The Rust obstruction and radial modules translate nine additional original
functions (the Mario-behind-surface wrapper is inlined). Verbatim excerpts from
the same pinned revision provide the native behavior. Helpers use original
integer products, strict extent checks, surface exclusions and signed sectors.
The wall scan preserves eight probes, coarse/fine query ordering, last-wall
selection and the source radius clamp. Radial movement preserves surface-entry
flags, conflicting rotations, first/second turn limits, stationary behavior,
promoted angle comparisons, outward offsets and zoom narrowing.

| Comparison | Authored fixtures | Owner ROM |
| --- | --- | --- |
| Avoid yaw | Every s16 starting yaw with ten relative boundary angles (655,360 cases) | Existing helper uses original integer angles; no ROM needed |
| Vertex/sector tests | Loaded authored surfaces, exact-plane/reversed winding, 149/150/151-unit walls and 20,000 randomized vertex/bounds cases, including full s16 coordinates | Real BOB walls exercised through obstruction scans |
| Obstruction scan | 10,000 generated pairs × four query-flag combinations (40,000); all three outcomes occur; targeted low/ignored/near/clear walls | 20,000 × four (80,000), original BOB collision and ROM tables |
| Radial rotation, outward offsets and zoom | 20,000 states; floor transitions, both modes, first/second flags, signed extremes and fractional distance bounds; persistent surface turns reach source limits | 40,000 states |
| Persistent movement → zoom → radial goals → Lakitu | 1,800 ticks at 15/30/60/120/144 Hz with interpolation off/on (18,000 ticks); all shared words compared after each stage | Same 18,000 composed ticks on BOB with original tables |

Both sides retain independent persistent state. The 94-word shared record and
controller second-rotation flags, area yaw and collision flags compare exactly.
No Rust post-tick value is loaded into native state. Authored Mario paths and
floor inputs make this a **stage-composition check**, not combined Mario/camera
gameplay or complete mode_radial_camera; those are covered by the
complete-frame section below. Original-N64 traces remain missing.

Selecting the first wall instead of the last, making the low-wall cutoff
inclusive, or narrowing the radial condition before its comparison each causes
a native divergence. All three deliberate mutations are reverted; restored
authored and owner-ROM comparisons pass.

```sh
cargo test --locked -p rustario64-oracle --test radial -- --nocapture
cargo test --locked --release -p rustario64-oracle --test radial -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test radial -- --include-ignored --nocapture
```

### Look-ahead pan (small follow-up, 2026-10-09)

Rig's `pan_ahead_of_player` preserves the original two rotations and float
operation order; replacing these with a single sine expression changes bits.
The fixed 0.025 approach runs once per tick, even with smooth movement disabled.
Long jumps and non-top pole actions reverse pan; sleeping approaches zero.
40,000 authored comparisons and 40,000 with ROM trig tables cover these paths,
signed angles/zero and coincident/vertical eyes. All 94 shared words compare,
with independent assertions that yaw/eye do not change. The existing persistent
radial-goal/Lakitu compositions now include pan before Lakitu (18,000 ticks per
terrain at five render rates and both interpolation settings). Run the same
`--test radial` commands above.

## Complete frames with the original camera (session 12, 2026-10-09)

`simulation::game` runs one original frame: controller read, the area update
counter, Mario's object update (his `set_camera_mode` and
`set_camera_shake_from_hit` calls are applied after it in call order, with his
own later reads of the mode mirrored as the shared `struct Camera` would show
them), `update_camera`, then the render pass's perspective node (FOV), camera
node (graph camera) and Mario's animation frame. The oracle runs the same frame
in `oracle/c/tick.c` with camera.c's verbatim update path linked
(oracle/README.md). Both sides start from the same fresh-boot camera globals,
area camera node, act and RNG seed and then run independently. After the entry
and after every frame, both report Mario's words and every camera word: the
area's `struct Camera`, all of Lakitu, the mode transition and mode info,
Mario's camera geometry, FOV, flags, the handheld shake, C-Up's stored camera,
the cutscene values the dispatcher reads, the RNG seed, the HUD camera status
and the graph camera's position, focus, roll and field of view.

| Check | Authored playground (CI) | BOB, owner ROM (ignored test) |
| --- | --- | --- |
| World | 9x9 tiles with close-camera, boss-fight, rotate-left/right/middle, free-roam and no-camera-collision tiles, tall blocks and a tunnel that obstruct the camera, hangable ceilings over close and radial floor, a ramp and a pit; BOB's area rules; computed trig tables; authored animations | BOB collision, ROM trig tables and animations, the area camera node from the ROM (mode 1, its position and focus), the level script's start; act 1 |
| Scenarios | 15 scripted programs (idle to sleep and the FOV, radial running with every C button, C-Up look and exits, close-camera tiles, steps and ramp, hanging under close and radial floor, boss-fight tiles, rotation surfaces, the R-button Mario camera, tunnel and walls, jumps/dives/pounds, the C-Up exit search against a wall, free-roam and slippery tiles) and 16 fuzzed 900-frame runs from eight starts, each with its own RNG seed | The 15 scripts from the script start; 40 fuzzed 1,800-frame runs from starts on BOB's close-camera, boss-fight, rotation and hangable surfaces and random floors |
| Frames compared, all identical | 19,512 | 77,112 |
| Actions / camera modes reached | 45 / radial, close, free-roam, C-Up, boss-fight (all five required by the test) | 62 / radial, close, C-Up, boss-fight (all four required) |
| Presentation | `radial-run-and-c-buttons` at 15/30/60/120/144 Hz: identical | 900 fuzzed frames from the script start at 30/60/144 Hz: identical |

Also compared: the 1,800-frame `tick_trace --camera` program on BOB (camera
buttons, R and C-Up over the Mario program) and the played sessions above. A
mode, transition or cutscene the port does not model ends a Rust run with
`Unsupported` after the completed frame (the frames before it are compared);
the C side aborts on the same paths. The area camera node's callbacks must
resolve to `geo_camera_main` and `geo_camera_fov` before the camera starts.

Seeded mutations (each reverted afterwards): 14 one-line changes to the Rust
port were tried against the authored suite. Ten fail it: the hanging goal
factor, the RNG constant, C-Up's head-look scale, the boss-fight distance
factor, C-Up's return distance, the FOV's sleeping approach, a handheld spline
coefficient (0.7), the random spline offset range, and, after the scenarios
were strengthened (raised close-camera floors and a corrected hanging spawn,
and a C-Up exit against a wall), the default camera's floor-scan step and the
C-Up exit search step. Four survive because the reached states do not
distinguish them: adding BOB's boss-fight height as one 250 instead of two
125s, a one-ulp change to a spline coefficient, raising the handheld
increment floor from 0.02 to 0.03, and reassociating `move_into_c_up`'s
distance fraction.

```sh
cargo test --locked -p rustario64-oracle --test camera_tick -- --nocapture
cargo test --locked --release -p rustario64-oracle --test camera_tick -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test camera_tick -- --include-ignored --nocapture
cargo run --locked --release -p rustario64-oracle --example tick_trace -- /path/to/sm64.z64 private/camera-traces --camera
```

Limits of this evidence:

- Native host C, not N64 execution; original-execution traces remain the
  authority.
- Areas without camera triggers only. Levels whose original `sCameraTriggers`
  entry is not NULL are refused on both sides; the cannon, behind-Mario,
  water-surface, 8-direction, outward-radial, parallel-tracking, slide, fixed
  and spiral-stairs modes and every cutscene are unported (cutscene starts are
  detected and end play). The pause screen's camera path is ported but never
  reached because no pause runs.
- Mario alone: no objects, so object-driven camera behavior (cutscene focus
  objects, King Bob-omb's boss fight, the cannon) is absent.
- Camera sounds are compared as recorded events in call order with Mario's;
  no audio runs.


## Presentation regression and development pause — session 13

Mario draw-list changes (especially the 64-tick blink cycle) previously disabled
all model interpolation. The new owner-ROM regression evaluates 256 completed
idle frames, adds an independently authored 30-unit-per-tick presentation offset,
and changes among three LOD distances. Across 32 mesh switches it compares the
half-frame vertices against independently posed endpoints in the current mesh.
The test catches the old build-equality snap. Authored tests also cover a material
change, a switch to previously inactive bones, animation-entry changes and reset.
These are presentation checks; the per-frame native oracle remains the authority
for simulation, camera and animation-frame advancement.

The existing 3,600-frame held-control/native replay comparison now snaps
presentation every 17 frames, checking every authoritative word and the input
count remain unchanged. Desktop scheduler tests discard menu/focus/inspection
time and an existing backlog, then resume from a fresh clock anchor. The pause
freezes all original frame stages; it does not reproduce the original pause menu
or its camera zoom. All settings in this increment are graphics-only.

The windowed Xvfb/software-Vulkan smoke also checks invalid-ROM recovery, clean
launcher/game close, a held tick count while paused, input clearing on resume,
and live interpolation/fog/VSync changes. A fresh 68-frame owner-ROM recording
with movement/jump inputs and a pause replays every Mario and camera word
exactly via `tick_trace --inputs`. ROM content and exported state traces stay
under ignored private paths; neither is distributed or staged.

## Landing and action-change interpolation — session 14

The remaining animation-entry check in `MarioDrawer::frame` discarded the
previous model pose on clip changes. The new owner-ROM regression reproduces
17 such changes, including five landing transitions, in three 180-frame BOB
sessions: jump → landing → run, jump → landing → turn, and jump → landing →
stop/restart. It uses real completed positions and animation frames, without
the artificial translation used by the blink regression. For unchanged geometry
it compares the displayed vertices at alpha 0, 0.25, 0.5, 0.75 and 1 against
the independently completed endpoint vertices. The old guard fails on every
one of those 17 switches; the fix preserves every intermediate pose.

The authored regression also changes joint rotations and root translations
between clips, including simultaneous clip/material and clip/skeleton-branch
changes. It checks the expected coordinates, interpolation-off and non-finite
alpha behavior, and level reset. The existing blink/LOD regression covers mesh
changes under the current selection. These tests verify presentation; they do
not advance the original animation clock or change the 30 Hz game frame.

To export the three authored input programs into a private ignored directory:

```bash
RUSTARIO64_ROM=/path/to/sm64.z64 \
RUSTARIO64_TRANSITION_LOG_DIR=private/transitions \
cargo test --locked --release -p rustario64 --test mario_model \
  local_us_rom_landing_and_action_changes_keep_interpolating -- --ignored --nocapture
```

Replay each recording (`landing-run`, `landing-turn`, `landing-stop-restart`)
with the linked reference camera, using a new output directory per recording:

```bash
cargo run --locked --release -p rustario64-oracle --example tick_trace -- \
  /path/to/sm64.z64 NEW_PRIVATE_REPLAY_DIR \
  --inputs private/transitions/landing-run.inputs.json
```

All three 180-frame recordings replay every Mario and camera word exactly against
the native reference (540 frames total). The optimized core/oracle suite passes
128 tests with all owner-ROM checks enabled; desktop/render checks pass another
10 with offscreen GPU required. Warnings-denied workspace Clippy and formatting
pass. Window smoke checks use Xvfb/software Vulkan. This is automated presentation
and native-oracle coverage, with physical-GPU human transition playtesting still
pending.

## Objects and coins (session 17, 2026-10-10)

`simulation::object` runs the original object system each frame inside the
game frame: `update_objects` over the lists in `sObjectListUpdateOrder`, the
ROM's behavior scripts interpreted by `cur_obj_update`, object collision,
`interact_coin` through Mario's interaction pass, unloading with respawn bits,
and the render pass's `oAnimState` writes for objects in the camera's view.
The oracle runs the verbatim C for all of it (`oracle/c/tick.c`; see
oracle/README.md) with the original camera linked. Both sides enter the same
placements: the area's macro entries and spawn infos whose scripts the port
runs, in the original list order; the rest are skipped on both sides.

After the entry and every frame both report Mario's and the camera's words
(as in session 12) plus every object word: each list's slots in order, the
free list, the current object, the time-stop state, the RNG seed, the
compared macro entries' respawn bits and spawn infos' arguments, and for every
non-Mario object its graph node (flags, area indices, model, angles, position,
scale, animation state, throw matrix), all 0x50 raw words, active flags,
collisions and their interact types, the behavior stack and delay timer,
respawn record, hitbox and hurtbox, `behavior`, `curBhvCommand` (both as
segment 0x13 addresses), platform, collision data and parent.

| Check | Authored field (CI) | BOB, owner ROM (ignored test) |
| --- | --- | --- |
| World | 10x10 flat tiles with a void beyond x = 5000 and a raised block; computed trig tables; authored animations, scripts (`authored_scripts`, the same command words as the verbatim scripts) and coin/sparkle model traversals | BOB collision, ROM trig tables and animations, the ROM's behavior segment, presets and coin/sparkle models; act 1's placements |
| Placements | 18 macro entries: eight yellow coins (one 700 units above the floor, one over the void), every formation type (horizontal and vertical line, horizontal and vertical ring, arrow, flying line and ring), list parameters selecting the type, and a formation 2,600 units away with preset respawn bits | BOB act 1: 14 coin placements (five yellow coins, nine formations) and the spin airborne warp spawn; 94 other placements are skipped |
| Scenarios | Six starts × 900 frames of seeded movement, jumps, dives, ground pounds and camera buttons | Four ground-formation starts × four yaws × 600 frames |
| Frames compared, all identical | 5,400 | 9,600 |
| Coverage asserted | up to 12 coins collected; up to 75 objects; 93 golden-sparkle and 930 coin-sparkle object-frames; 79 formation-respawn frames; 174 frames with unloads; 40,293 shadowless-coin object-frames | up to six coins collected; up to 26 objects; 111 golden-sparkle and 1,110 coin-sparkle object-frames; 11 formation-respawn frames; 142 frames with unloads; 48,000 shadowless-coin object-frames |

The camera suite's BOB frames (77,112, identical) and the 30/60/144 Hz
presentation check now enter BOB with its act-1 coins on both sides as well.
`tests/objects.rs` (core crate, owner ROM) pins BOB act 1's placement counts
(88 macro objects, 21 spawn infos, 14 coin placements spawned, 94 recorded as
unported) and collects a yellow coin in a played session.

```sh
cargo test --locked --release -p rustario64-oracle --test objects -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test objects -- --include-ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release --test objects -- --ignored --nocapture
```

Seeded mutations (each reverted afterwards), run against the authored suite:
the yellow coin hitbox radius (100 → 99), the vertical ring's height offset
(+200 → +201), the formation unload distance (2100 → 2000), the anim-state
switch's reset test (`>=` → `>`), the list update order (OBJ_LIST_LEVEL and
OBJ_LIST_DEFAULT swapped) and the free list's order (freed slots appended
instead of pushed to the front). All six fail it at a per-frame word
difference.

Limits of this evidence:

- Native host C, not N64 execution.
- Only the coin behaviors, the spin airborne warp's BREAK and Mario run.
  Skipping the other placements changes slot assignment and removes their
  random draws on both sides equally, so BOB's comparison is of the subset,
  not of the full level's object state.
- Particles are not spawned on either side (Mario's particle flags are
  compared); object animations, object collision models, platforms, held
  objects and every other interaction handler are unported and abort or are
  never spawned.
- The render pass's object writes are compared. Session 18 draws the coins
  and sparkles; its visual checks are not original-N64 image comparisons.


## Coin presentation (session 18, 2026-10-10)

The viewer now reads the completed object traversal to build and draw ROM
coin/sparkle models. The importer builds all 28 switch cases (eight yellow
coin, eight shadowless yellow coin, twelve sparkle), yielding the original
four coin and six sparkle texture frames, each repeated by its geo layout.
Authored tests cover selected geometry changes while positions interpolate,
billboard axes, despawn/reappearance, pool-slot reuse, reset and multiple
instances sharing a template. The GPU regression checks buffer growth,
shrink, empty frames and reappearance without stale triangles. An ignored
owner-ROM GPU test draws every case from two camera directions and checks
visible textured pixels and transparent background.

`drawing_bob_coins_and_sparkles_preserves_every_authoritative_word` draws a
120-frame BOB coin-collection session at five interpolation fractions with
interpolation on/off, snapping history every 17 frames. Every compared Mario,
camera, HUD, object, list, free-list and RNG word remains unchanged. The
existing independent native frame comparisons remain the simulation evidence;
these new drawing tests establish read-only presentation and exercised visual
paths, not pixel equivalence with the original renderer. Coin shadows and
original HUD typography remain absent.


## Desktop controller input (session 19, 2026-10-10)

Desktop profile v1 maps normalized axes into raw stick bytes, before the existing
reference controller processing. Authored host-event tests cover partial/full
and diagonal deflection, button aliases, short taps, camera hysteresis and D-pad
sources, neutral gates, Start edges, selection, unplug/reconnect and keyboard
composition. The existing 3,600-frame keyboard session replay remains intact.
A separate 3,600-frame analog session test uses six authored starts and verifies
every Mario/camera word, serialized log replay, pause snapping and reset against
the native decomp. With the owner's ROM, a further 600-frame analog session at
BOB's script start compares every Mario, camera, HUD, object/list/free-list and
RNG word against native C. All are identical in optimized builds.

The desktop tests also validate local ROM import, return/reselection, failure
recovery and reuse of the imported level, without a window. egui form generation
is tested at 800×720 and 640×480. These tests are not physical controller, native
file-dialog, desktop double-click or N64 hardware validation. Those human checks
remain in docs/PLAYTEST.md. No movement, camera or object rules are changed.

## Enemy movement component (session 20, 2026-10-10)

`oracle/tests/object_motion.rs` compares the Rust `object_step` component with
verbatim `obj_behaviors.c` motion functions and native `math_util.c`/
`surface_collision.c`. Every call compares all 80 raw object words, untouched
object fields, collision-query flags, sObjFloor identity, movement flags, each
of the 16 floor-matrix float words and ordered splash requests. No float tolerance
is used. Matrix allocation and wave/bubble/sound calls are captured boundaries;
particle behavior, pool allocation and full actor/frame updates are not compared.

| Suite | Data and coverage | Exact calls |
| --- | --- | --- |
| Targeted CI boundaries | Authored dry/wet floors, steep panels, walls/grates; terminal speeds, bounce cutoff, tiny velocities, water skipping, floor-height truncation, friction threshold, negative no-floor yaws, billboarding, allocation failure and orientation suppression | 1,574 |
| Generated/chained CI | 40,000 states with sentinel raw fields, varied flags/hitboxes/parameters plus 120 trajectories of 180 ticks at patrol/chase speed or free thrown motion; computed tables, authored terrain/water boxes | 61,600 |
| Local owner-ROM | 80,000 states plus 120 trajectories of 360 ticks; BOB's actual collision and ROM trig tables | 123,200 |

All pass in optimized Linux x86_64 native comparisons. Generated authored calls
exercise all four movement flags (mask 0xF), 762 matrix-producing calls and
17,359 no-floor calls. BOB calls exercise mask 0xB (no underwater terrain in
these sampled cases), 1,924 matrices and 20,408 no-floor calls. Targeted tests
assert that matrix, water-wave, bubble and water-entry-sound paths are reached.
Original wall pushes plus final movement, and floor matrix timing, are retained.
Changing the bounce cutoff or querying the floor at the initial position makes
the differential suite fail; both mutations were reverted before validation.

This supplies the ordinary Bob-omb movement prerequisite. No new actor runs in
the viewer; enemy animations, grab/kick/throw interactions, explosion/loot/
respawn, water particle scripts and motion-output integration remain pending.
King Bob-omb needs a different movement family and dialog/cutscene mechanics.
Dynamic surfaces, invalid/out-of-range conversions and original N64 execution
remain outside this evidence. Existing Mario/camera/coin frame comparisons
continue to protect the playable exploration baseline.

## Controller comfort changes (session 21, 2026-10-10)

Desktop profile v2 gates left-stick deflections at or below 10% before rounding,
uses the west face button for N64 B, and releases disconnected inputs without
requesting pause. Host tests cover the formerly admitted 9.4% drift, exact
10% cardinal/diagonal boundaries, unchanged values outside the circle, west
button taps with the east button held, disconnect cleanup, keyboard composition
and neutral reconnection. Run them with
`cargo test --locked -p rustario64-render --lib controller::tests`.

The raw conversion and simulation controller are unchanged; recorded final
tick bytes still replay through the reference controller. This is host-input
behavior coverage, not physical USB/Bluetooth or native-window validation.

## Bob-ombs, explosions and object animations (session 22, 2026-10-10)

Complete frames now include Bob-ombs. Each frame compares every named word of
the previous object suites (Mario, the linked camera, every object's graph
node, raw words, behavior stack, hitboxes and collisions, the lists, the
RNG seed and the frame's events) plus each object's animation state
(`curAnim` as a segmented address, frame, acceleration, translation) and its
throw-matrix words. There is no tolerance; a mismatch reports the first
differing word.

```sh
cargo test --locked --release -p rustario64-oracle --test bobombs -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test bobombs -- --include-ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release --test objects -- --ignored --nocapture
```

| Suite | Data | Identical frames | Coverage |
| --- | --- | --- | --- |
| Authored Bob-ombs (CI) | Authored field with ramp, block, lava tile and death-plane pit; 12 Bob-ombs (one stationary); invented Bob-omb-shaped animations; 10 seeded runs of 900 frames | 9,000 | 793 lit-fuse and 757 chase Bob-omb frames, 37 explosions with environmental shakes, 123 lava and 35 death-plane deaths, 170 respawns, a loot coin collected, a knockback |
| Authored encounters (CI) | The stationary Bob-omb, jump kicks from four distances and a punch | 1,284 | 92 launched frames, 24 explosions; the punch stops at the grab (frame 13) |
| BOB Bob-ombs (owner ROM) | BOB's collision, the ROM's scripts, presets, animation table (0x0802396C, two 13-part animations) and models; Mario beside each of the 12 act-1 Bob-ombs at four yaws, 600 frames each | 27,055 | 7,275 lit-fuse and 6,975 chase frames, 52 explosions, 29 respawns, 14 loot coins, 23 knockbacks; three runs stop at grabs |

The existing coin suites (5,400 authored and 9,600 BOB frames) and the BOB
camera frames now run with Bob-ombs present and stay identical. A core test
(`tests/objects.rs`, owner ROM) plays BOB until a Bob-omb chases Mario,
explodes, drops a coin and its respawner brings it back, and checks that the
viewer's object drawer builds every model drawn (Bob-omb, explosion, smoke,
yellow coin).

**Seeded mutations.** Thirteen single-point changes were applied one at a
time and the authored suites re-run (development helper, not committed):
the chase turn rate, the blink threshold, the fuse length, the knockback
strength table, the push-out padding, the explosion growth, the respawn
distance, the Bob-omb's touch attack type, an animation's start side, the
frame advance moved behind the view test, the loot coin's intangible frames
and the chase step-sound frame were each rejected at a named word and frame.
Changing the facing test's `coss(d) > 0` to `>= 0` was not rejected; it is
equivalent for every range the ported behaviors pass (0x2000: the cosine is
zero only where the sine is ±1, outside the range), so no test can tell them
apart.

**Not covered.** Holding, carrying, dropping and throwing (Mario's
picking-up/hold/throw actions, the hand position the render pass writes, the
held/dropped/thrown Bob-omb loops beyond their entry), underwater explosions
and water particles, shock camera shakes deferred from objects (asserted
absent), object shadows, dynamic object surfaces and original N64 execution.
The authored animation values are invented; ROM animation decoding is checked
by its shape and by the BOB comparisons, not by an independent reference
checker. Presentation (skinned object models, billboards, explosion and smoke
textures) is inspected in local screenshots only and needs a human comparison
with the original.

## Holding and Mario's render pass (session 23, 2026-10-10)

Mario's object node now goes through the render pass in every frame with the
linked camera, on both sides: in Rust `simulation::mario::render`, in the
oracle the verbatim rendering_graph_node.c traversal and mario_misc.c
callbacks over a graph built with the verbatim graph_node.c constructors. Its
state writes are compared through the existing words: the HOLP
(`body.heldObjLastPosition`), the torso and head angles, the punch state, the
animation frame, and the held object's frame and anim state. The camera, coin
and Bob-omb suites therefore all cover the pass (the camera-less Mario tick
suites keep the earlier minimal step on both sides).

```sh
cargo test --locked --release -p rustario64-oracle --test bobombs -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test bobombs -- --include-ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release --test objects bob_bobomb_is_picked -- --ignored --nocapture
```

| Suite | Data | Identical frames | Coverage |
| --- | --- | --- | --- |
| Authored held slide (CI) | A plateau, a slippery slope and a stationary Bob-omb; carried down the slope into a held butt slide, then thrown | 756 | 442 held frames, ACT_HOLD_BUTT_SLIDE reached, 1 throw, 2 drops |
| Authored encounters (CI) | Authored field, the stationary Bob-omb, an authored Mario-shaped model (mario_geo's hierarchy and callbacks, invented lengths) and an authored animation seed whose holding animations play forward; four jump kicks, seven holding scripts after a punch (carry and throw, fuse in hand, drop, air throw, jump and land, walk and turn, re-grab) and a dive grab | 3,203 | 558 held frames, 4 throws, 3 drops, 10 holding actions, 558 HOLP updates, 59 explosions |
| BOB Bob-ombs (owner ROM) | Session 22's 48 runs plus the seven holding scripts beside BOB's stationary Bob-omb, with the ROM's Mario model and animations | 30,511 | 897 held frames, 6 throws, 5 drops, 13 holding actions, 896 HOLP updates, 62 explosions, 31 knockbacks |

The authored random Bob-omb runs (9,000 frames), the authored and BOB coin
suites (5,400 and 9,600) and the camera suites stay identical with the pass
on both sides. A ROM play test (`tests/objects.rs`) picks BOB's stationary
Bob-omb up, carries it for 101 frames with `held_visible_object` placing it at
the HOLP in Mario's hand, and throws it into a launch.

**Seeded mutations.** Twelve single-point changes were applied one at a
time against the authored Bob-omb and camera suites (development helper, not
committed): the light-object hand offset, the level-of-detail distance sign,
the hand scaler's once-per-frame guard, the hand translation row copied into
the held matrix, the throw's release frame, the throw's lead distance, the
drop height, the heavy-object grab test, the carrying walk speed, the
torso-reset action list, the quarter scale of the hand offset and the held
object's scale. Ten were rejected at a named word and frame in the first run.
The torso-reset change was not: no script carried a Bob-omb into a held butt
slide, so `authored_bobomb_carried_into_a_held_butt_slide_matches_the_decomp`
was added (a slippery slope below a stationary Bob-omb's plateau: 756 frames,
442 held) and now rejects it. Dropping the held object's scale is invisible to
simulated state (the scale touches only the matrix's rotation rows; the HOLP
reads its translation row), so no comparison can see it; the drawer applies
the same scale.

**Not covered.** Heavy holds (King Bob-omb) and Bowser's swing, holdable
objects other than Bob-ombs, the castle mirror's Mario, Mario's prevObj
(burning Mario), camera-less frames' render pass, frames where Mario is out of
view while holding (covered only incidentally), original N64 execution, and
the visual placement of the held object against the original game (checked in
local screenshots only).
