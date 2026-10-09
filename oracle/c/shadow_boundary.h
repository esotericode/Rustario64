/* Authored declarations for the original shadow helper excerpts (MIT). */
#ifndef ORACLE_SHADOW_BOUNDARY_H
#define ORACLE_SHADOW_BOUNDARY_H
#include <math.h>
#define FLOOR_LOWER_LIMIT_SHADOW -10000.0f
s16 round_float(f32 num);
void make_vertex(Vtx *vtx, s32 n, s16 x, s16 y, s16 z, s16 tx, s16 ty,
                 u8 r, u8 g, u8 b, u8 a);
#endif
