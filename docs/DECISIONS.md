# Foundation decisions — 2026-10-08

Cargo workspace, Rust 1.99.0 / edition 2024, Linux x86_64 first. The `rustario64`
core crate stays GPU-free so import, simulation, and replay comparisons run
headless. The optional `rustario64-render` crate depends on the core; the core
never depends on it. No ECS, universal VM, or plugin system.

| Module | Responsibility |
| --- | --- |
| import | Bounded reads, identity/version metadata, MIO0, segments, static scripts, geo layouts, Fast3D, collision, textures |
| content | Typed course/level/area/act IDs, placements/warps, static/dynamic collision, behavior registry, transitions, engine-owned visual models |
| simulation | Exact 30 Hz scheduler/input edges, original collision and math, Mario's core update, non-object actions and per-frame tick; objects, camera and cutscene/submerged actions still missing |
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

The foundation's generic interpolation uses completed snapshots for the same
entity/epoch/animation marker. Played Mario uses a constant marker for world
placement and a separate model drawer for the joints; his clip changes now
interpolate (session 14). Spawn and teleport/death/area transitions snap. A
discontinuity flag covers same-area teleports; despawn removes the displayed entity.
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

- `MarioState` mirrors struct MarioState with original member widths and
  names; Mario's object (`obj`), body state (`body`) and camera status
  (`camera_status`) are nested as the original's pointers are. The object's
  `gfx` fields are written by gameplay code and read back by it (the floor
  fallback reads `gfx.pos`), so they are authoritative state.
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
- `mario/constants.rs` (2,246 constants) is generated by
  `oracle/tools/gen_constants.py`, which evaluates every name in selected
  families (actions, inputs, flags, sounds, surfaces, animations, object fields
  and flags, camera modes and events, warps, ...) from the vendored headers
  with a C compiler and gives each its declared C type. The oracle exports the
  same table, and a CI test compares every value.

Not ported: the bully collision helpers. Sound playback has no gameplay state;
calls are recorded as events (below).

## Pre-action input stage — 2026-10-08

`simulation/controller.rs` translates the connected-controller path and analog
normalization from `game_init.c`. `simulation/mario/inputs.rs` translates
`update_mario_inputs`, its helpers and floor classification. No new runtime
libraries. Controller sampling occurs once per 30 Hz tick; held-button history
is an initial replay condition. No added dead zone or buffering. Original f32
operation order and signed angle wrap are retained.

`MarioState`'s zeroed `Default` is fixture storage (and the fresh-boot BSS the
level entry starts from), not spawn initialization. Camera yaw and movement
flags live in `StepWorld`; the interaction status and collided types live in
Mario's object. There is no reference camera/object implementation.
`obj.gfx.pos`, used by the original floor fallback, is written only by
authoritative step/action code; never copy an interpolated presentation pose
into it. A missing floor returns a must-use death-warp request and records a
warp event. Lives, warp timers and saves are future level logic.

The native oracle now runs the vendored mario.c for this stage (it first used
seven verbatim excerpts). Debug text is disabled; warp calls are recorded at the
boundary. The original empty step stub runs. Native host results are still not
original N64 execution evidence.

Both schema-1 producers carry independent state between ticks. Every exposed
field is recorded, floats as bits. Zero action timers/RNG and empty objects are
explicitly excluded placeholders. The digest covers ordered collision words
and trig tables; initial snapshots include level/terrain, controller history,
camera/object context. The BOB fixture converts raw `MARIO_POS` yaw degrees with
`(i32::from(yaw) * 0x8000 / 180) as i16`, like `level_cmd_set_mario_start_pos`.
It does not claim an original spawn. Render schedules only sample completed
snapshots, snapping geometry corrections. They never add gameplay ticks.
The exporter uses the existing serde_json version as a dev dependency and
refuses an existing output directory. Keep the exported traces private.

## Mario animation import — 2026-10-09

Mario's animations are game data loaded from the owner's ROM: the DMA table
(`gMarioAnims`) at 0x4EC000 (sm64tools' US map) decodes into engine-owned
`content::animation` records with every attribute's frame range bounds-checked,
so later frame lookups cannot read outside an entry. Each table entry decodes
from its own DMA range, as the game loads it, even where two entries share
arrays. `tools/check_mario_anims_reference.py` rebuilds the N64 table from a
clean pinned checkout's `assets/anims` sources with the decomp converter's rules
and matches it byte for byte with the ROM; only a canonical digest is stored.

## Original headers in the oracle — 2026-10-09

The oracle compiles against the real vendored decomp headers and whole files
(`mario.c` and the five non-cutscene action files) instead of hand-copied
declarations. Items from files that pull in large subsystems are generated as
verbatim excerpts by `oracle/tools/extract_excerpts.py` with per-item hashes.
The SDK-derived `include/PR` headers are not vendored; small authored shims
declare the types they name. Calls the vendored code makes into systems outside
the comparison are defined in `runtime_glue.c` as recorded boundaries, explicit
inputs or aborting stubs (objects). Implicit function declarations are build
errors, because C would silently pass floats as doubles to an undeclared
function; two excerpts had exactly that bug until their includes were fixed.

## Mario's update and actions — 2026-10-09

`simulation/mario/` follows the original file split: `core` (mario.c),
`inputs`, `step`, `animation` (the mario.c helpers and graph_node.c's frame
update), `interaction` (the interaction.c parts Mario needs without objects),
one module per action file, and `tick` (the frame around Mario's object).
Functions keep their original names, so each can be read next to its source.

- **Globals.** Every global the code touches is a `StepWorld` field:
  timers, level/area, controller 1, the area camera's mode/yaw, movement flags,
  save inputs, interaction.c's statics, the floor-align matrices, the animation
  DMA buffer, and Mario's platform.
- **Boundaries.** Calls into sound, the camera, the level runtime and particle
  effects change no simulated state, so they become `Event`s recorded in call
  order and compared with the oracle's log. Warps are requests: the level
  runtime that would execute them (and set Mario's invincibility timer) is not
  ported, so a warp event ends faithful comparison for that run. The cutscene
  and submerged groups record an unsupported event and end the action loop,
  exactly as the oracle's stubs do.
- **Objects.** No objects exist yet. Code that would dereference a held, ridden,
  used or interacting object panics; the hold/ride/pole/cannon actions panic at
  dispatch. Object handles (`ObjectId`) are reserved so those paths can be
  filled in without changing state shapes.
- **Numerics.** C promotions are explicit: s16 values widen to int and stores
  truncate; float-to-integer conversions go through `f32_to_s32`/`f32_to_s16`,
  which panic where the N64 would raise its "speed crash" exception rather than
  saturate; double-precision literals (6.25, 0.9, 0.05) compute in f64; missing
  returns use the decomp's `AVOID_UB` values (`act_air_hit_wall` returns the
  animation frame).
- **Animation as gameplay.** Actions read the animation frame, which the
  original advances during rendering. `animation::update_animation_frame` is the
  authoritative part of `geo_set_animation_globals`; the tick schedules it where
  `render_game` runs, after the object update, and presentation never calls it.
  The animation in the DMA buffer is tracked by table entry instead of pointers.
- **Lints.** Translation-only lints (manual clamps and range
  checks, literal precision) are allowed in this module so comparisons and
  literals stay as written.

## Full-tick oracle — 2026-10-09

`oracle/c/tick.c` keeps the decomp's own state (MarioState, Mario's
`struct Object`, body and camera-status state, globals) across frames and runs
each frame in the original order; Rust runs the same level entry and inputs on
its own state. Each side reports named 32-bit words, so the comparison needs no
shared struct layout and every word has a readable name; the Rust names live in
`oracle/src/tick_trace.rs`. A Rust panic ends that run's comparison at the
previous tick instead of letting the C side reach an abort.

The level entry is `init_level`'s no-warp branch from a fresh boot
(`init_mario_from_save_file`, spawn, `init_mario`, `ACT_IDLE`), which demos and
the level select use; the painting entry needs the cutscene group. bhvMario's
pre-loop commands, the object-pool slot values and the render condition are
authored on both sides because their original files need the object system;
everything else runs original code. Scenarios use the existing schema-1 trace
format with the words as fields, so the CLI comparator and private exports work
unchanged.

## Viewer play — 2026-10-09

`rustario64::play` (core, GPU-free) drives the compared tick from held
controls; the viewer maps keys and draws.

- **Same code path.** A `Session` enters the level with `tick::enter_level` and
  advances with `tick::tick`, the functions the oracle compares. The viewer's
  entry is `LevelEntry::script_start`, built from the import: the script's Mario
  start, the start area's terrain type, and the camera mode of the area's
  GEO_CAMERA node, which create_camera copies into the mode and default mode.
  The oracle's native setup converts from the same `LevelEntry`.
- **Camera yaw.** (Superseded in session 12: the session runs the original
  camera; see "Complete BOB reference camera" below.) The reference camera is
  not ported. A follow camera supplies the yaw: it starts behind Mario and turns only on request, 0x300 per tick,
  after Mario's update as the original camera updates after Mario. The yaw is
  part of each tick's input, so play is reproducible; it does not claim to be
  the original camera. Only the eye position (interpolated yaw, fixed distance
  and height) is presentation.
- **Keyboard to stick.** Keys give raw stick bytes past the original dead zone
  and clamp (80, or 57 per axis on diagonals); the walk modifier gives about
  half the magnitude. A button pressed since the last tick counts as held for
  the next tick, so a tap shorter than a tick still reaches the game, as a
  press held through one controller poll would. Each tick still reads one
  sample; nothing is buffered or repeated.
- **Stops.** Play stops where faithful simulation stops: a panic on a path the
  port does not implement, an unsupported action group, or a warp request
  (falling onto a death plane). R re-enters the level. Nothing invented runs
  past those points.
- **Input logs.** Every tick's `TickInput` is kept. `trace::InputLog` (schema 1:
  producer, ROM identity, entry name, 30 Hz, inputs) stores one run, and the
  oracle's `tick_trace --inputs` replays it against the native decomp. Logs are
  the player's inputs rather than ROM data, but they go to private directories
  like traces.
- **Presentation.** Mario's pose interpolates between the last two completed
  ticks; paused and stopped sessions hold the latest pose. The renderer places
  a model with a per-model translation and yaw (the original's convention: yaw
  0 faces +Z, positive turns toward +X). Mario is a hitbox-sized placeholder
  box until his geo layout and display lists are imported and posed from the
  animation table.
- **Lifetime.** The viewer keeps one level for the life of the process and
  leaks its collision, trig tables and animations once, so sessions borrow them
  for `'static` without self-referential state.

## Mario's model — 2026-10-09

- **Import.** `import::mario` scans `level_main_scripts_entry` (segment 0x15)
  for group0's loads and MODEL_MARIO's layout instead of assuming addresses, and
  requires the ranges to equal sm64tools' config. The generic geo decoder
  decodes `mario_geo`. Native callbacks resolve by address through
  `version::MARIO_GEO_CALLBACKS`, located by tools/check_mario_model_reference.py
  (a parallel walk of the ROM's layout and the pinned source); an unknown
  address fails the import rather than being skipped.
- **Builds per draw list.** Which display lists Mario draws depends on his
  switches (body variant, level of detail, eyes, hands, cap, wings), and the
  RSP/RDP state and vertex cache carry from one list to the next in layer order.
  So a traversal yields a draw list, and each distinct draw list is built once
  with the existing Fast3D builder, which now tags each vertex with the matrix
  node current when it was loaded. The segments stay in memory for these builds
  and are never exported.
- **Posing.** `presentation::mario` follows rendering_graph_node.c: the object
  matrix (position and angles, or the floor-alignment matrix the tick's render
  stage now reports), the animation type from geo_set_animation_globals,
  animated parts consuming attributes in traversal order, and Mario's callbacks
  from his body state. Matrices use the ROM's trig tables; the CPU skins
  positions and lit normals once per tick, and frames interpolate between the
  last two ticks of the same build (otherwise they snap).
- **Read-only callbacks.** The original callbacks also write the body state
  while drawing: they zero the torso and head angles outside the actions that
  use them, and count the punch scale down once per frame. Presentation never
  writes simulation state, so it draws the zeroed angles and keeps the punch
  countdown itself, restarting when a tick sets a new punch state (a repeated
  identical punch state does not restart it). The tick does not model these
  writes either, on both sides of the oracle; only drawing reads them.
- **Entry and level of detail.** The original renders a level's first frame
  after Mario's first update, so the entry state (no animation yet) is not
  drawn. The level-of-detail distance is the depth of Mario's origin in front
  of the presentation camera, which is the follow camera rather than the
  original camera, so the distances (not the thresholds) differ from the game's.
- **Switch parameter.** geo_layout.c stores GEO_SWITCH_CASE's parameter as
  `numCases` and starts every switch at case 0 (its comment calls the parameter
  the initial case). The decoder's field is now `num_cases`, and the static
  model builder draws case 0 for switches it cannot run; BOB's models have no
  switches, so its import is unchanged.

## Reference camera foundation — 2026-10-09

`simulation::camera` ports 33 functions from the same pinned CC0 camera.c:
scalar/vector approaches, C-button precedence, angle/distance/rotation helpers,
strict rotated trigger bounds, area clamps, camera wall/vertical corrections,
Mario's camera geometry queries, slope pitch, floor/water height offsets,
focus placement and radial goal construction. No new runtime dependency.
Globals are explicit parameters or `RadialState`; collision flags are passed
by mutable reference where the original changes them. The core stays GPU-free.

Important source behaviors are preserved: integer promotions before division,
s16 stores and wraparound (including MIN increments/divisors), pointer and
value float-approach forms differing on multipliers above 1, camera-filtered
surface types but gameplay-filtered heights, last-wall reuse in
`collide_with_walls`, and the repeated water query. An intangible floor with
no lower contact retains a height in the original collision query; camera.c
then dereferences NULL. Rust panics with an explicit message on that boundary,
rather than supplying a made-up surface type. Pole height offsets require
objects and panic before dispatch.

The development oracle compiles generated verbatim excerpts of those functions
with the original headers; an authored adapter holds the same process-wide
lock as all other C comparisons. `RadialGoal` is an intermediate goal, not a
complete camera tick. BOB's `sCamBOB` table is explicitly unused in the pinned
source and is not activated here. The full mode dispatcher, radial/free-roam
movement, obstruction rotation, zoom and initialization still need independent
state and complete per-tick comparisons. The persistent Lakitu/transition stage
is now implemented and documented below, and the complete BOB camera
(session 12) replaced the viewer's follow camera. No partial reference mode
is advertised as faithful, and no graphics setting changes movement inputs.

## Desktop playtesting and GUI — 2026-10-09

Windows x86_64 and Linux x86_64 are early distribution targets. CI builds the
two runtime binaries natively on Windows Server 2025 and Ubuntu 24.04, without
building the C oracle; native core tests run on both. The Linux headless job
still owns the native-C/GPU comparison suite. Windows native oracle fidelity
is not implied by a successful Windows runtime build.

`tools/package_desktop.py` uses an explicit file allowlist: runtime binaries,
playtest instructions, project/upstream licenses and provenance. It collects
third-party notices from Cargo's target-filtered runtime dependency graph
(excluding dev dependencies and the oracle), fails if license files are absent,
and includes the notices in the ZIP. It never recursively packages a checkout,
target directory, ROM, cache or trace. ZIPs are CI artifacts for review/testing,
not public release announcements. Physical GPU/driver and desktop OS smoke tests
remain separate from compile and authored-fixture evidence.

Before broader human movement testing, add a small launcher and pause/settings
UI in the application/render crate. The launch flow selects a local ROM,
validates its existing fingerprint adapter, shows useful errors and starts BOB;
remembering the local path must be opt-in. Keep settings/saves outside asset
caches. Use the existing winit/wgpu window and select a permissively licensed UI
and native-dialog library only after verifying compatibility with the pinned
wgpu release; no extra engine/framework dependency is added speculatively.
Graphics controls read/write presentation options only. Camera, input and future
gameplay modes must remain separate, named and included in replay metadata.
Opening settings must pause ticks, clear elapsed backlog on resume, and release
held/latching input so menu keys cannot cause gameplay actions. These behaviors
need replay/pause regression checks before the UI ships.

## Persistent Lakitu and transitions — 2026-10-09

`simulation::camera::lakitu` owns the persistent camera/Lakitu/transition state.
It translates `update_lakitu`, `next_lakitu_state`, level-oriented mode
transitions, deterministic pitch/yaw/roll shakes, hit-shake requests and FOV
shake setup from the same CC0 revision. Mode controllers supply goals before
this stage; `last_frame_action` is written by the outer camera update afterward.
These stages are separate because their ordering affects yaw, transitions and
Mario's future inputs. No runtime dependency or frame-rate smoothing is added.

Preserve the source's compound float-to-angle conversion (truncate the whole
sum, then narrow), the special -0x8000 shake phase, pitch reconstruction when
only yaw shakes, asymmetric phase ordering, collision-filter flags that remain
set on corrected/missing floors, and final pitch clamp/mode copy even while
paused. The 94-word comparison record includes every modeled value read/written
by this stage and transition/hit setup, plus collision flags. Authored transport
generation shares field names/layout only; C and Rust behavior remain independent.

(Superseded in session 12: the shared original RNG is ported and both random
shakes run.) Active RNG-driven handheld shake and shock requests return
`Unsupported` before state mutation. Decay of existing handheld angle offsets with no active random
request is supported. The native adapter compiles full verbatim handheld/spline/
random-vector excerpts but aborts if an unavailable RNG call is reached. Its hit
function is renamed during preprocessing so the separate Mario tick harness
continues recording camera requests without applying them. FOV setup is compared;
the perspective node's FOV animation/rendering is not implemented here.

Authored goals and Mario paths drive persistent comparisons. Owner-ROM tests use
BOB collision, original trig tables and the imported spawn; they do not run the
complete mode dispatcher or validate original camera-relative Mario play. The
viewer is unchanged. Wall-avoidance/rotation and radial movement/zoom now have
the compared stage described below. Next: full radial and free-roam mode
controllers, course surface selection and initialization; then compare combined
camera-and-Mario ticks before replacing the viewer's follow camera.

## Camera obstruction and radial movement — 2026-10-09

`simulation::camera::obstruction` owns camera.c's vertex-based surface tests and
eight-probe wall scan. It calls the existing original collision query, preserving
the capped coarse radius, fine-radius growth only after a coarse hit, repeated
queries from the original probe, last-wall selection, near-wall flag clearing
and low-wall exclusion. Recompute the unnormalized integer cross product for
behind-surface tests; cached normalized plane tests change boundary rounding.
Integer products wrap under the native oracle's existing `-fwrapv` contract.

`simulation::camera::radial::RadialMovement` holds the area center and second
rotation flags. Shared movement/status/zoom/offset fields stay in `Rig`, so
controllers and Lakitu stages use one authoritative copy. Rotation surface entry,
return-to-middle, first/second rotation limits, obstruction limits, stationary
camera behavior, outward offsets and zoom follow the source, including conflicting
rotation flags and promoted (not prematurely narrowed) angle comparisons.
Other areas' outward-offset branches are retained and component-tested; this
does not enable those courses.

Native comparisons initialize each side once, then independently compose radial
movement, zoom, existing radial goals and Lakitu updates. An authored zoom event
toggles each side's own flags; no Rust result repairs native state between stages.
Record all 94 shared words plus second-rotation flags, area yaw and collision
flags. This composition deliberately omits mode input, camera height/pan, free
roam, surface-mode selection, initialization and outer dispatch. Those need their
own comparisons before this replaces the viewer camera. The runtime remains
entirely Rust; generated C excerpts are development-only, with pinned hashes.

## Complete BOB reference camera — 2026-10-09 (session 12)

- **One camera system, explicit inputs.** `simulation::camera::system::CameraSystem`
  holds camera.c's globals for the frame path (mode info, transitions, C-Up's
  stored camera, FOV state, Mario's camera geometry, the area's camera, Lakitu
  through the existing `Rig`, flags and timers). Each `update` takes a `Frame`:
  the collision world, trig tables, collision flags, controller 1, Mario's
  camera report (`PlayerCameraState`), his action and speed view, the shared RNG
  and the event list. The camera never reaches into Mario's state directly.
- **Only what BOB reaches, refused otherwise.** Ported: level entry, course
  processing for areas without trigger tables (BOB's surface rules), the radial,
  close (R/Mario), free-roam, boss-fight and C-Up
  modes, transitions, HUD status and the render pass's FOV and graph camera.
  A mode, transition or cutscene start outside that set, or a level whose
  original trigger table is not NULL, returns a typed `Unsupported` after the
  frame completes; play stops there rather than inventing behavior. Porting a
  new area adds its modes, triggers and cutscenes with their own comparisons.
- **No cutscene, spline or trigger data in the repository.** The oracle's
  excerpt holds only the functions above and the tables they read
  (`sZoomOutAreaMasks`); `sCameraTriggers` is authored all-NULL and the frame
  adapter checks each level's original entry by name from `level_defines.h`.
- **RNG ownership.** `simulation::rng::Rng` is gRandomSeed16. The game frame
  owns one seed and passes it to every caller in the original call order;
  today only the camera's handheld and shock shakes draw from it. Objects will
  share the same seed when they arrive, so their order relative to the camera
  is part of the simulation.
- **Frame order and Mario's camera requests.** `simulation::game::Game::frame`
  runs controller read, `gAreaUpdateCounter++`, Mario's object update,
  `update_camera`, then the render pass's perspective node, camera node and
  Mario's animation frame, and `gGlobalTimer++`. Mario's `set_camera_mode` and
  `set_camera_shake_from_hit` calls are recorded during his update and applied
  to the camera immediately after it, in call order, against the camera report
  he wrote the frame before; within his own update, his later reads of the mode
  see the requested mode (`StepWorld::set_camera_mode` mirrors it) as the
  shared `struct Camera` would show them. The per-frame comparisons cover this
  ordering.
- **The render stages are authoritative.** `geo_camera_fov` can set
  `CAM_FLAG_SLEEPING`, and `update_graph_node_camera` feeds the next frame's
  state, so both run in the simulation frame, not per rendered frame.
- **The area's camera node is imported and verified.** `GameEntry::script_start`
  takes the area's GEO_CAMERA node (mode, position, focus) and requires its
  callback and the enclosing perspective node's callback to equal the version
  adapter's `geo_camera_main` and `geo_camera_fov` addresses.
- **Presentation.** The viewer draws from the graph camera's position, focus,
  roll and field of view, interpolated between the last two completed frames
  (roll by the shortest signed difference) and snapped when either point moves
  more than `CAMERA_CUT_DISTANCE` (1,500 units) in one frame. Mario's level of
  detail and C-Up head look read the same authoritative camera; nothing
  presentation-side writes back. The free inspection camera remains a separate
  presentation tool.
- **Input logs, schema 2.** `InputLog` gains `camera`: `recorded-yaw` (schema 1
  logs, where each input's yaw drove Mario) or `reference` (the yaw Mario read
  from the original camera; replays run the camera and check each yaw). Schema
  1 logs still load as recorded-yaw.
- **Controls.** The pad gains R and the four C buttons (arrow keys and E in the
  viewer). They use the same latch as A/B/Z: a tap between two frames reaches
  the next frame.


## Desktop launcher, pause, and Mario mesh switches — 2026-10-09 (session 13)

The same winit event loop, window and wgpu device serve the launcher and play.
The optional render crate uses egui 0.36.2 and its winit/wgpu integrations;
wgpu remains 30.0.1. rfd 0.17.2 provides a local native file picker. Launcher
path entry and drag/drop remain available if the desktop portal is unavailable.
A full import must succeed before replacing the launcher; validation errors
remain visible and recoverable. Only presentation options and an opted-in ROM
path are persisted in the platform user config directory, outside asset caches.
Neither the launcher nor runtime uploads ROM data. CLI diagnostics still work.

This increment provides a **development pause**, not the original game's pause
mode. Esc or lost focus freezes the complete game frame, including the camera.
Pause, resume, mode switches and focus loss clear held input/tap latches and
reset the presentation clock; no menu time or pending catch-up backlog reaches
gameplay. Running stalls still retain backlog, draining up to eight ticks per
frame. Snapshot history is snapped on resume without changing Mario, camera,
RNG or the logged input stream. `zoom_out_if_paused_and_outside` remains ported
but unreached. No behavior-changing option is added to the faithful profile.

The reported recurring animation stutter was traced to `MarioDrawer::frame`:
it interpolated only when consecutive poses had the same draw-list build.
Mario's blink cycle changes those lists, so it periodically snapped his entire
skinned model. Geometry changes now skin the newly selected mesh at both tick
endpoints, traversing yesterday's skeleton with today's discrete switches and
LOD. This also initializes previously inactive bones; unrelated vertex arrays
are never blended. Animation-entry changes initially snapped (superseded by
session 14); level entry still snaps. The authoritative
30 Hz animation advancement is unchanged. Authored switch/material/animation
regressions and an owner-ROM test of blink cycles plus three detail levels cover
the fix. Interpolation still adds approximately one simulation tick of delay.

## Animation transition interpolation — session 14

The follow-up landing/turn/run report exposed the remaining animation-entry
guard in `MarioDrawer::frame`. It snapped the entire model to the latest tick
when the clip changed, skipping the intermediate pose and root placement while
the camera continued to interpolate. The previous mesh-switch fix already makes
both endpoints refer to the same geometry, so clip identity is not a reason to
discard that interval. Jump, landing, run, braking and idle changes now blend
between the two completed, skinned poses over one simulation interval. There is
no extended crossfade, animation-clock resampling or extra animation advancement.

Each 30 Hz endpoint remains the original completed pose. Geometry/material
choices still come from the current tick at both endpoints; incompatible vertex
arrays are never blended. Hidden poses, level re-entry and explicit pause/resume
history resets retain their snap behavior, as do disabled interpolation and
non-finite alpha. Simulation, input timing, camera state and replay formats are
unchanged. Tests reproduce five landing transitions among 17 real clip changes
in 540 owner-ROM BOB frames and cover different authored joint poses plus a
simultaneous clip/geometry switch.

## Object system and coins — 2026-10-10 (session 17)

**Mario's object lives in `MarioState`.** Mario's update reads and writes
`m->marioObj` constantly; keeping that object beside the rest of his state
avoids a pool borrow on every access. His pool slot still exists and is linked
into OBJ_LIST_PLAYER in the original position, so list order, the free list and
slot numbers are the original ones; `object`, `object_mut` and `pair_mut`
resolve any handle (a slot, Mario's, or gMacroObjectDefaultParent).

**The ROM's behavior scripts run, not re-authored ones.** Segment 0x13 is
loaded as the main level scripts load it, and `cur_obj_update` interprets those
words in place. Addresses stay segmented (0x13xxxxxx) wherever the original
holds a virtual address (`behavior`, `curBhvCommand`, the behavior stack), which
is a one-to-one renaming the snapshots compare directly. CALL_NATIVE targets
resolve through the version adapter's name/address table to Rust translations;
`tools/check_behavior_reference.py` derives every address from the pinned
source laid over the ROM. CI tests without a ROM use `authored_scripts`, which
encodes the same commands for the ported scripts; the oracle runs the verbatim
`behavior_data.c` excerpt, so a re-encoding error would fail the comparison.

**A level spawns only fully ported placements.** Before spawning, a script is
followed through every command it can reach (GOTO, CALL, SPAWN_CHILD and the
behaviors natives spawn) and must use only ported commands, natives and flags.
Anything else is recorded in `AreaObjects::skipped` with the reason, never
silently spawned with missing logic. The native harness receives the same
filtered placements, so both sides run the same subset. This changes slot
assignment and removes the skipped objects' RNG draws relative to the full
original level; comparisons are therefore of the subset, which the docs state.

**The render pass's object writes are simulation.** `geo_switch_anim_state`
resets an object's `oAnimState` when it reaches the switch's case count, and it
runs only while `obj_is_in_view` accepts the object from the current camera.
That changes gameplay state (and coin animation phase), so the simulation runs
the pass's object traversal each frame from the authoritative graph camera
with the original matrices. Display lists, interpolation and drawing remain
presentation. Graphics settings (resolution, aspect, FOV options) must not feed
this stage; it uses the original perspective node's field of view (the
original view test has no aspect term).

**Particles are not spawned.** Mario's particle flags are still computed and
compared, but `spawn_particle` and the particle behaviors are not ported, on
either side. The 100-coin star is recorded as an unsupported event, like
other unported outside calls.
