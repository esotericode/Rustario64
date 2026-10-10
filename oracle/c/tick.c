/* Rustario64 tick oracle (authored, MIT). Development comparison tool only;
 * never linked into the game runtime. See oracle/README.md.
 *
 * Runs complete frames of the vendored decomp's object system in the
 * original order (src/simulation/mario/tick.rs and src/simulation/object
 * mirror this sequence): read_controller_inputs for controller 1,
 * gAreaUpdateCounter++, the verbatim update_objects (every list in
 * sObjectListUpdateOrder, object collisions, cur_obj_update interpreting the
 * verbatim behavior scripts, unloading, update_mario_platform), the
 * verbatim update_hud_values, the authoritative part of the render pass for
 * Mario and the other objects,
 * and gGlobalTimer++. State persists across ticks and is only read back.
 *
 * The level entry is init_level's from a fresh boot: the pool as
 * clear_objects leaves it, the area's macro objects (verbatim
 * spawn_macro_objects over the macro entries whose scripts the port runs,
 * with the ROM's presets) and spawn infos, Mario from gMarioSpawnInfo
 * (verbatim spawn_objects_from_info), init_mario, the camera reset and the
 * idle action.
 *
 * With the camera linked, the area's camera is the original camera.c's (see
 * camera_unit.c): created when the area loads, reset at level entry, updated
 * by update_camera after the objects and drawn by the render pass's camera
 * nodes, and Mario's camera calls run the original functions as well as
 * being recorded. Otherwise the camera's mode and yaw are explicit inputs,
 * and no object but Mario may exist (the render pass needs the camera).
 *
 * Original functions run unmodified (vendored files and verbatim excerpts).
 * Authored here: bhv_mario_update without spawn_particle (particle objects
 * are not ported), the level-entry sequence, the render pass's object
 * traversal around the verbatim obj_is_in_view and geo_switch_anim_state
 * (object_render_unit.c), and the snapshot. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "sm64.h"
#include "behavior_data.h"
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
#include "excerpts/macro_preset_struct.inc.c"
#include "macro_preset_boundary.h"
#include "runtime.h"

/* Defined in vendored files or excerpts whose headers are not vendored. */
void adjust_analog_stick(struct Controller *controller);
void copy_mario_state_to_object(void);
void spawn_macro_objects(s16 areaIndex, s16 *macroObjList);
void spawn_objects_from_info(s32 unused, struct SpawnInfo *spawnInfo);
void clear_objects(void);
void update_objects(s32 unused);
void update_hud_values(void);
void oracle_set_rng_seed(u16 seed);
u16 oracle_rng_seed(void);
extern struct Object gObjectPool[];
extern struct ObjectNode gFreeObjectList;
extern struct GraphNode gObjParentGraphNode;
extern struct GraphNode **gLoadedGraphNodes;
extern struct MacroPreset sMacroObjectPresets[];
extern struct ObjectNode gObjectListArray[];
extern s16 gCurrAreaIndex;
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
void oracle_frame_arena_reset(void);
const f32 *oracle_frame_matrix(const void *p);
u32 oracle_object_animation_address(const void *p);
u32 oracle_object_animation_table_address(const void *p);

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
    /* gCurrCourseNum. */
    s32 courseNum;
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

static struct Area sTickArea;
static struct Camera sTickCamera;
static s8 sRootAreaIndex;
static s32 sTickReady;
static s32 sCameraLinked;

static void fail(const char *what) {
    fprintf(stderr, "tick oracle: %s\n", what);
    abort();
}

/* ---- Object content supplied by the Rust side ---- */

/* The verbatim scripts (behavior_data_unit.c) and their segmented addresses
 * in the scripts the Rust side runs (ROM or authored fixture). */
typedef struct {
    const BehaviorScript *start;
    s32 count;
} OracleScript;
s32 oracle_script_count(void);
const OracleScript *oracle_script(s32 i);
#define MAX_SCRIPTS 32
static u32 sScriptAddresses[MAX_SCRIPTS];

void oracle_tick_set_scripts(const u32 *segmented, s32 count) {
    s32 i;
    if (count != oracle_script_count() || count > MAX_SCRIPTS) {
        fail("script table size");
    }
    for (i = 0; i < count; i++) {
        sScriptAddresses[i] = segmented[i];
    }
}

/* A behavior command pointer as its segmented address; 0 for NULL. */
static u32 script_address(const BehaviorScript *p) {
    s32 i;
    if (p == NULL) {
        return 0;
    }
    for (i = 0; i < oracle_script_count(); i++) {
        const OracleScript *script = oracle_script(i);
        if (p >= script->start && p < script->start + script->count) {
            return sScriptAddresses[i] + 4 * (u32) (p - script->start);
        }
    }
    fail("a behavior pointer outside the verbatim scripts");
    return 0;
}

/* The verbatim script that starts at a segmented address. */
static const BehaviorScript *script_at(u32 segmented) {
    s32 i;
    for (i = 0; i < oracle_script_count(); i++) {
        if (sScriptAddresses[i] == segmented) {
            return oracle_script(i)->start;
        }
    }
    fail("a spawned behavior has no verbatim script");
    return NULL;
}

/* The authored render traversal and its model table (object_render_unit.c). */
typedef struct {
    s32 model;
    s32 root;
    s32 nodeCount;
    const s32 *kinds;
    const s32 *params;
    const s32 *childStart;
    const s32 *childCount;
    const s32 *children;
} OracleModel;
void oracle_render_set_models(const OracleModel *models, s32 count);
s32 oracle_render_model_of(struct GraphNode *node);
void oracle_render_load_models(void);
void oracle_render_object(struct Object *obj, s8 rootAreaIndex);
void oracle_render_begin(void);
/* Mario's object node through the verbatim traversal (mario_render_unit.c). */
s32 oracle_render_has_mario(void);
void oracle_render_load_mario(void);
void oracle_render_prime_scaler(void);
void oracle_render_mario(s8 rootAreaIndex);

void oracle_tick_set_models(const OracleModel *models, s32 count) {
    oracle_render_set_models(models, count);
}

/* The area's macro entries the port spawns (5 shorts each, as in the area's
 * list, then -1), each entry's index in the area's full list, and the ROM's
 * preset table as (segmented behavior, model, param). */
#define MAX_MACROS 512
static s16 sMacroList[MAX_MACROS * 5 + 1];
static s32 sMacroOriginal[MAX_MACROS];
static s32 sMacroCount;

void oracle_tick_set_macros(const s16 *list, const s32 *original, s32 count, const u32 *presetBehaviors,
                            const s16 *presetModels, const s16 *presetParams) {
    s32 i;
    if (count > MAX_MACROS) {
        fail("too many macro objects");
    }
    memcpy(sMacroList, list, sizeof(s16) * (size_t) count * 5);
    sMacroList[count * 5] = -1;
    memcpy(sMacroOriginal, original, sizeof(s32) * (size_t) count);
    sMacroCount = count;
    for (i = 0; i < ORACLE_MACRO_PRESET_COUNT; i++) {
        s32 known = FALSE;
        s32 j;
        for (j = 0; j < oracle_script_count(); j++) {
            known |= sScriptAddresses[j] == presetBehaviors[i];
        }
        sMacroObjectPresets[i].behavior = known ? script_at(presetBehaviors[i]) : NULL;
        sMacroObjectPresets[i].model = presetModels[i];
        sMacroObjectPresets[i].param = presetParams[i];
    }
}

/* The area's spawn infos the port spawns, in list order, with their
 * indices in the area's full list. */
typedef struct {
    s32 startPos[3];
    s32 startAngle[3];
    s32 areaIndex;
    s32 activeAreaIndex;
    u32 behaviorArg;
    u32 behaviorScript;
    s32 model;
    s32 original;
} OracleSpawnInfo;
#define MAX_SPAWN_INFOS 128
static struct SpawnInfo sAreaSpawnInfos[MAX_SPAWN_INFOS];
static s32 sSpawnOriginal[MAX_SPAWN_INFOS];
static OracleSpawnInfo sSpawnSetup[MAX_SPAWN_INFOS];
static s32 sSpawnCount;

void oracle_tick_set_spawn_infos(const OracleSpawnInfo *infos, s32 count) {
    if (count > MAX_SPAWN_INFOS) {
        fail("too many spawn infos");
    }
    memcpy(sSpawnSetup, infos, sizeof(OracleSpawnInfo) * (size_t) count);
    sSpawnCount = count;
}

/* bhvMario's native, without spawn_particle: particle objects are not
 * ported, so their flags stay in oMarioParticleFlags. */
void bhv_mario_update(void) {
    u32 particleFlags = 0;
    particleFlags = execute_mario_action(gCurrentObject);
    gCurrentObject->oMarioParticleFlags = particleFlags;
    copy_mario_state_to_object();
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
    gMarioObject = NULL;
    gCurrentObject = NULL;
    oracle_set_rng_seed((u16) s->rngSeed);
    gCurrLevelNum = (s16) s->levelNum;
    gCurrCourseNum = (s16) s->courseNum;
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
    gCurrAreaIndex = (s16) s->rootAreaIndex;
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

    /* INIT_LEVEL: the object parent node and clear_objects, over a pool as a
     * fresh boot's BSS leaves it. */
    memset(gObjectPool, 0, sizeof(struct Object) * OBJECT_POOL_CAPACITY);
    memset(&gMacroObjectDefaultParent, 0, sizeof(gMacroObjectDefaultParent));
    memset(&gObjParentGraphNode, 0, sizeof(gObjParentGraphNode));
    init_graph_node_start(NULL, (struct GraphNodeStart *) &gObjParentGraphNode);
    clear_objects();
    oracle_render_load_models();
    oracle_render_load_mario();
    oracle_render_prime_scaler();

    /* level_cmd_init_mario and level_cmd_set_mario_start_pos */
    memset(gMarioSpawnInfo, 0, sizeof(*gMarioSpawnInfo));
    for (i = 0; i < 3; i++) {
        gMarioSpawnInfo->startPos[i] = (s16) s->startPos[i];
        gMarioSpawnInfo->startAngle[i] = (s16) s->startAngle[i];
    }
    gMarioSpawnInfo->areaIndex = (s8) s->areaIndex;
    gMarioSpawnInfo->activeAreaIndex = (s8) s->activeAreaIndex;
    gMarioSpawnInfo->behaviorArg = s->behaviorArg;
    gMarioSpawnInfo->behaviorScript = (void *) bhvMario;
    gMarioSpawnInfo->model = gLoadedGraphNodes[MODEL_MARIO];

    /* level_cmd_place_object's list: each command prepends, so the setup is
     * already in list order. */
    for (i = 0; i < sSpawnCount; i++) {
        const OracleSpawnInfo *in = &sSpawnSetup[i];
        struct SpawnInfo *info = &sAreaSpawnInfos[i];
        s32 j;
        memset(info, 0, sizeof(*info));
        for (j = 0; j < 3; j++) {
            info->startPos[j] = (s16) in->startPos[j];
            info->startAngle[j] = (s16) in->startAngle[j];
        }
        info->areaIndex = (s8) in->areaIndex;
        info->activeAreaIndex = (s8) in->activeAreaIndex;
        info->behaviorArg = in->behaviorArg;
        info->behaviorScript = (void *) script_at(in->behaviorScript);
        info->model = gLoadedGraphNodes[in->model];
        info->next = i + 1 < sSpawnCount ? &sAreaSpawnInfos[i + 1] : NULL;
        sSpawnOriginal[i] = in->original;
    }

    /* lvl_init_from_save_file, then init_level's load_mario_area: the area's
     * terrain load spawns its macro objects, then its spawn infos; then
     * Mario's. */
    gMarioState = &gMarioStates[0];
    init_mario_from_save_file();
    /* init_level, with no credits entry. */
    gHudDisplay.flags = HUD_DISPLAY_DEFAULT;
    if (sMacroCount > 0) {
        spawn_macro_objects(sRootAreaIndex, sMacroList);
    }
    if (sSpawnCount > 0) {
        spawn_objects_from_info(0, &sAreaSpawnInfos[0]);
    }
    spawn_objects_from_info(0, gMarioSpawnInfo);
    if (gMarioObject == NULL) {
        fail("Mario's spawn info made no gMarioObject");
    }
    init_mario();
    if (sCameraLinked) {
        oracle_camera_reset();
    }
    set_mario_action(gMarioState, ACT_IDLE, 0);
    sTickReady = TRUE;
}

/* geo_process_node_and_siblings and geo_process_object for Mario's node:
 * the parts that change object state. */
static void tick_render_mario(void) {
    struct Object *node = gMarioObject;
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

/* The other object nodes (Mario's is first among gObjParentGraphNode's
 * children). No ported render-pass write depends on another object, so the
 * list order gives the same state as the graph's child order. */
static void tick_render_objects(void) {
    s32 list;
    oracle_render_begin();
    for (list = 0; list < NUM_OBJ_LISTS; list++) {
        struct ObjectNode *head = &gObjectListArray[list];
        struct ObjectNode *node = head->next;
        while (node != head) {
            struct Object *obj = (struct Object *) node;
            node = node->next;
            if (obj == gMarioObject) {
                continue;
            }
            if (!sCameraLinked) {
                fail("objects other than Mario need the linked camera's render pass");
            }
            oracle_render_object(obj, sRootAreaIndex);
        }
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
    /* select_gfx_pool: this frame's display-list allocations start over. */
    oracle_frame_arena_reset();
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
    update_objects(0);
    update_hud_values();
    if (sCameraLinked) {
        oracle_camera_update();
    }
    /* render_game: the camera nodes enclose the object nodes. */
    if (sCameraLinked) {
        oracle_camera_render();
    }
    if (sCameraLinked && oracle_render_has_mario()) {
        oracle_render_mario(sRootAreaIndex);
    } else {
        tick_render_mario();
    }
    tick_render_objects();
    gGlobalTimer++;
}

/* ---- Snapshot: every compared value as a named 32-bit word ---- */
#define MAX_WORDS 49152
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

/* An object reference: its pool slot, -2 for gMacroObjectDefaultParent,
 * -1 for NULL. */
static s32 object_id(struct Object *o) {
    if (o == NULL) {
        return -1;
    }
    if (o == &gMacroObjectDefaultParent) {
        return -2;
    }
    if (o < &gObjectPool[0] || o >= &gObjectPool[OBJECT_POOL_CAPACITY]) {
        fail("an object reference outside the pool");
    }
    return (s32) (o - &gObjectPool[0]);
}

/* A respawn record: the full-list index of the macro entry or spawn info,
 * -2 for gMarioSpawnInfo's argument, -1 for none. */
static s32 respawn_id(struct Object *o) {
    s32 i;
    if (o->respawnInfo == NULL) {
        return -1;
    }
    if (o->respawnInfoType == RESPAWN_INFO_TYPE_16) {
        s16 *p = (s16 *) o->respawnInfo;
        s32 at = (s32) (p - sMacroList) - 4;
        if (at < 0 || at % 5 != 0 || at / 5 >= sMacroCount) {
            fail("a macro respawn record outside the list");
        }
        return sMacroOriginal[at / 5];
    }
    if (o->respawnInfo == &gMarioSpawnInfo->behaviorArg) {
        return -2;
    }
    for (i = 0; i < sSpawnCount; i++) {
        if (o->respawnInfo == &sAreaSpawnInfos[i].behaviorArg) {
            return sSpawnOriginal[i];
        }
    }
    fail("a respawn record outside the spawn infos");
    return -1;
}

static void snapshot_mario_state(void) {
    struct MarioState *m = gMarioState;
    if (m->marioObj != gMarioObject || m->area != &sTickArea || m->controller != &gControllers[0]
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

/* Every compared field of an object, under `prefix`. */
static void put_object(const char *prefix, struct Object *o) {
    struct GraphNodeObject *gfx = &o->header.gfx;
    char buf[64];
    s32 i;
    s32 curAnim;
    s32 throwMatrix;
    s32 model = -1;
#define NAME(field) (snprintf(buf, sizeof(buf), "%s.%s", prefix, field), buf)
    /* curAnim: 0 for NULL, 1 for Mario's DMA buffer, otherwise the
     * animation's segmented address. */
    if (gfx->animInfo.curAnim == NULL) {
        curAnim = 0;
    } else if (o == gMarioObject && (void *) gfx->animInfo.curAnim == gMarioAnimsBuf.bufTarget) {
        curAnim = 1;
    } else if ((curAnim = (s32) oracle_object_animation_address(gfx->animInfo.curAnim)) == 0) {
        fail("curAnim points outside the loaded animations");
    }
    /* throwMatrix: -1 for NULL, a floor-align matrix's index, or 2 for a
     * display-list matrix (obj_orient_graph's), whose words follow. */
    if (gfx->throwMatrix == NULL) {
        throwMatrix = -1;
    } else if (gfx->throwMatrix == &sFloorAlignMatrix[0]) {
        throwMatrix = 0;
    } else if (gfx->throwMatrix == &sFloorAlignMatrix[1]) {
        throwMatrix = 1;
    } else if (oracle_frame_matrix(gfx->throwMatrix) != NULL) {
        throwMatrix = 2;
    } else {
        fail("throwMatrix points outside the floor-align matrices and the frame arena");
    }
    if (gfx->sharedChild != NULL) {
        model = oracle_render_model_of(gfx->sharedChild);
    }
    put_i(NAME("gfx.flags"), gfx->node.flags);
    put_i(NAME("gfx.areaIndex"), gfx->areaIndex);
    put_i(NAME("gfx.activeAreaIndex"), gfx->activeAreaIndex);
    put_i(NAME("gfx.sharedChild"), model);
    put_s16v(NAME("gfx.angle"), gfx->angle, 3);
    put_f32v(NAME("gfx.pos"), gfx->pos, 3);
    put_f32v(NAME("gfx.scale"), gfx->scale, 3);
    put_i(NAME("gfx.anim.animID"), gfx->animInfo.animID);
    put_i(NAME("gfx.anim.animYTrans"), gfx->animInfo.animYTrans);
    put_i(NAME("gfx.anim.curAnim"), curAnim);
    put_i(NAME("gfx.anim.animFrame"), gfx->animInfo.animFrame);
    put_i(NAME("gfx.anim.animTimer"), gfx->animInfo.animTimer);
    put_i(NAME("gfx.anim.animFrameAccelAssist"), gfx->animInfo.animFrameAccelAssist);
    put_i(NAME("gfx.anim.animAccel"), gfx->animInfo.animAccel);
    put_i(NAME("gfx.throwMatrix"), throwMatrix);
    if (throwMatrix == 2) {
        const f32 *words = oracle_frame_matrix(gfx->throwMatrix);
        for (i = 0; i < 16; i++) {
            char field[32];
            snprintf(field, sizeof(field), "gfx.throwMatrixWords[%d]", i);
            put_f(NAME(field), words[i]);
        }
    }
    put(NAME("collidedObjInteractTypes"), o->collidedObjInteractTypes);
    put_i(NAME("activeFlags"), o->activeFlags);
    put_i(NAME("numCollidedObjs"), o->numCollidedObjs);
    for (i = 0; i < 4; i++) {
        char field[32];
        snprintf(field, sizeof(field), "collidedObjs[%d]", i);
        put_i(NAME(field), i < o->numCollidedObjs ? object_id(o->collidedObjs[i]) : -1);
    }
    /* The N64's pointer fields share the raw words; on the host they are
     * ptrData. A pointer becomes the segmented address the Rust side stores:
     * an animation table's or a behavior script's. */
    for (i = 0; i < 0x50; i++) {
        char field[32];
        u32 word = o->rawData.asU32[i];
        const void *pointer = o->ptrData.asVoidPtr[i];
        if (pointer != NULL) {
            if (word != 0) {
                fail("an object field used as both a pointer and a number");
            }
            word = oracle_object_animation_table_address(pointer);
            if (word == 0) {
                word = script_address((const BehaviorScript *) pointer);
            }
        }
        snprintf(field, sizeof(field), "raw[0x%02X]", i);
        put(NAME(field), word);
    }
    put(NAME("unused1"), o->unused1);
    put(NAME("bhvStackIndex"), o->bhvStackIndex);
    for (i = 0; i < 8; i++) {
        char field[32];
        snprintf(field, sizeof(field), "bhvStack[%d]", i);
        /* Addresses become segmented; repeat counts are small integers. */
        put(NAME(field), i < (s32) o->bhvStackIndex
                             ? (o->bhvStack[i] < 0x10000 ? (u32) o->bhvStack[i]
                                                          : script_address((const BehaviorScript *) o->bhvStack[i]))
                             : 0);
    }
    put_i(NAME("bhvDelayTimer"), o->bhvDelayTimer);
    put_i(NAME("respawnInfoType"), o->respawnInfoType);
    put_i(NAME("respawnInfo"), respawn_id(o));
    put_f(NAME("hitboxRadius"), o->hitboxRadius);
    put_f(NAME("hitboxHeight"), o->hitboxHeight);
    put_f(NAME("hurtboxRadius"), o->hurtboxRadius);
    put_f(NAME("hurtboxHeight"), o->hurtboxHeight);
    put_f(NAME("hitboxDownOffset"), o->hitboxDownOffset);
    put(NAME("behavior"), script_address(o->behavior));
    put(NAME("curBhvCommand"), script_address(o->curBhvCommand));
    put_i(NAME("platform"), object_id(o->platform));
    put_i(NAME("collisionData"), o->collisionData == NULL ? -1 : 1);
    put_i(NAME("parentObj"), object_id(o->parentObj));
    put_i(NAME("prevObj"), object_id(o->prevObj));
#undef NAME
}

static void snapshot_mario_object(void) {
    put_object("obj", gMarioObject);
}

/* The pool: every listed object other than Mario by slot, each list's
 * order, and the free list's order. */
static void snapshot_objects(void) {
    char buf[64];
    s32 list, i;
    struct ObjectNode *node;
    for (list = 0; list < NUM_OBJ_LISTS; list++) {
        struct ObjectNode *head = &gObjectListArray[list];
        s32 count = 0;
        for (node = head->next; node != head; node = node->next) {
            struct Object *obj = (struct Object *) node;
            snprintf(buf, sizeof(buf), "lists[%d][%d]", list, count);
            put_i(buf, object_id(obj));
            if (obj != gMarioObject) {
                char prefix[32];
                snprintf(prefix, sizeof(prefix), "objects[%d]", object_id(obj));
                put_object(prefix, obj);
            }
            count++;
        }
        snprintf(buf, sizeof(buf), "lists[%d].count", list);
        put_i(buf, count);
    }
    i = 0;
    for (node = gFreeObjectList.next; node != NULL; node = node->next) {
        snprintf(buf, sizeof(buf), "free[%d]", i);
        put_i(buf, object_id((struct Object *) node));
        i++;
    }
    put_i("free.count", i);
    put_i("world.marioObject", object_id(gMarioObject));
    put_i("world.currentObject", object_id(gCurrentObject));
    put("world.timeStopState", gTimeStopState);
    put_i("world.rngSeed", oracle_rng_seed());
    for (i = 0; i < sMacroCount; i++) {
        snprintf(buf, sizeof(buf), "area.macro[%d].params", sMacroOriginal[i]);
        put_i(buf, (u16) sMacroList[i * 5 + 4]);
    }
    for (i = 0; i < sSpawnCount; i++) {
        snprintf(buf, sizeof(buf), "area.spawnInfo[%d].behaviorArg", sSpawnOriginal[i]);
        put(buf, sAreaSpawnInfos[i].behaviorArg);
    }
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
    put_i("hud.lives", gHudDisplay.lives);
    put_i("hud.coins", gHudDisplay.coins);
    put_i("hud.stars", gHudDisplay.stars);
    put_i("hud.wedges", gHudDisplay.wedges);
    put_i("hud.keys", gHudDisplay.keys);
    put_i("hud.flags", gHudDisplay.flags);
    put_i("hud.timer", gHudDisplay.timer);
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
    snapshot_objects();
    snapshot_body_and_camera_status();
    snapshot_world();
    if (sCameraLinked) {
        oracle_camera_snapshot(put);
    }
    *names = sNamePtrs;
    *words = sWords;
    return sCount;
}
