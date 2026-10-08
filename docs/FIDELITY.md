# Comparison contract and coverage

Original movement, actions, camera, RNG, objects, and interactions have **zero
validated gameplay coverage**. Static collision loading and collision queries now
have exact coverage against the pinned decomp compiled natively (below); that is
component coverage, not per-tick gameplay coverage. The diagnostic marker
visualizes a tick counter; it is not a Mario approximation.

Target US v1.0 at n64decomp/sm64 revision
9921382a68bb0c865e5e45eb594d9c64db59b1af, ROM SHA-1
9bef1128717f958171a4afac3ed78ee2bb4e86ce, 30 ticks/second, reference camera/input
profile. Prefer unmodified matching N64/original execution; native-port float
differences need separate evidence. Physics-modified ports are not unquestioned
oracles. A supported owner ROM is available and static asset checks pass (see
[ROM_VALIDATION.md](ROM_VALIDATION.md)). A matching build, emulator trace setup,
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
identical input. Port and compare original stick normalization and button edges;
recording raw bytes alone does not implement them.

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
and macro records against source-derived digests, plus script counts and entry
metadata. It does not validate geometry rendering, all assets, or gameplay.

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
| Comparisons / non-trivial hits | 795,000; 48,806 floors, 14,040 ceilings, 25,548 wall pushes, 1,783 intangible-affected floors, water and gas hits | 4,064,920; 377,720 floors, 17,648 ceilings, 60,924 wall pushes |

Query points include random positions, every vertex with small offsets, all
cell borders with offsets around the 50-unit overlap, and large coordinates that
wrap through (s16) casts. Mutation checks: changing the cell insertion tie order,
the 78-unit floor buffer, or the wall push sign each fails the CI test. Rounding
instead of truncating the wall-query position was not detected, because the
original's 50-unit cell overlap makes it unobservable for these fixtures.

Math utilities: `sins`/`coss` for 196,608 integer inputs (three wraps), `atan2s`
and `atan2f` on 1.6 million points (signed zeros, infinities, equal magnitudes,
tiny and huge ratios), and `approach_s32`/`approach_f32` on 200,000 random cases
each match the natively compiled decomp bit for bit, both with authored tables
(CI) and with the owner ROM's tables (3,993,600 comparisons). NaN inputs to
atan2s are outside coverage.

Limits: dynamic (object) surfaces, rooms, and float-to-int casts of values
beyond the s32 range are not covered. Native IEEE single precision is assumed to
match the N64 for these operations until original-execution traces confirm it.

## Next reference work

1. Obtain an unmodified matching US reference build using the supplied ROM, or
   evaluate pinned libsm64 (CC0) as a Mario-movement oracle after auditing its
   changes to the decomp's movement and surface code.
2. Add a reference exporter around each completed simulation tick, reproducible
   initial world/state and tick input, and recorded build/emulator/platform
   configuration. Instrumentation must not change arithmetic or update order.
3. Capture a stationary spawn baseline, then running/turning/stopping/jump cases
   as Rust actions are ported. Compare all relevant fields each tick, exactly.
4. Replay genuine gameplay with multiple render caps/settings; extend to walls,
   slopes, ceilings, ledges, landings, dynamic surfaces, interactions, and long
   sequences. A finite exact suite proves its cases/platform, not all behavior.
