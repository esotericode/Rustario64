#!/usr/bin/env python3
"""Reproduce the Mario animation table from a clean pinned decomp checkout.

Parses assets/anims/*.inc.c with the rules of the pinned
tools/mario_anims_converter.py, rebuilds the big-endian N64 table layout
(32-bit pointers, 4-byte struct and 2-byte array alignment), and compares it
byte for byte with the ROM block. Prints the canonical record digest that the
Rust owner-ROM test pins. No ROM bytes or expanded data are stored here.
MIT; formats follow the CC0 decomp revision below. Standard library only.
"""
import argparse
import hashlib
import json
import re
import struct
import subprocess
from pathlib import Path

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
ROM_SHA1 = "9bef1128717f958171a4afac3ed78ee2bb4e86ce"
TABLE_START = 0x4EC000  # sm64tools configs/sm64.u.yaml "mario_animation"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--reference", type=Path, required=True, help="clean pinned sm64 git checkout")
parser.add_argument("--rom", type=Path, help="supported ROM for the byte comparison")
options = parser.parse_args()
if not __debug__:
    parser.error("validation assertions must not be disabled with -O")
root = options.reference
revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
if revision != REFERENCE:
    parser.error("reference checkout must be at " + REFERENCE)
if subprocess.check_output(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"], text=True).strip():
    parser.error("reference checkout has modified tracked files")

# Same tokenization as mario_anims_converter.py: strip comments, drop blank
# lines, then read "struct Animation" headers and u16/s16 arrays in file order.
items = []
for path in sorted((root / "assets/anims").glob("*.inc.c")):
    lines = []
    for line in path.read_text().splitlines():
        line = re.sub(r"/\*.*?\*/", "", line)
        assert "/*" not in line, path
        line = line.split("//", 1)[0].strip()
        if line:
            for prefix in ("static ", "const "):
                if line.startswith(prefix):
                    line = line[len(prefix):]
            lines.append(line)
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("struct Animation ") and line.endswith("[] = {"):
            name = line[len("struct Animation "):-len("[] = {")]
            fields = [lines[i + k].rstrip(",") for k in range(1, 10)]
            assert lines[i + 10] == "};", (path, name)
            items.append(("header", name, fields))
            i += 11
        elif (line.startswith("u16 ") or line.startswith("s16 ")) and line.endswith("[] = {"):
            name = line[4:-len("[] = {")]
            values = []
            i += 1
            while lines[i] != "};":
                values.extend(int(v, 0) for v in lines[i].rstrip(",").split(",") if v.strip())
                i += 1
            items.append(("u16" if line.startswith("u16 ") else "s16", name, values))
            i += 1
        else:
            raise SyntaxError(f"{path}: unexpected line {line!r}")

arrays = {name: values for kind, name, values in items if kind != "header"}
headers = [(name, fields) for kind, name, fields in items if kind == "header"]
count = len(headers)
assert count == 0xD1, count

# N64 layout of struct MarioAnimsObj: u32 count, 4-byte placeholder pointer,
# count (offset, size) pairs, then every item in order with natural alignment.
offsets = {}
position = 8 + 8 * count
for kind, name, payload in items:
    if kind == "header":
        position = (position + 3) & ~3
        offsets[name] = position
        position += 0x18
    else:
        position = (position + 1) & ~1
        offsets[name] = position
        position += 2 * len(payload)
total = position

records = []
blob = bytearray(total)
struct.pack_into(">II", blob, 0, count, 0)
for entry, (name, fields) in enumerate(headers):
    flags, divisor, start, loop_start, loop_end = (int(v, 0) for v in fields[:5])
    values_name, index_name = fields[6], fields[7]
    assert fields[5] == f"ANIMINDEX_NUMPARTS({index_name})", (name, fields[5])
    index = arrays[index_name]
    values = arrays[values_name]
    assert len(index) % 6 == 0
    bone_count = len(index) // 6 - 1
    header_at = offsets[name]
    end = offsets[values_name] + 2 * len(values)
    struct.pack_into(">II", blob, 8 + 8 * entry, header_at, end - header_at)
    struct.pack_into(">hhhhhhIII", blob, header_at, flags, divisor, start, loop_start, loop_end,
                     bone_count, offsets[values_name] - header_at, offsets[index_name] - header_at,
                     end - header_at)
    records.append(dict(flags=flags, y_trans_divisor=divisor, start_frame=start,
                        loop_start=loop_start, loop_end=loop_end, bone_count=bone_count,
                        index=[v & 0xFFFF for v in index],
                        values=[(v & 0xFFFF) - 0x10000 if v & 0x8000 else v & 0xFFFF for v in values]))
for kind, name, payload in items:
    if kind != "header":
        struct.pack_into(">%dH" % len(payload), blob, offsets[name], *(v & 0xFFFF for v in payload))

canonical = json.dumps(dict(animations=records), sort_keys=True, separators=(",", ":")).encode()
print("animations", count, "table bytes", hex(total))
print("canonical record sha1", hashlib.sha1(canonical).hexdigest())
print("rebuilt table sha1", hashlib.sha1(blob).hexdigest())

if options.rom:
    rom = options.rom.read_bytes()
    if len(rom) != 0x800000:
        parser.error("expected an 8 MiB ROM")
    magic = rom[:4]
    if magic == bytes.fromhex("37804012"):
        rom = b"".join(rom[i:i + 2][::-1] for i in range(0, len(rom), 2))
    elif magic == bytes.fromhex("40123780"):
        rom = b"".join(rom[i:i + 4][::-1] for i in range(0, len(rom), 4))
    if hashlib.sha1(rom).hexdigest() != ROM_SHA1:
        parser.error("unsupported normalized ROM fingerprint")
    actual = rom[TABLE_START:TABLE_START + total]
    if actual != bytes(blob):
        first = next(i for i in range(total) if actual[i] != blob[i])
        raise SystemExit(f"ROM table differs from source at table offset {first:#X}")
    print("ROM table matches the rebuilt source table byte for byte:", hex(TABLE_START),
          "..", hex(TABLE_START + total))
