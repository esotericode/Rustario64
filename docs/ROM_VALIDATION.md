# Owner-ROM validation — 2026-10-08

The supplied 8 MiB Z64 ROM matches the supported US v1.0 normalized SHA-1
`9bef1128717f958171a4afac3ed78ee2bb4e86ce`. ROM inspection, the real BOB import,
private schema-2 export, and the strengthened local integration test passed on
Linux x86_64 with Rust 1.90.0. The ROM and exported assets are excluded from git.

```sh
cargo run --locked -- inspect-rom /path/to/your/sm64.z64
cargo run --locked -- import-bob /path/to/your/sm64.z64 --out private/imports
RUSTARIO64_ROM=/path/to/your/sm64.z64 cargo test --locked --test import local_us_rom_import -- --ignored --exact
```

| Imported component | Observed result | Validation |
| --- | --- | --- |
| Segment 7 | 71,618 decompressed bytes | Real MIO0 block and independently decoded source-stream comparisons |
| Collision at 0x0700E958 | 570 vertices; 1,060 ordered triangles; 17 specials; no environment regions | Every decoded field/order and all 9,972 source bytes match the pinned collision macros |
| Macros at 0x0701104C | 88 records; 882 bytes including terminator | Every packed word, preset, signed coordinate, angle, raw parameter, address, and record order matches the pinned macro list |
| Level entry at 0x0E000264 | One area, 30 script placements, seven warps; start yaw 135 at (-6558, 0, 6464) | Unique reference segment-load pattern, source placement/warp counts and start fields; no runtime execution |
| Textures | Five segment-7 RGBA16 textures, 32×32 | Real decode/export succeeds; color/alpha arithmetic checked by authored fixtures; Fast3D material use and rendered appearance untested |

## Independent ground truth

Used [n64decomp/sm64 at 9921382a68bb0c865e5e45eb594d9c64db59b1af](https://github.com/n64decomp/sm64/tree/9921382a68bb0c865e5e45eb594d9c64db59b1af):
`levels/bob/areas/1/collision.inc.c`, `levels/bob/areas/1/macro.inc.c`,
`include/surface_terrains.h`, `include/special_presets.h`,
`include/macro_presets.h`, `include/level_misc_macros.h`, and
`include/dialog_ids.h`. These source placements were inspected outside the
tracked repository; none are included in authored fixtures.

A separate one-off Python source-macro expander resolved the upstream constants,
expanded each stream into big-endian shorts, and compared it with independently
decompressed ROM bytes. It also constructed expected typed records from those
source invocations and compared every field with exported Rust JSON. The
reference expectations did not come from the Rust parser's output.

| Reference data | SHA-1 |
| --- | --- |
| Expanded collision stream, 9,972 bytes | `fec575bc5aab234abf17da8ff02b49ff90c7c78e` |
| Expanded macro stream, 882 bytes | `d8ba1fc16dd1a82b72a8008a8bfc8cec17ce48d2` |
| Collision records, schema-2 compact JSON | `0b5c611a87b3e0afb76baeedfcba7352e96134e8` |
| Macro placements, schema-2 compact JSON | `b3bb06ce2e341e7b47fae1653f586464a76686b9` |

The ignored test compares the decoded collision and macro records with the last
two digests. JSON uses the declared struct-field order, no whitespace, signed
integer coordinates/angles, unsigned packed words/parameters, and `null` for
absent force words. Deliberate serialization changes require a schema update and
fresh independent expectations. The ordinary fixture tests exercise malformed
records, bounds, all 128 packed yaw values, legacy dispatch, and terminators.

## Remaining checks

Static script parsing reports global calls and missing dependent segments.
Original spawn ordering, preset defaults, respawn mutation, executable behaviors,
native callbacks, and object interactions remain unimplemented. No behavior
address has been promoted to an implemented runtime behavior.

Geometry layouts, Fast3D/material decoding, animation import, GPU presentation,
collision queries, Mario actions, camera, audio, and missions are missing. No
movement or collision fidelity claim follows from matching the asset streams.
There is still no matching oracle build/emulator exporter or genuine gameplay
trace. The next content task is geometry/Fast3D decoding and a viewer; the next
fidelity task is a reproducible per-tick original-game spawn trace.
