/* Authored declarations for a root-position-only original geo traversal. */
#ifndef ORACLE_SHADOW_GEO_BOUNDARY_H
#define ORACLE_SHADOW_GEO_BOUNDARY_H
#include "game/rendering_graph_node.h"
#include "game/memory.h"
extern s16 gMatStackIndex;
extern Mat4 gMatStack[32];
extern Mtx *gMatStackFixed[32];
extern u8 gCurrAnimType, gCurrAnimEnabled;
extern s16 gCurrAnimFrame;
extern f32 gCurrAnimTranslationMultiplier;
extern u16 *gCurrAnimAttribute;
extern s16 *gCurrAnimData;
extern s8 gMarioOnIceOrCarpet;
Gfx *create_shadow_below_xyz(f32, f32, f32, s16, u8, s8);
void geo_append_display_list(void *, s16);
void geo_set_animation_globals(struct AnimInfo *, s32);
#endif
