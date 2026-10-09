# Foundation decisions — 2026-10-08

Cargo workspace, Rust 1.99.0 / edition 2024, Linux x86_64 first. The `rustario64`
core crate stays GPU-free so import, simulation, and replay comparisons run
headless. The optional `rustario64-render` crate depends on the core; the core
never depends on it. No ECS, universal VM, or plugin system.

| Module | Responsibility |
| --- | --- |
| import | Bounded reads, identity/version metadata, MIO0, segments, static scripts, geo layouts, Fast3D, collision, textures |
| content | Typed course/level/area/act IDs, placements/warps, static/dynamic collision, behavior registry, transitions, engine-owned visual models |
| simulation | Exact 30 Hz scheduler/input edges, original collision/math/Mario physics steps; actions and per-tick gameplay still missing |
| presentation | Immutable snapshots, wrapped-angle interpolation, discontinuities, graphics-only settings |
| trace | Initial-state/world/input metadata, exact float-bit comparison, first divergence |
| diagnostics | Independently authored fixtures and a synthetic counter replay |
| Core binary | Headless CLI, private exports, and diagnostics |
| render crate | wgpu renderer, offscreen capture, winit development viewer |

Use sha1 0.11.0 for the upstream fingerprint instead of inventing a hash;
this is revision matching, not a security/authenticity guarantee. Serde 1.0.229
and serde_json 1.0.151 provide inspectable reports/traces. Compatible SemVer
requirements allow deliberate updates; Cargo.lock and `--locked` pin actual
builds. No math/physics library affects gameplay.
wgpu 30.0.1 and winit 0.30.13 (with pollster 1.0.1 and png 0.18.1) back the
renderer: the engine owns its pipelines and the application loop. All four are
the current stable releases checked on 2026-10-08 and declare permissive terms
(see PROVENANCE.md). The development oracle builds with cc 1.6.0.

Rust 1.99.0 is the current stable release, pinned to keep local and CI builds
aligned. The former compiler baseline and two-week dependency age restriction
had no project compatibility requirement and are removed. Update stable tooling
and compatible dependencies deliberately, then run authored, GPU, and owner-ROM
checks, including optimized native-decomp comparisons. Keep the original decomp
reference revision pinned: that defines behavior, independently of tool versions.
CI reads `rust-toolchain.toml` rather than duplicating the Rust version and uses
the SHA-pinned actions/checkout v7.0.1.

US v1.0 is exactly 0x800000 bytes. Supporting three byte orders now is cheap;
other regions, hacks, or padding require explicit adapters. US offsets stay in
import/version.rs. BOB entry discovery matches the unique aligned upstream
INIT_LEVEL plus exact segment-7 load pair instead of guessing a symbol offset.
The signature derives its range and segment ID from the version adapter.
The owner-ROM check validates that structural match on real bytes; BOB's entry
is at 0x0E000264 for this revision.

MIO0 adapts the MIT tooling algorithm with stream bounds, an allocation cap,
strict malformed-token rejection, and byte-wise overlapping copies. Collision
record widths follow the CC0 loader and preset tables, including the force word
for SURFACE_0004 omitted by one older tooling decoder. Integer values and surface
order remain intact. Original collision queries/partition ordering are now
ported and checked against the native decomp oracle below.

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

Area metadata follows level_script.c/area.c: two dialog slots default to 0xFF,
terrain type starts at zero and TERRAIN_TYPE bitwise-ORs its word, and music words
retain their signed 16-bit values with last-write behavior. SHOW_DIALOG indexes
outside the two slots are reported and ignored as the original ignores them.
This preserves data for future gameplay/audio; it does not execute either.

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

Exports use normalized hash/schema keys and reject existing destinations. Files
are written in a sibling staging directory, cleaned on errors, then renamed
into place under an OS file lock shared by exporters. The lock file remains
empty in the export parent so future processes lock the same inode. A process
interruption may leave an ignored staging directory; it is never reused as a
complete export. Importer schema 4 (current) adds area terrain/dialog/music;
schema 3 added visuals and schema 2 added macros. An export's manifest describes
decoded content rather than asserting a developer's integration-test result.
Collision and macro content expectations come from the pinned source through
`tools/check_bob_reference.py`; canonical compact JSON sorts object keys while
preserving arrays. Field declaration/JSON key order is not asset correctness.
See ROM_VALIDATION.md for the reproducible commands and validation boundary.
Cache reuse and save/settings persistence remain future work. The initial
oracle target is the pinned unmodified US decompilation/original ROM execution.
The native component oracle exists; an original-execution per-tick exporter is
still missing. No C runtime integration was introduced.

New engine code uses MIT; adapted MIT/CC0 portions retain notices in LICENSES/.
Project code licenses do not grant a license to Nintendo ROM content.

## Visible geometry import — 2026-10-08

Dependent segments are loaded from the ROM ranges named by the level script's
own LOAD_RAW/LOAD_MIO0 commands, after the script itself has been located through
the pinned manifest. Segment 7's script range must equal the manifest range. The
loaded ranges also match sm64tools' pinned US block boundaries. Segments loaded
by the global main scripts (0x15 level scripts, 0x03/0x16 common1, Mario's) are
not loaded yet; models that live there are reported.

Geo layouts decode into an import-stage arena graph with the original stack,
GEO_END/RETURN semantics, node depth list, and attachment rules (a second
depth-zero node is kept as "detached", as the original discards it). Drawing
layers live in the upper flag byte as in init_graph_node_*. Rotation fields go
through `(deg << 15) / 180` exactly. Callbacks (GEO_ASM, switch, camera,
background, held object) are recorded and reported, never executed.

Fast3D is interpreted at import, not emulated per frame. The interpreter keeps
the RSP/RDP state the original frame relies on: init_rsp/init_rdp defaults,
then for each master-list entry G_ZBUFFER and the layer render mode from
renderModeTable_1Cycle/2Cycle (values computed by compiling the pinned gbi.h).
Display lists then override state as in the original. Triangles become
non-indexed batches in original draw order; consecutive triangles with equal
materials merge. Batches are drawn by layer 0..7, then batch order, matching
the master-list order. Textures are decoded from source memory through the load
record that fills the render tile's TMEM address (LOADBLOCK uses the render
tile line as row stride; LOADTILE uses the image width). TMEM interleaving is not
emulated, which is equivalent for these loads. Unsupported commands and
approximations (TEXEL1 sampled as TEXEL0, ignored G_MTX/segment moves) are
reported with their segmented addresses.

Geo transforms are baked into presentation vertices with float sin/cos. That is
presentation-only; gameplay must use the original sine table and operation order.
Static resolution of runtime nodes: switch nodes draw their initial case, LOD
nodes resolve at distance zero, animated parts draw at rest, billboards do not
face the camera.

Texture formats follow sm64tools n64graphics.c channel expansion (SCALE_5_8,
SCALE_4_8, SCALE_3_8), except I4/I8 alpha equals intensity, as the RDP presents
TEXEL alpha. Validation compares against the decomp toolchain's PNGs for the
formats BOB uses (RGBA16, IA16).

## Rendering — 2026-10-08

The shader evaluates both RDP combiner cycles from the decoded G_CCMUX/G_ACMUX
selectors (noise as 0.5; key, K4/K5, and LOD fractions as 0). Translucency is
FORCE_BL with (CLR_MEM, 1-A) in the blender cycle that writes memory; cutout is
CVG_X_ALPHA or an alpha-compare threshold, discarding alpha below 0.5. Decals use
a negative depth bias with less-equal compare. Front faces are counter-clockwise;
authored GPU tests and the BOB render (terrain survives back-face culling)
support this.

Lighting is one directional light plus ambient, evaluated per vertex. SM64 keeps
the view matrix in the modelview stack, so light directions are treated as
camera-space. This assumption is visually plausible but not yet verified against
original output. Fog uses the gsSPFogPosition contract from gbi.h:
`alpha = clamp(z_ndc * multiplier + offset, 0, 255)`, where z_ndc comes from an
OpenGL-style projection of the original frustum near/far (100/30000 for BOB).
The fogged combiner still sees vertex alpha as SHADE alpha (the hardware
substitutes fog), which matters only for fogged translucent combiners.

Original colors are display-referred: textures upload as Rgba8Unorm, offscreen
targets are Rgba8Unorm, and windows prefer a non-sRGB surface format (the shader
compensates if only sRGB is offered). The skybox is not imported; a placeholder
clear color stands in. N64 three-point filtering, dithering, and coverage
antialiasing are not reproduced; bilinear filtering and optional MSAA are used.

Graphics options (resolution, MSAA, fog/culling toggles, free camera) live only
in the render crate and cannot reach simulation state. The viewer's inspection
camera moves with wall-clock time because it is presentation-only; the window
loop still feeds the fixed 30 Hz clock, which will drive gameplay ticks. On focus
loss the viewer drops its wall-clock anchor so no elapsed time accumulates.

## Collision port and native oracle — 2026-10-08

`simulation/collision.rs` translates surface_load.c's static loader and
surface_collision.c's floor, ceiling, wall, water, and gas queries. It keeps the
original's representation choices: surfaces in allocation order identified by
index, 16x16 cells with separate floor/ceiling/wall lists for static and dynamic
surfaces, insertion sorted by the first vertex's height (with the original
s16 priority arithmetic), the 50-unit cell overlap with s16 wraparound,
integer cross products converted to f32, the double-precision reciprocal and
thresholds (0.0001, 0.01, 0.707), f32 query arithmetic in source order with no
fused operations, (s16) position casts, first-match returns, the 78-unit buffers,
the SURFACE_INTANGIBLE retry, and the four-wall reference limit. Globals become
explicit inputs: `CollisionFlags` (camera checks; the intangible flag that
find_floor clears) and a `pass_vanish_walls` argument for the current-object
test. Pool limits (2300 surfaces, 7000 nodes) are errors instead of silent
overflow. Rooms, object (dynamic) surface loading, and debug counters are not
ported yet. Environment regions are owned and mutable because behaviors rewrite
them. Multiple environment blocks in one stream are rejected because the
original keeps only the last one.

The oracle crate compiles byte-identical CC0 decomp collision files natively
with authored shim headers, `-fwrapv`, and `-ffp-contract=off`, and compares the
Rust port bit for bit: every surface field, every cell list, and floor/ceiling/
wall/water/gas query results including pushed positions and wall lists. It is a
development tool, never a runtime dependency (see oracle/README.md). Native x86
IEEE single precision is assumed to match the N64 for these operations; that is
an assumption until original-execution traces confirm it.

## Original math tables — 2026-10-08

gSineTable/gCosineTable and gArctanTable are game data in the engine segment,
so they load from the identified ROM at fixed US offsets and are checked against
pinned SHA-1 digests before use; the values are not committed. The offsets were
found by compiling the pinned decomp's table source with gcc and matching it
uniquely in the verified ROM; they agree with sm64tools' engine-segment mapping
(VRAM 0x80386000 and 0x8038B000). One contiguous 0x1400-entry table reproduces
the original's sine reads running into the cosine table. `sins`/`coss` take
integer expressions and cast them to u16 like the macros. atan2f keeps the
original double-precision expression. Geo-layout presentation transforms still
use float trigonometry; gameplay code must use these tables.

## Mario oracle selection — 2026-10-08

Audited [libsm64 at fd11813208272b4271d92bd92feb8f3fdbe61be5](https://github.com/libsm64/libsm64/tree/fd11813208272b4271d92bd92feb8f3fdbe61be5)
(CC0) against the pinned decomp. Its collision replaces the 16x16 partition with
a scan of every loaded surface and keeps the highest floor / lowest ceiling rather
than the first match in the original's sorted cell lists, takes the water level
from a Mario field instead of water boxes, and compares normals in f32. Its Mario
sources follow an older decomp revision with adaptations (terrain type, animation
loading, removed object and sound hooks). Those changes alter defined original
quirks, so libsm64 is **not** used as a fidelity oracle.

The per-tick Mario oracle will instead extend the native-decomp approach already
verified for collision and math: compile the pinned, unmodified mario.c,
mario_step.c, and action files against the vendored decomp collision, with
authored shims for objects, camera inputs, sound, and animation data (libsm64's
shims are a useful map of what needs stubbing). It records per-tick state in the
trace schema. A native build is still not original-hardware evidence; traces
from original execution remain the eventual authority, and differences between
the two must be explained before claiming fidelity.

## Mario physics steps — 2026-10-08

`simulation/mario/step.rs` is a direct translation of the pinned mario_step.c
(ground, air, and stationary steps, ledge grabs, gravity, vertical wind, moving
sand, windy ground, bonk reflection, velocity-from-angle helpers) and the four
mario.c helpers it calls. It runs on the ported collision and the ROM trig
tables, with original f32 operation order, s16 angle wraparound, and quirks such
as the quarter-step wall/ceiling order and the 160-unit ceiling-gap rule.

Representation choices:

- `MarioState` holds only the original fields this code reads or writes, named
  after the struct members, so each later port adds fields with their own
  checks. `gfx_pos`/`gfx_angle` mirror `marioObj->header.gfx`, which the step
  code writes; they are presentation inputs, not extra gameplay state.
- Surfaces are referenced by `SurfaceRef`: a collision index, or the step code's
  water pseudo-floor (`gWaterSurfacePseudoFloor`, used while riding a shell),
  which is not a collision surface.
- Globals become explicit inputs in `StepWorld`: collision world and flags, trig
  tables, `gGlobalTimer`, the area terrain type, the current level, and the
  pseudo-floor's origin offset (mutable, as the original rewrites it).
- Where the original would dereference NULL or read past a table (no floor, a
  NULL ledge floor, terrain type 7, an out-of-range moving-sand force), the port
  panics with a message instead of inventing a value. Those paths are outside
  supported coverage until original-execution evidence says what they do.
- Action, state, flag, step-result, surface, terrain, sound, and level constants
  in `mario/constants.rs` are generated by evaluating the pinned headers with a C
  compiler, and a CI test compares every value with the same headers compiled
  into the oracle. Constants that are not used yet are left out.

Not ported: mario_update_quicksand and mario_push_off_steep_floor (they change
actions and belong with the action port), the bully collision helpers, and sound
playback, which has no gameplay state. The oracle stubs the action setters to
abort, so a test that reached them would fail instead of passing silently.
