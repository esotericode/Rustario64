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
