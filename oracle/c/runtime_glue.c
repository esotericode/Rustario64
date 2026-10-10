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
#include "dialog_ids.h"
#include "audio/external.h"
#include "engine/graph_node.h"
#include "engine/math_util.h"
#include "engine/surface_collision.h"
#include "engine/surface_load.h"
#include "game/area.h"
#include "game/camera.h"
#include "game/debug.h"
#include "game/game_init.h"
#include "game/hud.h"
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
#include "excerpts/macro_preset_struct.inc.c"
#include "macro_preset_boundary.h"
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
struct Controller gControllers[3];
/* game_init.c points the player controllers at the first two. */
struct Controller *gPlayer1Controller = &gControllers[0];
struct Controller *gPlayer2Controller = &gControllers[1];
/* No demo plays: the harness's level entry is a development entry. */
struct DemoInput *gCurrDemoInput;
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
s16 gCurrCourseNum;
s16 gCurrActNum;
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

/* Behavior scripts are compared by address only; objects never run them.
 * The scripts objects do run are verbatim (excerpts/behavior_data.c). */
const BehaviorScript bhvHauntedChair[1];
const BehaviorScript bhvMadPiano[1];
const BehaviorScript bhvMessagePanel[1];
const BehaviorScript bhvDDDWarp[1];
const BehaviorScript bhvGiantPole[1];
const BehaviorScript bhvJumpingBox[1];
const BehaviorScript bhvNormalCap[1];
const BehaviorScript bhvTree[1];
const BehaviorScript bhvBowser[1];
const BehaviorScript bhvKoopaShellUnderwater[1];
/* Water particles (obj_splash's waves and bubbles, underwater explosions'
 * bubbles) are not ported; the Rust side stops before spawning one, so the
 * native frames never run them. */
const BehaviorScript bhvObjectWaterWave[1];
const BehaviorScript bhvObjectBubble[1];
const BehaviorScript bhvBobombExplosionBubble[1];
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

/* Mario's calls into the camera. While the camera is linked (the complete
 * camera frame harness) they also run the original camera.c functions, which
 * camera_unit.c compiles under the native names; otherwise only the request
 * is recorded and the camera's state is an explicit input. */
s32 gOracleCameraLinked;

void set_camera_mode(struct Camera *c, s16 mode, s16 frames) {
    oracle_event(ORACLE_EVENT_CAMERA_MODE, mode, frames);
    if (gOracleCameraLinked) {
        oracle_camera_native_set_mode(c, mode, frames);
    }
}

void set_camera_shake_from_hit(s16 shake) {
    oracle_event(ORACLE_EVENT_CAMERA_SHAKE, shake, 0);
    if (gOracleCameraLinked) {
        oracle_camera_native_hit(shake);
    }
}

/* Objects' environmental shakes (explosions), recorded like Mario's. */
void set_environmental_camera_shake(s16 shake) {
    oracle_event(ORACLE_EVENT_ENV_CAMERA_SHAKE, shake, 0);
    if (gOracleCameraLinked) {
        oracle_camera_native_env_shake(shake);
    }
}

/* Dialog boundary: no dialog system runs, so no dialog is ever open. */
s16 get_dialog_id(void) {
    return DIALOG_NONE;
}

/* HUD boundary: the camera's HUD icon state, kept for the snapshot. */
s16 gOracleHudCameraStatus;
void set_hud_camera_status(s16 status) {
    gOracleHudCameraStatus = status;
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

/* memory.c under NO_SEGMENTED_MEMORY: host pointers are their own segmented
 * form, as segmented_to_virtual's are (behaviors compare as pointers). */
void *virtual_to_segmented(u32 segment, const void *addr) {
    (void) segment;
    return (void *) addr;
}

/* object_helpers.c's standard vertical movement: only a moving release
 * (cur_obj_move_after_thrown_or_dropped with speed) reaches it, which no
 * ported object does; the Rust port panics there too. */
void cur_obj_move_y(f32 gravity, f32 bounciness, f32 buoyancy) {
    (void) gravity;
    (void) bounciness;
    (void) buoyancy;
    unreachable_without_objects("cur_obj_move_y (thrown or placed objects)");
}

/* The display list's per-frame arena (alloc_display_list in game_init.c) as
 * obj_orient_graph uses it: Mat4-sized blocks, reset when each frame starts.
 * Exhausting the original's pool is not modelled. */
#define ORACLE_FRAME_MATRICES 1024
static Mat4 sFrameMatrices[ORACLE_FRAME_MATRICES];
static s32 sFrameMatrixCount;

void *oracle_frame_alloc_display_list(u32 size) {
    if (size != sizeof(Mat4) || sFrameMatrixCount >= ORACLE_FRAME_MATRICES) {
        fprintf(stderr, "oracle: unexpected display-list allocation\n");
        abort();
    }
    return &sFrameMatrices[sFrameMatrixCount++];
}

void oracle_frame_arena_reset(void) {
    sFrameMatrixCount = 0;
}

/* A matrix in the frame arena, or NULL. */
const f32 *oracle_frame_matrix(const void *p) {
    const Mat4 *m = (const Mat4 *) p;
    if (m >= &sFrameMatrices[0] && m < &sFrameMatrices[ORACLE_FRAME_MATRICES]) {
        return &(*m)[0][0];
    }
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
UNREACHABLE_HANDLER(interact_breakable)
UNREACHABLE_HANDLER(interact_koopa_shell)
UNREACHABLE_HANDLER(interact_pole)
UNREACHABLE_HANDLER(interact_hoot)
UNREACHABLE_HANDLER(interact_cap)
UNREACHABLE_HANDLER(interact_text)

/* ---- Object-system boundaries ---- */
/* gCurrAreaIndex (area.c): the loaded area, which the tick harness sets. */
s16 gCurrAreaIndex;

/* gCurrCreditsEntry (area.c): NULL, as no credits sequence runs. */
struct CreditsEntry *gCurrCreditsEntry;

/* gLoadedGraphNodes (area.c); the tick harness fills the loaded entries. */
struct GraphNode *D_8033A160[0x100];
struct GraphNode **gLoadedGraphNodes = D_8033A160;

/* The macro preset table's stand-in (macro_preset_boundary.h). */
struct MacroPreset sMacroObjectPresets[ORACLE_MACRO_PRESET_COUNT];

/* memory.c's object pool serves only chain chomps and wigglers. */
struct MemoryPool *mem_pool_init(u32 size, u32 side) {
    (void) size;
    (void) side;
    return NULL;
}

/* Sound sources have no simulated state. */
void stop_sounds_from_source(f32 *pos) {
    (void) pos;
}

struct Object *spawn_water_droplet(struct Object *parent, struct WaterDropletParams *params) {
    (void) parent;
    (void) params;
    unreachable_without_objects("spawn_water_droplet");
    return NULL;
}

void apply_platform_displacement(u32 isMario, struct Object *platform) {
    (void) isMario;
    (void) platform;
    unreachable_without_objects("apply_platform_displacement (platform objects)");
}

/* interact_coin's 100-coin star: recorded, as the Rust port records it. */
void bhv_spawn_star_no_level_exit(u32 sp20) {
    (void) sp20;
    oracle_event(ORACLE_EVENT_UNSUPPORTED, ORACLE_UNSUPPORTED_HUNDRED_COIN_STAR, 0);
}

/* ---- debug.c: the boot-time debug page (DEBUG_PAGE_OBJECTINFO) prints
 * nothing, debug object spawning is never enabled, and the profiler and
 * debug counters have no gameplay readers. ---- */
s64 get_current_clock(void) {
    return 0;
}

s64 get_clock_difference(s64 cycles) {
    (void) cycles;
    return 0;
}

void reset_debug_objectinfo(void) {
    gNumFindFloorMisses = 0;
}

void stub_debug_5(void) {
}

void try_print_debug_mario_object_info(void) {
}

void debug_unknown_level_select_check(void) {
}

void try_print_debug_mario_level_info(void) {
}

void try_do_mario_debug_object_spawn(void) {
}

void print_debug_top_down_objectinfo(const char *str, s32 number) {
    (void) str;
    (void) number;
}

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

/* memory.c's allocation from a pool the harness provides (create_camera's
 * Camera). Graph nodes are initialized in place with a NULL pool, which must
 * not allocate. */
void *alloc_only_pool_alloc(struct AllocOnlyPool *pool, s32 size) {
    void *addr = NULL;
    if (pool == NULL) {
        fprintf(stderr, "oracle: allocation without a pool\n");
        abort();
    }
    size = (size + 0x3) & ~0x3; /* memory.c's ALIGN4 */
    if (size > 0 && pool->usedSpace + size <= pool->totalSpace) {
        addr = pool->freePtr;
        pool->freePtr += size;
        pool->usedSpace += size;
    }
    return addr;
}

void *segmented_to_virtual(const void *addr) {
    return (void *) addr;
}

void reset_red_coins_collected(void) {
}

void obj_apply_scale_to_matrix(struct Object *obj, Mat4 dst, Mat4 src) {
    (void) obj;
    memcpy(dst, src, sizeof(Mat4));
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

/* libultra's float-to-fixed matrix conversion (authored from its documented
 * layout: the 16 integer halves of the 16.16 values first, then the 16
 * fraction halves, two per word). The level-of-detail node reads the integer
 * half of the translation's z. */
void guMtxF2L(float mf[4][4], Mtx *m) {
    s32 r, c;
    for (r = 0; r < 4; r++) {
        for (c = 0; c < 2; c++) {
            s32 first = (s32) (mf[r][2 * c] * 65536.0f);
            s32 second = (s32) (mf[r][2 * c + 1] * 65536.0f);
            m->m[r / 2][(r % 2) * 2 + c] =
                (s32) (((u32) first & 0xFFFF0000u) | (((u32) second >> 16) & 0xFFFFu));
            m->m[2 + r / 2][(r % 2) * 2 + c] =
                (s32) ((((u32) first << 16) & 0xFFFF0000u) | ((u32) second & 0xFFFFu));
        }
    }
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
