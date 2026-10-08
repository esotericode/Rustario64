/* Authored shim (MIT, Rustario64): declarations from src/game/mario.h used by
 * mario_step.c. The four helpers are copied verbatim into oracle.c. */
#include "sm64.h"
struct Surface *resolve_and_return_wall_collisions(Vec3f pos, f32 offset, f32 radius);
f32 vec3f_find_ceil(Vec3f pos, f32 height, struct Surface **ceil);
void mario_set_forward_vel(struct MarioState *m, f32 speed);
u32 mario_get_terrain_sound_addend(struct MarioState *m);
u32 set_mario_action(struct MarioState *m, u32 action, u32 actionArg);
s32 drop_and_set_mario_action(struct MarioState *m, u32 action, u32 actionArg);
void update_mario_sound_and_camera(struct MarioState *m);
