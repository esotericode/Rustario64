/* Authored (MIT, Rustario64): the context of the verbatim obj_behaviors.c
 * excerpt in the frame harness, standing in for that file's own includes.
 * Unlike the object_step component (object_step_boundary.h), its calls are
 * the real ones: spawn_object, cur_obj_play_sound_2 and the camera run, and
 * obj_orient_graph's matrices come from the per-frame display-list arena
 * (oracle_frame_alloc_display_list in runtime_glue.c). */
#ifndef ORACLE_OBJ_BEHAVIORS_BOUNDARY_H
#define ORACLE_OBJ_BEHAVIORS_BOUNDARY_H
#include "sm64.h"
#include "audio/external.h"
#include "behavior_data.h"
#include "engine/behavior_script.h"
#include "engine/graph_node.h"
#include "engine/math_util.h"
#include "engine/surface_collision.h"
#include "game/camera.h"
#include "game/game_init.h"
#include "game/interaction.h"
#include "game/obj_behaviors.h"
#include "game/object_helpers.h"
#include "game/object_list_processor.h"
#include "game/spawn_object.h"
#include "game/spawn_sound.h"

void *oracle_frame_alloc_display_list(u32 size);
#define alloc_display_list oracle_frame_alloc_display_list
#endif
