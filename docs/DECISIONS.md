# Foundation decisions — 2026-10-08

One package, Rust 1.90.0 / edition 2024, Linux x86_64 first. Headless import and
comparison work immediately without a GPU, ECS, universal VM, or plugin system.

| Module | Responsibility |
| --- | --- |
| import | Bounded reads, identity/version metadata, MIO0, segments, static scripts, collision, textures |
| content | Typed course/level/area/act IDs, placements/warps, static/dynamic collision, behavior registry, transitions |
| simulation | Exact 30 Hz scheduler and tick input edges; gameplay algorithms still missing |
| presentation | Immutable snapshots, wrapped-angle interpolation, discontinuities, graphics-only settings |
| trace | Initial-state/world/input metadata, exact float-bit comparison, first divergence |
| diagnostics | Independently authored fixtures and a synthetic counter replay |
| Application binary | CLI, private exports, and diagnostics |

Use sha1 0.10.6 for the upstream fingerprint instead of inventing a hash;
this is revision matching, not a security/authenticity guarantee. Serde 1.0.228
and serde_json 1.0.145 provide inspectable reports/traces. Exact direct versions
and Cargo.lock pin dependencies. No math/physics library affects gameplay.
Use wgpu with winit for M1: the engine owns GPU pipelines and the application
loop. Compatible versions will be pinned and built when introduced; neither is
a current dependency or a validated backend.

US v1.0 is exactly 0x800000 bytes. Supporting three byte orders now is cheap;
other regions, hacks, or padding require explicit adapters. US offsets stay in
import/version.rs. BOB entry discovery matches the unique aligned upstream
INIT_LEVEL plus exact segment-7 load pair instead of guessing a symbol offset.
The owner-ROM check validates that structural match on real bytes; BOB's entry
is at 0x0E000264 for this revision.

MIO0 adapts the MIT tooling algorithm with stream bounds, an allocation cap,
strict malformed-token rejection, and byte-wise overlapping copies. Collision
record widths follow the CC0 loader and preset tables, including the force word
for SURFACE_0004 omitted by one older tooling decoder. Integer values and surface
order remain intact. Original collision queries/partition ordering are missing.

Modern macro-object records use the pinned loader's five-short layout, preset
bias of 31, seven packed yaw bits, and termination rules. Keep the original word,
signed positions, exact angle units, raw parameter word, and source order.
Do not apply preset default parameters or respawn decisions until that runtime
exists; diagnostics identify each missing preset behavior. A first short in
0..29 selects the older hardcoded format and fails explicitly. Bounds, segment
crossing, preset-table size, and placement limits are checked.

Static extraction follows local JUMP_LINK/RETURN calls and preserves placement
encounter order. Original runtime loading prepends spawn/warp nodes; future
simulation must account for that before assuming object update order. The
extractor is not a gameplay VM. Unresolved calls, geometry, native callbacks,
and behaviors produce diagnostics. Unknown/unimplemented opcodes fail with a
location. Behavior scripts must resolve to stable BehaviorId values before
runtime use; raw ROM pointers stay in import data. Static and dynamic collision
have separate storage. Course rules do not enter the application loop.

The scheduler accumulates elapsed nanoseconds times 30 in u128, consuming
1,000,000,000 phase units per tick. This avoids rounding 1/30 s to integer
nanoseconds. Wall time only schedules ticks. Each diagnostic frame drains at
most eight ticks and retains all backlog. Presentation holds the latest pose
during backlog. The future window app should stop elapsed-time accumulation on
pause/focus loss and reset its wall-clock anchor on resume, retaining existing
tick backlog.

Interpolation uses completed snapshots for the same entity/epoch/animation.
Spawn, animation switches, teleport/death/area transitions snap. A discontinuity
flag covers same-area teleports; despawn removes the displayed entity.
Angles wrap as i16 on the shortest arc, with the negative arc for a half-turn
tie. Alpha is clamped and non-finite alpha snaps. No displayed pose can mutate
authoritative state. Lighting/shadow flags are configuration scaffolding only.

Completed-snapshot interpolation has about one tick of presentation delay
(roughly 33.3 ms relative to an extrapolated current pose). No real display/input
latency is measured yet. Scheduler comparisons are synthetic coverage only.

Exports use normalized hash/schema keys and reject existing destinations.
Importer schema 2 adds macro placements and explicit partial-import status. An
export's manifest describes decoded content rather than asserting a developer's
integration-test result. Full collision and macro records now match independently
expanded source macros; the ignored owner-ROM check pins schema-2 compact JSON
digests of those records. See ROM_VALIDATION.md for the validation boundary.
Cache reuse and save/settings persistence remain future work. The initial
oracle target is the pinned unmodified US decompilation/original ROM execution.
There is no oracle build/exporter yet. No C runtime integration was introduced.

New engine code uses MIT; adapted MIT/CC0 portions retain notices in LICENSES/.
Project code licenses do not grant a license to Nintendo ROM content.
