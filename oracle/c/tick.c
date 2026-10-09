/* Rustario64 tick oracle (authored, MIT). Development comparison tool only;
 * never linked into the game runtime. See oracle/README.md.
 *
 * Runs complete frames of the vendored decomp's Mario code with Mario's
 * object as the only object, in the original order (src/simulation/mario/
 * tick.rs mirrors this sequence): read_controller_inputs for controller 1,
 * gAreaUpdateCounter++, update_objects (clear_dynamic_surfaces,
 * clear_object_collision, cur_obj_update running bhvMario,
 * update_mario_platform), the authoritative part of the render pass, and
 * gGlobalTimer++. State persists across ticks and is only read back.
 *
 * With the camera linked, the area's camera is the original camera.c's (see
 * camera_unit.c): created when the area loads, reset at level entry, updated
 * by update_camera after the objects and drawn by the render pass's camera
 * nodes, and Mario's camera calls run the original functions as well as
 * being recorded. Otherwise the camera's mode and yaw are explicit inputs.
 *
 * Original functions run unmodified (vendored files and verbatim excerpts).
 * Authored here: the object-pool slot reset and allocation fields for Mario's
 * object, bhvMario's script steps, the render pass's object condition, and
 * the level-entry sequence. Particle objects are not spawned. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "sm64.h"
#include "audio/external.h"
#include "engine/graph_node.h"
#include "engine/math_util.h"
#include "engine/surface_collision.h"
#include "engine/surface_load.h"
#include "game/area.h"
#include "game/camera.h"
#include "game/game_init.h"
#include "game/interaction.h"
#include "game/level_update.h"
#include "game/main.h"
#include "game/mario.h"
#include "game/mario_misc.h"
#include "game/memory.h"
#include "game/object_list_processor.h"
#include "game/platform_displacement.h"
#include "game/save_file.h"
#include "runtime.h"

/* Defined in vendored files or excerpts whose headers are not vendored. */
void adjust_analog_stick(struct Controller *controller);
void clear_object_collision(struct Object *a);
void copy_mario_state_to_object(void);
void geo_set_animation_globals(struct AnimInfo *node, s32 hasAnimation);
s32 oracle_surface_index(struct Surface *s);
s32 oracle_anim_dma_loaded(void);
s32 oracle_events(OracleEvent *out, s32 max);
void oracle_interaction_state(s32 *out);
void oracle_reset_interaction_state(void);
extern struct Object *gMarioPlatform;
extern Mat4 sFloorAlignMatrix[2];
extern struct Surface gWaterSurfacePseudoFloor;
extern u32 gOracleSaveFlags;
extern s32 gOracleCapPosValid;
extern s32 gOracleTotalStars;

/* Object flags whose cur_obj_update handling needs object helpers that this
 * harness does not run; bhvMario sets none of them. */
#define UNSUPPORTED_OBJ_FLAGS                                                               \
    (OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE | OBJ_FLAG_MOVE_XZ_USING_FVEL                        \
     | OBJ_FLAG_MOVE_Y_WITH_TERMINAL_VEL | OBJ_FLAG_SET_FACE_YAW_TO_MOVE_YAW                \
     | OBJ_FLAG_SET_FACE_ANGLE_TO_MOVE_ANGLE | OBJ_FLAG_COMPUTE_DIST_TO_MARIO               \
     | OBJ_FLAG_TRANSFORM_RELATIVE_TO_PARENT | OBJ_FLAG_SET_THROW_MATRIX_FROM_TRANSFORM     \
     | OBJ_FLAG_COMPUTE_ANGLE_TO_MARIO)

typedef struct {
    s32 startPos[3];
    s32 startAngle[3];
    s32 areaIndex;
    s32 activeAreaIndex;
    u32 behaviorArg;
    s32 levelNum;
    u32 terrainType;
    s32 cameraMode;
    s32 cameraDefMode;
    u32 saveFlags;
    s32 totalStars;
    s32 rootAreaIndex;
    /* The complete camera (see camera_unit.c): linked, the area's GEO_CAMERA
     * node position and focus, gCurrActNum and gRandomSeed16. */
    s32 cameraLinked;
    f32 cameraPos[3];
    f32 cameraFocus[3];
    s32 actNum;
    u32 rngSeed;
} OracleTickSetup;

typedef struct {
    s32 mode;
    f32 pos[3];
    f32 focus[3];
    s32 actNum;
    u32 rngSeed;
} OracleCameraSetup;
struct Camera *oracle_camera_create(const OracleCameraSetup *s);
void oracle_camera_reset(void);
void oracle_camera_update(void);
void oracle_camera_render(void);
void oracle_camera_snapshot(void (*put)(const char *name, u32 value));

typedef struct {
    u32 buttons;
    s32 stick[2];
    s32 cameraYaw;
} OracleTickInput;

static struct Object sTickMario;
static struct Area sTickArea;
static struct Camera sTickCamera;
static s32 sBhvLoopEntered;
static s8 sRootAreaIndex;
static s32 sTickReady;
static s32 sCameraLinked;

static void fail(const char *what) {
    fprintf(stderr, "tick oracle: %s\n", what);
    abort();
}

/* spawn_objects_from_info for gMarioSpawnInfo, with the slot as
 * clear_objects leaves it (geo_reset_object_node) and allocate_object's
 * field initialization. */
static void tick_spawn_mario(void) {
    struct Object *obj = &sTickMario;
    struct SpawnInfo *spawnInfo = gMarioSpawnInfo;
    s32 i;

    memset(obj, 0, sizeof(*obj));
    /* geo_reset_object_node */
    init_graph_node_object(NULL, &obj->header.gfx, NULL, gVec3fZero, gVec3sZero, gVec3fOne);
    obj->header.gfx.node.flags &= ~GRAPH_RENDER_ACTIVE;

    /* spawn_objects_from_info: per-area state */
    gTimeStopState = 0;
    gMarioPlatform = NULL;

    /* allocate_object */
    obj->activeFlags = ACTIVE_FLAG_ACTIVE | ACTIVE_FLAG_UNK8;
    obj->parentObj = obj;
    obj->prevObj = NULL;
    obj->collidedObjInteractTypes = 0;
    obj->numCollidedObjs = 0;
    for (i = 0; i < 0x50; i++) {
        obj->rawData.asS32[i] = 0;
#if IS_64_BIT
        obj->ptrData.asVoidPtr[i] = NULL;
#endif
    }
    obj->unused1 = 0;
    obj->bhvStackIndex = 0;
    obj->bhvDelayTimer = 0;
    obj->hitboxRadius = 50.0f;
    obj->hitboxHeight = 100.0f;
    obj->hurtboxRadius = 0.0f;
    obj->hurtboxHeight = 0.0f;
    obj->hitboxDownOffset = 0.0f;
    obj->unused2 = 0;
    obj->platform = NULL;
    obj->collisionData = NULL;
    obj->oIntangibleTimer = -1;
    obj->oDamageOrCoinValue = 0;
    obj->oHealth = 2048;
    obj->oCollisionDistance = 1000.0f;
    obj->oDrawingDistance = gCurrLevelNum == LEVEL_TTC ? 2000.0f : 4000.0f;
    mtxf_identity(obj->transform);
    obj->respawnInfoType = RESPAWN_INFO_TYPE_NULL;
    obj->respawnInfo = NULL;
    obj->oDistanceToMario = 19000.0f;
    obj->oRoom = -1;
    obj->header.gfx.node.flags &= ~GRAPH_RENDER_INVISIBLE;
    obj->header.gfx.pos[0] = -10000.0f;
    obj->header.gfx.pos[1] = -10000.0f;
    obj->header.gfx.pos[2] = -10000.0f;
    obj->header.gfx.throwMatrix = NULL;

    /* spawn_objects_from_info: the spawned object */
    obj->oBhvParams = spawnInfo->behaviorArg;
    obj->oBhvParams2ndByte = ((spawnInfo->behaviorArg) >> 16) & 0xFF;
    obj->unused1 = 0;
    obj->respawnInfoType = RESPAWN_INFO_TYPE_32;
    obj->respawnInfo = &spawnInfo->behaviorArg;
    if (!(spawnInfo->behaviorArg & 0x01)) {
        fail("the spawn info is not Mario's");
    }
    gMarioObject = obj;
    geo_obj_init_spawninfo(&obj->header.gfx, spawnInfo);
    obj->oPosX = spawnInfo->startPos[0];
    obj->oPosY = spawnInfo->startPos[1];
    obj->oPosZ = spawnInfo->startPos[2];
    obj->oFaceAnglePitch = spawnInfo->startAngle[0];
    obj->oFaceAngleYaw = spawnInfo->startAngle[1];
    obj->oFaceAngleRoll = spawnInfo->startAngle[2];
    obj->oMoveAnglePitch = spawnInfo->startAngle[0];
    obj->oMoveAngleYaw = spawnInfo->startAngle[1];
    obj->oMoveAngleRoll = spawnInfo->startAngle[2];
}

/* A level entry without a warp destination (init_level's branch that demos
 * and the level select take), from a fresh boot: the save-file
 * initialization, Mario's spawn, init_mario, and the idle action. Requires
 * oracle_load (terrain) and oracle_set_mario_anims. */
void oracle_tick_begin(const OracleTickSetup *s) {
    s32 i;
    if (gMarioAnimsBuf.dmaTable == NULL) {
        fail("Mario's animations were not set");
    }
    oracle_clear_events();
    memset(&gMarioStates[0], 0, sizeof(gMarioStates[0]));
    memset(gBodyStates, 0, sizeof(gBodyStates));
    memset(gPlayerCameraState, 0, sizeof(gPlayerCameraState));
    memset(gControllers, 0, sizeof(gControllers));
    memset(&gHudDisplay, 0, sizeof(gHudDisplay));
    memset(&sTickArea, 0, sizeof(sTickArea));
    memset(&sTickCamera, 0, sizeof(sTickCamera));
    memset(sFloorAlignMatrix, 0, sizeof(Mat4) * 2);
    gMarioAnimsBuf.currentAddr = NULL;
    oracle_reset_interaction_state();
    gWaterSurfacePseudoFloor.originOffset = 0.0f;
    gCheckingSurfaceCollisionsForCamera = FALSE;
    gFindFloorIncludeSurfaceIntangible = FALSE;
    gGlobalTimer = 0;
    gAreaUpdateCounter = 0;
    gCameraMovementFlags = 0;
    gSpecialTripleJump = FALSE;
    gAudioRandom = 0;
    gDebugLevelSelect = FALSE;
    gShowDebugText = FALSE;
    gTimeStopState = 0;
    gMarioPlatform = NULL;
    gCurrLevelNum = (s16) s->levelNum;
    gOracleSaveFlags = s->saveFlags;
    gOracleTotalStars = s->totalStars;
    gOracleCapPosValid = FALSE;

    sTickCamera.mode = (u8) s->cameraMode;
    sTickCamera.defMode = (u8) s->cameraDefMode;
    sTickArea.index = (s8) s->rootAreaIndex;
    sTickArea.flags = 1;
    sTickArea.terrainType = (u16) s->terrainType;
    sTickArea.camera = &sTickCamera;
    gCurrentArea = &sTickArea;
    sRootAreaIndex = (s8) s->rootAreaIndex;
    sCameraLinked = s->cameraLinked != 0;
    gOracleCameraLinked = sCameraLinked;
    if (sCameraLinked) {
        /* lvl_init_from_save_file's select_mario_cam_mode, then the area's
         * load, whose GEO_CAMERA node creates the camera. */
        OracleCameraSetup camera;
        camera.mode = s->cameraMode;
        memcpy(camera.pos, s->cameraPos, sizeof(camera.pos));
        memcpy(camera.focus, s->cameraFocus, sizeof(camera.focus));
        camera.actNum = s->actNum;
        camera.rngSeed = s->rngSeed;
        sTickArea.camera = oracle_camera_create(&camera);
    }

    /* Object lists: Mario alone in OBJ_LIST_PLAYER. */
    gObjectLists = gObjectListArray;
    for (i = 0; i < NUM_OBJ_LISTS; i++) {
        gObjectListArray[i].next = &gObjectListArray[i];
        gObjectListArray[i].prev = &gObjectListArray[i];
    }

    /* level_cmd_init_mario and level_cmd_set_mario_start_pos */
    memset(gMarioSpawnInfo, 0, sizeof(*gMarioSpawnInfo));
    for (i = 0; i < 3; i++) {
        gMarioSpawnInfo->startPos[i] = (s16) s->startPos[i];
        gMarioSpawnInfo->startAngle[i] = (s16) s->startAngle[i];
    }
    gMarioSpawnInfo->areaIndex = (s8) s->areaIndex;
    gMarioSpawnInfo->activeAreaIndex = (s8) s->activeAreaIndex;
    gMarioSpawnInfo->behaviorArg = s->behaviorArg;

    /* lvl_init_from_save_file, then init_level */
    gMarioState = &gMarioStates[0];
    init_mario_from_save_file();
    tick_spawn_mario();
    gObjectListArray[OBJ_LIST_PLAYER].next = &sTickMario.header;
    gObjectListArray[OBJ_LIST_PLAYER].prev = &sTickMario.header;
    sTickMario.header.next = &gObjectListArray[OBJ_LIST_PLAYER];
    sTickMario.header.prev = &gObjectListArray[OBJ_LIST_PLAYER];
    gCurrentObject = &sTickMario;
    init_mario();
    if (sCameraLinked) {
        oracle_camera_reset();
    }
    set_mario_action(gMarioState, ACT_IDLE, 0);
    sBhvLoopEntered = FALSE;
    sTickReady = TRUE;
}

static void tick_reset_timer_on_action_change(struct Object *o) {
    if (o->oAction != o->oPrevAction) {
        (void) (o->oTimer = 0, o->oSubAction = 0, o->oPrevAction = o->oAction);
    }
}

/* cur_obj_update for Mario's object running bhvMario. */
static void tick_cur_obj_update_mario(void) {
    struct Object *o = gCurrentObject;
    s16 objFlags = o->oFlags;
    if (objFlags & UNSUPPORTED_OBJ_FLAGS) {
        fail("Mario's object flags need object helpers");
    }
    tick_reset_timer_on_action_change(o);
    if (!sBhvLoopEntered) {
        /* SET_INT(oIntangibleTimer, 0), OR_INT(oFlags, OBJ_FLAG_0100),
         * OR_INT(oUnk94, 0x0001), SET_HITBOX(37, 160), BEGIN_LOOP */
        o->oIntangibleTimer = 0;
        o->oFlags |= OBJ_FLAG_0100;
        o->oUnk94 |= 0x0001;
        o->hitboxRadius = 37;
        o->hitboxHeight = 160;
        sBhvLoopEntered = TRUE;
    }
    /* CALL_NATIVE(try_print_debug_mario_level_info): debug page 0 prints
     * nothing. CALL_NATIVE(bhv_mario_update), without spawn_particle. */
    {
        u32 particleFlags = execute_mario_action(gCurrentObject);
        gCurrentObject->oMarioParticleFlags = particleFlags;
        copy_mario_state_to_object();
    }
    /* CALL_NATIVE(try_do_mario_debug_object_spawn): debug pages only. */
    if (o->oTimer < 0x3FFFFFFF) {
        o->oTimer++;
    }
    tick_reset_timer_on_action_change(o);
    objFlags = (s16) o->oFlags;
    if (objFlags & UNSUPPORTED_OBJ_FLAGS) {
        fail("Mario's object flags need object helpers");
    }
    if (o->oRoom != -1) {
        fail("room visibility is not modelled");
    }
}

/* update_objects with Mario's object as the only object. */
static void tick_update_objects(void) {
    gTimeStopState &= ~TIME_STOP_MARIO_OPENED_DOOR;
    gCheckingSurfaceCollisionsForCamera = FALSE;
    gObjectLists = gObjectListArray;
    clear_dynamic_surfaces();
    /* update_terrain_objects: no spawners or surface objects. */
    if (gMarioPlatform != NULL) {
        fail("platform displacement needs objects");
    }
    /* detect_object_collisions: the other lists are empty. */
    clear_object_collision((struct Object *) &gObjectLists[OBJ_LIST_PLAYER]);
    /* update_objects_starting_at for the player list */
    gCurrentObject = &sTickMario;
    gCurrentObject->header.gfx.node.flags |= GRAPH_RENDER_HAS_ANIMATION;
    tick_cur_obj_update_mario();
    if ((sTickMario.activeFlags & ACTIVE_FLAG_ACTIVE) != ACTIVE_FLAG_ACTIVE) {
        fail("Mario's object was deactivated");
    }
    update_mario_platform();
}

/* geo_process_node_and_siblings and geo_process_object for Mario's node:
 * the parts that change object state. */
static void tick_render_mario(void) {
    struct Object *node = &sTickMario;
    if (node->header.gfx.node.flags & GRAPH_RENDER_ACTIVE) {
        s32 hasAnimation = (node->header.gfx.node.flags & GRAPH_RENDER_HAS_ANIMATION) != 0;
        if (node->header.gfx.areaIndex == sRootAreaIndex) {
            if (node->header.gfx.animInfo.curAnim != NULL) {
                geo_set_animation_globals(&node->header.gfx.animInfo, hasAnimation);
            }
            node->header.gfx.throwMatrix = NULL;
        }
    } else {
        node->header.gfx.throwMatrix = NULL;
    }
}

/* One frame. Events are cleared first, so they are this frame's calls. */
void oracle_tick_run(const OracleTickInput *in) {
    struct Controller *controller = &gControllers[0];
    u16 buttons = (u16) in->buttons;
    if (!sTickReady) {
        fail("oracle_tick_begin was not called");
    }
    oracle_clear_events();
    /* read_controller_inputs, connected controller */
    controller->rawStickX = (s16) in->stick[0];
    controller->rawStickY = (s16) in->stick[1];
    controller->buttonPressed = buttons & (buttons ^ controller->buttonDown);
    controller->buttonDown = buttons;
    adjust_analog_stick(controller);
    if (!sCameraLinked) {
        /* The camera computed this yaw during the previous frame. */
        sTickCamera.yaw = (s16) in->cameraYaw;
    }
    /* area_update_objects */
    gAreaUpdateCounter++;
    tick_update_objects();
    if (sCameraLinked) {
        /* update_hud_values does not touch the camera; then update_camera. */
        oracle_camera_update();
    }
    /* render_game: the camera nodes enclose the object nodes. */
    if (sCameraLinked) {
        oracle_camera_render();
    }
    tick_render_mario();
    gGlobalTimer++;
}

/* ---- Snapshot: every compared value as a named 32-bit word ---- */
#define MAX_WORDS 2048
static char sNames[MAX_WORDS][64];
static const char *sNamePtrs[MAX_WORDS];
static u32 sWords[MAX_WORDS];
static s32 sCount;

static void put(const char *name, u32 value) {
    if (sCount >= MAX_WORDS) {
        fail("snapshot overflow");
    }
    snprintf(sNames[sCount], sizeof(sNames[0]), "%s", name);
    sNamePtrs[sCount] = sNames[sCount];
    sWords[sCount] = value;
    sCount++;
}

static void put_i(const char *name, s32 value) {
    put(name, (u32) value);
}

static void put_f(const char *name, f32 value) {
    u32 bits;
    memcpy(&bits, &value, sizeof(bits));
    put(name, bits);
}

static void put_s16v(const char *name, const s16 *v, s32 n) {
    char buf[64];
    s32 i;
    for (i = 0; i < n; i++) {
        snprintf(buf, sizeof(buf), "%s[%d]", name, i);
        put_i(buf, v[i]);
    }
}

static void put_f32v(const char *name, const f32 *v, s32 n) {
    char buf[64];
    s32 i;
    for (i = 0; i < n; i++) {
        snprintf(buf, sizeof(buf), "%s[%d]", name, i);
        put_f(buf, v[i]);
    }
}

/* Objects are compared as NULL (-1); the harness has no others. */
static s32 object_id(struct Object *o) {
    if (o != NULL) {
        fail("a referenced object exists");
    }
    return -1;
}

static void snapshot_mario_state(void) {
    struct MarioState *m = gMarioState;
    if (m->marioObj != &sTickMario || m->area != &sTickArea || m->controller != &gControllers[0]
        || m->marioBodyState != &gBodyStates[0] || m->statusForCamera != &gPlayerCameraState[0]
        || m->animList != &gMarioAnimsBuf) {
        fail("MarioState pointers changed");
    }
    put_i("m.unk00", m->unk00);
    put_i("m.input", m->input);
    put("m.flags", m->flags);
    put("m.particleFlags", m->particleFlags);
    put("m.action", m->action);
    put("m.prevAction", m->prevAction);
    put("m.terrainSoundAddend", m->terrainSoundAddend);
    put_i("m.actionState", m->actionState);
    put_i("m.actionTimer", m->actionTimer);
    put("m.actionArg", m->actionArg);
    put_f("m.intendedMag", m->intendedMag);
    put_i("m.intendedYaw", m->intendedYaw);
    put_i("m.invincTimer", m->invincTimer);
    put_i("m.framesSinceA", m->framesSinceA);
    put_i("m.framesSinceB", m->framesSinceB);
    put_i("m.wallKickTimer", m->wallKickTimer);
    put_i("m.doubleJumpTimer", m->doubleJumpTimer);
    put_s16v("m.faceAngle", m->faceAngle, 3);
    put_s16v("m.angleVel", m->angleVel, 3);
    put_i("m.slideYaw", m->slideYaw);
    put_i("m.twirlYaw", m->twirlYaw);
    put_f32v("m.pos", m->pos, 3);
    put_f32v("m.vel", m->vel, 3);
    put_f("m.forwardVel", m->forwardVel);
    put_f("m.slideVelX", m->slideVelX);
    put_f("m.slideVelZ", m->slideVelZ);
    put_i("m.wall", oracle_surface_index(m->wall));
    put_i("m.ceil", oracle_surface_index(m->ceil));
    put_i("m.floor", oracle_surface_index(m->floor));
    put_f("m.ceilHeight", m->ceilHeight);
    put_f("m.floorHeight", m->floorHeight);
    put_i("m.floorAngle", m->floorAngle);
    put_i("m.waterLevel", m->waterLevel);
    put_i("m.interactObj", object_id(m->interactObj));
    put_i("m.heldObj", object_id(m->heldObj));
    put_i("m.usedObj", object_id(m->usedObj));
    put_i("m.riddenObj", object_id(m->riddenObj));
    put("m.collidedObjInteractTypes", m->collidedObjInteractTypes);
    put_i("m.numCoins", m->numCoins);
    put_i("m.numStars", m->numStars);
    put_i("m.numKeys", m->numKeys);
    put_i("m.numLives", m->numLives);
    put_i("m.health", m->health);
    put_i("m.unkB0", m->unkB0);
    put_i("m.hurtCounter", m->hurtCounter);
    put_i("m.healCounter", m->healCounter);
    put_i("m.squishTimer", m->squishTimer);
    put_i("m.fadeWarpOpacity", m->fadeWarpOpacity);
    put_i("m.capTimer", m->capTimer);
    put_i("m.prevNumStarsForDialog", m->prevNumStarsForDialog);
    put_f("m.peakHeight", m->peakHeight);
    put_f("m.quicksandDepth", m->quicksandDepth);
    put_f("m.gettingBlownGravity", m->gettingBlownGravity);
}

static void snapshot_mario_object(void) {
    struct Object *o = &sTickMario;
    struct GraphNodeObject *gfx = &o->header.gfx;
    char buf[64];
    s32 i;
    s32 curAnim;
    s32 throwMatrix;
    if (gfx->animInfo.curAnim == NULL) {
        curAnim = 0;
    } else if ((void *) gfx->animInfo.curAnim == gMarioAnimsBuf.bufTarget) {
        curAnim = 1;
    } else {
        fail("curAnim points outside Mario's animation buffer");
    }
    if (gfx->throwMatrix == NULL) {
        throwMatrix = -1;
    } else if (gfx->throwMatrix == &sFloorAlignMatrix[0]) {
        throwMatrix = 0;
    } else if (gfx->throwMatrix == &sFloorAlignMatrix[1]) {
        throwMatrix = 1;
    } else {
        fail("throwMatrix points outside the floor-align matrices");
    }
    put_i("obj.gfx.flags", gfx->node.flags);
    put_i("obj.gfx.areaIndex", gfx->areaIndex);
    put_i("obj.gfx.activeAreaIndex", gfx->activeAreaIndex);
    put_s16v("obj.gfx.angle", gfx->angle, 3);
    put_f32v("obj.gfx.pos", gfx->pos, 3);
    put_f32v("obj.gfx.scale", gfx->scale, 3);
    put_i("obj.gfx.anim.animID", gfx->animInfo.animID);
    put_i("obj.gfx.anim.animYTrans", gfx->animInfo.animYTrans);
    put_i("obj.gfx.anim.curAnim", curAnim);
    put_i("obj.gfx.anim.animFrame", gfx->animInfo.animFrame);
    put_i("obj.gfx.anim.animTimer", gfx->animInfo.animTimer);
    put_i("obj.gfx.anim.animFrameAccelAssist", gfx->animInfo.animFrameAccelAssist);
    put_i("obj.gfx.anim.animAccel", gfx->animInfo.animAccel);
    put_i("obj.gfx.throwMatrix", throwMatrix);
    put("obj.collidedObjInteractTypes", o->collidedObjInteractTypes);
    put_i("obj.activeFlags", o->activeFlags);
    put_i("obj.numCollidedObjs", o->numCollidedObjs);
    for (i = 0; i < 0x50; i++) {
        snprintf(buf, sizeof(buf), "obj.raw[0x%02X]", i);
        put(buf, o->rawData.asU32[i]);
    }
    put_f("obj.hitboxRadius", o->hitboxRadius);
    put_f("obj.hitboxHeight", o->hitboxHeight);
    put_f("obj.hurtboxRadius", o->hurtboxRadius);
    put_f("obj.hurtboxHeight", o->hurtboxHeight);
    put_f("obj.hitboxDownOffset", o->hitboxDownOffset);
    put_i("obj.platform", object_id(o->platform));
    put_i("obj.bhvLoopEntered", sBhvLoopEntered);
}

static void snapshot_body_and_camera_status(void) {
    struct MarioBodyState *b = &gBodyStates[0];
    struct PlayerCameraState *c = &gPlayerCameraState[0];
    put("body.action", b->action);
    put_i("body.capState", b->capState);
    put_i("body.eyeState", b->eyeState);
    put_i("body.handState", b->handState);
    put_i("body.wingFlutter", b->wingFlutter);
    put_i("body.modelState", b->modelState);
    put_i("body.grabPos", b->grabPos);
    put_i("body.punchState", b->punchState);
    put_s16v("body.torsoAngle", b->torsoAngle, 3);
    put_s16v("body.headAngle", b->headAngle, 3);
    put_f32v("body.heldObjLastPosition", b->heldObjLastPosition, 3);
    put("cam.action", c->action);
    put_f32v("cam.pos", c->pos, 3);
    put_s16v("cam.faceAngle", c->faceAngle, 3);
    put_s16v("cam.headRotation", c->headRotation, 3);
    put_i("cam.unused", c->unused);
    put_i("cam.cameraEvent", c->cameraEvent);
    put_i("cam.usedObj", object_id(c->usedObj));
}

static void snapshot_world(void) {
    struct Controller *controller = &gControllers[0];
    OracleEvent events[64];
    s32 interaction[5];
    char buf[64];
    s32 i, j, n;
    put("world.globalTimer", gGlobalTimer);
    put_i("world.areaUpdateCounter", gAreaUpdateCounter);
    put_i("world.cameraMovementFlags", gCameraMovementFlags);
    put_i("world.camera.mode", sTickArea.camera->mode);
    put_i("world.camera.defMode", sTickArea.camera->defMode);
    put_i("world.camera.yaw", sTickArea.camera->yaw);
    put_i("world.levelNum", gCurrLevelNum);
    put_i("world.terrainType", sTickArea.terrainType);
    put_i("world.specialTripleJump", gSpecialTripleJump);
    put_f("world.waterPseudoFloorOriginOffset", gWaterSurfacePseudoFloor.originOffset);
    put_i("world.findFloorIncludeSurfaceIntangible", gFindFloorIncludeSurfaceIntangible);
    put_i("world.checkingSurfaceCollisionsForCamera", gCheckingSurfaceCollisionsForCamera);
    put_i("world.marioPlatform", object_id(gMarioPlatform));
    oracle_interaction_state(interaction);
    put_i("world.delayInvincTimer", interaction[0]);
    put_i("world.invulnerable", interaction[1]);
    put_i("world.displayingDoorText", interaction[2]);
    put_i("world.justTeleported", interaction[3]);
    put_i("world.pssSlideStarted", interaction[4]);
    for (i = 0; i < 2; i++) {
        for (j = 0; j < 16; j++) {
            snprintf(buf, sizeof(buf), "world.floorAlignMatrix[%d][%d]", i, j);
            put_f(buf, sFloorAlignMatrix[i][j / 4][j % 4]);
        }
    }
    put_i("world.animDmaLoaded", oracle_anim_dma_loaded());
    put_i("ctl.rawStickX", controller->rawStickX);
    put_i("ctl.rawStickY", controller->rawStickY);
    put_f("ctl.stickX", controller->stickX);
    put_f("ctl.stickY", controller->stickY);
    put_f("ctl.stickMag", controller->stickMag);
    put_i("ctl.buttonDown", controller->buttonDown);
    put_i("ctl.buttonPressed", controller->buttonPressed);
    n = oracle_events(events, 64);
    if (n < 0) {
        fail("event log overflow");
    }
    put_i("events.count", n);
    for (i = 0; i < n && i < 64; i++) {
        snprintf(buf, sizeof(buf), "events[%d].kind", i);
        put_i(buf, events[i].kind);
        snprintf(buf, sizeof(buf), "events[%d].a", i);
        put_i(buf, events[i].a);
        snprintf(buf, sizeof(buf), "events[%d].b", i);
        put_i(buf, events[i].b);
    }
}

/* Fills the static snapshot; names and words stay valid until the next call.
 * Returns the number of words. */
s32 oracle_tick_snapshot(const char *const **names, const u32 **words) {
    if (!sTickReady) {
        fail("oracle_tick_begin was not called");
    }
    sCount = 0;
    snapshot_mario_state();
    snapshot_mario_object();
    snapshot_body_and_camera_status();
    snapshot_world();
    if (sCameraLinked) {
        oracle_camera_snapshot(put);
    }
    *names = sNamePtrs;
    *words = sWords;
    return sCount;
}
