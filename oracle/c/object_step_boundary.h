/* Authored development-only object_step boundary (MIT). Motion, queries and
 * matrices run verbatim; display-arena allocation and splash requests are
 * captured without enabling unported water particle behavior scripts. */
#ifndef RUSTARIO64_OBJECT_STEP_BOUNDARY_H
#define RUSTARIO64_OBJECT_STEP_BOUNDARY_H
#include "sm64.h"
#include "audio/external.h"
#include "engine/math_util.h"
#include "engine/surface_collision.h"
#include "game/game_init.h"
#include "game/object_list_processor.h"

typedef struct {
    u32 raw[0x50];
    f32 hitboxRadius, hitboxHeight;
    s32 gfxFlags, activeFlags;
    u32 globalTimer;
    s32 includeIntangible, forCamera, orientWithFloor, matrixAvailable;
} OracleObjectStepInput;

typedef struct {
    u32 raw[0x50];
    f32 matrix[16];
    s32 hasMatrix, floor, collisionFlags, includeAfter;
    s32 effectCount, effects[9];
} OracleObjectStepOutput;

extern const BehaviorScript oracle_step_wave_behavior[1];
extern const BehaviorScript oracle_step_bubble_behavior[1];
void *oracle_step_alloc(u32 size);
struct Object *oracle_step_spawn(struct Object *parent, s32 model, const BehaviorScript *behavior);
void oracle_step_sound(s32 sound);

/* The frame harness compiles the same functions with their real calls
 * (obj_behaviors_unit.c); this component copy takes prefixed names. */
#define turn_obj_away_from_surface oracle_component_turn_obj_away_from_surface
#define obj_find_wall oracle_component_obj_find_wall
#define turn_obj_away_from_steep_floor oracle_component_turn_obj_away_from_steep_floor
#define obj_orient_graph oracle_component_obj_orient_graph
#define calc_obj_friction oracle_component_calc_obj_friction
#define calc_new_obj_vel_and_pos_y oracle_component_calc_new_obj_vel_and_pos_y
#define calc_new_obj_vel_and_pos_y_underwater oracle_component_calc_new_obj_vel_and_pos_y_underwater
#define obj_update_pos_vel_xz oracle_component_obj_update_pos_vel_xz
#define obj_splash oracle_component_obj_splash
#define object_step oracle_component_object_step
#define object_step_without_floor_orient oracle_component_object_step_without_floor_orient

/* These substitutions affect only the component excerpt's boundary calls. */
#define alloc_display_list oracle_step_alloc
#define spawn_object oracle_step_spawn
#define cur_obj_play_sound_2 oracle_step_sound
#define bhvObjectWaterWave oracle_step_wave_behavior
#define bhvObjectBubble oracle_step_bubble_behavior
#endif
