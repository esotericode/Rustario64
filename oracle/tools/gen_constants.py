#!/usr/bin/env python3
"""Generate the Rust mirror of decomp constants and the oracle's check table.

Collects object-like macros and enum members from the vendored, unmodified
decomp headers in oracle/c/decomp, evaluates each with the C compiler (so the
values are exactly what the vendored C sees), and writes:

  src/simulation/mario/constants.rs   Rust constants with declared types
  oracle/c/constants.inc.c            the name/value table the oracle exports

The oracle test `generated_constants_match_the_decomp_headers` compares both
at run time. Rerun after vendoring a header or changing a group below.
MIT (Rustario64). Python standard library and a C compiler only.
"""
import re
import subprocess
import tempfile
from pathlib import Path

ORACLE = Path(__file__).resolve().parent.parent
ROOT = ORACLE.parent
DECOMP = ORACLE / "c" / "decomp"
FLAGS = ["-std=gnu99", "-DNON_MATCHING", "-DVERSION_US=1", "-DAVOID_UB", "-DNO_SEGMENTED_MEMORY",
         "-D_LANGUAGE_C", "-DOBJECT_FIELDS_INDEX_DIRECTLY", "-I" + str(ORACLE / "c" / "shim"),
         "-I" + str(DECOMP / "include"), "-I" + str(DECOMP / "src"), "-I" + str(DECOMP)]
HEADERS = ["sm64.h", "dialog_ids.h", "surface_terrains.h", "sounds.h", "mario_animation_ids.h", "level_table.h",
           "object_constants.h", "object_fields.h", "game/camera.h", "game/interaction.h",
           "game/level_update.h", "game/mario.h", "game/save_file.h", "engine/graph_node.h",
           "model_ids.h", "course_table.h", "engine/surface_collision.h",
           "game/object_list_processor.h"]

# (name pattern, Rust type), first match wins. Names that match no group are
# not mirrored.
GROUPS = [
    (r"SOUND_TERRAIN_\w+", "i8"),
    (r"SOUND_BANK_\w+", "u8"),
    (r"SOUND_\w+", "u32"),
    (r"ACT_\w+", "u32"),
    (r"(AIR|GROUND|WATER)_STEP_\w+", "u32"),
    (r"INPUT_\w+", "u16"),
    (r"PARTICLE_\w+", "u32"),
    (r"MODEL_STATE_\w+", "i16"),
    (r"MARIO_ANIM_\w+", "i32"),
    (r"MARIO_(EYES|HAND|HAS)_\w+", "i8"),
    (r"MARIO_\w+", "u32"),
    (r"GRAB_POS_\w+", "i8"),
    (r"SURFACE_FLAG_\w+", "i8"),
    (r"SURFACE_\w+", "i16"),
    (r"TERRAIN_\w+", "u16"),
    (r"LEVEL_\w+", "i16"),
    (r"CAMERA_MODE_\w+", "i16"),
    (r"CAM_MOVE_\w+", "u16"),
    (r"CAM_MOVING_INTO_MODE", "u16"),
    (r"CAM_FLAG_\w+", "i16"),
    (r"CAM_MODE_\w+", "i16"),
    (r"CAM_SOUND_\w+", "i16"),
    (r"CAM_SELECTION_\w+", "i32"),
    (r"CAM_ANGLE_\w+", "i32"),
    (r"CAM_FOV_\w+", "u8"),
    (r"CAM_STATUS_\w+", "i16"),
    (r"HAND_CAM_SHAKE_\w+", "u8"),
    (r"DOOR_(DEFAULT|LEAVING_SPECIAL|ENTER_LOBBY)", "u8"),
    (r"CUTSCENE_(STOP|LOOP)", "i16"),
    (r"CUTSCENE_\w+", "u8"),
    (r"DIALOG_NONE", "i16"),
    (r"AREA_\w+", "i32"),
    (r"CAM_EVENT_\w+", "i16"),
    (r"SHAKE_\w+", "i16"),
    (r"INT_STATUS_\w+", "u32"),
    (r"INT_SUBTYPE_\w+", "u32"),
    (r"INTERACT_\w+", "u32"),
    (r"WARP_OP_\w+", "i32"),
    (r"TIMER_CONTROL_\w+", "i32"),
    (r"HUD_DISPLAY_\w+", "u16"),
    (r"ACTIVE_FLAG_\w+", "i16"),
    (r"ACTIVE_PARTICLE_\w+", "u32"),
    (r"OBJ_FLAG_\w+", "u32"),
    (r"HELD_\w+", "i32"),
    (r"SAVE_FLAG_\w+", "u32"),
    (r"GRAPH_RENDER_\w+", "i16"),
    (r"OBJ_LIST_\w+", "i32"),
    (r"TIME_STOP_\w+", "u32"),
    (r"OBJECT_POOL_CAPACITY", "usize"),
    (r"RESPAWN_INFO_\w+", "i16"),
    (r"COIN_FORMATION_\w+", "i32"),
    (r"OBJ_MOVE_\w+", "u32"),
    (r"FLOOR_LOWER_LIMIT(_MISC)?", "i32"),
    (r"MODEL_\w+", "i32"),
    (r"COURSE_\w+", "i16"),
    (r"o[A-Z]\w+", "usize"),
]
# Object field sections beyond the common and Mario fields, by their
# object_fields.h headings.
FIELD_SECTIONS = ["Coin"]


def compile_and_run(source):
    with tempfile.TemporaryDirectory() as tmp:
        c = Path(tmp) / "eval.c"
        exe = Path(tmp) / "eval"
        c.write_text(source)
        result = subprocess.run(["gcc", *FLAGS, str(c), "-o", str(exe)], capture_output=True, text=True)
        if result.returncode:
            return None, result.stderr
        return subprocess.check_output([str(exe)], text=True), ""


def candidates():
    prelude = "".join(f'#include "{h}"\n' for h in HEADERS)
    macros = subprocess.check_output(["gcc", *FLAGS, "-E", "-dM", "-x", "c", "-"], input=prelude, text=True)
    names = []
    for line in macros.splitlines():
        m = re.match(r"#define (\w+) (.+)$", line)
        if m and not m.group(1).startswith("_"):
            names.append(m.group(1))
    # Enum members, read from preprocessed output so included level lists and
    # macro-built members are expanded.
    expanded = subprocess.check_output(["gcc", *FLAGS, "-E", "-P", "-x", "c", "-"], input=prelude, text=True)
    for body in re.findall(r"\benum\s+\w*\s*\{(.*?)\}", expanded, re.S):
        names += [item.split("=")[0].strip() for item in body.split(",") if item.strip()]
    # Object fields: common fields through the Mario section only.
    lines = (DECOMP / "include/object_fields.h").read_text().splitlines()
    end = next(i for i, l in enumerate(lines) if l.startswith("/* Hidden 1-Up */"))
    selected = lines[:end]
    for section in FIELD_SECTIONS:
        start = lines.index(f"/* {section} */")
        stop = next(i for i in range(start + 1, len(lines)) if not lines[i].startswith("#define"))
        selected += lines[start:stop]
    fields = [m.group(1) for l in selected for m in [re.match(r"#define\s+/\*0x\w+\*/\s+(o[A-Z]\w+)\s", l)] if m]
    out = []
    for name in dict.fromkeys(names + fields):
        rust = next((t for pattern, t in GROUPS if re.fullmatch(pattern, name)), None)
        if rust:
            out.append((name, rust))
    return prelude, out


def main():
    prelude, names = candidates()
    # Drop names that are not integer constant expressions (types, strings,
    # function-like uses) by compiling one probe per name.
    probe = prelude + "#include <stdio.h>\nint main(void) {\n"
    probe += "".join(f'    printf("%s %lld\\n", "{n}", (long long) ({n}));\n' for n, _ in names)
    probe += "    return 0;\n}\n"
    output, errors = compile_and_run(probe)
    while output is None:
        bad = set(re.findall(r"\"(\w+)\"", errors)) | {n for n, _ in names if f"'{n}'" in errors}
        lines = [int(m) for m in re.findall(r"eval\.c:(\d+):", errors)]
        offset = prelude.count("\n") + 3
        bad |= {names[l - offset][0] for l in lines if 0 <= l - offset < len(names)}
        if not bad:
            raise SystemExit(errors)
        names = [(n, t) for n, t in names if n not in bad]
        probe = prelude + "#include <stdio.h>\nint main(void) {\n"
        probe += "".join(f'    printf("%s %lld\\n", "{n}", (long long) ({n}));\n' for n, _ in names)
        probe += "    return 0;\n}\n"
        output, errors = compile_and_run(probe)
    values = dict(line.split() for line in output.splitlines())
    rust = [
        "//! Original constants used by the ported Mario code. GENERATED by",
        "//! oracle/tools/gen_constants.py from the vendored, unmodified CC0 decomp",
        "//! headers (pinned n64decomp/sm64) as evaluated by the C compiler; do not",
        "//! edit. `rustario64-oracle` tests that every value still matches them.",
        "#![allow(dead_code)]",
        "",
    ]
    table = ["/* GENERATED by oracle/tools/gen_constants.py (MIT, Rustario64): every",
             " * constant mirrored in src/simulation/mario/constants.rs. */",
             "static const struct { const char *name; long long value; } sConstants[] = {"]
    all_rows = []
    for name, rust_type in names:
        value = int(values[name])
        rust_name = name.upper() if name[0] == "o" else name
        if name[0] == "o":
            rust_name = "O_" + re.sub(r"(?<!^)(?=[A-Z])", "_", name[1:]).upper()
        if rust_type.startswith("u") and value < 0:
            bits = 64 if rust_type == "usize" else int(rust_type[1:])
            literal = f"0x{value & ((1 << bits) - 1):X}"
        elif rust_type.startswith("u"):
            literal = f"0x{value:X}"
        else:
            # Signed mirrors wrap like the C assignment to the signed field.
            bits = int(rust_type[1:])
            value = (value + (1 << (bits - 1))) % (1 << bits) - (1 << (bits - 1))
            literal = str(value)
        rust.append(f"pub const {rust_name}: {rust_type} = {literal};")
        c_type = {"u8": "u8", "u16": "u16", "u32": "u32", "usize": "u64", "i8": "s8", "i16": "s16",
                  "i32": "s32"}[rust_type]
        table.append(f'    {{ "{name}", (long long) ({c_type}) ({name}) }},')
        all_rows.append((name, rust_name))
    rust.append("")
    rust.append("/// Every generated constant by original name, for the oracle consistency test.")
    rust.append(f"pub static ALL: [(&str, i64); {len(all_rows)}] = [")
    rust += [f'    ("{name}", {rust_name} as i64),' for name, rust_name in all_rows]
    rust.append("];")
    table.append("};")
    (ROOT / "src/simulation/mario/constants.rs").write_text("\n".join(rust) + "\n")
    (ORACLE / "c/constants.inc.c").write_text("\n".join(table) + "\n")
    print(len(all_rows), "constants")


main()
