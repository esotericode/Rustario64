/* Authored MIT transport; the movement excerpt is verbatim pinned CC0. */
#ifndef RUSTARIO64_STANDARD_MOTION_BOUNDARY_H
#define RUSTARIO64_STANDARD_MOTION_BOUNDARY_H
#include "sm64.h"

typedef struct {
    u32 raw[0x50];
    s32 activeFlags, includeIntangible, forCamera;
    s32 operation, angle, otherAngle;
    f32 gravity, bounciness, buoyancy, drag;
    f32 marioPos[3];
} OracleStandardMotionInput;

typedef struct {
    u32 raw[0x50];
    s32 result, includeAfter;
} OracleStandardMotionOutput;
#endif
