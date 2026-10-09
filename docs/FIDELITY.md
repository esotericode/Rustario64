# Comparison contract and coverage

Mario's movement now has **per-tick coverage against the natively compiled
decomp**: complete frames of Mario alone (input stage, every non-object action
group, Mario's object update and the animation frame advance) match bit for bit
on an authored playground and on Bob-omb Battlefield with the owner ROM's data.
That is not yet coverage against original N64 execution, and the complete camera update,
objects, interactions with objects, cutscene/submerged actions and RNG-driven
behaviors have **zero validated coverage**. Collision, math, physics steps and
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
- The camera is a recorded input: its yaw comes with each tick, and camera
  requests are events that do not change the mode (the real camera would).
- The level entry is the no-warp branch with a fresh boot, not a painting entry
  (which uses cutscene spawn actions).
- Authored animations make action timing differ from the game; the BOB suite
  uses the real table. The harness's object-pool and bhvMario steps are authored
  mirrors on both sides (oracle/README.md).
- The render pass's presentation-only writes (torso/head angle resets when Mario
  is in view, the hand-scale counter) are not modelled on either side; they
  never feed back into gameplay.

## Played sessions

The viewer's Mario mode runs `play::Session`, which enters the level with the
same `enter_level` and advances with the same `tick` as the suites above, and
keeps each tick's input: buttons, stick bytes and the follow camera's yaw. The
follow camera is not the original camera; its yaw is an input like the
recorded yaws above. Two checks cover this path:

| Check | Evidence |
| --- | --- |
| CI: `played_sessions_replay_exactly_in_the_decomp` | Six sessions on the authored playground driven by held controls (directions, walking, held buttons, single-tick taps, camera turns): 3,600 ticks, 31 actions, 322 camera yaws. The decomp replaying each session's input log reports the session's own words after every tick; the log survives a JSON round trip; re-entering the level with the same controls gives the same words. `level_script_entries_match_the_native_setup` checks the entry conversion |
| Owner ROM: viewer recordings | `rustario64-viewer view ROM --mario --record DIR` writes one input log per run; `tick_trace ROM NEW_DIR --inputs RUN` replays it in the native decomp. Two windowed BOB runs driven by synthetic key events under Xvfb (154 ticks: running onto the cannon mound, a jump, a camera turn; 79 ticks after a reset: a dive into a stomach slide) compared exactly, and the CLI comparator agrees. The ROM test also checks that the viewer's entry equals the script-start scenarios' |

These show the port matches the decomp for the inputs given. They do not show
that a player using the original camera would give the same inputs, and play
stops at the same boundaries as the suites (unsupported paths and warps).

## Next reference work

1. Port the reference camera (camera.c) so camera yaw and mode stop being inputs,
   with its own per-tick comparisons; then the submerged and cutscene groups BOB
   needs (water is absent from BOB, but deaths, star dances and spawn actions are
   not).
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
original camera-relative Mario movement. The viewer's follow camera is unchanged.

```sh
cargo test --locked -p rustario64-oracle --test camera -- --nocapture
cargo test --locked --release -p rustario64-oracle --test camera -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test camera bob_camera_components -- --ignored --nocapture
```

The owner-ROM camera check imports BOB's raw collision and original trig tables.
The ROM attached on 2026-10-09 validates as supported US v1.0 and passes all
20,000 positions x four collision-flag combinations. The complete reference
camera and original-N64 execution remain unvalidated.

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

This is **not** `update_camera` coverage: initialization, surface-based mode
selection, complete radial/free-roam mode controllers,
C-Up/R-trigger handling, cutscene dispatch and the original RNG remain missing.
Active handheld/random shock requests are explicit unsupported boundaries;
existing handheld angle offsets decay when no random request is active.
FOV setup is compared, not the perspective node's FOV animation or rendering.
The outer update must write `last_frame_action` after this stage; tests compare
that assignment separately. Original camera-relative Mario movement is not
validated, and the viewer continues using its approximate follow camera.

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
gameplay or complete mode_radial_camera. Input, height/pan, free roam, mode
selection, initialization and full update_camera remain pending. The viewer
still uses its approximate follow camera, and original-N64 traces remain missing.

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
`--test radial` commands above. No complete camera or viewer fidelity is claimed.
