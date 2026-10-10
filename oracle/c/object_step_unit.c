/* Authored transport (MIT); the excerpt and math/collision are pinned CC0.
 * The caller holds the oracle's process-wide lock. No game runtime links C. */
#include <stdlib.h>
#include <string.h>
#include "object_step_boundary.h"

static OracleObjectStepOutput *sOutput;
static Mat4 sMatrix;
static s32 sMatrixAvailable;
const BehaviorScript oracle_step_wave_behavior[1] = { 1 };
const BehaviorScript oracle_step_bubble_behavior[1] = { 2 };
int oracle_surface_index(struct Surface *surface);

static void effect(s32 kind, s32 a, s32 b) {
    s32 i = sOutput->effectCount++;
    if (i >= 3) abort();
    sOutput->effects[3 * i] = kind;
    sOutput->effects[3 * i + 1] = a;
    sOutput->effects[3 * i + 2] = b;
}

void *oracle_step_alloc(u32 size) {
    if (size != sizeof(Mat4)) abort();
    return sMatrixAvailable ? &sMatrix : NULL;
}

struct Object *oracle_step_spawn(struct Object *parent, s32 model, const BehaviorScript *behavior) {
    if (parent != gCurrentObject) abort();
    if (behavior == oracle_step_wave_behavior) effect(1, model, 1);
    else if (behavior == oracle_step_bubble_behavior) effect(1, model, 2);
    else abort();
    return NULL; /* obj_splash never inspects the returned object. */
}

void oracle_step_sound(s32 sound) { effect(2, sound, 0); }

#include "excerpts/object_step.c"

void oracle_object_step(const OracleObjectStepInput *in, OracleObjectStepOutput *out) {
    struct Object object;
    struct Object *saved = gCurrentObject;
    memset(&object, 0, sizeof(object));
    memset(out, 0, sizeof(*out));
    memcpy(object.rawData.asU32, in->raw, sizeof(in->raw));
    object.hitboxRadius = in->hitboxRadius;
    object.hitboxHeight = in->hitboxHeight;
    object.header.gfx.node.flags = in->gfxFlags;
    object.activeFlags = in->activeFlags;
    gCurrentObject = &object;
    gGlobalTimer = in->globalTimer;
    gCheckingSurfaceCollisionsForCamera = in->forCamera;
    gFindFloorIncludeSurfaceIntangible = in->includeIntangible;
    sMatrixAvailable = in->matrixAvailable;
    sOutput = out;
    out->collisionFlags = in->orientWithFloor ? object_step() : object_step_without_floor_orient();
    memcpy(out->raw, object.rawData.asU32, sizeof(out->raw));
    out->floor = oracle_surface_index(sObjFloor);
    out->includeAfter = gFindFloorIncludeSurfaceIntangible;
    out->hasMatrix = object.header.gfx.throwMatrix != NULL;
    if (out->hasMatrix) memcpy(out->matrix, sMatrix, sizeof(out->matrix));
    sOutput = NULL;
    gCurrentObject = saved;
}
