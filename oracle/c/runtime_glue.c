/* Rustario64 oracle runtime glue (authored, MIT). Development comparison tool
 * only; never linked into the game runtime. See oracle/README.md.
 *
 * Defines the globals and functions that the vendored CC0 decomp files and
 * excerpts reference but whose original definitions are not vendored. Calls
 * into systems outside the compared simulation are recorded as boundary events
 * in call order (sound, camera, warps, particles) or abort when reaching them
 * would need objects the oracle does not have. None of them changes compared
 * state; explicit inputs (camera mode, save data) are set by the harness. */
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
#include "game/debug.h"
#include "game/game_init.h"
#include "game/ingame_menu.h"
#include "game/interaction.h"
#include "game/level_update.h"
#include "game/macro_special_objects.h"
#include "game/main.h"
#include "game/mario.h"
#include "game/mario_misc.h"
#include "game/memory.h"
#include "game/object_helpers.h"
#include "game/object_list_processor.h"
#include "game/print.h"
#include "game/save_file.h"
#include "game/sound_init.h"
#include "interaction_boundary.h"
#include "runtime.h"

/* ---- Event log ---- */
#define ORACLE_MAX_EVENTS 512
static OracleEvent sEvents[ORACLE_MAX_EVENTS];
static s32 sEventCount;
static s32 sEventOverflow;

void oracle_event(s32 kind, s32 a, s32 b) {
    if (sEventCount < ORACLE_MAX_EVENTS) {
        sEvents[sEventCount].kind = kind;
        sEvents[sEventCount].a = a;
        sEvents[sEventCount].b = b;
        sEventCount++;
    } else {
        sEventOverflow = 1;
    }
}

void oracle_clear_events(void) {
    sEventCount = 0;
    sEventOverflow = 0;
}

/* Copies up to `max` events; returns the count, or -1 after an overflow. */
s32 oracle_events(OracleEvent *out, s32 max) {
    s32 i;
    if (sEventOverflow) {
        return -1;
    }
    for (i = 0; i < sEventCount && i < max; i++) {
        out[i] = sEvents[i];
    }
    return sEventCount;
}

static void unreachable_without_objects(const char *what) {
    fprintf(stderr, "oracle: %s needs objects, which the oracle does not model\n", what);
    abort();
}

/* ---- Globals referenced by the vendored code ---- */
struct MarioState gMarioStates[1];
struct MarioState *gMarioState = &gMarioStates[0];
struct MarioBodyState gBodyStates[2];
struct PlayerCameraState gPlayerCameraState[2];
struct Controller gControllers[3];
struct SpawnInfo gPlayerSpawnInfos[1];
struct SpawnInfo *gMarioSpawnInfo = &gPlayerSpawnInfos[0];
struct Area *gCurrentArea;
struct HudDisplay gHudDisplay;
struct DmaHandlerList gMarioAnimsBuf;
struct Object *gMarioObject;
struct Object *gCurrentObject;
struct ObjectNode *gObjectLists;
struct ObjectNode gObjectListArray[16];
struct Object gMacroObjectDefaultParent;
struct NumTimesCalled gNumCalls;
u32 gGlobalTimer;
u16 gAreaUpdateCounter;
s16 gCurrLevelNum;
s16 gCurrSaveFileNum = 1;
s16 gCameraMovementFlags;
u8 gSpecialTripleJump;
u32 gAudioRandom;
f32 gGlobalSoundSource[3];
s8 gDebugLevelSelect;
s8 gShowDebugText;
u32 gTimeStopState;
s16 gCheckingSurfaceCollisionsForCamera;
s16 gFindFloorIncludeSurfaceIntangible;
TerrainData *gEnvironmentRegions;
s32 gEnvironmentLevels[20];
s16 gCCMEnteredSlide;
s32 gNumFindFloorMisses;
s32 gSurfaceNodesAllocated;
s32 gSurfacesAllocated;
s32 gNumStaticSurfaceNodes;
s32 gNumStaticSurfaces;

/* Behavior scripts are compared by address only; objects never run them. */
const BehaviorScript bhvDDDWarp[1];
const BehaviorScript bhvGiantPole[1];
const BehaviorScript bhvJumpingBox[1];
const BehaviorScript bhvNormalCap[1];
const BehaviorScript bhvTree[1];
const BehaviorScript bhvBowser[1];
const BehaviorScript bhvKoopaShellUnderwater[1];
const BehaviorScript bhvCarrySomething3[1];
const BehaviorScript bhvCarrySomething4[1];
const BehaviorScript bhvCarrySomething5[1];
/* Referenced by the vendored special preset table. */
const BehaviorScript bhvBetaChestBottom[1];
const BehaviorScript bhvBigBully[1];
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
const BehaviorScript bhvTumblingBridge[1];
const BehaviorScript bhvWFRotatingWoodenPlatform[1];
const BehaviorScript bhvWFSlidingPlatform[1];
const BehaviorScript bhvYellowCoin[1];

/* ---- Explicit inputs set by the harness ---- */
u32 gOracleSaveFlags;
s32 gOracleCapPosValid;
Vec3s gOracleCapPos;
s32 gOracleTotalStars;

u32 save_file_get_flags(void) {
    return gOracleSaveFlags;
}

s32 save_file_get_cap_pos(Vec3s capPos) {
    if (gOracleCapPosValid) {
        vec3s_copy(capPos, gOracleCapPos);
    }
    return gOracleCapPosValid;
}

s32 save_file_get_total_star_count(s32 fileIndex, s32 minCourse, s32 maxCourse) {
    (void) fileIndex;
    (void) minCourse;
    (void) maxCourse;
    return gOracleTotalStars;
}

void save_file_set_cap_pos(s16 x, s16 y, s16 z) {
    (void) x;
    (void) y;
    (void) z;
    unreachable_without_objects("save_file_set_cap_pos (cap object)");
}

/* ---- Recorded boundaries ---- */
void play_sound(s32 soundBits, f32 *pos) {
    (void) pos;
    oracle_event(ORACLE_EVENT_SOUND, soundBits, 0);
}

void stop_sound(u32 soundBits, f32 *pos) {
    (void) pos;
    oracle_event(ORACLE_EVENT_STOP_SOUND, (s32) soundBits, 0);
}

void set_sound_moving_speed(u8 bank, u8 speed) {
    oracle_event(ORACLE_EVENT_MOVING_SPEED, bank, speed);
}

void raise_background_noise(s32 a) {
    oracle_event(ORACLE_EVENT_RAISE_NOISE, a, 0);
}

void lower_background_noise(s32 a) {
    oracle_event(ORACLE_EVENT_LOWER_NOISE, a, 0);
}

void stop_cap_music(void) {
    oracle_event(ORACLE_EVENT_STOP_CAP_MUSIC, 0, 0);
}

void fadeout_cap_music(void) {
    oracle_event(ORACLE_EVENT_FADEOUT_CAP_MUSIC, 0, 0);
}

/* The original only acts in the castle's endless-stairs room; elsewhere its
 * state never changes. Other levels are the only supported case. */
void play_infinite_stairs_music(void) {
    if (gCurrLevelNum == LEVEL_CASTLE) {
        oracle_event(ORACLE_EVENT_UNSUPPORTED, ORACLE_UNSUPPORTED_INFINITE_STAIRS, 0);
    }
}

void set_camera_mode(struct Camera *c, s16 mode, s16 frames) {
    (void) c;
    oracle_event(ORACLE_EVENT_CAMERA_MODE, mode, frames);
}

void set_camera_shake_from_hit(s16 shake) {
    oracle_event(ORACLE_EVENT_CAMERA_SHAKE, shake, 0);
}

/* Level runtime boundary: the request is recorded and no warp starts, so the
 * original return value is the "no transition" 0. */
s16 level_trigger_warp(struct MarioState *m, s32 warpOp) {
    (void) m;
    oracle_event(ORACLE_EVENT_WARP, warpOp, 0);
    return 0;
}

void load_level_init_text(u32 arg) {
    oracle_event(ORACLE_EVENT_LEVEL_INIT_TEXT, (s32) arg, 0);
}

void spawn_wind_particles(s16 pitch, s16 yaw) {
    oracle_event(ORACLE_EVENT_WIND_PARTICLES, pitch, yaw);
}

s32 mario_execute_cutscene_action(struct MarioState *m) {
    oracle_event(ORACLE_EVENT_UNSUPPORTED, ORACLE_UNSUPPORTED_CUTSCENE_GROUP, (s32) m->action);
    return FALSE;
}

s32 mario_execute_submerged_action(struct MarioState *m) {
    oracle_event(ORACLE_EVENT_UNSUPPORTED, ORACLE_UNSUPPORTED_SUBMERGED_GROUP, (s32) m->action);
    return FALSE;
}

/* ---- Unreachable without objects ---- */
struct Object *spawn_object(struct Object *parent, s32 model, const BehaviorScript *behavior) {
    (void) parent;
    (void) model;
    (void) behavior;
    unreachable_without_objects("spawn_object");
    return NULL;
}

void obj_set_held_state(struct Object *obj, const BehaviorScript *heldBehavior) {
    (void) obj;
    (void) heldBehavior;
    unreachable_without_objects("obj_set_held_state");
}

void stop_shell_music(void) {
    unreachable_without_objects("stop_shell_music (shell object)");
}

void spawn_default_star(f32 homeX, f32 homeY, f32 homeZ) {
    (void) homeX;
    (void) homeY;
    (void) homeZ;
    unreachable_without_objects("spawn_default_star");
}

u16 level_control_timer(s32 timerOp) {
    (void) timerOp;
    fprintf(stderr, "oracle: level timer (PSS) is not modelled\n");
    abort();
    return 0;
}

void *virtual_to_segmented(u32 segment, const void *addr) {
    (void) segment;
    (void) addr;
    unreachable_without_objects("virtual_to_segmented (object behavior)");
    return NULL;
}

#define UNREACHABLE_HANDLER(name)                                                   \
    u32 name(struct MarioState *m, u32 interactType, struct Object *o) {          \
        (void) m;                                                                   \
        (void) interactType;                                                        \
        (void) o;                                                                   \
        unreachable_without_objects(#name);                                         \
        return FALSE;                                                               \
    }
UNREACHABLE_HANDLER(interact_coin)
UNREACHABLE_HANDLER(interact_water_ring)
UNREACHABLE_HANDLER(interact_star_or_key)
UNREACHABLE_HANDLER(interact_bbh_entrance)
UNREACHABLE_HANDLER(interact_warp)
UNREACHABLE_HANDLER(interact_warp_door)
UNREACHABLE_HANDLER(interact_door)
UNREACHABLE_HANDLER(interact_cannon_base)
UNREACHABLE_HANDLER(interact_igloo_barrier)
UNREACHABLE_HANDLER(interact_tornado)
UNREACHABLE_HANDLER(interact_whirlpool)
UNREACHABLE_HANDLER(interact_strong_wind)
UNREACHABLE_HANDLER(interact_flame)
UNREACHABLE_HANDLER(interact_snufit_bullet)
UNREACHABLE_HANDLER(interact_clam_or_bubba)
UNREACHABLE_HANDLER(interact_bully)
UNREACHABLE_HANDLER(interact_shock)
UNREACHABLE_HANDLER(interact_mr_blizzard)
UNREACHABLE_HANDLER(interact_hit_from_below)
UNREACHABLE_HANDLER(interact_bounce_top)
UNREACHABLE_HANDLER(interact_unknown_08)
UNREACHABLE_HANDLER(interact_damage)
UNREACHABLE_HANDLER(interact_breakable)
UNREACHABLE_HANDLER(interact_koopa_shell)
UNREACHABLE_HANDLER(interact_pole)
UNREACHABLE_HANDLER(interact_hoot)
UNREACHABLE_HANDLER(interact_cap)
UNREACHABLE_HANDLER(interact_grabbable)
UNREACHABLE_HANDLER(interact_text)

/* ---- Debug text (disabled, as gShowDebugText is FALSE) ---- */
void print_text_fmt_int(s32 x, s32 y, const char *str, s32 n) {
    (void) x;
    (void) y;
    (void) str;
    (void) n;
}

void set_text_array_x_y(s32 xOffset, s32 yOffset) {
    (void) xOffset;
    (void) yOffset;
}

void print_debug_top_down_mapinfo(const char *str, s32 number) {
    (void) str;
    (void) number;
}

/* ---- Memory and object-transform stand-ins for the collision loader ---- */
static void *sPools[2];
static int sPoolCount;

void oracle_free_pools(void) {
    int i;
    for (i = 0; i < sPoolCount; i++) {
        free(sPools[i]);
    }
    sPoolCount = 0;
}

void *main_pool_alloc(u32 size, u32 side) {
    void *p;
    (void) side;
    p = calloc(1, size);
    if (sPoolCount < 2) {
        sPools[sPoolCount++] = p;
    }
    return p;
}

/* Graph nodes are initialized in place (pool NULL); nothing allocates. */
void *alloc_only_pool_alloc(struct AllocOnlyPool *pool, s32 size) {
    (void) pool;
    (void) size;
    fprintf(stderr, "oracle: alloc-only pools are not modelled\n");
    abort();
    return NULL;
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

/* Object spawning is outside the oracle's scope: only the stream walk matters. */
void spawn_macro_abs_yrot_2params(s32 model, const BehaviorScript *behavior, s16 x, s16 y, s16 z,
                                  s16 ry, s16 params) {
    (void) model; (void) behavior; (void) x; (void) y; (void) z; (void) ry; (void) params;
}

void spawn_macro_abs_yrot_param1(s32 model, const BehaviorScript *behavior, s16 x, s16 y, s16 z,
                                 s16 ry, s16 param) {
    (void) model; (void) behavior; (void) x; (void) y; (void) z; (void) ry; (void) param;
}

void spawn_macro_abs_special(s32 model, const BehaviorScript *behavior, s16 x, s16 y, s16 z,
                             s16 unkA, s16 unkB, s16 unkC) {
    (void) model; (void) behavior; (void) x; (void) y; (void) z; (void) unkA; (void) unkB;
    (void) unkC;
}

void guMtxF2L(float mf[4][4], Mtx *m) {
    (void) mf;
    (void) m;
}

/* ---- Mario animation DMA (memory.c boundary) ----
 * Host stand-in for load_patchable_table: the same contract and checks, with
 * dma_read replaced by a copy from a host-layout table the harness builds. */
s32 load_patchable_table(struct DmaHandlerList *list, s32 index) {
    s32 ret = FALSE;
    struct DmaTable *table = list->dmaTable;

    if ((u32) index < table->count) {
        u8 *addr = table->srcAddr + table->anim[index].offset;
        s32 size = table->anim[index].size;

        if (addr != list->currentAddr) {
            memcpy(list->bufTarget, addr, size);
            list->currentAddr = addr;
            ret = TRUE;
        }
    }
    return ret;
}

static u8 *sAnimBlob;
static struct DmaTable *sAnimTable;
static void *sAnimBuffer;

/* Build a host-layout copy of the decoded table: per entry a native struct
 * Animation whose values/index fields hold offsets from the header (as in the
 * ROM entry), followed by host-endian arrays. */
s32 oracle_set_mario_anims(s32 count, const s16 *headers, const u16 *const *indices,
                           const s32 *indexLens, const s16 *const *values, const s32 *valueLens) {
    size_t total = 0;
    size_t largest = 0;
    s32 i;
    free(sAnimBlob);
    free(sAnimTable);
    free(sAnimBuffer);
    for (i = 0; i < count; i++) {
        size_t size = sizeof(struct Animation) + 2 * (size_t) indexLens[i] + 2 * (size_t) valueLens[i];
        size = (size + 7) & ~(size_t) 7;
        total += size;
        if (size > largest) {
            largest = size;
        }
    }
    sAnimBlob = calloc(1, total);
    sAnimTable = calloc(1, sizeof(struct DmaTable) + sizeof(struct OffsetSizePair) * (size_t) count);
    sAnimBuffer = calloc(1, largest);
    sAnimTable->count = (u32) count;
    sAnimTable->srcAddr = sAnimBlob;
    total = 0;
    for (i = 0; i < count; i++) {
        struct Animation *anim = (struct Animation *) (sAnimBlob + total);
        size_t indexAt = sizeof(struct Animation);
        size_t valuesAt = indexAt + 2 * (size_t) indexLens[i];
        size_t size = valuesAt + 2 * (size_t) valueLens[i];
        anim->flags = headers[i * 6 + 0];
        anim->animYTransDivisor = headers[i * 6 + 1];
        anim->startFrame = headers[i * 6 + 2];
        anim->loopStart = headers[i * 6 + 3];
        anim->loopEnd = headers[i * 6 + 4];
        anim->unusedBoneCount = headers[i * 6 + 5];
        anim->values = (const s16 *) (uintptr_t) valuesAt;
        anim->index = (const u16 *) (uintptr_t) indexAt;
        anim->length = (u32) size;
        memcpy((u8 *) anim + indexAt, indices[i], 2 * (size_t) indexLens[i]);
        memcpy((u8 *) anim + valuesAt, values[i], 2 * (size_t) valueLens[i]);
        sAnimTable->anim[i].offset = (u32) total;
        sAnimTable->anim[i].size = (u32) size;
        total += (size + 7) & ~(size_t) 7;
    }
    gMarioAnimsBuf.dmaTable = sAnimTable;
    gMarioAnimsBuf.currentAddr = NULL;
    gMarioAnimsBuf.bufTarget = sAnimBuffer;
    return 0;
}

/* Which table entry the DMA buffer holds: -1 when none. */
s32 oracle_anim_dma_loaded(void) {
    s32 i;
    if (gMarioAnimsBuf.currentAddr == NULL || sAnimTable == NULL) {
        return -1;
    }
    for (i = 0; i < (s32) sAnimTable->count; i++) {
        if (sAnimTable->srcAddr + sAnimTable->anim[i].offset == gMarioAnimsBuf.currentAddr) {
            return i;
        }
    }
    abort();
    return -1;
}

void oracle_set_anim_dma_loaded(s32 entry) {
    if (entry < 0 || sAnimTable == NULL) {
        gMarioAnimsBuf.currentAddr = NULL;
    } else {
        struct Animation *anim;
        u8 *addr = sAnimTable->srcAddr + sAnimTable->anim[entry].offset;
        memcpy(gMarioAnimsBuf.bufTarget, addr, sAnimTable->anim[entry].size);
        /* Same pointer patch set_mario_animation applies after a fresh load. */
        anim = gMarioAnimsBuf.bufTarget;
        anim->values = (void *) VIRTUAL_TO_PHYSICAL((u8 *) anim + (uintptr_t) anim->values);
        anim->index = (void *) VIRTUAL_TO_PHYSICAL((u8 *) anim + (uintptr_t) anim->index);
        gMarioAnimsBuf.currentAddr = addr;
    }
}
