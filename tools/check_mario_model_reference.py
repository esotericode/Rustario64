#!/usr/bin/env python3
"""Check Mario's model in the ROM against a clean pinned decomp checkout.

Independently of the Rust importer, this decodes `mario_geo` from the ROM
(found through level_main_scripts_entry's LOAD_MODEL_FROM_GEO) and expands the
pinned actors/mario/geo.inc.c source the same way, following branches. Every
command must match: opcodes, layers, translations, rotations, scales, switch
parameters, render ranges, shadow fields, branch targets (against the
source's address comments) and display-list pointers (against
actors/mario/model.inc.c's address comments). Native callbacks are matched by
position; the script prints the address each callback name has in this ROM
and fails if a name maps to two addresses or an address to two names.

It then rebuilds the geo node tree from the source (nodes numbered in
registration order, as the importer's arena is) and, for named switch
configurations, traverses it as the render pass does and runs the selected
display lists through a 16-slot vertex cache in layer order. For each
configuration it prints the triangle count and a SHA-1 over every triangle
vertex's (bone, position, color/normal bytes), which tests/mario_model.rs pins
for the importer's build of the same configuration.

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
MODEL_MARIO = 1

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
if subprocess.check_output(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"], text=True).strip():
    parser.error("reference checkout has modified tracked files")
rom = options.rom.read_bytes()
if hashlib.sha1(rom).hexdigest() != ROM_SHA1:
    parser.error("ROM is not the identified US revision in Z64 byte order")


def be16(data, at, signed=True):
    return int.from_bytes(data[at:at + 2], "big", signed=signed)


def be32(data, at):
    return int.from_bytes(data[at:at + 4], "big")


# The main scripts' loads before MODEL_MARIO, as level_script.c reads them.
scripts = rom[MAIN_LEVEL_SCRIPTS[0]:MAIN_LEVEL_SCRIPTS[1]]
loads = {}
offset = 0
while True:
    opcode, length = scripts[offset], scripts[offset + 1]
    if opcode in (0x17, 0x18):
        assert length == 12
        loads[be16(scripts, offset + 2, False)] = (be32(scripts, offset + 4), be32(scripts, offset + 8), opcode == 0x18)
    elif opcode == 0x22:
        assert length == 8 and be16(scripts, offset + 2, False) & 0xFFF == MODEL_MARIO
        mario_geo = be32(scripts, offset + 4)
        break
    else:
        assert opcode == 0x1D and length == 4, f"unexpected main-script command 0x{opcode:02X}"
    offset += length
start, end, mio0 = loads[0x17]
assert not mio0 and mario_geo >> 24 == 0x17
geo_segment = rom[start:end]


def clean(text):
    return re.sub(r"/\*.*?\*/|//[^\n]*", "", text, flags=re.S)


def defines(path, prefix):
    out = {}
    for m in re.finditer(r"#define\s+(" + prefix + r"\w*)\s+(-?(?:0x[0-9A-Fa-f]+|\d+))\b", path.read_text()):
        out[m.group(1)] = int(m.group(2), 0)
    return out


def enum_values(path, enum):
    body = re.search(r"enum " + enum + r"\s*\{(.*?)\}", clean(path.read_text()), re.S).group(1)
    out, value = {}, -1
    for item in body.split(","):
        item = item.strip()
        if not item:
            continue
        name, _, given = (part.strip() for part in item.partition("="))
        value = int(given, 0) if given else value + 1
        out[name] = value
    return out


constants = defines(root / "include/sm64.h", "LAYER_")
constants.update(enum_values(root / "src/game/shadow.h", "ShadowType"))
constants["NULL"] = 0


def addressed_symbols(path, kind):
    """Symbols whose definition follows an address comment."""
    text = path.read_text()
    pattern = r"//\s*(0x[0-9A-Fa-f]{8})[^\n]*\n(?:static\s+)?const\s+" + kind + r"\s+(\w+)\s*\[\]"
    return {m.group(2): int(m.group(1), 16) for m in re.finditer(pattern, text)}


display_lists = addressed_symbols(root / "actors/mario/model.inc.c", "Gfx")
geo_source = (root / "actors/mario/geo.inc.c").read_text()
geo_addresses = addressed_symbols(root / "actors/mario/geo.inc.c", "GeoLayout")
layouts = {}
for m in re.finditer(r"const\s+GeoLayout\s+(\w+)\s*\[\]\s*=\s*\{(.*?)\};", clean(geo_source), re.S):
    layouts[m.group(1)] = [(c.group(1), [a.strip() for a in c.group(2).split(",") if a.strip()])
                           for c in re.finditer(r"\b(GEO_\w+)\s*\(([^()]*)\)", m.group(2))]
assert geo_addresses["mario_geo"] == mario_geo, "the ROM registers MODEL_MARIO elsewhere"


def value(arg):
    if arg in constants:
        return constants[arg]
    return int(arg, 0)


def expand_source():
    """The command sequence the source produces from mario_geo, branches followed."""
    out = []
    stack = [("mario_geo", 0)]
    while stack:
        name, index = stack.pop()
        commands = layouts[name]
        while index < len(commands):
            macro, args = commands[index]
            index += 1
            if macro == "GEO_BRANCH":
                out.append((macro, [value(args[0]), geo_addresses[args[1]]]))
                if value(args[0]) == 1:
                    stack.append((name, index))
                stack.append((args[1], 0))
                break
            if macro in ("GEO_RETURN", "GEO_END"):
                out.append((macro, []))
                break
            out.append((macro, args))
        # A branch pushed the continuation; RETURN/END pops back to it.
    return out


def decode_rom():
    """The same walk over the ROM's raw segment 0x17."""
    out = []
    stack = []
    address = mario_geo
    while True:
        at = address & 0xFFFFFF
        op = geo_segment[at]
        if op == 0x02:
            kind, target = geo_segment[at + 1], be32(geo_segment, at + 4)
            out.append((op, [kind, target]))
            if kind == 1:
                stack.append(address + 8)
            address = target
            continue
        if op in (0x01, 0x03):
            out.append((op, []))
            if op == 0x01 or not stack:
                return out
            address = stack.pop()
            continue
        if op in (0x04, 0x05, 0x0B):
            out.append((op, []))
            address += 4
        elif op == 0x0D:
            out.append((op, [be16(geo_segment, at + 4), be16(geo_segment, at + 6)]))
            address += 8
        elif op in (0x0E, 0x18):
            out.append((op, [be16(geo_segment, at + 2), be32(geo_segment, at + 4)]))
            address += 8
        elif op == 0x10:
            params = geo_segment[at + 1]
            assert params & 0x70 == 0, "only the translate-and-rotate layout occurs"
            fields = [params & 0xF] + [be16(geo_segment, at + 4 + 2 * i) for i in range(6)]
            size = 16
            if params & 0x80:
                fields.append(be32(geo_segment, at + 16))
                size = 20
            out.append((op, fields))
            address += size
        elif op == 0x12:
            params = geo_segment[at + 1]
            fields = [params & 0xF, be16(geo_segment, at + 2), be16(geo_segment, at + 4), be16(geo_segment, at + 6)]
            size = 8
            if params & 0x80:
                fields.append(be32(geo_segment, at + 8))
                size = 12
            out.append((op, fields))
            address += size
        elif op == 0x13:
            out.append((op, [geo_segment[at + 1], be16(geo_segment, at + 2), be16(geo_segment, at + 4),
                             be16(geo_segment, at + 6), be32(geo_segment, at + 8)]))
            address += 12
        elif op == 0x15:
            out.append((op, [geo_segment[at + 1], be32(geo_segment, at + 4)]))
            address += 8
        elif op == 0x16:
            out.append((op, [be16(geo_segment, at + 2), be16(geo_segment, at + 4), be16(geo_segment, at + 6)]))
            address += 8
        elif op == 0x1C:
            out.append((op, [geo_segment[at + 1], be16(geo_segment, at + 2), be16(geo_segment, at + 4),
                             be16(geo_segment, at + 6), be32(geo_segment, at + 8)]))
            address += 12
        elif op == 0x1D:
            params = geo_segment[at + 1]
            fields = [params & 0xF, be32(geo_segment, at + 4)]
            size = 8
            if params & 0x80:
                fields.append(be32(geo_segment, at + 8))
                size = 12
            out.append((op, fields))
            address += size
        else:
            raise AssertionError(f"unexpected geo opcode 0x{op:02X} at 0x{address:08X}")


OPCODES = {
    "GEO_BRANCH": 0x02, "GEO_END": 0x01, "GEO_RETURN": 0x03, "GEO_OPEN_NODE": 0x04,
    "GEO_CLOSE_NODE": 0x05, "GEO_NODE_START": 0x0B, "GEO_RENDER_RANGE": 0x0D,
    "GEO_SWITCH_CASE": 0x0E, "GEO_ASM": 0x18, "GEO_TRANSLATE_ROTATE": 0x10,
    "GEO_ROTATION_NODE": 0x12, "GEO_ANIMATED_PART": 0x13, "GEO_DISPLAY_LIST": 0x15,
    "GEO_SHADOW": 0x16, "GEO_HELD_OBJECT": 0x1C, "GEO_SCALE": 0x1D,
}

source = expand_source()
decoded = decode_rom()
assert len(source) == len(decoded), f"{len(source)} source commands, {len(decoded)} in the ROM"
callbacks = {}
names_at = {}
used_lists = set()
for index, ((macro, args), (op, fields)) in enumerate(zip(source, decoded)):
    where = f"command {index} ({macro})"
    assert OPCODES[macro] == op, f"{where}: ROM opcode 0x{op:02X}"
    if macro == "GEO_BRANCH":
        expected = args
    elif macro in ("GEO_SWITCH_CASE", "GEO_ASM"):
        callbacks.setdefault(args[1], set()).add(fields[1])
        names_at.setdefault(fields[1], set()).add(args[1])
        expected = [value(args[0]), fields[1]]
    elif macro == "GEO_HELD_OBJECT":
        callbacks.setdefault(args[4], set()).add(fields[4])
        names_at.setdefault(fields[4], set()).add(args[4])
        expected = [value(a) for a in args[:4]] + [fields[4]]
    elif macro in ("GEO_ANIMATED_PART", "GEO_DISPLAY_LIST"):
        dl = args[-1]
        if dl != "NULL":
            used_lists.add(dl)
        expected = [value(a) for a in args[:-1]] + [display_lists[dl] if dl != "NULL" else 0]
    else:
        expected = [value(a) for a in args]
    assert expected == fields, f"{where}: source {expected} vs ROM {fields}"
for name, addresses in callbacks.items():
    assert len(addresses) == 1, f"{name} has several addresses"
for address, names in names_at.items():
    assert len(names) == 1, f"0x{address:08X} is several callbacks"
print(f"mario_geo at 0x{mario_geo:08X}: {len(decoded)} commands match the pinned source,")
print(f"including {len(used_lists)} distinct display lists at their commented addresses.")
print("Native callbacks in this ROM (for src/import/version.rs):")
for name, (address,) in sorted(callbacks.items(), key=lambda item: min(item[1])):
    print(f'    ("{name}", 0x{address:08X}),')


# ---- The node tree, as geo_layout.c registers it --------------------------

NODE_MACROS = {
    "GEO_SHADOW", "GEO_SCALE", "GEO_ASM", "GEO_SWITCH_CASE", "GEO_NODE_START",
    "GEO_RENDER_RANGE", "GEO_ANIMATED_PART", "GEO_ROTATION_NODE",
    "GEO_TRANSLATE_ROTATE", "GEO_DISPLAY_LIST", "GEO_HELD_OBJECT",
}
TRANSFORMS = {"GEO_SCALE", "GEO_ROTATION_NODE", "GEO_TRANSLATE_ROTATE", "GEO_ANIMATED_PART"}
nodes = []  # (macro, args, children)
depth_nodes = {}
depth = 0
for macro, args in source:
    if macro == "GEO_OPEN_NODE":
        depth += 1
    elif macro == "GEO_CLOSE_NODE":
        depth -= 1
    elif macro in NODE_MACROS:
        index = len(nodes)
        nodes.append((macro, args, []))
        depth_nodes[depth] = index
        if depth > 0:
            nodes[depth_nodes[depth - 1]][2].append(index)
        else:
            assert index == 0, "one root"
    else:
        assert macro in ("GEO_BRANCH", "GEO_RETURN", "GEO_END"), macro
OBJECT_BONE = len(nodes)


def traverse(config):
    """(layer, display list, bone) in the order the render pass appends them."""
    draws = []

    def visit(index, bone, wings):
        macro, args, children = nodes[index]
        if macro == "GEO_SWITCH_CASE":
            role, param = args[1], value(args[0])
            case = {
                "geo_switch_mario_stand_run": lambda: config["stand_run"],
                "geo_switch_mario_cap_effect": lambda: config["cap_effect"],
                "geo_switch_mario_cap_on_off": lambda: config["cap"],
                "geo_switch_mario_eyes": lambda: config["eyes"],
                "geo_switch_mario_hand": lambda: config["right_hand" if param == 0 else "left_hand"],
            }[role]()
            siblings([children[case]], bone)
            return
        if macro == "GEO_RENDER_RANGE":
            if value(args[0]) <= config["lod"] < value(args[1]):
                siblings(children, bone)
            return
        if macro == "GEO_HELD_OBJECT":
            return
        if macro == "GEO_TRANSLATE_ROTATE" and not wings:
            return
        if macro in TRANSFORMS:
            bone = index
            if args[-1] != "NULL" and macro in ("GEO_ANIMATED_PART",):
                draws.append((value(args[0]), args[-1], bone))
        elif macro == "GEO_DISPLAY_LIST":
            draws.append((value(args[0]), args[1], bone))
        siblings(children, bone)

    def siblings(indices, bone):
        wings = any(nodes[i][0] == "GEO_SWITCH_CASE" and nodes[i][1][1] == "geo_switch_mario_cap_on_off"
                    for i in indices) and config["wings"]
        for i in indices:
            visit(i, bone, wings)

    siblings([0], OBJECT_BONE)
    return draws


# ---- Display lists and vertices from actors/mario/model.inc.c --------------

model_source = clean((root / "actors/mario/model.inc.c").read_text())
vertices = {}
for m in re.finditer(r"Vtx\s+(\w+)\s*\[\]\s*=\s*\{(.*?)\};", model_source, re.S):
    vertices[m.group(1)] = [
        tuple(int(x, 0) for x in v.groups())
        for v in re.finditer(
            r"\{\{\{\s*(-?\w+),\s*(-?\w+),\s*(-?\w+)\},\s*-?\w+,\s*\{\s*-?\w+,\s*-?\w+\},\s*"
            r"\{\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+)\}\}\}",
            m.group(2))
    ]
gfx = {}
for m in re.finditer(r"Gfx\s+(\w+)\s*\[\]\s*=\s*\{(.*?)\};", model_source, re.S):
    gfx[m.group(1)] = [(c.group(1), [a.strip() for a in c.group(2).split(",")])
                       for c in re.finditer(r"\b(gsSPVertex|gsSP1Triangle|gsSP2Triangles|gsSPDisplayList|gsSPEndDisplayList)\s*\(([^()]*)\)",
                                            m.group(2))]


def triangles(draws):
    """Triangle vertices (bone, x, y, z, r, g, b, a) in draw order."""
    slots = [None] * 16
    out = []

    def run(name, bone):
        for macro, args in gfx[name]:
            if macro == "gsSPVertex":
                array, count, first = args[0], value(args[1]), value(args[2])
                for i in range(count):
                    slots[first + i] = (bone,) + vertices[array][i]
            elif macro == "gsSP1Triangle":
                out.append(tuple(slots[value(a)] for a in args[:3]))
            elif macro == "gsSP2Triangles":
                out.append(tuple(slots[value(a)] for a in args[:3]))
                out.append(tuple(slots[value(a)] for a in args[4:7]))
            elif macro == "gsSPDisplayList":
                run(args[0], bone)
            elif macro == "gsSPEndDisplayList":
                return

    for layer in range(8):
        for draw_layer, name, bone in draws:
            if draw_layer == layer:
                run(name, bone)
    return out


def digest(tris):
    data = bytearray()
    for tri in tris:
        for bone, x, y, z, r, g, b, a in tri:
            data += bone.to_bytes(2, "big") + b"".join(c.to_bytes(2, "big", signed=True) for c in (x, y, z))
            data += bytes((r, g, b, a))
    return hashlib.sha1(bytes(data)).hexdigest()


CONFIGURATIONS = {
    # Standing (full detail, no level of detail), cap on, eyes open, fists.
    "standing": dict(stand_run=0, cap_effect=0, cap=0, wings=False, eyes=0, left_hand=0, right_hand=0, lod=5000),
    # Moving close: full detail, mid-blink, holding the cap in the right hand.
    "moving-near-holding-cap": dict(stand_run=1, cap_effect=0, cap=0, wings=False, eyes=2, left_hand=0, right_hand=3, lod=100),
    # Moving at medium range: cap off, dead eyes, open hands.
    "moving-medium-cap-off": dict(stand_run=1, cap_effect=0, cap=1, wings=False, eyes=7, left_hand=1, right_hand=1, lod=1000),
    # Moving far: low detail, wing cap with wings, closed eyes, peace sign.
    "moving-far-wing-cap": dict(stand_run=1, cap_effect=0, cap=0, wings=True, eyes=2, left_hand=0, right_hand=2, lod=2000),
    # Metal body standing, holding the wing cap.
    "metal-standing": dict(stand_run=0, cap_effect=2, cap=0, wings=False, eyes=0, left_hand=0, right_hand=4, lod=0),
    # Vanish body flying near, wing cap, open fists (flying opens them).
    "vanish-flying": dict(stand_run=1, cap_effect=1, cap=0, wings=True, eyes=0, left_hand=1, right_hand=1, lod=100),
}
print("Built configurations (for tests/mario_model.rs):")
for name, config in CONFIGURATIONS.items():
    tris = triangles(traverse(config))
    print(f'    ("{name}", {len(tris)}, "{digest(tris)}"),')
