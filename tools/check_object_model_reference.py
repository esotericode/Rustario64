#!/usr/bin/env python3
"""Check the ROM's coin, sparkle, mist and King Bob-omb models against a clean pinned decomp checkout.

Independently of the Rust importer, this reads level_main_scripts_entry from
the ROM (sm64tools "main_level_scripts", segment 0x15) up to its
FREE_LEVEL_POOL: the segment loads must be the version adapter's (group0 and
common1 MIO0/geo blocks and the behavior segment), and the models the ported
object behaviors use (MODEL_YELLOW_COIN, MODEL_YELLOW_COIN_NO_SHADOW,
MODEL_SPARKLES) must be registered with LOAD_MODEL_FROM_GEO at the layouts
the pinned actors/coin/geo.inc.c and actors/sparkle/geo.inc.c place at their
address comments. Each layout is then decoded from the ROM's raw geo segment
and compared command by command with the source: opcodes, shadow fields,
switch case counts, layers and display-list addresses (the source names
encode them). Native callbacks are matched by position; the script prints
the address each callback name has in this ROM, fails if a name maps to two
addresses, and checks the version adapter's OBJECT_GEO_CALLBACKS table in
src/import/version.rs.

No ROM or expanded reference data is stored in this script. MIT; source formats
follow the CC0 decomp revision below. Python standard library only.
"""
import argparse
import hashlib
import re
import subprocess
from pathlib import Path

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
ROM_SHA1 = "9bef1128717f958171a4afac3ed78ee2bb4e86ce"
# sm64tools configs/sm64.u.yaml "main_level_scripts" (segment 0x15).
MAIN_LEVEL_SCRIPTS = (0x2ABCA0, 0x2AC6B0)
# The loads level_main_scripts_entry must make (segment: start, end, MIO0).
LOADS = {
    0x04: (0x114750, 0x1279B0, True),
    0x03: (0x201410, 0x218DA0, True),
    0x17: (0x1279B0, 0x12A7E0, False),
    0x16: (0x218DA0, 0x219E00, False),
    0x13: (0x219E00, 0x21F4C0, False),
}
# model ID: (source file, layout name)
MODELS = {
    0x74: ("actors/coin/geo.inc.c", "yellow_coin_geo"),
    0x75: ("actors/coin/geo.inc.c", "yellow_coin_no_shadow_geo"),
    0x95: ("actors/sparkle/geo.inc.c", "sparkles_geo"),
    0x8E: ("actors/mist/geo.inc.c", "mist_geo"),
    0x56: ("actors/king_bobomb/geo.inc.c", "king_bobomb_geo"),
}
ROOT = Path(__file__).resolve().parent.parent

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rom", type=Path, required=True, help="the identified US ROM in Z64 byte order")
parser.add_argument("--reference", type=Path, required=True, help="clean pinned sm64 git checkout")
options = parser.parse_args()
if not __debug__:
    parser.error("validation assertions must not be disabled with -O")
root = options.reference
revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
if revision != REFERENCE:
    parser.error("reference checkout must be at " + REFERENCE)
if subprocess.check_output(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"],
                           text=True).strip():
    parser.error("reference checkout has modified tracked files")
rom = options.rom.read_bytes()
if hashlib.sha1(rom).hexdigest() != ROM_SHA1:
    parser.error("ROM is not the identified US revision in Z64 byte order")


def be16(data, at, signed=True):
    return int.from_bytes(data[at:at + 2], "big", signed=signed)


def be32(data, at):
    return int.from_bytes(data[at:at + 4], "big")


# level_main_scripts_entry up to FREE_LEVEL_POOL.
scripts = rom[MAIN_LEVEL_SCRIPTS[0]:MAIN_LEVEL_SCRIPTS[1]]
loads, registered = {}, {}
offset = 0
while True:
    opcode, length = scripts[offset], scripts[offset + 1]
    if opcode in (0x17, 0x18):
        assert length == 12
        loads[be16(scripts, offset + 2, False)] = (be32(scripts, offset + 4), be32(scripts, offset + 8),
                                                    opcode == 0x18)
    elif opcode in (0x21, 0x22):
        assert length == 8
        registered[be16(scripts, offset + 2, False) & 0xFFF] = (opcode, be32(scripts, offset + 4))
    elif opcode == 0x1E:
        break
    else:
        assert opcode == 0x1D and length == 4, f"unexpected main-script command 0x{opcode:02X}"
    offset += length
assert loads == LOADS, "main-script loads differ from the version adapter"
segments = {seg: rom[start:end] for seg, (start, end, mio0) in LOADS.items() if not mio0}

# Follow BOB's main-script model-loading calls; model IDs are reused by
# other actor groups, so scanning every global registration is ambiguous.
bob_script = rom[0x405A60:0x405FB0]
king = []
for at in range(0, len(bob_script) - 7, 4):
    if bob_script[at:at + 2] != bytes([6, 8]):
        continue
    target = be32(bob_script, at + 4)
    if target >> 24 != 0x15:
        continue
    pc = target & 0xFFFFFF
    while scripts[pc:pc + 2] in (bytes([0x21, 8]), bytes([0x22, 8])):
        if be16(scripts, pc + 2, False) & 0xFFF == 0x56:
            king.append((scripts[pc], be32(scripts, pc + 4)))
        pc += 8
    assert scripts[pc:pc + 2] == bytes([7, 4]), "model-loading subroutine return"
assert king == [(0x22, 0x0C000000)], "King Bob-omb's original model registration"
registered[0x56] = king[0]
# BOB's script loads group3's raw geo segment. The animation/model data in
# segment 5 remains private; this checker needs only geometry commands.
bob_script = rom[0x405A60:0x405FB0]
geo_loads = [(be32(bob_script, at + 4), be32(bob_script, at + 8))
             for at in range(0, len(bob_script) - 11, 4)
             if bob_script[at:at + 4] == bytes([0x17, 12, 0, 0x0C])]
assert len(geo_loads) == 1, "BOB group3 geo load"
start, end = geo_loads[0]
assert start < end <= len(rom)
segments[0x0C] = rom[start:end]


def clean(text):
    return re.sub(r"/\*.*?\*/|//[^\n]*", "", text, flags=re.S)


constants = {}
for m in re.finditer(r"#define\s+(LAYER_\w+)\s+(\d+)", (root / "include/sm64.h").read_text()):
    constants[m.group(1)] = int(m.group(2))
body = re.search(r"enum ShadowType\s*\{(.*?)\}", clean((root / "src/game/shadow.h").read_text()), re.S).group(1)
value = -1
for item in (i.strip() for i in body.split(",")):
    if item:
        name, _, given = (p.strip() for p in item.partition("="))
        value = int(given, 0) if given else value + 1
        constants[name] = value


def number(arg):
    return constants[arg] if arg in constants else int(arg, 0)


callbacks = {}
checked = 0
for model, (path, layout) in sorted(MODELS.items()):
    text = (root / path).read_text()
    address = int(re.search(r"//\s*(0x[0-9A-Fa-f]{8})\s*\nconst GeoLayout " + layout + r"\[\]", text).group(1), 16)
    assert registered.get(model) == (0x22, address), f"model 0x{model:X} is not {layout} at 0x{address:08X}"
    source = re.search(r"const\s+GeoLayout\s+" + layout + r"\s*\[\]\s*=\s*\{(.*?)\};", clean(text), re.S).group(1)
    commands = [(c.group(1), [a.strip() for a in c.group(2).split(",") if a.strip()])
                for c in re.finditer(r"\b(GEO_\w+)\s*\(([^()]*)\)", source)]
    data = segments[address >> 24]
    at = address & 0xFFFFFF
    for macro, args in commands:
        op = data[at]
        if macro == "GEO_SHADOW":
            assert (op, be16(data, at + 2, False), be16(data, at + 4, False), be16(data, at + 6)) == (
                0x16, number(args[0]), number(args[1]), number(args[2])), (layout, macro)
            at += 8
        elif macro == "GEO_SWITCH_CASE":
            assert op == 0x0E and be16(data, at + 2) == number(args[0]), (layout, macro)
            name, function = args[1], be32(data, at + 4)
            assert callbacks.setdefault(name, function) == function, f"{name} has two addresses"
            at += 8
        elif macro == "GEO_ASM":
            assert op == 0x18 and be16(data, at + 2) == number(args[0]), (layout, macro)
            name, function = args[1], be32(data, at + 4)
            assert callbacks.setdefault(name, function) == function, f"{name} has two addresses"
            at += 8
        elif macro == "GEO_SCALE":
            assert (op, data[at + 1], be32(data, at + 4)) == (0x1D, number(args[0]), number(args[1])), (layout, macro)
            at += 8
        elif macro == "GEO_ANIMATED_PART":
            dl = 0 if args[4] == "NULL" else int(re.search(r"_([0-9A-Fa-f]{8})$", args[4]).group(1), 16)
            assert (op, data[at + 1], [be16(data, at + k) for k in (2, 4, 6)], be32(data, at + 8)) == (0x13, number(args[0]), [number(a) for a in args[1:4]], dl), (layout, macro)
            at += 12
        elif macro == "GEO_BILLBOARD":
            assert data[at:at + 8] == bytes([0x14, 0, 0, 0, 0, 0, 0, 0]), (layout, macro)
            at += 8
        elif macro == "GEO_DISPLAY_LIST":
            dl = int(re.search(r"_([0-9A-Fa-f]{8})$", args[1]).group(1), 16)
            assert (op, data[at + 1], be32(data, at + 4)) == (0x15, number(args[0]), dl), (layout, macro, args)
            at += 8
        else:
            expected = {"GEO_OPEN_NODE": 0x04, "GEO_CLOSE_NODE": 0x05, "GEO_END": 0x01, "GEO_NODE_START": 0x0B}
            assert op == expected[macro], (layout, macro)
            at += 4
        checked += 1
    print(f"model 0x{model:02X} {layout} at 0x{address:08X}: {len(commands)} commands match")
assert len(set(callbacks.values())) == len(callbacks), "two callback names share an address"

version = (ROOT / "src" / "import" / "version.rs").read_text()
m = re.search(r"pub const OBJECT_GEO_CALLBACKS: \[\(&str, u32\); \d+\] = \[(.*?)\];", version, re.S)
assert m, "src/import/version.rs has no OBJECT_GEO_CALLBACKS table"
for name, address in re.findall(r'\("(\w+)",\s*0x([0-9A-Fa-f]+)\)', m.group(1)):
    assert callbacks.get(name) == int(address, 16), f"{name} is not at 0x{address} in this ROM"
for name, address in sorted(callbacks.items()):
    print(f"callback {name} 0x{address:08X}")
print(f"geo commands checked: {checked}")
