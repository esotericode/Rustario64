# Collision oracle (development only)

`rustario64-oracle` compiles a minimal, byte-identical set of CC0 sources from
[n64decomp/sm64 at 9921382a68bb0c865e5e45eb594d9c64db59b1af](https://github.com/n64decomp/sm64/tree/9921382a68bb0c865e5e45eb594d9c64db59b1af)
natively, so tests can compare the Rust collision port with the original code
bit for bit.

**Boundary.** This crate is a test/comparison tool. Nothing in the game runtime
or renderer depends on it, and it is never shipped. The C keeps global state, so
all access goes through one process-wide lock. It is compiled with `-fwrapv`
(MIPS integer arithmetic wraps) and `-ffp-contract=off` (no fused multiply-add),
giving IEEE single-precision results that match the N64's for these operations.
Float-to-integer casts of values outside the s32 range are outside coverage.

**Replacement plan.** Native compilation of the decomp is a practical oracle,
not original-hardware evidence. Once per-tick traces from original execution
cover collision queries, those traces become the authority and this harness can
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

`c/oracle.c` also contains `spawn_special_objects` copied verbatim from
`src/game/macro_special_objects.c` (function text SHA-1
`92797dca1409789dd6c872b7f8e164143e10883f`), so the terrain stream is walked
exactly as the original walks it. Object spawning itself is stubbed. The CC0
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
```

The first runs on independently authored collision streams and tables (CI-safe);
the others use BOB's real collision and the real trig tables from the owner's ROM.
