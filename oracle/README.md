# Decomp oracle (development only)

`rustario64-oracle` compiles a minimal, byte-identical set of CC0 sources from
[n64decomp/sm64 at 9921382a68bb0c865e5e45eb594d9c64db59b1af](https://github.com/n64decomp/sm64/tree/9921382a68bb0c865e5e45eb594d9c64db59b1af)
natively, so tests can compare the Rust collision, math, Mario step and input ports
with the original code bit for bit.

**Boundary.** This crate is a test/comparison tool. Nothing in the game runtime
or renderer depends on it, and it is never shipped. The C keeps global state, so
all access goes through one process-wide lock. It is compiled with `-fwrapv`
(MIPS integer arithmetic wraps) and `-ffp-contract=off` (no fused multiply-add),
giving native IEEE single-precision results. Agreement with N64 execution
still needs original-execution traces.
Float-to-integer casts of values outside the s32 range are outside coverage.

**Replacement plan.** Native compilation of the decomp is a practical oracle,
not original-hardware evidence. Once per-tick traces from original execution
cover these components, those traces become the authority and this harness can
be retired or kept only as a fast regression check.

## Vendored files (unmodified)

| Path under `c/decomp/` | Upstream path | SHA-1 |
| --- | --- | --- |
| `src/engine/surface_load.c` | same | `08d285b51bfcac820bc66115634048413a25ce7e` |
| `src/engine/surface_load.h` | same | `33ac6ed474e3fd448d7358c2e0fb8f39b63015b5` |
| `src/engine/surface_collision.c` | same | `0faf5848862dd3095bd5937465b099bf3f3755e1` |
| `src/engine/surface_collision.h` | same | `a1831d897984db03d0aad19788ef97187af2aa1e` |
| `include/surface_terrains.h` | same | `aad2a42d27af78761951f049123f87366c408892` |
| `include/special_presets.h` | same | `3b89bcccd3de40d53c875ceb4bc25b43c53fadfc` |
| `include/special_presets.inc.c` | same | `1c02b0ae05c082c0cdf831ced112d1fcd381a374` |
| `include/model_ids.h` | same | `6e5dc997e9f77988605aed5bb2d036bf9168f161` |
| `src/engine/math_util.c` | same | `e241d6af4c04fcdbb9fe6f09b7f8f1cdae5ce5d2` |
| `src/engine/math_util.h` | same | `daf60500cc3323860118a6597bfab20642413be9` |
| `src/game/mario_step.c` | same | `176c4b79766adc4b862198ce92fbc333dee088da` |
| `src/game/mario_step.h` | same | `44583bb1a487482b436d4553649ab427e5d7c9f0` |
| `include/sm64.h` | same | `a466574f93a5fe4f21a54c18de3237e52c28e34c` |
| `include/config.h` | same | `0023855e97abf59132cbb904263cf47bd72b1dab` |
| `include/macros.h` | same | `7c0f73ad49b3c5b3f9987492920c0474b99517da` |
| `include/platform_info.h` | same | `7ca56535b3e1b47d6b3b26735593e129c9f91170` |
| `include/object_constants.h` | same | `9d939f2cc292276a2bbce9741375056e9a3b6a78` |
| `include/sounds.h` | same | `66eac59b63b16d97049f5cc1a083c27c2f267c6f` |
| `include/mario_animation_ids.h` | same | `105601ac91f4adf8c135c2f539f1d4ca523a89d2` |
| `include/mario_geo_switch_case_ids.h` | same | `0e69b18d06be85c84cbfa8338ef0a66be71eb29b` |
| `include/level_table.h` | same | `d99c0bec67305cc442591b755288b08a0402762a` |
| `levels/level_defines.h` | same | `4dd4a6a66db8f4a440f8d2ddcaa3a96472b2d1e5` |

`c/oracle.c` also contains `spawn_special_objects` copied verbatim from
`src/game/macro_special_objects.c` (function text SHA-1
`92797dca1409789dd6c872b7f8e164143e10883f`), so the terrain stream is walked
exactly as the original walks it. Object spawning itself is stubbed.

For the Mario step checks, `c/oracle.c` also copies these items verbatim from
`src/game/mario.c` (item text SHA-1s):

| Item | SHA-1 |
| --- | --- |
| `sTerrainSounds` | `c29eca31e9991876f569dfe3229dbbaaa1a9baab` |
| `mario_set_forward_vel` | `595fd4f0e274ea10b0221242b4b7e1d359917084` |
| `mario_get_terrain_sound_addend` | `7a250287b4bd32343e287e940e8d9fb9acab1280` |
| `resolve_and_return_wall_collisions` | `0ce1df518da82902618e6b18d0a158af7f0d294b` |
| `vec3f_find_ceil` | `f0457bf24fc7347ae65e69fc8d2307b065438b8d` |

`set_mario_action`, `drop_and_set_mario_action`, and
`update_mario_sound_and_camera` are stubbed to abort, because the ported step
functions must not reach them; `play_sound` is a no-op. `oracle_mario_call`
copies a flat `OracleMario` record into the original `MarioState` fields the
step code uses, runs one function, and copies the fields back. The shim
`MarioState`, `Object`, and `Area` declare only those fields, so their layouts
are not the original ones. The oracle also exports the values of the decomp
constants that `src/simulation/mario/constants.rs` mirrors. The CC0
dedication is in `c/decomp/LICENSE-CC0.txt`. The trigonometric tables are game
data, so `c/shim/trig_tables.inc.c` declares empty storage that tests fill from
the owner's ROM or with authored values (`AVOID_UB` selects the contiguous
sine/cosine layout the original reads). The shim headers under `c/shim/` and
the glue code are newly authored (MIT); they declare only what the vendored files
need, so object and Mario structs there are not the original layouts.

## Tests

```sh
cargo test --locked -p rustario64-oracle
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test collision bob_collision -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked -p rustario64-oracle --test math rom_trig -- --ignored --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_step bob_steps -- --ignored --nocapture
```

The first runs on independently authored collision streams, terrain, and tables
(CI-safe); the others use BOB's real collision and the real trig tables from the
owner's ROM. Coverage counts are in [docs/FIDELITY.md](../docs/FIDELITY.md).

## Pre-action input oracle

`c/input_reference.c` contains these **verbatim function excerpts** from the same
pinned CC0 decomp. SHA-1 includes the function text and its trailing newline:

| Upstream | Function | SHA-1 |
| --- | --- | --- |
| `src/game/game_init.c` | `adjust_analog_stick` | `c18431659aa9f2a2af4526a55f1d1ee88036ff17` |
| `src/game/mario.c` | `mario_get_floor_class` | `0084735627314e7b8c086c5c8d2ac5f5e748d625` |
| Same | `mario_floor_is_slippery` | `db70023b656516ddc34fd49a5c6852f92e754e55` |
| Same | `update_mario_button_inputs` | `892d3aab8c1017863c24d712d6b732aa43d34aec` |
| Same | `update_mario_joystick_inputs` | `1359741d55df34eacb6aa4d6029f7b02fb3a1505` |
| Same | `update_mario_geometry_inputs` | `a65f628f0ffc61949be0f6b9f8d931c1ea563f84` |
| Same | `update_mario_inputs` | `3d3b7816ad9c4eee95f0e4feacffd2fef11f619d` |

All seven were re-extracted and byte-compared after the workspace reset.
The CC0 notice remains `c/decomp/LICENSE-CC0.txt`. `input_reference.h` and
`oracle_input_tick` are authored declarations/glue (MIT). Camera/interaction/warp
macros come from the pinned `camera.h`, `interaction.h`, `level_update.h`;
input/floor constants are evaluated from the real headers and checked by the
existing constant test. Button masks follow `include/PR/os_cont.h` definitions.

The shim structs add only referenced fields. Debug text is disabled. A death
warp records a request; it does not execute warp timers, lives or save state.
The original empty `stub_mario_step_1` runs. Action setters still abort, and no
unimplemented action is silently accepted. Camera yaw and object status are
explicit inputs, not original camera/object implementations.

```sh
cargo test --locked -p rustario64-oracle --test mario_input -- --nocapture
RUSTARIO64_ROM=/path/to/sm64.z64 cargo test --locked --release -p rustario64-oracle --test mario_input bob_input -- --ignored --nocapture
cargo run --locked --release -p rustario64-oracle --example input_trace -- /path/to/sm64.z64 private/input-traces
```

The example compares 1,200 input-stage ticks using imported BOB collision and
ROM tables and writes two private schema-1 traces into a new directory. It uses
explicit fixture initialization at the imported script start. This is not a
spawn or action replay. Full movement, dynamic surfaces and original N64
execution remain separate gates; see docs/FIDELITY.md.
