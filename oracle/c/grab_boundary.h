/* Authored MIT fixture transport. No gameplay implementation here. */
#ifndef RUSTARIO64_GRAB_BOUNDARY_H
#define RUSTARIO64_GRAB_BOUNDARY_H
#include "sm64.h"
typedef struct {
    u32 action, actionArg, subtype, objectStatus, marioStatus;
    s32 invincTimer, objectYaw, marioYaw, anchorState, parentActive, holding;
    f32 objectPos[3], anchorPos[3], gfxPos[3], forwardVel, velY;
} OracleGrabFixture;
typedef struct {
    s32 operation, argument;
    f32 a, b;
    u32 status;
} OracleGrabCall;
#endif
