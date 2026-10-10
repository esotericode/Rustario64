#!/usr/bin/env python3
"""Check the ROM's behavior scripts against a clean pinned decomp checkout.

Independently of the Rust importer, this preprocesses the pinned
data/behavior_data.c with the host C compiler, lays its 533 behavior arrays
out in source order from the start of segment 0x13 (sm64tools
"behavior_data", ROM 0x219E00..0x21F4C0) and compares every word with the
ROM: each command and constant argument must equal the compiled value, and
each pointer argument that names a behavior (`bhvX` or `bhvX + n`) must
equal that behavior's segmented address. The arrays must fill the segment
exactly. Other pointers (native functions, animation tables, collision data,
droplet parameters) are recorded by name; a name with two addresses or an
address with two names fails.

It then locates sMacroObjectPresets (include/macro_presets.inc.c) as the
unique 4-aligned run in the ROM whose 366 entries equal the source's
(behavior address, model, param), evaluated by the same compiler.

Finally it checks the version adapter: every (name, address) entry of
BEHAVIOR_SCRIPTS and BEHAVIOR_NATIVES in src/import/version.rs, and
MACRO_PRESET_TABLE's ROM range, must equal what was located. It prints the
located addresses for the names the adapter lists, the preset table offset
and a SHA-1 over the segment.

No ROM content or expanded reference data is stored in this script. MIT;
source formats follow the CC0 decomp revision below. Python standard
library and a C compiler only.
"""
import argparse
import hashlib
import re
import subprocess
import tempfile
from pathlib import Path

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
ROM_SHA1 = "9bef1128717f958171a4afac3ed78ee2bb4e86ce"
# sm64tools configs/sm64.u.yaml "behavior_data" (segment 0x13).
BEHAVIOR_DATA = (0x219E00, 0x21F4C0)
SEGMENT = 0x13
MACRO_PRESET_COUNT = 366
FLAGS = ["-std=gnu99", "-DVERSION_US=1", "-DNON_MATCHING=1", "-DAVOID_UB=1", "-D_LANGUAGE_C",
         "-DF3D_OLD", "-DTARGET_N64"]
ROOT = Path(__file__).resolve().parent.parent

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rom", type=Path, required=True, help="the identified US ROM in Z64 byte order")
parser.add_argument("--reference", type=Path, required=True, help="clean pinned sm64 git checkout")
parser.add_argument("--list", action="store_true", help="print every located behavior and pointer")
options = parser.parse_args()
if not __debug__:
    parser.error("validation assertions must not be disabled with -O")
ref = options.reference.resolve()
revision = subprocess.check_output(["git", "-C", str(ref), "rev-parse", "HEAD"], text=True).strip()
if revision != REFERENCE:
    parser.error("reference checkout must be at " + REFERENCE)
if subprocess.check_output(["git", "-C", str(ref), "status", "--porcelain", "--untracked-files=no"],
                           text=True).strip():
    parser.error("reference checkout has modified tracked files")
rom = options.rom.read_bytes()
if hashlib.sha1(rom).hexdigest() != ROM_SHA1:
    parser.error("ROM is not the identified US revision in Z64 byte order")
INCLUDES = ["-I" + str(ref / d) for d in ("include", "src", ".", "include/libc")]


def be32(data, at):
    return int.from_bytes(data[at:at + 4], "big")


def be16s(data, at):
    return int.from_bytes(data[at:at + 2], "big", signed=True)


def preprocess(path):
    return subprocess.check_output(["gcc", *FLAGS, *INCLUDES, "-E", "-P", str(path)], text=True)


def split_top_level(body):
    items, depth, start = [], 0, 0
    for i, ch in enumerate(body):
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        elif ch == "," and depth == 0:
            items.append(body[start:i].strip())
            start = i + 1
    tail = body[start:].strip()
    if tail:
        items.append(tail)
    return [item for item in items if item]


def evaluate(prelude, expressions):
    """Evaluate integer constant expressions with the compiler, as u32."""
    with tempfile.TemporaryDirectory() as tmp:
        c = Path(tmp) / "eval.c"
        exe = Path(tmp) / "eval"
        lines = [prelude, "int printf(const char *, ...);", "static const unsigned long long sValues[] = {"]
        lines += ["    (unsigned long long)(unsigned int)(%s)," % e for e in expressions]
        lines += ["};", "int main(void) {",
                  "    for (unsigned i = 0; i < sizeof(sValues) / sizeof(sValues[0]); i++)",
                  "        printf(\"%llu\\n\", sValues[i]);", "    return 0;", "}"]
        c.write_text("\n".join(lines) + "\n")
        subprocess.check_call(["gcc", *FLAGS, *INCLUDES, "-w", str(c), "-o", str(exe)])
        values = [int(v) for v in subprocess.check_output([str(exe)], text=True).split()]
    assert len(values) == len(expressions)
    return values


# ---- Behavior scripts ----
source = preprocess(ref / "data" / "behavior_data.c")
# Includes `UNUSED static` arrays, which still occupy the segment.
array = re.compile(r"^[^\n;{}]*?\bBehaviorScript\s+(\w+)\[\]\s*=\s*\{(.*?)\};", re.S | re.M)
scripts = [(m.group(1), split_top_level(m.group(2)), m.start(), m.end()) for m in array.finditer(source)]
assert len(scripts) == 534, len(scripts)
prelude = source[:scripts[0][2]]
assert not array.search(source[scripts[-1][3]:]), "behavior arrays are not contiguous"
pointer = re.compile(r"^\(\(uintptr_t\)\((?!u32\))(.*)\)\)$", re.S)
constants = []
layout = []  # (name, [("const", index) | ("ptr", expression)])
for name, items, _, _ in scripts:
    words = []
    for item in items:
        m = pointer.match(item)
        if m:
            words.append(("ptr", " ".join(m.group(1).split())))
        else:
            words.append(("const", len(constants)))
            constants.append(item)
    layout.append((name, words))
values = evaluate(prelude, constants)

start, end = BEHAVIOR_DATA
segment = rom[start:end]
# The file's other .data objects: `UNUSED static const u64 behavior_data_unused_N = 0;`,
# laid out between the arrays at 8-byte alignment.
filler = re.compile(r"^[^\n;{}]*?\bu64\s+(\w+)\s*=\s*(\w+)\s*;", re.M)
fillers = [(m.start(), m.group(1), int(m.group(2), 0)) for m in filler.finditer(source, scripts[0][2])]
assert [name for _, name, _ in fillers] == ["behavior_data_unused_0", "behavior_data_unused_1"], fillers
assert not re.search(r"^[^\n;{}]*?=", "\n".join(
    line for line in source[scripts[0][2]:].splitlines() if "BehaviorScript" not in line and "u64" not in line
    and not line.startswith((" ", "\t", "}"))), re.M), "behavior_data.c defines other data"
offsets = {}
padding = []
at = 0
order = sorted([(position, "array", index) for index, (_, _, position, _) in enumerate(scripts)]
               + [(position, "u64", value) for position, _, value in fillers])
for _, kind, item in order:
    if kind == "u64":
        while at % 8:
            padding.append((at, 0))
            at += 4
        padding += [(at, item >> 32), (at + 4, item & 0xFFFFFFFF)]
        at += 8
    else:
        name, words = layout[item]
        assert name not in offsets, name
        offsets[name] = at
        at += 4 * len(words)
# The segment ends at the next 16-byte boundary, zero-filled.
assert (at + 15) // 16 * 16 == len(segment), "behavior data covers 0x%X of 0x%X bytes" % (at, len(segment))
padding += [(offset, 0) for offset in range(at, len(segment), 4)]
for offset, value in padding:
    assert be32(segment, offset) == value, "data at 0x%X is not the source's u64" % offset

symbol = re.compile(r"^&?(\w+)((?:\s*\+\s*\d+)*)$")
by_name = {}
by_address = {}
natives = {}
for name, words in layout:
    base = offsets[name]
    previous = None
    for index, (kind, value) in enumerate(words):
        rom_word = be32(segment, base + 4 * index)
        if kind == "const":
            expected = values[value]
            assert rom_word == expected, "%s word %d: ROM 0x%08X, source 0x%08X" % (name, index, rom_word, expected)
        else:
            m = symbol.match(value)
            assert m, "%s word %d: unrecognised pointer %r" % (name, index, value)
            target, plus = m.group(1), sum(int(n) for n in re.findall(r"\d+", m.group(2)))
            if target in offsets:
                expected = (SEGMENT << 24) + offsets[target] + 4 * plus
                assert rom_word == expected, "%s word %d: ROM 0x%08X, %s is 0x%08X" % (
                    name, index, rom_word, value, expected)
            else:
                assert plus == 0, value
                if by_name.setdefault(target, rom_word) != rom_word:
                    raise AssertionError("%s has two addresses" % target)
                if by_address.setdefault(rom_word, target) != target:
                    raise AssertionError("0x%08X is both %s and %s" % (rom_word, by_address[rom_word], target))
                if previous is not None and previous >> 24 == 0x0C and words[index - 1][0] == "const":
                    natives[target] = rom_word
        previous = values[value] if kind == "const" else None
behaviors = {name: (SEGMENT << 24) + offset for name, offset in offsets.items()}

# ---- Macro object presets ----
preset_source = (ref / "include" / "macro_presets.inc.c").read_text()
entries = re.findall(r"\{\s*(\w+)\s*,\s*([^,]+?)\s*,\s*([^}]+?)\s*\}", preset_source)
assert len(entries) == MACRO_PRESET_COUNT, len(entries)
preset_prelude = '#include "sm64.h"\n#include "model_ids.h"\n#include "object_constants.h"\n'
preset_values = evaluate(preset_prelude, [e for _, model, param in entries for e in (model, param)])
expected_table = b""
for index, (bhv, _, _) in enumerate(entries):
    model = preset_values[2 * index] & 0xFFFF
    param = preset_values[2 * index + 1] & 0xFFFF
    expected_table += behaviors[bhv].to_bytes(4, "big") + model.to_bytes(2, "big") + param.to_bytes(2, "big")
matches = [m.start() for m in re.finditer(re.escape(expected_table), rom) if m.start() % 4 == 0]
assert len(matches) == 1, "sMacroObjectPresets found %d times" % len(matches)
preset_table = (matches[0], matches[0] + len(expected_table))

# ---- The version adapter ----
if options.list:
    for name, address in sorted(behaviors.items(), key=lambda item: item[1]):
        print("behavior %-40s 0x%08X" % (name, address))
    for address, name in sorted(by_address.items()):
        print("pointer  %-40s 0x%08X%s" % (name, address, " (native)" if name in natives else ""))
version = (ROOT / "src" / "import" / "version.rs").read_text()


def adapter_table(name):
    m = re.search(r"pub const %s: \[\(&str, u32\); \d+\] = \[(.*?)\];" % name, version, re.S)
    assert m, "src/import/version.rs has no %s table" % name
    return re.findall(r'\("(\w+)",\s*0x([0-9A-Fa-f]+)\)', m.group(1))


checked = 0
for table, located in (("BEHAVIOR_SCRIPTS", behaviors), ("BEHAVIOR_NATIVES", natives)):
    for name, address in adapter_table(table):
        assert name in located, "%s lists %s, which the reference does not define" % (table, name)
        assert located[name] == int(address, 16), "%s: %s is 0x%08X in this ROM, not 0x%s" % (
            table, name, located[name], address)
        print("%-17s %-40s 0x%08X" % (table, name, located[name]))
        checked += 1
m = re.search(r"pub const MACRO_PRESET_TABLE: Range<usize> = 0x([0-9A-Fa-f]+)\.\.0x([0-9A-Fa-f]+);", version)
assert m, "src/import/version.rs has no MACRO_PRESET_TABLE range"
assert (int(m.group(1), 16), int(m.group(2), 16)) == preset_table, "MACRO_PRESET_TABLE differs"

print("behavior scripts: %d, words: %d, pointers: %d (%d natives)" % (
    len(layout), len(segment) // 4, len(by_name), len(natives)))
print("segment 0x13 ROM 0x%X..0x%X SHA-1 %s" % (start, end, hashlib.sha1(segment).hexdigest()))
print("sMacroObjectPresets ROM 0x%X..0x%X SHA-1 %s" % (
    preset_table[0], preset_table[1], hashlib.sha1(rom[preset_table[0]:preset_table[1]]).hexdigest()))
print("version adapter entries checked: %d" % checked)
