#!/usr/bin/env python3
"""Regenerate or check the oracle's verbatim CC0 excerpts.

Each file in oracle/c/excerpts/ holds items copied byte for byte from one file
of a clean n64decomp/sm64 checkout at the pinned revision, plus authored
include lines. An item's text runs from its first line through its closing
line, plus one newline; its SHA-1 is printed for oracle/README.md.

    extract_excerpts.py --reference /path/to/sm64 [--check]

MIT (Rustario64). Python standard library only.
"""
import argparse
import hashlib
import re
import subprocess
from pathlib import Path

REFERENCE = "9921382a68bb0c865e5e45eb594d9c64db59b1af"
OUT = Path(__file__).resolve().parent.parent / "c" / "excerpts"

# (excerpt file, upstream path, authored includes, items). An item is
# ("function", name), ("line", exact text), or ("range", first line, last line).
EXCERPTS = [
    ("camera.c", "src/game/camera.c",
     ['"sm64.h"', '"engine/math_util.h"', '"engine/surface_collision.h"',
      '"game/object_list_processor.h"', '"game/level_update.h"',
      '"game/camera.h"', '"camera_boundary.h"'],
     [("line", "#define CBUTTON_MASK (U_CBUTTONS | D_CBUTTONS | L_CBUTTONS | R_CBUTTONS)"),
      ("line", "struct PlayerGeometry sMarioGeometry;"),
      ("line", "struct Camera *gCamera;"),
      ("line", "s16 sStatusFlags;"), ("line", "s16 sAreaYaw;"),
      ("line", "s16 sLakituDist;"), ("line", "s16 sLakituPitch;"),
      ("line", "s16 sModeOffsetYaw;"), ("line", "s32 gCurrLevelArea = 0;"),
      ("line", "struct PlayerCameraState *sMarioCamState = &gPlayerCameraState[0];"),
      *[("function", name) for name in [
          "find_c_buttons_pressed", "approach_f32_asymptotic_bool", "approach_f32_asymptotic",
          "set_or_approach_f32_asymptotic", "approach_s16_asymptotic_bool",
          "approach_s16_asymptotic", "approach_vec3f_asymptotic", "approach_vec3s_asymptotic",
          "set_or_approach_vec3f_asymptotic", "camera_approach_s16_symmetric_bool",
          "camera_approach_s16_symmetric", "set_or_approach_s16_symmetric",
          "camera_approach_f32_symmetric_bool", "camera_approach_f32_symmetric",
          "calculate_pitch", "calculate_yaw", "calculate_angles", "calc_abs_dist",
          "calc_hor_dist", "rotate_in_xz", "rotate_in_yz", "scale_along_line",
          "clamp_pitch", "is_pos_in_bounds", "clamp_positions_and_find_yaw",
          "find_in_bounds_yaw_wdw_bob_thi", "collide_with_walls",
          "resolve_geometry_collisions", "find_mario_floor_and_ceil", "look_down_slopes",
          "calc_y_to_curr_floor", "focus_on_mario", "update_radial_camera"]]]),
    ("game_init.c", "src/game/game_init.c",
     ['"sm64.h"', '"game/game_init.h"'],
     [("function", "adjust_analog_stick")]),
    ("graph_node.c", "src/engine/graph_node.c",
     ['"sm64.h"', '"engine/graph_node.h"', '"engine/math_util.h"', '"game/area.h"',
      '"game/memory.h"'],
     [("line", "Vec3f gVec3fZero = { 0.0f, 0.0f, 0.0f };"),
      ("line", "Vec3s gVec3sZero = { 0, 0, 0 };"),
      ("line", "Vec3f gVec3fOne = { 1.0f, 1.0f, 1.0f };"),
      ("function", "retrieve_animation_index"), ("function", "geo_update_animation_frame"),
      ("function", "init_scene_graph_node_links"), ("function", "init_graph_node_object"),
      ("function", "geo_obj_init_spawninfo")]),
    ("rendering_graph_node.c", "src/game/rendering_graph_node.c",
     ['"sm64.h"', '"engine/graph_node.h"', '"game/memory.h"', '"game/rendering_graph_node.h"'],
     [("line", "u8 gCurrAnimType;"),
      ("line", "u8 gCurrAnimEnabled;"),
      ("line", "s16 gCurrAnimFrame;"),
      ("line", "f32 gCurrAnimTranslationMultiplier;"),
      ("line", "u16 *gCurrAnimAttribute;"),
      ("line", "s16 *gCurrAnimData;"),
      ("function", "geo_set_animation_globals")]),
    ("macro_special_objects.c", "src/game/macro_special_objects.c",
     ['"sm64.h"', '"model_ids.h"', '"behavior_data.h"', '"engine/surface_load.h"',
      '"game/macro_special_objects.h"', '"game/object_list_processor.h"', '"special_presets.inc.c"'],
     [("function", "spawn_special_objects"), ("function", "get_special_objects_size")]),
    ("interaction.c", "src/game/interaction.c",
     ['"sm64.h"', '"behavior_data.h"', '"audio/external.h"', '"engine/math_util.h"',
      '"engine/surface_collision.h"', '"game/area.h"', '"game/camera.h"',
      '"game/game_init.h"', '"game/interaction.h"', '"game/level_update.h"', '"game/mario.h"',
      '"game/memory.h"', '"game/object_helpers.h"', '"game/save_file.h"', '"game/sound_init.h"',
      '"interaction_boundary.h"'],
     [("line", "u8 sDelayInvincTimer;"),
      ("line", "s16 sInvulnerable;"),
      ("range", "u32 interact_coin(struct MarioState *, u32, struct Object *);",
       "u32 interact_text(struct MarioState *, u32, struct Object *);"),
      ("range", "struct InteractionHandler {", "};"),
      ("range", "static struct InteractionHandler sInteractionHandlers[] = {", "};"),
      ("line", "static u8 sDisplayingDoorText = FALSE;"),
      ("line", "static u8 sJustTeleported = FALSE;"),
      ("line", "static u8 sPSSSlideStarted = FALSE;"),
      ("function", "mario_obj_angle_to_object"),
      ("function", "mario_stop_riding_object"),
      ("function", "mario_grab_used_object"),
      ("function", "mario_drop_held_object"),
      ("function", "mario_throw_held_object"),
      ("function", "mario_stop_riding_and_holding"),
      ("function", "does_mario_have_normal_cap_on_head"),
      ("function", "mario_blow_off_cap"),
      ("function", "mario_get_collided_object"),
      ("function", "mario_check_object_grab"),
      ("function", "check_kick_or_punch_wall"),
      ("function", "mario_process_interactions"),
      ("function", "check_death_barrier"),
      ("function", "check_lava_boost"),
      ("function", "pss_begin_slide"),
      ("function", "pss_end_slide"),
      ("function", "mario_handle_special_floors")]),
    ("object_collision.c", "src/game/object_collision.c",
     ['"sm64.h"', '"game/object_list_processor.h"'],
     [("function", "clear_object_collision")]),
    ("object_helpers.c", "src/game/object_helpers.c",
     ['"sm64.h"', '"game/object_helpers.h"'],
     [("function", "absf")]),
    ("object_list_processor.c", "src/game/object_list_processor.c",
     ['"sm64.h"', '"game/level_update.h"', '"game/object_list_processor.h"'],
     [("function", "copy_mario_state_to_object")]),
    ("platform_displacement.c", "src/game/platform_displacement.c",
     ['"sm64.h"', '"engine/math_util.h"', '"engine/surface_collision.h"',
      '"game/object_helpers.h"', '"game/object_list_processor.h"',
      '"game/platform_displacement.h"'],
     [("line", "struct Object *gMarioPlatform = NULL;"),
      ("function", "update_mario_platform")]),
]


def extract(lines, item, path):
    kind = item[0]
    if kind == "line":
        hits = [i for i, line in enumerate(lines) if line == item[1]]
        assert len(hits) == 1, (path, item)
        return lines[hits[0]] + "\n"
    if kind == "range":
        start = [i for i, line in enumerate(lines) if line == item[1]]
        assert len(start) == 1, (path, item)
        end = next(i for i in range(start[0], len(lines)) if lines[i] == item[2])
        return "\n".join(lines[start[0]:end + 1]) + "\n"
    name = item[1]
    # Return-type macros such as BAD_RETURN(f32) occur in camera.c.
    pattern = re.compile(r"^[A-Za-z_][\w \*()]*\b" + re.escape(name) + r"\(")
    starts = []
    for i, line in enumerate(lines):
        if pattern.match(line):
            # Skip prototypes: the declaration reaches ';' before any '{'.
            j = i
            while "{" not in lines[j] and ";" not in lines[j]:
                j += 1
            if "{" in lines[j]:
                starts.append(i)
    assert len(starts) == 1, (path, name, starts)
    end = next(i for i in range(starts[0], len(lines)) if lines[i] == "}")
    return "\n".join(lines[starts[0]:end + 1]) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--check", action="store_true", help="fail if any excerpt differs")
    options = parser.parse_args()
    root = options.reference
    revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    if revision != REFERENCE:
        parser.error("reference checkout must be at " + REFERENCE)
    if subprocess.check_output(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"],
                               text=True).strip():
        parser.error("reference checkout has modified tracked files")
    failures = 0
    for name, upstream, includes, items in EXCERPTS:
        lines = (root / upstream).read_text().split("\n")
        out = [
            f"/* Verbatim CC0 excerpts from n64decomp/sm64 {REFERENCE}\n",
            f" * {upstream}, between the item markers. Generated by\n",
            " * oracle/tools/extract_excerpts.py from a clean pinned checkout; do not\n",
            " * edit. The include lines are authored (MIT). Item SHA-1s: oracle/README.md. */\n",
        ]
        out += [f"#include {include}\n" for include in includes]
        for item in items:
            text = extract(lines, item, upstream)
            label = item[1] if item[0] == "function" else item[1].rstrip("{; =")
            print(f"{upstream} | {label} | {hashlib.sha1(text.encode()).hexdigest()}")
            out.append(f"\n/* {upstream}: {label} */\n")
            out.append(text)
        text = "".join(out)
        path = OUT / name
        if options.check:
            if not path.exists() or path.read_text() != text:
                print(f"MISMATCH {path}")
                failures += 1
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    if failures:
        raise SystemExit(f"{failures} excerpt files differ from the pinned source")


main()
