#!/usr/bin/env python3
"""Independently decode the owner-ROM shadow texture using pinned metadata.

Only hashes/counts are printed. No pixels or ROM data are written. --reference
needs assets.json and bin/segment2.c from the exact pinned CC0 sm64 revision;
their file hashes are checked, so a complete git checkout is optional. MIT.
"""
import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
SOURCE_HASHES = {
    "assets.json": "12afb37d0e1cb110aba18984ef75257aeca5db34",
    "bin/segment2.c": "fe0bbcff1dce0f0e85d2dd5bca8cb6508e302b2c",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    options = parser.parse_args()
    sources = {}
    for name, digest in SOURCE_HASHES.items():
        data = (options.reference / name).read_bytes()
        if hashlib.sha1(data).hexdigest() != digest:
            parser.error(f"{name} must match pinned sm64 {REFERENCE}")
        sources[name] = data.decode()
    data = bytearray(options.rom.read_bytes())
    magic = data[:4]
    width = {bytes.fromhex("80371240"): 1, bytes.fromhex("37804012"): 2,
             bytes.fromhex("40123780"): 4}.get(bytes(magic))
    if width is None or len(data) % width:
        parser.error("invalid N64 byte order")
    if width > 1:
        for i in range(0, len(data), width):
            data[i:i + width] = data[i:i + width][::-1]
    if hashlib.sha1(data).hexdigest() != "9bef1128717f958171a4afac3ed78ee2bb4e86ce":
        parser.error("requires the supported US v1.0 ROM")
    metadata = json.loads(sources["assets.json"])["textures/segment2/shadow_quarter_circle.ia8.png"]
    w, h, length, locations = metadata
    base, offset = locations["us"]
    # The independently pinned sm64tools font_graphics block ends at 0x114750.
    segment = data[base:0x114750]
    if segment[:4] != b"MIO0":
        parser.error("shadow segment is not MIO0")
    size, compressed, literal = struct.unpack_from(">III", segment, 4)
    output = bytearray()
    bit = 0
    while len(output) < size:
        if segment[16 + bit // 8] & (0x80 >> (bit % 8)):
            output.append(segment[literal])
            literal += 1
        else:
            token = struct.unpack_from(">H", segment, compressed)[0]
            compressed += 2
            count, distance = (token >> 12) + 3, (token & 0xFFF) + 1
            for _ in range(count):
                output.append(output[-distance])
        bit += 1
    pixels = output[offset:offset + length]
    rgba = bytes(c for p in pixels for c in [(p >> 4) * 17] * 3 + [(p & 15) * 17])
    triangles = re.search(r"const Gfx dl_shadow_9_verts\[\] = \{(.*?)\};",
                          sources["bin/segment2.c"], re.S).group(1)
    indices = []
    for args in re.findall(r"gsSP2Triangles\((.*?)\)", triangles):
        values = [int(v.strip(), 0) for v in args.split(",")]
        indices.extend(values[:3] + values[4:7])
    print(f"{w}x{h} IA8 at US ROM MIO0 0x{base:X}, decoded 0x{offset:X}")
    print(f"RGBA SHA-1 {hashlib.sha1(rgba).hexdigest()}; {len(indices) // 3} triangles: {indices}")


if __name__ == "__main__":
    main()
