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
    ("object_step.c", "src/game/obj_behaviors.c",
     ['"object_step_boundary.h"'],
     [("line", "#define o gCurrentObject"),
      *[("line", line) for line in [
          "#define OBJ_COL_FLAG_GROUNDED   (1 << 0)",
          "#define OBJ_COL_FLAG_HIT_WALL   (1 << 1)",
          "#define OBJ_COL_FLAG_UNDERWATER (1 << 2)",
          "#define OBJ_COL_FLAG_NO_Y_VEL   (1 << 3)",
          "#define OBJ_COL_FLAGS_LANDED    (OBJ_COL_FLAG_GROUNDED | OBJ_COL_FLAG_NO_Y_VEL)",
          "static struct Surface *sObjFloor;",
          "static s8 sOrientObjWithFloor = TRUE;",
      ]],
      *[("function", name) for name in [
          "turn_obj_away_from_surface", "obj_find_wall", "turn_obj_away_from_steep_floor",
          "obj_orient_graph", "calc_obj_friction", "calc_new_obj_vel_and_pos_y",
          "calc_new_obj_vel_and_pos_y_underwater", "obj_update_pos_vel_xz",
          "obj_splash", "object_step", "object_step_without_floor_orient"]]]),
    ("shadow_geo.c", "src/game/rendering_graph_node.c",
     ['"shadow_geo_boundary.h"'], [("function", "geo_process_shadow")]),
    ("shadow.c", "src/game/shadow.c",
     ['"sm64.h"', '"mario_animation_ids.h"', '"engine/math_util.h"',
      '"engine/surface_collision.h"', '"game/object_list_processor.h"', '"shadow_boundary.h"'],
     [("range", "struct Shadow {", "};"),
      *[("line", line) for line in [
          "#define SHADOW_SOLIDITY_NO_SHADOW 0", "#define SHADOW_SOILDITY_ALREADY_SET 1",
          "#define SHADOW_SOLIDITY_NOT_YET_SET 2", "#define SHADOW_WITH_9_VERTS 0",
          "#define SHADOW_WITH_4_VERTS 1", "s8 gShadowAboveWaterOrLava;",
          "s8 sMarioOnFlyingCarpet;",
      ]],
      *[("function", name) for name in [
          "atan2_deg", "scale_shadow_with_distance", "dim_shadow_with_distance",
          "get_water_level_below_shadow", "init_shadow", "get_texture_coords_9_vertices",
          "get_texture_coords_4_vertices", "make_shadow_vertex_at_xyz",
          "extrapolate_vertex_y_position", "get_vertex_coords", "calculate_vertex_xyz",
          "floor_local_tilt", "make_shadow_vertex", "linearly_interpolate_solidity_positive",
          "linearly_interpolate_solidity_negative", "correct_shadow_solidity_for_animations",
      ]]]),
    ("geo_misc.c", "src/game/geo_misc.c", ['"sm64.h"', '"shadow_boundary.h"'],
     [("function", "round_float"),
      ("range", "#ifndef GBI_FLOATS", "}")]),
    ("camera.c", "src/game/camera.c",
     ['"sm64.h"', '"dialog_ids.h"', '"audio/external.h"', '"engine/behavior_script.h"',
      '"engine/graph_node.h"', '"engine/math_util.h"', '"engine/surface_collision.h"',
      '"game/area.h"', '"game/game_init.h"', '"game/hud.h"', '"game/ingame_menu.h"',
      '"game/object_list_processor.h"', '"game/level_update.h"',
      '"game/camera.h"', '"camera_boundary.h"'],
     [("line", "#define CBUTTON_MASK (U_CBUTTONS | D_CBUTTONS | L_CBUTTONS | R_CBUTTONS)"),
      ("line", "struct PlayerGeometry sMarioGeometry;"),
      ("line", "struct Camera *gCamera;"),
      ("line", "s16 sStatusFlags;"), ("line", "s16 sAreaYaw;"),
      ("line", "s16 sLakituDist;"), ("line", "s16 sLakituPitch;"),
      ("line", "s16 sModeOffsetYaw;"), ("line", "s32 gCurrLevelArea = 0;"),
      ("line", "struct PlayerCameraState *sMarioCamState = &gPlayerCameraState[0];"),
      *[("line", line) for line in [
          "struct LakituState gLakituState;", "struct CameraFOVStatus sFOVState;",
          "struct TransitionInfo sModeTransition;", "struct ModeTransitionInfo sModeInfo;",
          "Vec3f sOldPosition;", "Vec3f sOldFocus;", "Vec3f sPlayer2FocusOffset;",
          "s16 sYawSpeed = 0x400;", "s16 sCUpCameraPitch;", "s16 sAreaYawChange;",
          "f32 sPanDistance;", "f32 sCannonYOffset;", "s16 unusedSplinePitch;",
          "s16 unusedSplineYaw;", "struct HandheldShakePoint sHandheldShakeSpline[4];",
          "s16 sHandheldShakeMag;", "f32 sHandheldShakeTimer;", "f32 sHandheldShakeInc;",
          "s16 sHandheldShakePitch;", "s16 sHandheldShakeYaw;", "s16 sHandheldShakeRoll;",
          "s16 s2ndRotateFlags;",
      ]],
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
          "calc_y_to_curr_floor", "focus_on_mario", "update_radial_camera",
          "transition_next_state", "transition_to_camera_mode", "next_lakitu_state",
          "set_camera_pitch_shake", "set_camera_yaw_shake", "set_camera_roll_shake",
          "increment_shake_offset", "shake_camera_pitch", "shake_camera_yaw", "shake_camera_roll",
          "set_fov_shake", "set_camera_shake_from_hit", "evaluate_cubic_spline",
          "random_vec3s", "shake_camera_handheld", "update_lakitu",
          "calc_avoid_yaw", "is_surf_within_bounding_box", "is_behind_surface",
          "is_range_behind_surface", "is_mario_behind_surface", "rotate_camera_around_walls",
          "offset_yaw_outward_radial", "radial_camera_move", "lakitu_zoom",
          "pan_ahead_of_player"]],
      # The complete update_camera path for areas without camera triggers
      # (BOB): mode dispatch, initialization, area processing and the modes
      # that path reaches. Unreachable modes and cutscenes are authored
      # aborting stubs in c/camera_unit.c; no cutscene or spline data is copied.
      *[("line", line) for line in [
          "struct PlayerCameraState gPlayerCameraState[2];",
          "s16 sCreditsPlayer2Pitch;", "s16 sCreditsPlayer2Yaw;", "u8 sFramesPaused;",
          "s16 unusedFreeRoamWallYaw;", "s16 sAvoidYawVel;", "s16 sCameraYawAfterDoorCutscene;",
          "struct CutsceneSplinePoint sCurCreditsSplinePos[32];",
          "struct CutsceneSplinePoint sCurCreditsSplineFocus[32];",
          "f32 sCutsceneSplineSegmentProgress;", "s16 sCutsceneSplineSegment;",
          "s16 unused8033B6E8;", "u32 gCutsceneObjSpawn;", "s32 gObjCutsceneDone;",
          "u32 unused8033B30C;", "u32 unused8033B310;", "s16 sSelectionFlags;",
          "s16 gCameraMovementFlags;", "s16 unused8033B316;", "s16 unused8033B31A;",
          "s16 sCameraSoundFlags;", "u16 sCButtonsPressed;", "s16 sCutsceneShot;",
          "s16 gCutsceneTimer;", "f32 sZoomAmount;", "s16 sCSideButtonYaw;",
          "s16 sBehindMarioSoundTimer;", "f32 sZeroZoomDist;", "s16 sSpiralStairsYawOffset;",
          "s16 s8DirModeBaseYaw;", "s16 s8DirModeYawOffset;",
          "struct CutsceneVariable sCutsceneVars[10];", "Vec3f sCastleEntranceOffset;",
          "struct CameraStoredInfo sCameraStoreCUp;", "struct Object *gCutsceneFocus = NULL;",
          "u32 unused8032CFC8 = 0;", "u32 unused8032CFCC = 0;",
          "struct Object *gSecondCameraFocus = NULL;", "u32 gPrevLevel = 0;",
          "f32 gCameraZoomDist = 800.0f;", "u8 sObjectCutscene = 0;", "u8 gRecentCutscene = 0;",
          "u8 sFramesSinceCutsceneEnded = 0;",
          "struct PlayerCameraState *sLuigiCamState = &gPlayerCameraState[1];",
          "Vec3f sFixedModeBasePosition    = { 646.0f, 143.0f, -1513.0f };",
          "typedef s32 (*CameraTransition)(struct Camera *c, Vec3f, Vec3f);",
      ]],
      ("range", "u8 sZoomOutAreaMasks[] = {", "};"),
      *[("function", name) for name in [
          "vec3f_sub", "object_pos_to_vec3f", "vec3f_compare", "is_within_100_units_of_mario",
          "offset_rotated", "set_environmental_camera_shake", "set_handheld_shake",
          "play_sound_cbutton_up", "play_sound_cbutton_down", "play_sound_cbutton_side",
          "play_sound_button_change_blocked", "play_sound_rbutton_changed",
          "play_camera_buzz_if_cdown", "play_sound_if_cam_switched_to_lakitu_or_mario",
          "cam_select_alt_mode", "set_cam_angle", "update_camera_hud_status",
          "approach_camera_height", "set_camera_height", "store_lakitu_cam_info_for_c_up",
          "set_mode_c_up", "radial_camera_input", "radial_camera_input_default",
          "update_yaw_and_dist_from_c_up", "mode_radial_camera", "handle_c_button_movement",
          "update_mario_camera", "update_default_camera", "mode_default_camera",
          "mode_lakitu_camera", "mode_mario_camera", "update_boss_fight_camera",
          "mode_boss_fight_camera", "set_camera_mode_boss_fight", "set_camera_mode_radial",
          "exit_c_up", "update_c_up", "move_mario_head_c_up", "move_into_c_up",
          "mode_c_up_camera", "nop_update_water_camera", "set_camera_mode",
          "check_blocking_area_processing", "surface_type_modes",
          "set_mode_if_not_set_by_surface", "surface_type_modes_thi",
          "camera_course_processing", "clear_cutscene_vars", "start_cutscene",
          "open_door_cutscene", "get_cutscene_from_mario_status", "stub_camera_2",
          "stub_camera_3", "reset_camera", "init_camera", "select_mario_cam_mode",
          "create_camera", "zoom_out_if_paused_and_outside", "update_graph_node_camera",
          "geo_camera_main", "update_camera", "shake_camera_fov",
          "set_fov_30", "approach_fov_20", "set_fov_45", "set_fov_29", "zoom_fov_30",
          "fov_default", "approach_fov_30", "approach_fov_60", "approach_fov_45",
          "approach_fov_80", "set_fov_bbh", "geo_camera_fov", "set_fov_function"]]]),
    ("behavior_script.c", "src/engine/behavior_script.c",
     ['"sm64.h"', '"behavior_data.h"', '"engine/behavior_script.h"', '"engine/graph_node.h"',
      '"engine/surface_collision.h"', '"game/area.h"', '"game/game_init.h"', '"game/mario.h"',
      '"game/memory.h"', '"game/object_helpers.h"', '"game/object_list_processor.h"'],
     [("range", "#define BHV_CMD_GET_1ST_U8(index)  (u8)((gCurBhvCommand[index] >> 24) & 0xFF) // unused",
       "#define BHV_CMD_GET_ADDR_OF_CMD(index) (uintptr_t)(&gCurBhvCommand[index])"),
      ("line", "static u16 gRandomSeed16;"),
      ("function", "random_u16"), ("function", "random_float"),
      *[("function", name) for name in [
          "obj_update_gfx_pos_and_angle", "cur_obj_bhv_stack_push", "cur_obj_bhv_stack_pop",
          "bhv_cmd_hide", "bhv_cmd_disable_rendering", "bhv_cmd_billboard", "bhv_cmd_set_model",
          "bhv_cmd_spawn_child", "bhv_cmd_spawn_obj", "bhv_cmd_spawn_child_with_param",
          "bhv_cmd_deactivate", "bhv_cmd_break", "bhv_cmd_break_unused", "bhv_cmd_call",
          "bhv_cmd_return", "bhv_cmd_delay", "bhv_cmd_delay_var", "bhv_cmd_goto",
          "bhv_cmd_begin_repeat_unused", "bhv_cmd_begin_repeat", "bhv_cmd_end_repeat",
          "bhv_cmd_end_repeat_continue", "bhv_cmd_begin_loop", "bhv_cmd_end_loop"]],
      ("line", "typedef void (*NativeBhvFunc)(void);"),
      *[("function", name) for name in [
          "bhv_cmd_call_native", "bhv_cmd_set_float", "bhv_cmd_set_int", "bhv_cmd_set_int_unused",
          "bhv_cmd_set_random_float", "bhv_cmd_set_random_int", "bhv_cmd_set_int_rand_rshift",
          "bhv_cmd_add_random_float", "bhv_cmd_add_int_rand_rshift", "bhv_cmd_add_float",
          "bhv_cmd_add_int", "bhv_cmd_or_int", "bhv_cmd_bit_clear", "bhv_cmd_load_animations",
          "bhv_cmd_animate", "bhv_cmd_drop_to_floor", "bhv_cmd_nop_1", "bhv_cmd_nop_3",
          "bhv_cmd_nop_2", "bhv_cmd_sum_float", "bhv_cmd_sum_int", "bhv_cmd_set_hitbox",
          "bhv_cmd_set_hurtbox", "bhv_cmd_set_hitbox_with_offset", "bhv_cmd_nop_4", "bhv_cmd_begin",
          "bhv_cmd_load_collision_data", "bhv_cmd_set_home", "bhv_cmd_set_interact_type",
          "bhv_cmd_set_interact_subtype", "bhv_cmd_scale", "bhv_cmd_set_obj_physics",
          "bhv_cmd_parent_bit_clear", "bhv_cmd_spawn_water_droplet", "bhv_cmd_animate_texture",
          "stub_behavior_script_2"]],
      ("line", "typedef s32 (*BhvCommandProc)(void);"),
      ("range", "static BhvCommandProc BehaviorCmdTable[] = {", "};"),
      ("function", "cur_obj_update")]),
    ("behavior_data.c", "data/behavior_data.c", ['"behavior_data_boundary.h"'],
     [("range", "#define BC_B(a) _SHIFTL(a, 24, 8)", "    BC_PTR(dropletParams)"),
      *[("range", f"const BehaviorScript {name}[] = {{", "};") for name in [
          "bhvCoinFormationSpawn", "bhvCoinFormation", "bhvYellowCoin", "bhvCoinSparkles",
          "bhvGoldenCoinSparkles", "bhvMario", "bhvSpinAirborneWarp"]]]),
    ("coin.c", "src/game/behaviors/coin.inc.c",
     ['"sm64.h"', '"behavior_data.h"', '"model_ids.h"', '"engine/behavior_script.h"',
      '"engine/math_util.h"', '"engine/surface_collision.h"', '"game/interaction.h"',
      '"game/object_helpers.h"', '"game/object_list_processor.h"', '"coin_boundary.h"'],
     [("range", "struct ObjectHitbox sYellowCoinHitbox = {", "};"),
      *[("function", name) for name in [
          "bhv_coin_sparkles_init", "bhv_yellow_coin_init", "bhv_yellow_coin_loop",
          "bhv_coin_formation_spawn_loop"]],
      ("range", "s16 sCoinArrowPositions[][2] = {", "};"),
      *[("function", name) for name in [
          "spawn_coin_in_formation", "bhv_coin_formation_init", "bhv_coin_formation_loop",
          "bhv_coin_sparkles_loop", "bhv_golden_coin_sparkles_loop"]]]),
    ("rendering_view.c", "src/game/rendering_graph_node.c",
     ['"sm64.h"', '"engine/graph_node.h"', '"engine/math_util.h"'],
     [("function", "obj_is_in_view")]),
    ("geo_layout.c", "src/engine/geo_layout.c", ['"sm64.h"', '"engine/graph_node.h"'],
     [("line", "struct GraphNode gObjParentGraphNode;")]),
    ("macro_preset_struct.inc.c", "include/macro_presets.inc.c", ['"sm64.h"'],
     [("range", "struct MacroPreset {", "};")]),
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
      ("function", "geo_obj_init_spawninfo"),
      *[("function", name) for name in [
          "init_graph_node_start", "geo_add_child", "geo_remove_child", "geo_make_first_child",
          "geo_reset_object_node", "geo_obj_init", "geo_obj_init_animation"]]]),
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
      '"game/macro_special_objects.h"', '"game/object_helpers.h"', '"game/object_list_processor.h"',
      '"special_presets.inc.c"', '"excerpts/macro_preset_struct.inc.c"', '"macro_preset_boundary.h"'],
     [("function", "convert_rotation"),
      *[("line", f"#define MACRO_OBJ_{name} {i}") for i, name in
        enumerate(["Y_ROT", "X", "Y", "Z", "PARAMS"])],
      ("range", "struct LoadedPreset {", "};"),
      ("function", "spawn_macro_objects"),
      ("function", "spawn_special_objects"), ("function", "get_special_objects_size")]),
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
      ("function", "interact_coin"),
      ("function", "mario_get_collided_object"),
      ("function", "mario_check_object_grab"),
      ("function", "check_kick_or_punch_wall"),
      ("function", "mario_process_interactions"),
      ("function", "check_death_barrier"),
      ("function", "check_lava_boost"),
      ("function", "pss_begin_slide"),
      ("function", "pss_end_slide"),
      ("function", "mario_handle_special_floors")]),
    ("object_helpers.c", "src/game/object_helpers.c",
     ['"sm64.h"', '"behavior_data.h"', '"level_table.h"', '"engine/graph_node.h"',
      '"engine/math_util.h"', '"engine/surface_collision.h"', '"game/area.h"',
      '"game/object_helpers.h"', '"game/object_list_processor.h"', '"game/rendering_graph_node.h"',
      '"game/spawn_object.h"'],
     [("line", "static s16 sPowersOfTwo[] = { 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80 };"),
      ("line", "static s8 sLevelsWithRooms[] = { LEVEL_BBH, LEVEL_CASTLE, LEVEL_HMC, -1 };"),
      ("line", "#define o gCurrentObject"),
      ("range_at", "Gfx *geo_switch_anim_state(s32 callContext, struct GraphNode *node, UNUSED void *context)",
       1, "}"),
      *[("function", name) for name in [
          "dist_between_objects", "obj_angle_to_object", "obj_set_parent_relative_pos", "obj_set_pos",
          "obj_set_angle", "spawn_object_abs_with_rot", "spawn_object_at_origin", "spawn_object",
          "obj_build_relative_transform", "spawn_object_relative", "obj_copy_pos_and_angle",
          "obj_copy_pos", "obj_copy_angle", "obj_scale", "cur_obj_scale", "cur_obj_enable_rendering",
          "cur_obj_disable_rendering", "cur_obj_hide", "find_unimportant_object", "cur_obj_set_model",
          "obj_mark_for_deletion", "cur_obj_update_floor_height", "cur_obj_set_behavior",
          "cur_obj_has_behavior", "obj_translate_local", "obj_build_transform_from_pos_and_angle",
          "obj_apply_scale_to_transform", "obj_set_face_angle_to_move_angle",
          "cur_obj_move_xz_using_fvel_and_yaw", "cur_obj_move_y_with_terminal_vel",
          "obj_build_transform_relative_to_parent", "obj_set_throw_matrix_from_transform",
          "cur_obj_enable_rendering_if_mario_in_room", "cur_obj_become_tangible",
          "cur_obj_become_intangible", "obj_set_hitbox", "absf", "bit_shift_left",
          "is_item_in_array", "bhv_init_room"]]]),
    ("object_list_processor.c", "src/game/object_list_processor.c",
     ['"sm64.h"', '"engine/behavior_script.h"', '"engine/graph_node.h"', '"engine/surface_load.h"',
      '"game/debug.h"', '"game/interaction.h"', '"game/level_update.h"', '"game/mario.h"',
      '"game/memory.h"', '"game/object_collision.h"', '"game/object_helpers.h"',
      '"game/object_list_processor.h"', '"game/platform_displacement.h"', '"game/spawn_object.h"'],
     [*[("line", line) for line in [
         "s32 gDebugInfoFlags;", "s32 gUnknownWallCount;", "u32 gObjectCounter;",
         "struct Object gObjectPool[OBJECT_POOL_CAPACITY];", "struct ObjectNode gFreeObjectList;",
         "const BehaviorScript *gCurBhvCommand;", "s16 gPrevFrameObjectCount;",
         "struct MemoryPool *gObjectMemoryPool;", "RoomData gDoorAdjacentRooms[60][2];",
         "s16 gMarioCurrentRoom;", "s16 D_8035FEE2;", "s16 D_8035FEE4;", "s16 gTHIWaterDrained;",
         "s16 gNumRoomedObjectsInMarioRoom;", "s16 gNumRoomedObjectsNotInMarioRoom;",
         "s16 gWDWWaterLevelChanging;", "s16 gMarioOnMerryGoRound;"]],
      ("range", "s8 sObjectListUpdateOrder[] = { OBJ_LIST_SPAWNER,", "                                -1 };"),
      *[("function", name) for name in [
          "copy_mario_state_to_object", "update_objects_starting_at",
          "update_objects_during_time_stop", "update_objects_in_list",
          "unload_deactivated_objects_in_list", "set_object_respawn_info_bits",
          "spawn_objects_from_info", "stub_obj_list_processor_1", "clear_objects",
          "update_terrain_objects", "update_non_terrain_objects", "unload_deactivated_objects",
          "update_objects"]]]),
    ("level_update.c", "src/game/level_update.c",
     ['"sm64.h"', '"sounds.h"', '"course_table.h"', '"audio/external.h"', '"game/area.h"',
      '"game/game_init.h"', '"game/level_update.h"', '"game/mario.h"'],
     [("function", "update_hud_values")]),
    ("platform_displacement.c", "src/game/platform_displacement.c",
     ['"sm64.h"', '"engine/math_util.h"', '"engine/surface_collision.h"',
      '"game/object_helpers.h"', '"game/object_list_processor.h"',
      '"game/platform_displacement.h"'],
     [("line", "struct Object *gMarioPlatform = NULL;"),
      ("function", "update_mario_platform"),
      ("function", "apply_mario_platform_displacement"),
      ("function", "clear_mario_platform")]),
]


def extract(lines, item, path):
    kind = item[0]
    if kind == "line":
        hits = [i for i, line in enumerate(lines) if line == item[1]]
        assert len(hits) == 1, (path, item)
        return lines[hits[0]] + "\n"
    if kind == "range_at":
        start = [i for i, line in enumerate(lines) if line == item[1]]
        assert len(start) == 1, (path, item)
        first = start[0] - item[2]
        end = next(i for i in range(start[0], len(lines)) if lines[i] == item[3])
        return "\n".join(lines[first:end + 1]) + "\n"
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
            (" * oracle/tools/extract_excerpts.py from pinned source; do not\n"
             if name in {"shadow.c", "shadow_geo.c", "geo_misc.c"} else
             " * oracle/tools/extract_excerpts.py from a clean pinned checkout; do not\n"),
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
