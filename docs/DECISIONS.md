# Foundation decisions — 2026-10-08

Cargo workspace, Rust 1.90.0 / edition 2024, Linux x86_64 first. The `rustario64`
core crate stays GPU-free so import, simulation, and replay comparisons run
headless. The optional `rustario64-render` crate depends on the core; the core
never depends on it. No ECS, universal VM, or plugin system.

| Module | Responsibility |
| --- | --- |
| import | Bounded reads, identity/version metadata, MIO0, segments, static scripts, geo layouts, Fast3D, collision, textures |
| content | Typed course/level/area/act IDs, placements/warps, static/dynamic collision, behavior registry, transitions, engine-owned visual models |
| simulation | Exact 30 Hz scheduler and tick input edges; gameplay algorithms still missing |
| presentation | Immutable snapshots, wrapped-angle interpolation, discontinuities, graphics-only settings |
| trace | Initial-state/world/input metadata, exact float-bit comparison, first divergence |
| diagnostics | Independently authored fixtures and a synthetic counter replay |
| Core binary | Headless CLI, private exports, and diagnostics |
| render crate | wgpu renderer, offscreen capture, winit development viewer |

Use sha1 0.10.6 for the upstream fingerprint instead of inventing a hash;
this is revision matching, not a security/authenticity guarantee. Serde 1.0.228
and serde_json 1.0.145 provide inspectable reports/traces. Exact direct versions
and Cargo.lock pin dependencies. No math/physics library affects gameplay.
wgpu 30.0.1 and winit 0.30.13 (with pollster 1.0.1 and png 0.18.1) back the
renderer: the engine owns its pipelines and the application loop. All four were
published more than two weeks before selection, support Rust 1.90, and declare
permissive terms (see PROVENANCE.md).

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
Importer schema 3 (current) adds visual.json, models.json, visual textures, and
dependent-segment/visible-geometry manifest fields; schema 2 added macro
placements and explicit partial-import status. An export's manifest describes
decoded content rather than asserting a developer's integration-test result.
Collision and macro record serialization is unchanged since schema 2, so the
ignored owner-ROM check keeps the same compact JSON digests for those records. See ROM_VALIDATION.md for the validation boundary.
Cache reuse and save/settings persistence remain future work. The initial
oracle target is the pinned unmodified US decompilation/original ROM execution.
There is no oracle build/exporter yet. No C runtime integration was introduced.

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
