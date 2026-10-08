/* Rustario64 collision oracle glue. Authored code is MIT; the marked function
 * below is copied verbatim from CC0 n64decomp/sm64 at
 * 9921382a68bb0c865e5e45eb594d9c64db59b1af (src/game/macro_special_objects.c).
 * This is a development comparison tool only; it is never linked into the game
 * runtime. See oracle/README.md for its boundary and replacement plan. */
#include <stdlib.h>
#include <string.h>
#include "sm64.h"
#include "surface_collision.h"
#include "surface_load.h"
#include "math_util.h"
#include "mario.h"
#include "game/mario_step.h"

s16 gCheckingSurfaceCollisionsForCamera;
s16 gFindFloorIncludeSurfaceIntangible;
TerrainData *gEnvironmentRegions;
s32 gEnvironmentLevels[20];
s16 gCCMEnteredSlide;
struct MarioState *gMarioState;
s32 gNumFindFloorMisses;
u32 gTimeStopState;
struct Object gMacroObjectDefaultParent;
struct Object *gMarioObject;
struct Object *gCurrentObject;
s32 gSurfaceNodesAllocated;
s32 gSurfacesAllocated;
s32 gNumStaticSurfaceNodes;
s32 gNumStaticSurfaces;
struct NumTimesCalled gNumCalls;
const BehaviorScript bhvDDDWarp[1];

/* Behavior symbols referenced by the vendored special preset table. */
const BehaviorScript bhvBetaChestBottom[1];
const BehaviorScript bhvBigBully[1];
const BehaviorScript bhvBowser[1];
const BehaviorScript bhvBowserBomb[1];
const BehaviorScript bhvButterfly[1];
const BehaviorScript bhvCastleFloorTrap[1];
const BehaviorScript bhvCourtyardBooTriplet[1];
const BehaviorScript bhvDoor[1];
const BehaviorScript bhvDoorWarp[1];
const BehaviorScript bhvJetStreamRingSpawner[1];
const BehaviorScript bhvLLLBowserPuzzle[1];
const BehaviorScript bhvLLLDrawbridgeSpawner[1];
const BehaviorScript bhvLLLFloatingWoodBridge[1];
const BehaviorScript bhvLLLMovingOctagonalMeshPlatform[1];
const BehaviorScript bhvLLLRotatingBlockWithFireBars[1];
const BehaviorScript bhvLLLRotatingHexagonalRing[1];
const BehaviorScript bhvLLLSinkingRectangularPlatform[1];
const BehaviorScript bhvLLLSinkingSquarePlatforms[1];
const BehaviorScript bhvLLLTiltingInvertedPyramid[1];
const BehaviorScript bhvLLLTumblingBridge[1];
const BehaviorScript bhvLargeBomp[1];
const BehaviorScript bhvMovingBlueCoin[1];
const BehaviorScript bhvMrI[1];
const BehaviorScript bhvRotatingCounterClockwise[1];
const BehaviorScript bhvSmallBomp[1];
const BehaviorScript bhvSmallBully[1];
const BehaviorScript bhvSnowBall[1];
const BehaviorScript bhvStaticObject[1];
const BehaviorScript bhvTowerPlatformGroup[1];
const BehaviorScript bhvTree[1];
const BehaviorScript bhvTumblingBridge[1];
const BehaviorScript bhvWFRotatingWoodenPlatform[1];
const BehaviorScript bhvWFSlidingPlatform[1];
const BehaviorScript bhvYellowCoin[1];

static struct Object sMario;
static struct Object sOtherObject;
static struct MarioState sMarioState;
static void *sPools[2];
static int sPoolCount;
static TerrainData *sData;

void *main_pool_alloc(u32 size, u32 side) {
    (void) side;
    void *p = calloc(1, size);
    if (sPoolCount < 2) {
        sPools[sPoolCount++] = p;
    }
    return p;
}

void *segmented_to_virtual(const void *addr) {
    return (void *) addr;
}

void reset_red_coins_collected(void) {
}

f32 dist_between_objects(struct Object *obj1, struct Object *obj2) {
    (void) obj1;
    (void) obj2;
    return 0.0f;
}

void obj_build_transform_from_pos_and_angle(struct Object *obj, s16 posIndex, s16 angleIndex) {
    (void) obj;
    (void) posIndex;
    (void) angleIndex;
}

void obj_apply_scale_to_matrix(struct Object *obj, Mat4 dst, Mat4 src) {
    (void) obj;
    memcpy(dst, src, sizeof(Mat4));
}

void spawn_macro_objects(s16 areaIndex, s16 *macroObjList) {
    (void) areaIndex;
    (void) macroObjList;
}

void spawn_macro_objects_hardcoded(s16 areaIndex, s16 *macroObjList) {
    (void) areaIndex;
    (void) macroObjList;
}

void set_text_array_x_y(s32 xOffset, s32 yOffset) {
    (void) xOffset;
    (void) yOffset;
}

void print_debug_top_down_mapinfo(const char *str, s32 number) {
    (void) str;
    (void) number;
}

/* Object spawning is outside the oracle's scope: only the stream walk matters. */
static void spawn_macro_abs_yrot_2params(s32 model, const BehaviorScript *behavior, s16 x, s16 y,
                                         s16 z, s16 ry, s16 params) {
    (void) model; (void) behavior; (void) x; (void) y; (void) z; (void) ry; (void) params;
}

static void spawn_macro_abs_yrot_param1(s32 model, const BehaviorScript *behavior, s16 x, s16 y,
                                        s16 z, s16 ry, s16 param) {
    (void) model; (void) behavior; (void) x; (void) y; (void) z; (void) ry; (void) param;
}

static void spawn_macro_abs_special(s32 model, const BehaviorScript *behavior, s16 x, s16 y,
                                    s16 z, s16 unkA, s16 unkB, s16 unkC) {
    (void) model; (void) behavior; (void) x; (void) y; (void) z; (void) unkA; (void) unkB;
    (void) unkC;
}

#include "model_ids.h"
#include "special_presets.inc.c"

/* ---- Verbatim from src/game/macro_special_objects.c (CC0) ---- */
void spawn_special_objects(s16 areaIndex, TerrainData **specialObjList) {
    s32 numOfSpecialObjects;
    s32 i;
    s32 offset;
    s16 x;
    s16 y;
    s16 z;
    s16 extraParams[4];
    u8 model;
    u8 type;
    u8 presetID;
    u8 defaultParam;
    const BehaviorScript *behavior;

    numOfSpecialObjects = **specialObjList;
    (*specialObjList)++;

    gMacroObjectDefaultParent.header.gfx.areaIndex = areaIndex;
    gMacroObjectDefaultParent.header.gfx.activeAreaIndex = areaIndex;

    for (i = 0; i < numOfSpecialObjects; i++) {
        presetID = (u8) **specialObjList;
        (*specialObjList)++;
        x = **specialObjList;
        (*specialObjList)++;
        y = **specialObjList;
        (*specialObjList)++;
        z = **specialObjList;
        (*specialObjList)++;

        offset = 0;
        while (TRUE) {
            if (sSpecialObjectPresets[offset].presetID == presetID) {
                break;
            }

            if (sSpecialObjectPresets[offset].presetID == special_null_end) {
            }

            offset++;
        }

        model = sSpecialObjectPresets[offset].model;
        behavior = sSpecialObjectPresets[offset].behavior;
        type = sSpecialObjectPresets[offset].type;
        defaultParam = sSpecialObjectPresets[offset].defParam;

        switch (type) {
            case SPTYPE_NO_YROT_OR_PARAMS:
                spawn_macro_abs_yrot_2params(model, behavior, x, y, z, 0, 0);
                break;
            case SPTYPE_YROT_NO_PARAMS:
                extraParams[0] = **specialObjList; // Y-rotation
                (*specialObjList)++;
                spawn_macro_abs_yrot_2params(model, behavior, x, y, z, extraParams[0], 0);
                break;
            case SPTYPE_PARAMS_AND_YROT:
                extraParams[0] = **specialObjList; // Y-rotation
                (*specialObjList)++;
                extraParams[1] = **specialObjList; // Params
                (*specialObjList)++;
                spawn_macro_abs_yrot_2params(model, behavior, x, y, z, extraParams[0], extraParams[1]);
                break;
            case SPTYPE_UNKNOWN:
                extraParams[0] =
                    **specialObjList; // Unknown, gets put into obj->oMacroUnk108 as a float
                (*specialObjList)++;
                extraParams[1] =
                    **specialObjList; // Unknown, gets put into obj->oMacroUnk10C as a float
                (*specialObjList)++;
                extraParams[2] =
                    **specialObjList; // Unknown, gets put into obj->oMacroUnk110 as a float
                (*specialObjList)++;
                spawn_macro_abs_special(model, behavior, x, y, z, extraParams[0], extraParams[1],
                                        extraParams[2]);
                break;
            case SPTYPE_DEF_PARAM_AND_YROT:
                extraParams[0] = **specialObjList; // Y-rotation
                (*specialObjList)++;
                spawn_macro_abs_yrot_param1(model, behavior, x, y, z, extraParams[0], defaultParam);
                break;
            default:
                break;
        }
    }
}
/* ---- End of verbatim section ---- */

typedef struct {
    s16 type;
    s16 force;
    s8 flags;
    s8 room;
    s16 lowerY;
    s16 upperY;
    s16 vertices[9];
    f32 normal[3];
    f32 originOffset;
} OracleSurface;

int oracle_load(const s16 *data, int words) {
    int i;
    for (i = 0; i < sPoolCount; i++) {
        free(sPools[i]);
    }
    sPoolCount = 0;
    free(sData);
    sData = malloc(sizeof(TerrainData) * (size_t) words);
    memcpy(sData, data, sizeof(TerrainData) * (size_t) words);
    memset(&sMario, 0, sizeof(sMario));
    memset(&sOtherObject, 0, sizeof(sOtherObject));
    memset(&sMarioState, 0, sizeof(sMarioState));
    gMarioObject = &sMario;
    gMarioState = &sMarioState;
    gCurrentObject = &sOtherObject;
    gTimeStopState = 0;
    gCheckingSurfaceCollisionsForCamera = 0;
    gFindFloorIncludeSurfaceIntangible = 0;
    alloc_surface_pools();
    clear_dynamic_surfaces();
    load_area_terrain(0, sData, NULL, NULL);
    return gSurfacesAllocated;
}

int oracle_node_count(void) {
    return gSurfaceNodesAllocated;
}

void oracle_get_surface(int index, OracleSurface *out) {
    struct Surface *s = &sSurfacePool[index];
    out->type = s->type;
    out->force = s->force;
    out->flags = s->flags;
    out->room = s->room;
    out->lowerY = s->lowerY;
    out->upperY = s->upperY;
    memcpy(&out->vertices[0], s->vertex1, sizeof(Vec3Terrain));
    memcpy(&out->vertices[3], s->vertex2, sizeof(Vec3Terrain));
    memcpy(&out->vertices[6], s->vertex3, sizeof(Vec3Terrain));
    out->normal[0] = s->normal.x;
    out->normal[1] = s->normal.y;
    out->normal[2] = s->normal.z;
    out->originOffset = s->originOffset;
}

static int surface_index(struct Surface *s) {
    return s == NULL ? -1 : (int) (s - sSurfacePool);
}

int oracle_cell_list(int dynamic, int cellZ, int cellX, int kind, s32 *out, int max) {
    struct SurfaceNode *node = dynamic ? gDynamicSurfacePartition[cellZ][cellX][kind].next
                                       : gStaticSurfacePartition[cellZ][cellX][kind].next;
    int n = 0;
    while (node != NULL) {
        if (n < max) {
            out[n] = surface_index(node->surface);
        }
        n++;
        node = node->next;
    }
    return n;
}

static void set_context(s16 forCamera, int passVanishWalls) {
    gCheckingSurfaceCollisionsForCamera = forCamera;
    sOtherObject.activeFlags = passVanishWalls ? ACTIVE_FLAG_MOVE_THROUGH_GRATE : 0;
    gCurrentObject = &sOtherObject;
}

f32 oracle_find_floor(f32 x, f32 y, f32 z, s16 includeIntangible, s16 forCamera, s32 *index,
                      s16 *includeAfter) {
    struct Surface *floor;
    f32 height;
    set_context(forCamera, 0);
    gFindFloorIncludeSurfaceIntangible = includeIntangible;
    height = find_floor(x, y, z, &floor);
    *index = surface_index(floor);
    *includeAfter = gFindFloorIncludeSurfaceIntangible;
    return height;
}

f32 oracle_find_ceil(f32 x, f32 y, f32 z, s16 forCamera, s32 *index) {
    struct Surface *ceil;
    f32 height;
    set_context(forCamera, 0);
    height = find_ceil(x, y, z, &ceil);
    *index = surface_index(ceil);
    return height;
}

s32 oracle_find_walls(f32 *position, f32 offsetY, f32 radius, s16 forCamera, s32 passVanishWalls,
                      s16 *numWalls, s32 *walls) {
    struct WallCollisionData data;
    s32 n, i;
    set_context(forCamera, passVanishWalls);
    memset(&data, 0, sizeof(data));
    data.x = position[0];
    data.y = position[1];
    data.z = position[2];
    data.offsetY = offsetY;
    data.radius = radius;
    n = find_wall_collisions(&data);
    position[0] = data.x;
    position[1] = data.y;
    position[2] = data.z;
    *numWalls = data.numWalls;
    for (i = 0; i < 4; i++) {
        walls[i] = i < data.numWalls ? surface_index(data.walls[i]) : -1;
    }
    return n;
}

f32 oracle_find_water_level(f32 x, f32 z) {
    return find_water_level(x, z);
}

f32 oracle_find_poison_gas_level(f32 x, f32 z) {
    return find_poison_gas_level(x, z);
}

/* ---- math_util.c oracle ---- */
Vec3f gVec3fZero = { 0.0f, 0.0f, 0.0f };

void guMtxF2L(Mat4 mf, Mtx *m) {
    (void) mf;
    (void) m;
}

extern s16 gArctanTable[0x401];

void oracle_set_trig(const f32 *sine, const s16 *arctan) {
    memcpy(gSineTable, sine, sizeof(f32) * 0x1400);
    memcpy(gArctanTable, arctan, sizeof(s16) * 0x401);
}

f32 oracle_sins(s32 x) {
    return sins(x);
}

f32 oracle_coss(s32 x) {
    return coss(x);
}

s16 oracle_atan2s(f32 y, f32 x) {
    return atan2s(y, x);
}

f32 oracle_atan2f(f32 y, f32 x) {
    return atan2f(y, x);
}

s32 oracle_approach_s32(s32 current, s32 target, s32 inc, s32 dec) {
    return approach_s32(current, target, inc, dec);
}

f32 oracle_approach_f32(f32 current, f32 target, f32 inc, f32 dec) {
    return approach_f32(current, target, inc, dec);
}

/* ---- Constant table for the Rust consistency test ---- */
static const struct { const char *name; long long value; } sConstants[] = {
    { "ACT_TWIRLING", (long long) (ACT_TWIRLING) },
    { "ACT_SHOT_FROM_CANNON", (long long) (ACT_SHOT_FROM_CANNON) },
    { "ACT_LONG_JUMP", (long long) (ACT_LONG_JUMP) },
    { "ACT_SLIDE_KICK", (long long) (ACT_SLIDE_KICK) },
    { "ACT_BBH_ENTER_SPIN", (long long) (ACT_BBH_ENTER_SPIN) },
    { "ACT_LAVA_BOOST", (long long) (ACT_LAVA_BOOST) },
    { "ACT_FALL_AFTER_STAR_GRAB", (long long) (ACT_FALL_AFTER_STAR_GRAB) },
    { "ACT_GETTING_BLOWN", (long long) (ACT_GETTING_BLOWN) },
    { "ACT_GROUND_POUND", (long long) (ACT_GROUND_POUND) },
    { "ACT_FLYING", (long long) (ACT_FLYING) },
    { "ACT_CRAWLING", (long long) (ACT_CRAWLING) },
    { "ACT_QUICKSAND_DEATH", (long long) (ACT_QUICKSAND_DEATH) },
    { "ACT_IDLE", (long long) (ACT_IDLE) },
    { "ACT_WALKING", (long long) (ACT_WALKING) },
    { "ACT_FREEFALL", (long long) (ACT_FREEFALL) },
    { "ACT_JUMP", (long long) (ACT_JUMP) },
    { "ACT_FLAG_STATIONARY", (long long) (ACT_FLAG_STATIONARY) },
    { "ACT_FLAG_MOVING", (long long) (ACT_FLAG_MOVING) },
    { "ACT_FLAG_AIR", (long long) (ACT_FLAG_AIR) },
    { "ACT_FLAG_INTANGIBLE", (long long) (ACT_FLAG_INTANGIBLE) },
    { "ACT_FLAG_INVULNERABLE", (long long) (ACT_FLAG_INVULNERABLE) },
    { "ACT_FLAG_RIDING_SHELL", (long long) (ACT_FLAG_RIDING_SHELL) },
    { "ACT_FLAG_CONTROL_JUMP_HEIGHT", (long long) (ACT_FLAG_CONTROL_JUMP_HEIGHT) },
    { "ACT_FLAG_METAL_WATER", (long long) (ACT_FLAG_METAL_WATER) },
    { "MARIO_VANISH_CAP", (long long) (MARIO_VANISH_CAP) },
    { "MARIO_METAL_CAP", (long long) (MARIO_METAL_CAP) },
    { "MARIO_WING_CAP", (long long) (MARIO_WING_CAP) },
    { "MARIO_UNKNOWN_08", (long long) (MARIO_UNKNOWN_08) },
    { "MARIO_UNKNOWN_30", (long long) (MARIO_UNKNOWN_30) },
    { "INPUT_A_DOWN", (long long) (INPUT_A_DOWN) },
    { "GROUND_STEP_LEFT_GROUND", (long long) (GROUND_STEP_LEFT_GROUND) },
    { "GROUND_STEP_NONE", (long long) (GROUND_STEP_NONE) },
    { "GROUND_STEP_HIT_WALL", (long long) (GROUND_STEP_HIT_WALL) },
    { "GROUND_STEP_HIT_WALL_STOP_QSTEPS", (long long) (GROUND_STEP_HIT_WALL_STOP_QSTEPS) },
    { "GROUND_STEP_HIT_WALL_CONTINUE_QSTEPS", (long long) (GROUND_STEP_HIT_WALL_CONTINUE_QSTEPS) },
    { "AIR_STEP_CHECK_LEDGE_GRAB", (long long) (AIR_STEP_CHECK_LEDGE_GRAB) },
    { "AIR_STEP_CHECK_HANG", (long long) (AIR_STEP_CHECK_HANG) },
    { "AIR_STEP_NONE", (long long) (AIR_STEP_NONE) },
    { "AIR_STEP_LANDED", (long long) (AIR_STEP_LANDED) },
    { "AIR_STEP_HIT_WALL", (long long) (AIR_STEP_HIT_WALL) },
    { "AIR_STEP_GRABBED_LEDGE", (long long) (AIR_STEP_GRABBED_LEDGE) },
    { "AIR_STEP_GRABBED_CEILING", (long long) (AIR_STEP_GRABBED_CEILING) },
    { "AIR_STEP_HIT_LAVA_WALL", (long long) (AIR_STEP_HIT_LAVA_WALL) },
    { "SURFACE_DEFAULT", (long long) (SURFACE_DEFAULT) },
    { "SURFACE_BURNING", (long long) (SURFACE_BURNING) },
    { "SURFACE_HANGABLE", (long long) (SURFACE_HANGABLE) },
    { "SURFACE_SLOW", (long long) (SURFACE_SLOW) },
    { "SURFACE_VERY_SLIPPERY", (long long) (SURFACE_VERY_SLIPPERY) },
    { "SURFACE_SLIPPERY", (long long) (SURFACE_SLIPPERY) },
    { "SURFACE_NOT_SLIPPERY", (long long) (SURFACE_NOT_SLIPPERY) },
    { "SURFACE_SHALLOW_QUICKSAND", (long long) (SURFACE_SHALLOW_QUICKSAND) },
    { "SURFACE_DEEP_QUICKSAND", (long long) (SURFACE_DEEP_QUICKSAND) },
    { "SURFACE_INSTANT_QUICKSAND", (long long) (SURFACE_INSTANT_QUICKSAND) },
    { "SURFACE_DEEP_MOVING_QUICKSAND", (long long) (SURFACE_DEEP_MOVING_QUICKSAND) },
    { "SURFACE_SHALLOW_MOVING_QUICKSAND", (long long) (SURFACE_SHALLOW_MOVING_QUICKSAND) },
    { "SURFACE_QUICKSAND", (long long) (SURFACE_QUICKSAND) },
    { "SURFACE_MOVING_QUICKSAND", (long long) (SURFACE_MOVING_QUICKSAND) },
    { "SURFACE_INSTANT_MOVING_QUICKSAND", (long long) (SURFACE_INSTANT_MOVING_QUICKSAND) },
    { "SURFACE_HORIZONTAL_WIND", (long long) (SURFACE_HORIZONTAL_WIND) },
    { "SURFACE_VERTICAL_WIND", (long long) (SURFACE_VERTICAL_WIND) },
    { "SURFACE_HARD", (long long) (SURFACE_HARD) },
    { "SURFACE_HARD_SLIPPERY", (long long) (SURFACE_HARD_SLIPPERY) },
    { "SURFACE_HARD_VERY_SLIPPERY", (long long) (SURFACE_HARD_VERY_SLIPPERY) },
    { "SURFACE_HARD_NOT_SLIPPERY", (long long) (SURFACE_HARD_NOT_SLIPPERY) },
    { "SURFACE_ICE", (long long) (SURFACE_ICE) },
    { "SURFACE_NOISE_DEFAULT", (long long) (SURFACE_NOISE_DEFAULT) },
    { "SURFACE_NOISE_SLIPPERY", (long long) (SURFACE_NOISE_SLIPPERY) },
    { "SURFACE_NOISE_VERY_SLIPPERY_73", (long long) (SURFACE_NOISE_VERY_SLIPPERY_73) },
    { "SURFACE_NOISE_VERY_SLIPPERY_74", (long long) (SURFACE_NOISE_VERY_SLIPPERY_74) },
    { "SURFACE_NOISE_VERY_SLIPPERY", (long long) (SURFACE_NOISE_VERY_SLIPPERY) },
    { "SURFACE_NO_CAM_COL_SLIPPERY", (long long) (SURFACE_NO_CAM_COL_SLIPPERY) },
    { "SURFACE_NO_CAM_COL_VERY_SLIPPERY", (long long) (SURFACE_NO_CAM_COL_VERY_SLIPPERY) },
    { "SURFACE_SWITCH", (long long) (SURFACE_SWITCH) },
    { "TERRAIN_MASK", (long long) (TERRAIN_MASK) },
    { "TERRAIN_SLIDE", (long long) (TERRAIN_SLIDE) },
    { "SOUND_TERRAIN_DEFAULT", (long long) (SOUND_TERRAIN_DEFAULT) },
    { "SOUND_TERRAIN_GRASS", (long long) (SOUND_TERRAIN_GRASS) },
    { "SOUND_TERRAIN_WATER", (long long) (SOUND_TERRAIN_WATER) },
    { "SOUND_TERRAIN_STONE", (long long) (SOUND_TERRAIN_STONE) },
    { "SOUND_TERRAIN_SPOOKY", (long long) (SOUND_TERRAIN_SPOOKY) },
    { "SOUND_TERRAIN_SNOW", (long long) (SOUND_TERRAIN_SNOW) },
    { "SOUND_TERRAIN_ICE", (long long) (SOUND_TERRAIN_ICE) },
    { "SOUND_TERRAIN_SAND", (long long) (SOUND_TERRAIN_SAND) },
    { "LEVEL_LLL", (long long) (LEVEL_LLL) },
    { "LEVEL_BOB", (long long) (LEVEL_BOB) },
};

int oracle_constant_count(void) {
    return (int) (sizeof(sConstants) / sizeof(sConstants[0]));
}

const char *oracle_constant(int i, long long *value) {
    *value = sConstants[i].value;
    return sConstants[i].name;
}

/* ---- Mario step oracle ---- */
u32 gGlobalTimer;
s16 gCurrLevelNum;
static struct Area sArea;
static struct MarioBodyState sBodyState;

void play_sound(s32 soundBits, f32 *pos) {
    (void) soundBits;
    (void) pos;
}

/* The ported step functions never reach these; reaching one is a test error. */
u32 set_mario_action(struct MarioState *m, u32 action, u32 actionArg) {
    (void) m; (void) action; (void) actionArg;
    abort();
}

s32 drop_and_set_mario_action(struct MarioState *m, u32 action, u32 actionArg) {
    (void) m; (void) action; (void) actionArg;
    abort();
}

void update_mario_sound_and_camera(struct MarioState *m) {
    (void) m;
    abort();
}

/* ---- Verbatim from src/game/mario.c (CC0) ---- */
s8 sTerrainSounds[7][6] = {
    // default,              hard,                 slippery,
    // very slippery,        noisy default,        noisy slippery
    { SOUND_TERRAIN_DEFAULT, SOUND_TERRAIN_STONE,  SOUND_TERRAIN_GRASS,
      SOUND_TERRAIN_GRASS,   SOUND_TERRAIN_GRASS,  SOUND_TERRAIN_DEFAULT }, // TERRAIN_GRASS
    { SOUND_TERRAIN_STONE,   SOUND_TERRAIN_STONE,  SOUND_TERRAIN_STONE,
      SOUND_TERRAIN_STONE,   SOUND_TERRAIN_GRASS,  SOUND_TERRAIN_GRASS }, // TERRAIN_STONE
    { SOUND_TERRAIN_SNOW,    SOUND_TERRAIN_ICE,    SOUND_TERRAIN_SNOW,
      SOUND_TERRAIN_ICE,     SOUND_TERRAIN_STONE,  SOUND_TERRAIN_STONE }, // TERRAIN_SNOW
    { SOUND_TERRAIN_SAND,    SOUND_TERRAIN_STONE,  SOUND_TERRAIN_SAND,
      SOUND_TERRAIN_SAND,    SOUND_TERRAIN_STONE,  SOUND_TERRAIN_STONE }, // TERRAIN_SAND
    { SOUND_TERRAIN_SPOOKY,  SOUND_TERRAIN_SPOOKY, SOUND_TERRAIN_SPOOKY,
      SOUND_TERRAIN_SPOOKY,  SOUND_TERRAIN_STONE,  SOUND_TERRAIN_STONE }, // TERRAIN_SPOOKY
    { SOUND_TERRAIN_DEFAULT, SOUND_TERRAIN_STONE,  SOUND_TERRAIN_GRASS,
      SOUND_TERRAIN_ICE,     SOUND_TERRAIN_STONE,  SOUND_TERRAIN_ICE }, // TERRAIN_WATER
    { SOUND_TERRAIN_STONE,   SOUND_TERRAIN_STONE,  SOUND_TERRAIN_STONE,
      SOUND_TERRAIN_STONE,   SOUND_TERRAIN_ICE,    SOUND_TERRAIN_ICE }, // TERRAIN_SLIDE
};
void mario_set_forward_vel(struct MarioState *m, f32 forwardVel) {
    m->forwardVel = forwardVel;

    m->slideVelX = sins(m->faceAngle[1]) * m->forwardVel;
    m->slideVelZ = coss(m->faceAngle[1]) * m->forwardVel;

    m->vel[0] = (f32) m->slideVelX;
    m->vel[2] = (f32) m->slideVelZ;
}
u32 mario_get_terrain_sound_addend(struct MarioState *m) {
    s16 floorSoundType;
    s16 terrainType = m->area->terrainType & TERRAIN_MASK;
    s32 ret = SOUND_TERRAIN_DEFAULT << 16;
    s32 floorType;

    if (m->floor != NULL) {
        floorType = m->floor->type;

        if ((gCurrLevelNum != LEVEL_LLL) && (m->floorHeight < (m->waterLevel - 10))) {
            // Water terrain sound, excluding LLL since it uses water in the volcano.
            ret = SOUND_TERRAIN_WATER << 16;
        } else if (SURFACE_IS_QUICKSAND(floorType)) {
            ret = SOUND_TERRAIN_SAND << 16;
        } else {
            switch (floorType) {
                default:
                    floorSoundType = 0;
                    break;

                case SURFACE_NOT_SLIPPERY:
                case SURFACE_HARD:
                case SURFACE_HARD_NOT_SLIPPERY:
                case SURFACE_SWITCH:
                    floorSoundType = 1;
                    break;

                case SURFACE_SLIPPERY:
                case SURFACE_HARD_SLIPPERY:
                case SURFACE_NO_CAM_COL_SLIPPERY:
                    floorSoundType = 2;
                    break;

                case SURFACE_VERY_SLIPPERY:
                case SURFACE_ICE:
                case SURFACE_HARD_VERY_SLIPPERY:
                case SURFACE_NOISE_VERY_SLIPPERY_73:
                case SURFACE_NOISE_VERY_SLIPPERY_74:
                case SURFACE_NOISE_VERY_SLIPPERY:
                case SURFACE_NO_CAM_COL_VERY_SLIPPERY:
                    floorSoundType = 3;
                    break;

                case SURFACE_NOISE_DEFAULT:
                    floorSoundType = 4;
                    break;

                case SURFACE_NOISE_SLIPPERY:
                    floorSoundType = 5;
                    break;
            }

            ret = sTerrainSounds[terrainType][floorSoundType] << 16;
        }
    }

    return ret;
}
struct Surface *resolve_and_return_wall_collisions(Vec3f pos, f32 offset, f32 radius) {
    struct WallCollisionData collisionData;
    struct Surface *wall = NULL;

    collisionData.x = pos[0];
    collisionData.y = pos[1];
    collisionData.z = pos[2];
    collisionData.radius = radius;
    collisionData.offsetY = offset;

    if (find_wall_collisions(&collisionData)) {
        wall = collisionData.walls[collisionData.numWalls - 1];
    }

    pos[0] = collisionData.x;
    pos[1] = collisionData.y;
    pos[2] = collisionData.z;

    // This only returns the most recent wall and can also return NULL
    // there are no wall collisions.
    return wall;
}
f32 vec3f_find_ceil(Vec3f pos, f32 height, struct Surface **ceil) {
    UNUSED u8 filler[4];

    return find_ceil(pos[0], height + 80.0f, pos[2], ceil);
}
/* ---- End of verbatim section ---- */

extern struct Surface gWaterSurfacePseudoFloor;
/* Defined in mario_step.c but not declared in mario_step.h. */
void apply_gravity(struct MarioState *m);
void apply_vertical_wind(struct MarioState *m);
void set_vel_from_pitch_and_yaw(struct MarioState *m);
void set_vel_from_yaw(struct MarioState *m);

typedef struct {
    u16 input;
    u32 flags;
    u32 action;
    u32 terrainSoundAddend;
    s16 faceAngle[3];
    s16 angleVel[3];
    f32 pos[3];
    f32 vel[3];
    f32 forwardVel;
    f32 slideVelX;
    f32 slideVelZ;
    s32 wall, ceil, floor; /* surface index, -1 NULL, -2 water pseudo-floor */
    f32 ceilHeight;
    f32 floorHeight;
    s16 floorAngle;
    s16 waterLevel;
    f32 peakHeight;
    f32 quicksandDepth;
    f32 gettingBlownGravity;
    s8 wingFlutter;
    f32 gfxPos[3];
    s16 gfxAngle[3];
    /* World inputs and outputs. */
    u32 globalTimer;
    u16 areaTerrainType;
    s16 levelNum;
    f32 waterPseudoOriginOffset;
    s16 includeIntangible;
} OracleMario;

static struct MarioState sState;

static struct Surface *surface_ref(s32 i) {
    return i == -1 ? NULL : i == -2 ? &gWaterSurfacePseudoFloor : &sSurfacePool[i];
}

static s32 surface_id(struct Surface *s) {
    return s == NULL ? -1 : s == &gWaterSurfacePseudoFloor ? -2 : (s32) (s - sSurfacePool);
}

static struct MarioState *mario_in(const OracleMario *o) {
    struct MarioState *m = &sState;
    memset(m, 0, sizeof(*m));
    m->input = o->input;
    m->flags = o->flags;
    m->action = o->action;
    m->terrainSoundAddend = o->terrainSoundAddend;
    memcpy(m->faceAngle, o->faceAngle, sizeof(Vec3s));
    memcpy(m->angleVel, o->angleVel, sizeof(Vec3s));
    memcpy(m->pos, o->pos, sizeof(Vec3f));
    memcpy(m->vel, o->vel, sizeof(Vec3f));
    m->forwardVel = o->forwardVel;
    m->slideVelX = o->slideVelX;
    m->slideVelZ = o->slideVelZ;
    m->wall = surface_ref(o->wall);
    m->ceil = surface_ref(o->ceil);
    m->floor = surface_ref(o->floor);
    m->ceilHeight = o->ceilHeight;
    m->floorHeight = o->floorHeight;
    m->floorAngle = o->floorAngle;
    m->waterLevel = o->waterLevel;
    m->peakHeight = o->peakHeight;
    m->quicksandDepth = o->quicksandDepth;
    m->gettingBlownGravity = o->gettingBlownGravity;
    sBodyState.wingFlutter = o->wingFlutter;
    m->marioBodyState = &sBodyState;
    memset(&sMario, 0, sizeof(sMario));
    memcpy(sMario.header.gfx.pos, o->gfxPos, sizeof(Vec3f));
    memcpy(sMario.header.gfx.angle, o->gfxAngle, sizeof(Vec3s));
    m->marioObj = &sMario;
    sArea.terrainType = o->areaTerrainType;
    m->area = &sArea;
    gGlobalTimer = o->globalTimer;
    gCurrLevelNum = o->levelNum;
    gWaterSurfacePseudoFloor.originOffset = o->waterPseudoOriginOffset;
    gFindFloorIncludeSurfaceIntangible = o->includeIntangible;
    gCheckingSurfaceCollisionsForCamera = FALSE;
    gMarioObject = &sMario;
    gCurrentObject = &sMario;
    gMarioState = m;
    return m;
}

static void mario_out(const struct MarioState *m, OracleMario *o) {
    o->input = m->input;
    o->flags = m->flags;
    o->action = m->action;
    o->terrainSoundAddend = m->terrainSoundAddend;
    memcpy(o->faceAngle, m->faceAngle, sizeof(Vec3s));
    memcpy(o->angleVel, m->angleVel, sizeof(Vec3s));
    memcpy(o->pos, m->pos, sizeof(Vec3f));
    memcpy(o->vel, m->vel, sizeof(Vec3f));
    o->forwardVel = m->forwardVel;
    o->slideVelX = m->slideVelX;
    o->slideVelZ = m->slideVelZ;
    o->wall = surface_id(m->wall);
    o->ceil = surface_id(m->ceil);
    o->floor = surface_id(m->floor);
    o->ceilHeight = m->ceilHeight;
    o->floorHeight = m->floorHeight;
    o->floorAngle = m->floorAngle;
    o->waterLevel = m->waterLevel;
    o->peakHeight = m->peakHeight;
    o->quicksandDepth = m->quicksandDepth;
    o->gettingBlownGravity = m->gettingBlownGravity;
    o->wingFlutter = sBodyState.wingFlutter;
    memcpy(o->gfxPos, sMario.header.gfx.pos, sizeof(Vec3f));
    memcpy(o->gfxAngle, sMario.header.gfx.angle, sizeof(Vec3s));
    o->waterPseudoOriginOffset = gWaterSurfacePseudoFloor.originOffset;
    o->includeIntangible = gFindFloorIncludeSurfaceIntangible;
}

/* Run one ported function: 0 ground step, 1 air step, 2 stationary ground step,
 * 3 stop and set height, 4 bonk reflection, 5 gravity, 6 vertical wind,
 * 7 vel from pitch and yaw, 8 vel from yaw, 9 set forward vel (arg as f32 bits),
 * 10 moving sand, 11 windy ground. Returns the function's result (0 if void). */
s32 oracle_mario_call(OracleMario *o, s32 which, u32 arg) {
    struct MarioState *m = mario_in(o);
    s32 r = 0;
    f32 farg;
    memcpy(&farg, &arg, sizeof(farg));
    switch (which) {
        case 0: r = perform_ground_step(m); break;
        case 1: r = perform_air_step(m, arg); break;
        case 2: r = stationary_ground_step(m); break;
        case 3: stop_and_set_height_to_floor(m); break;
        case 4: mario_bonk_reflection(m, arg); break;
        case 5: apply_gravity(m); break;
        case 6: apply_vertical_wind(m); break;
        case 7: set_vel_from_pitch_and_yaw(m); break;
        case 8: set_vel_from_yaw(m); break;
        case 9: mario_set_forward_vel(m, farg); break;
        case 10: r = mario_update_moving_sand(m); break;
        case 11: r = mario_update_windy_ground(m); break;
        default: abort();
    }
    mario_out(m, o);
    return r;
}
