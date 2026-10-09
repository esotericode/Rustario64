# Comparison contract and coverage

Original movement, actions, camera, RNG, objects, and interactions have **zero
validated gameplay coverage**. Collision, math, physics steps and pre-action
inputs have native-decomp component coverage. Input-stage traces compare at
every tick; these are not complete gameplay ticks because no action runs yet. The diagnostic
marker visualizes a tick counter; it is not a Mario approximation.

Target US v1.0 at n64decomp/sm64 revision
9921382a68bb0c865e5e45eb594d9c64db59b1af, ROM SHA-1
9bef1128717f958171a4afac3ed78ee2bb4e86ce, 30 ticks/second, reference camera/input
profile. Prefer unmodified matching N64/original execution; native-port float
differences need separate evidence. Physics-modified ports are not unquestioned
oracles. Static asset checks passed with the supported owner ROM before the
workspace reset; the ROM needs to be supplied again for fresh integration runs
(see [ROM_VALIDATION.md](ROM_VALIDATION.md)). A matching build, emulator trace setup,
and per-tick exporter are still unavailable, so reference-vs-Rust gameplay
checks have not run.

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
differences, ordered objects, and invalid traces. The optional wgpu viewer now
renders imported BOB terrain and feeds the same fixed 30 Hz clock, but no
gameplay runs in it, so there is still no real gameplay render-cap coverage.
Enhanced lighting/shadows are not implemented; the renderer's options (MSAA,
fog/culling toggles, free camera) live in the render crate and have no path into
simulation state.

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

Not covered: mario_update_quicksand and mario_push_off_steep_floor (not ported),
the paths where the original dereferences NULL or reads past a table (the port
panics), object (dynamic) surfaces, and anything that depends on action code or the camera. Input processing
is covered separately below. Single calls from generated states show the
step functions match; they do not show that Mario reaches those states the same
way, which needs action ports and per-tick traces.

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
suite and example passed on the supplied ROM before a workspace reset; recovered
source has fresh authored checks, but rerunning owner-ROM tests on the final
commit needs the attachment again (PROJECT_PLAN session 5).

## Next reference work

1. Extend the input-stage oracle to spawn initialization, action setters/dispatch,
   and stationary/moving actions. Represent animation state where it affects
   transitions; declare object/camera/sound boundaries. libsm64 remains excluded
   as a fidelity oracle because it changes collision ordering.
2. Compare complete spawn/idle/walking/turning/stopping ticks on BOB before viewer
   integration, then airborne actions and long movement sequences.
3. Obtain original-execution traces from a matching US build and emulator. Those
   remain unavailable and are the eventual authority over native-host results.
4. Extend real gameplay replays to multiple presentation caps, slopes, walls,
   ceilings, ledges, landings, dynamic surfaces and interactions. A finite exact
   suite covers its cases/platforms, not all behavior.
