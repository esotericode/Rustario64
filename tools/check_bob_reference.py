#!/usr/bin/env python3
"""Reproduce BOB collision/macro checks against a clean pinned decomp checkout.

No ROM or expanded reference data is stored in this script. MIT; source formats
follow the CC0 decomp revision specified below. Python standard library only.
"""
import argparse
import subprocess
from pathlib import Path
import hashlib
import json
import re
import struct

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
ROM_SHA1 = "9bef1128717f958171a4afac3ed78ee2bb4e86ce"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rom", type=Path, required=True)
parser.add_argument("--export", type=Path, required=True, help="import-bob's bob directory")
parser.add_argument("--reference", type=Path, required=True, help="clean pinned sm64 git checkout")
options = parser.parse_args()
if not __debug__:
    parser.error("validation assertions must not be disabled with -O")
root = options.reference
export = options.export
revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
if revision != REFERENCE:
    parser.error("reference checkout must be at " + REFERENCE)
if subprocess.check_output(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"], text=True).strip():
    parser.error("reference checkout has modified tracked files")
manifest = json.loads((export / "manifest.json").read_text())
if manifest["rom_sha1"] != ROM_SHA1 or manifest["reference_revision"] != REFERENCE:
    parser.error("export identity/reference does not match")

def clean(text):
    return re.sub(r'/\*.*?\*/|//[^\n]*', '', text, flags=re.S)

def enum_values(path, enum):
    text = clean(path.read_text())
    body = re.search(r'enum ' + enum + r'\s*\{(.*?)\}', text, re.S).group(1)
    out = {}
    value = -1
    for item in body.split(','):
        item = item.strip()
        if not item:
            continue
        parts = item.split('=')
        name = parts[0].strip()
        if len(parts) == 1:
            value += 1
        else:
            v = parts[1].strip()
            value = out[v] if v in out else int(v, 0)
        out[name] = value
    return out

def invocations(path):
    text = clean(path.read_text())
    return [(m.group(1), [v.strip() for v in m.group(2).split(',') if v.strip()])
            for m in re.finditer(r'\b((?:COL_|SPECIAL_OBJECT|MACRO_OBJECT)\w*)\s*\(([^()]*)\)', text)]

def binary_words(values):
    return b''.join(struct.pack('>H', value & 0xFFFF) for value in values)

surfaces = {m.group(1): int(m.group(2), 0) for m in re.finditer(
    r'^#define\s+(SURFACE_\w+)\s+(0x[0-9A-Fa-f]+|[0-9]+)\b',
    (root / 'include/surface_terrains.h').read_text(), re.M)}
specials = enum_values(root / 'include/special_presets.h', 'SpecialPresets')
expected = dict(vertices=[], triangles=[], specials=[], environment=[])
collision_words = []
surface = None
for op, args in invocations(root / 'levels/bob/areas/1/collision.inc.c'):
    if op == 'COL_INIT':
        collision_words.append(0x40)
    elif op == 'COL_VERTEX_INIT':
        assert int(args[0], 0) == 570
        collision_words.append(int(args[0], 0))
    elif op == 'COL_VERTEX':
        v = [int(x, 0) for x in args]
        expected['vertices'].append(v)
        collision_words.extend(v)
    elif op == 'COL_TRI_INIT':
        surface = surfaces[args[0]]
        collision_words.extend([surface, int(args[1], 0)])
    elif op in ('COL_TRI', 'COL_TRI_SPECIAL'):
        v = [int(x, 0) for x in args]
        expected['triangles'].append(dict(indices=v[:3], surface=surface,
            force=v[3] if len(v) == 4 else None))
        collision_words.extend(v)
    elif op == 'COL_TRI_STOP':
        collision_words.append(0x41)
    elif op == 'COL_SPECIAL_INIT':
        assert int(args[0], 0) == 17
        collision_words.extend([0x43, int(args[0], 0)])
    elif op.startswith('SPECIAL_OBJECT'):
        preset = specials[args[0]]
        v = [int(x, 0) for x in args[1:]]
        expected['specials'].append(dict(preset=preset, position=v[:3], extra=v[3:]))
        collision_words.extend([preset] + v)
    elif op == 'COL_END':
        collision_words.append(0x42)
    else:
        raise ValueError(op)
actual = json.loads((export / 'collision.json').read_text())
assert actual == expected, 'collision reference mismatch'

presets = enum_values(root / 'include/macro_presets.h', 'MacroPresets')
dialogs = enum_values(root / 'include/dialog_ids.h', 'DialogID')
assert presets['macro_count'] == 366
expected_macros = []
macro_words = []
base = 0x0701104C
for op, args in invocations(root / 'levels/bob/areas/1/macro.inc.c'):
    if op == 'MACRO_OBJECT_END':
        macro_words.append(30)
        continue
    assert op in ('MACRO_OBJECT', 'MACRO_OBJECT_WITH_BHV_PARAM')
    preset = presets[args[0]]
    yaw = int(args[1], 0)
    yaw_steps = (abs(yaw * 16) // 45) * (1 if yaw >= 0 else -1)
    packed = ((yaw_steps << 9) | (preset + 31)) & 0xFFFF
    position = [int(v, 0) for v in args[2:5]]
    params = 0
    if len(args) == 6:
        params = dialogs[args[5]] if args[5] in dialogs else int(args[5], 0)
    angle = packed & 0xFE00
    expected_macros.append(dict(source_address=base + len(macro_words) * 2,
        packed_preset_and_yaw=packed, preset_id=preset, position=position,
        yaw=angle if angle < 0x8000 else angle - 0x10000, raw_params=params & 0xFFFF))
    macro_words.extend([packed] + position + [params])
level = json.loads((export / 'level.json').read_text())
assert level['areas'][0]['macro_spawns'] == expected_macros, 'macro reference mismatch'

# Independently expand the source macros and compare raw streams to the ROM.
rom = options.rom.read_bytes()
if len(rom) != 0x800000:
    parser.error("expected an 8 MiB ROM")
magic = rom[:4]
if magic == bytes.fromhex("37804012"):
    rom = b"".join(rom[i:i+2][::-1] for i in range(0, len(rom), 2))
elif magic == bytes.fromhex("40123780"):
    rom = b"".join(rom[i:i+4][::-1] for i in range(0, len(rom), 4))
if hashlib.sha1(rom).hexdigest() != ROM_SHA1:
    parser.error("unsupported normalized ROM fingerprint")
block = rom[0x3FC2B0:0x405A60]
magic, size, token_at, literal_at = struct.unpack('>4sIII', block[:16])
assert magic == b'MIO0'
out = bytearray()
bit_at = 0
while len(out) < size:
    mask = block[16 + bit_at // 8] & (0x80 >> (bit_at % 8))
    bit_at += 1
    if mask:
        out.append(block[literal_at])
        literal_at += 1
    else:
        token = int.from_bytes(block[token_at:token_at+2], 'big')
        token_at += 2
        length, distance = (token >> 12) + 3, (token & 0xFFF) + 1
        for _ in range(length):
            out.append(out[-distance])
assert len(out) == size == 71618
for name, start, words in [('collision', 0xE958, collision_words), ('macros', 0x1104C, macro_words)]:
    source = binary_words(words)
    assert out[start:start + len(source)] == source, name + ' binary mismatch'
    print(name, 'bytes', len(source), 'sha1', hashlib.sha1(source).hexdigest())
for name, records in [("collision", expected), ("macros", expected_macros)]:
    canonical = json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    print(name, "canonical record sha1", hashlib.sha1(canonical).hexdigest())
print('Exact source comparison passed:', len(expected['vertices']), 'vertices,',
      len(expected['triangles']), 'ordered triangles,', len(expected['specials']),
      'specials,', len(expected_macros), 'macro placements; empty environment regions.')
