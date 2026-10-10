#!/usr/bin/env python3
"""Check the ROM's dialog table against a clean pinned decomp checkout.

Independently of the Rust importer, this builds the pinned decomp's own
tools/textconv with the host C compiler and runs the original text pipeline
for the US version: the C preprocessor over charmap.txt and over
text/define_text.inc.c with text/us/, then textconv. From its output it reads
every dialog's (unused, linesPerBox, leftOffset, width) and encoded string
in seg2_dialog_table order.

It decodes the ROM's segment 2 (MIO0 at ROM 0x108A40, sm64tools'
font_graphics block, ending at 0x114750) with its own decoder, locates the
unique 4-aligned run of segment-2 pointers whose entries and strings equal
the reference and that ends in NULL, and checks the version adapter's
DIALOG_TABLE and DIALOG_COUNT in src/import/version.rs.

Only offsets, counts and a SHA-1 are printed; no ROM content or expanded
reference data is stored. MIT; source formats follow the CC0 decomp revision
below. Python standard library and a C compiler only.
"""
import argparse
import hashlib
import re
import struct
import subprocess
import tempfile
from pathlib import Path

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
ROM_SHA1 = "9bef1128717f958171a4afac3ed78ee2bb4e86ce"
SEGMENT2 = (0x108A40, 0x114750)
ROOT = Path(__file__).resolve().parent.parent

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rom", type=Path, required=True, help="the identified US ROM in Z64 byte order")
parser.add_argument("--reference", type=Path, required=True, help="clean pinned sm64 git checkout")
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


def mio0(data):
    assert data[:4] == b"MIO0", "segment 2 is not MIO0"
    size, compressed, literal = struct.unpack_from(">III", data, 4)
    out = bytearray()
    bit = 0
    while len(out) < size:
        if data[16 + bit // 8] & (0x80 >> (bit % 8)):
            out.append(data[literal])
            literal += 1
        else:
            token = struct.unpack_from(">H", data, compressed)[0]
            compressed += 2
            distance = (token & 0xFFF) + 1
            for _ in range((token >> 12) + 3):
                out.append(out[-distance])
        bit += 1
    return bytes(out)


# The reference: the decomp's own encoder over its own sources.
cpp = ["gcc", "-E", "-P", "-x", "c", "-Wno-trigraphs", "-D_LANGUAGE_ASSEMBLY", "-DVERSION_US=1",
       "-I" + str(ref / "include"), "-I" + str(ref)]
with tempfile.TemporaryDirectory() as tmp:
    tmp = Path(tmp)
    tools = ref / "tools"
    subprocess.check_call(["gcc", "-O1", "-o", str(tmp / "textconv"),
                           *(str(tools / f) for f in ("textconv.c", "utf8.c", "hashtable.c"))])
    (tmp / "charmap.txt").write_text(subprocess.check_output(cpp + [str(ref / "charmap.txt")], text=True))
    text = subprocess.check_output(cpp + ["-I" + str(ref / "text/us"), str(ref / "text/define_text.inc.c")])
    (tmp / "text.in.c").write_bytes(text)
    subprocess.check_call([str(tmp / "textconv"), str(tmp / "charmap.txt"), str(tmp / "text.in.c"),
                           str(tmp / "text.c")])
    encoded = (tmp / "text.c").read_text()

strings = {m.group(1): bytes(int(b, 16) for b in re.findall(r"0x([0-9A-Fa-f]{2})", m.group(2)))
           for m in re.finditer(r"dialog_text_(DIALOG_\d+)\[\] = \{(.*?)\};", encoded, re.S)}
entries = {m.group(1): tuple(int(v) for v in m.group(2, 3, 4, 5))
           for m in re.finditer(r"dialog_entry_(DIALOG_\d+) = \{\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),"
                                r"\s*(-?\d+),\s*dialog_text_\1\s*\}", encoded)}
table_body = re.search(r"seg2_dialog_table\[\] = \{(.*?)NULL", encoded, re.S).group(1)
order = re.findall(r"&dialog_entry_(DIALOG_\d+)", table_body)
assert order and set(order) == set(entries) == set(strings), "reference text did not parse"
for name, s in strings.items():
    assert s.endswith(b"\xFF") and s.count(b"\xFF") == 1, f"{name} has no single terminator"

segment = mio0(rom[SEGMENT2[0]:SEGMENT2[1]])


def be32(at):
    return struct.unpack_from(">I", segment, at)[0] if at + 4 <= len(segment) else None


def matches(table):
    for i, name in enumerate(order):
        pointer = be32(table + 4 * i)
        if pointer is None or pointer >> 24 != 2:
            return False
        at = pointer & 0xFFFFFF
        if at + 16 > len(segment):
            return False
        unused, lines, left, width, text = struct.unpack_from(">IbxhhxxI", segment, at)
        if (unused, lines, left, width) != entries[name] or text >> 24 != 2:
            return False
        start = text & 0xFFFFFF
        if segment[start:start + len(strings[name])] != strings[name]:
            return False
    return be32(table + 4 * len(order)) == 0


found = [at for at in range(0, len(segment) - 4 * len(order), 4) if matches(at)]
assert len(found) == 1, f"expected one matching table, found {len(found)}"
table = found[0]

version = (ROOT / "src/import/version.rs").read_text()
adapter_table = int(re.search(r"pub const DIALOG_TABLE: usize = (0x[0-9A-Fa-f]+);", version).group(1), 16)
adapter_count = int(re.search(r"pub const DIALOG_COUNT: usize = (\d+);", version).group(1))
assert adapter_table == table, f"DIALOG_TABLE is 0x{adapter_table:X}, located 0x{table:X}"
assert adapter_count == len(order), f"DIALOG_COUNT is {adapter_count}, reference has {len(order)}"
digest = hashlib.sha1(b"".join(strings[n] for n in order)).hexdigest()
print(f"seg2_dialog_table at decoded segment 2 offset 0x{table:X}: {len(order)} entries and "
      f"strings match the pinned reference (strings SHA-1 {digest})")
