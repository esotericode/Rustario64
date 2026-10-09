/* Authored host declarations (MIT). Macro values from pinned CC0 headers. */
#ifndef ORACLE_INPUT_REFERENCE_H
#define ORACLE_INPUT_REFERENCE_H
#include "sm64.h"
#define A_BUTTON 0x8000
#define B_BUTTON 0x4000
#define Z_TRIG 0x2000
#define CAM_MOVE_C_UP_MODE              0x2000
#define INT_STATUS_MARIO_STUNNED         (1 <<  0) /* 0x00000001 */
#define INT_STATUS_MARIO_KNOCKBACK_DMG   (1 <<  1) /* 0x00000002 */
#define INT_STATUS_MARIO_SHOCKWAVE       (1 <<  4) /* 0x00000010 */
#define WARP_OP_DEATH         0x12
extern u16 gCameraMovementFlags;
void adjust_analog_stick(struct Controller *controller);
void update_mario_inputs(struct MarioState *m);
void update_mario_button_inputs(struct MarioState *m);
void update_mario_joystick_inputs(struct MarioState *m);
s32 mario_get_floor_class(struct MarioState *m);
u32 mario_floor_is_slippery(struct MarioState *m);
void debug_print_speed_action_normal(struct MarioState *m);
s16 level_trigger_warp(struct MarioState *m, s32 warpOp);
#endif
