/* Authored adapters for verbatim camera.c excerpts; MIT, development only.
 * Component adapters exercise single functions; the frame adapters at the
 * end run the complete update_camera for the tick harness (c/tick.c). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
/* Preserve Mario's recorded boundaries: inside the excerpts the original
 * set_camera_mode and set_camera_shake_from_hit are renamed, and the
 * recorders in runtime_glue.c forward to them while the camera is linked. */
#define set_camera_shake_from_hit oracle_camera_native_hit
#define set_camera_mode oracle_camera_native_set_mode
#include "excerpts/camera.c"
#undef set_camera_mode
#undef set_camera_shake_from_hit
#include "engine/surface_load.h"
#include "runtime.h"

int oracle_surface_index(struct Surface *surface);
static struct Camera sComponentCamera;
static u32 bits(f32 value) { u32 word; memcpy(&word, &value, sizeof(word)); return word; }
#include "lakitu_state.inc.c"

/* ---- Paths the modelled areas never reach: aborting stubs ----
 * Their originals need objects, cutscenes or other levels' data. Reaching one
 * fails the comparison loudly instead of inventing behavior. */
static void camera_unmodelled(const char *what) {
    fprintf(stderr, "camera oracle: %s is not modelled\n", what);
    abort();
}
#define UNMODELLED_MODE(name) \
    void name(struct Camera *c) { (void) c; camera_unmodelled(#name); }
UNMODELLED_MODE(mode_behind_mario_camera)
UNMODELLED_MODE(mode_water_surface_camera)
UNMODELLED_MODE(mode_cannon_camera)
UNMODELLED_MODE(mode_8_directions_camera)
UNMODELLED_MODE(mode_outward_radial_camera)
UNMODELLED_MODE(mode_parallel_tracking_camera)
UNMODELLED_MODE(mode_slide_camera)
UNMODELLED_MODE(mode_fixed_camera)
UNMODELLED_MODE(mode_spiral_stairs_camera)
UNMODELLED_MODE(play_cutscene)
#define UNMODELLED_TRANSITION(name) \
    s32 name(struct Camera *c, Vec3f focus, Vec3f pos) { \
        (void) c; (void) focus; (void) pos; camera_unmodelled(#name); return 0; }
UNMODELLED_TRANSITION(update_outward_radial_camera)
UNMODELLED_TRANSITION(update_behind_mario_camera)
UNMODELLED_TRANSITION(unused_update_mode_5_camera)
UNMODELLED_TRANSITION(update_slide_or_0f_camera)
UNMODELLED_TRANSITION(update_in_cannon)
UNMODELLED_TRANSITION(update_parallel_tracking_camera)
UNMODELLED_TRANSITION(update_fixed_camera)
UNMODELLED_TRANSITION(update_8_directions_camera)
UNMODELLED_TRANSITION(update_spiral_stairs_camera)
s32 determine_dance_cutscene(struct Camera *c) {
    (void) c;
    camera_unmodelled("determine_dance_cutscene (star dance)");
    return 0;
}
void set_fixed_cam_axis_sa_lobby(s16 preset) {
    (void) preset;
    camera_unmodelled("set_fixed_cam_axis_sa_lobby");
}

/* Authored: the original sModeTransitions order (camera.c), with verbatim
 * functions for the modes the oracle models and the stubs above otherwise. */
CameraTransition sModeTransitions[] = {
    NULL,
    update_radial_camera,
    update_outward_radial_camera,
    update_behind_mario_camera,
    update_mario_camera,
    unused_update_mode_5_camera,
    update_c_up,
    update_mario_camera,
    nop_update_water_camera,
    update_slide_or_0f_camera,
    update_in_cannon,
    update_boss_fight_camera,
    update_parallel_tracking_camera,
    update_fixed_camera,
    update_8_directions_camera,
    update_slide_or_0f_camera,
    update_mario_camera,
    update_spiral_stairs_camera
};

/* Authored: no level's trigger table is copied, so every entry is NULL. The
 * frame adapters only run levels whose original entry is NULL as well; the
 * original entries' names come from the vendored level_defines.h. */
struct CameraTrigger *sCameraTriggers[LEVEL_COUNT + 1];
#define STUB_LEVEL(_0, _1, _2, _3, _4, _5, _6, _7, cameratable) #cameratable,
#define DEFINE_LEVEL(_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, cameratable) #cameratable,
static const char *const sOriginalCameraTriggers[] = {
    "NULL",
#include "levels/level_defines.h"
};
#undef STUB_LEVEL
#undef DEFINE_LEVEL

s32 oracle_camera_avoid_yaw(s16 yaw, s16 wallYaw) { return calc_avoid_yaw(yaw, wallYaw); }

/* Authored transport only: the vertex-based helpers do not use cached normals. */
void oracle_camera_surface(const s16 *vertices, s16 type, s32 present,
                           const f32 *from, const f32 *to, s16 range, s16 exclude,
                           const f32 *bounds, u32 *out) {
    struct Surface surf;
    memset(&surf, 0, sizeof(surf));
    memcpy(surf.vertex1, vertices, 3 * sizeof(s16));
    memcpy(surf.vertex2, vertices + 3, 3 * sizeof(s16));
    memcpy(surf.vertex3, vertices + 6, 3 * sizeof(s16));
    surf.type = type;
    out[0] = is_surf_within_bounding_box(&surf, bounds[0], bounds[1], bounds[2]);
    out[1] = is_behind_surface((f32 *)to, &surf);
    out[2] = is_range_behind_surface((f32 *)from, (f32 *)to, present ? &surf : NULL, range, exclude);
}

void oracle_camera_obstruction(const f32 *mario, const f32 *pos, s16 yaw, s16 range,
                               s16 status, s16 forCamera, s16 intangible, u32 *out) {
    vec3f_copy(sMarioCamState->pos, (f32 *)mario);
    sStatusFlags = status;
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    out[0] = rotate_camera_around_walls(&sComponentCamera, (f32 *)pos, &yaw, range);
    out[1] = (u32)(s32)yaw;
    out[2] = (u32)(s32)sStatusFlags;
    out[3] = gCheckingSurfaceCollisionsForCamera;
    out[4] = gFindFloorIncludeSurfaceIntangible;
}

/* Persistent radial globals are initialized once; later calls never load Rust
 * output. Shared Rig words use the existing transport. */
void oracle_camera_radial_reset(s32 area, const f32 *center, u16 secondRotate) {
    gCurrLevelArea = area;
    sComponentCamera.areaCenX = center[0];
    sComponentCamera.areaCenZ = center[1];
    s2ndRotateFlags = secondRotate;
    sAreaYaw = 0;
}
void oracle_camera_radial_move(const f32 *mario, f32 forwardVel, s16 currFloor, s16 prevFloor,
                               s16 forCamera, s16 intangible, u32 *out, u32 *extra) {
    vec3f_copy(sMarioCamState->pos, (f32 *)mario);
    gMarioStates[0].forwardVel = forwardVel;
    sMarioGeometry.currFloorType = currFloor;
    sMarioGeometry.prevFloorType = prevFloor;
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    radial_camera_move(gCamera);
    store_lakitu_words(out);
    extra[0] = (u16)s2ndRotateFlags;
    extra[1] = gCheckingSurfaceCollisionsForCamera;
    extra[2] = gFindFloorIncludeSurfaceIntangible;
}
void oracle_camera_radial_zoom(f32 rangeDist, s16 rangePitch, u32 *out) {
    lakitu_zoom(rangeDist, rangePitch);
    store_lakitu_words(out);
}
/* Authored input setup only; not the result of a Rust camera computation. */
void oracle_camera_radial_toggle_zoom(void) { gCameraMovementFlags ^= CAM_MOVE_ZOOMED_OUT; }
s32 oracle_camera_radial_offset(const f32 *mario, f32 forwardVel, s16 areaYaw) {
    vec3f_copy(sMarioCamState->pos, (f32 *)mario);
    gMarioStates[0].forwardVel = forwardVel;
    return offset_yaw_outward_radial(gCamera, areaYaw);
}

/* Composes compared components for the stage test. This is NOT the complete
 * mode_radial_camera: it deliberately excludes set_camera_height/pan/input. */
void oracle_camera_radial_goal_stage(const f32 *mario, u32 action, f32 floorHeight,
                                     s16 forCamera, s16 intangible, u32 *out, u32 *extra) {
    Vec3f pos;
    vec3f_copy(sMarioCamState->pos, (f32 *)mario);
    sMarioCamState->action = action;
    sMarioGeometry.currFloorHeight = floorHeight;
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    gCamera->nextYaw = update_radial_camera(gCamera, gCamera->focus, pos);
    vec3f_copy(gCamera->pos, pos);
    store_lakitu_words(out);
    extra[0] = (u32)(s32)sAreaYaw;
    extra[1] = gCheckingSurfaceCollisionsForCamera;
    extra[2] = gFindFloorIncludeSurfaceIntangible;
}

s32 oracle_camera_lakitu_word_count(void) { return LAKITU_STATE_WORDS; }
void oracle_camera_pan(const f32 *mario, u32 action, s16 faceYaw, u32 *out) {
    vec3f_copy(sMarioCamState->pos, (f32 *)mario);
    sMarioCamState->action = action;
    sMarioCamState->faceAngle[1] = faceYaw;
    pan_ahead_of_player(gCamera);
    store_lakitu_words(out);
}
void oracle_camera_lakitu_reset(const u32 *words) {
    memset(&sComponentCamera, 0, sizeof(sComponentCamera));
    memset(&gLakituState, 0, sizeof(gLakituState));
    memset(&sModeTransition, 0, sizeof(sModeTransition));
    memset(&sModeInfo, 0, sizeof(sModeInfo));
    memset(&sFOVState, 0, sizeof(sFOVState));
    sHandheldShakeTimer = 0.f;
    load_lakitu_words(words);
    gCamera = &sComponentCamera;
}
void oracle_camera_lakitu_goal(const f32 *mario, u32 action, const f32 *pos,
                              const f32 *focus, s16 nextYaw, u8 mode, u8 defMode, u8 cutscene) {
    vec3f_copy(sMarioCamState->pos, (f32 *)mario);
    sMarioCamState->action = action;
    vec3f_copy(sComponentCamera.pos, (f32 *)pos);
    vec3f_copy(sComponentCamera.focus, (f32 *)focus);
    sComponentCamera.nextYaw = nextYaw;
    sComponentCamera.mode = mode;
    sComponentCamera.defMode = defMode;
    sComponentCamera.cutscene = cutscene;
    gCamera = &sComponentCamera;
}
void oracle_camera_lakitu_control(s32 operation, s16 value, s16 frames) {
    switch (operation) {
        case 0: transition_next_state(gCamera, frames); break;
        case 1: transition_to_camera_mode(gCamera, value, frames); break;
        case 2: oracle_camera_native_hit(value); break;
        default: abort();
    }
}
void oracle_camera_lakitu_snapshot(u32 *out) { store_lakitu_words(out); }
void oracle_camera_lakitu_update(s16 forCamera, s16 intangible, u32 *out, u32 *flags) {
    if (!(gCameraMovementFlags & CAM_MOVE_PAUSE_SCREEN) && sHandheldShakeMag != 0) abort();
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    update_lakitu(gCamera);
    store_lakitu_words(out);
    flags[0] = gCheckingSurfaceCollisionsForCamera;
    flags[1] = gFindFloorIncludeSurfaceIntangible;
}
/* update_camera, not update_lakitu, writes lastFrameAction. */
void oracle_camera_lakitu_end_frame(void) { gLakituState.lastFrameAction = sMarioCamState->action; }

/* Float selector order is mirrored by CameraFloat in src/camera.rs. */
s32 oracle_camera_float(s32 which, f32 current, f32 target, f32 amount, s16 status, f32 *out) {
    s32 result = 0;
    sStatusFlags = status;
    switch (which) {
        case 0: result = approach_f32_asymptotic_bool(&current, target, amount); break;
        case 1: current = approach_f32_asymptotic(current, target, amount); break;
        case 2: result = set_or_approach_f32_asymptotic(&current, target, amount); break;
        case 3: result = camera_approach_f32_symmetric_bool(&current, target, amount); break;
        case 4: current = camera_approach_f32_symmetric(current, target, amount); break;
        default: abort();
    }
    *out = current;
    return result;
}

s32 oracle_camera_angle(s32 which, s16 current, s16 target, s16 amount, s16 status, s16 *out) {
    s32 result = 0;
    sStatusFlags = status;
    switch (which) {
        case 0: result = approach_s16_asymptotic_bool(&current, target, amount); break;
        case 1: current = approach_s16_asymptotic(current, target, amount); break;
        case 2: result = camera_approach_s16_symmetric_bool(&current, target, amount); break;
        case 3: current = camera_approach_s16_symmetric(current, target, amount); break;
        case 4: result = set_or_approach_s16_symmetric(&current, target, amount); break;
        default: abort();
    }
    *out = current;
    return result;
}

/* Inputs are copied to exercise the original functions' in-place aliases. */
void oracle_camera_vectors(s32 which, const f32 *a, const f32 *b, const f32 *amount,
                           s16 angle, s16 status, u32 *out) {
    Vec3f v, target;
    s16 pitch, yaw;
    vec3f_copy(v, (f32 *) a);
    vec3f_copy(target, (f32 *) b);
    memset(out, 0, 4 * sizeof(u32));
    sStatusFlags = status;
    switch (which) {
        case 0:
            calculate_angles(v, target, &pitch, &yaw);
            out[0] = (u32)(s32)pitch; out[1] = (u32)(s32)yaw;
            out[2] = bits(calc_abs_dist(v, target)); out[3] = bits(calc_hor_dist(v, target));
            return;
        case 1: rotate_in_xz(v, v, angle); break;
        case 2: rotate_in_yz(v, v, angle); break;
        case 3: scale_along_line(v, v, target, amount[0]); break;
        case 4: approach_vec3f_asymptotic(v, target, amount[0], amount[1], amount[2]); break;
        case 5: set_or_approach_vec3f_asymptotic(v, target, amount[0], amount[1], amount[2]); break;
        case 6: out[3] = is_pos_in_bounds(v, target, (f32 *) amount, angle); return;
        case 7: out[3] = clamp_pitch(target, v, angle, status); break;
        case 8:
            gCurrLevelArea = (s32)status;
            gCamera = &sComponentCamera;
            out[3] = find_in_bounds_yaw_wdw_bob_thi(v, target, angle); break;
        default: abort();
    }
    out[0] = bits(v[0]); out[1] = bits(v[1]); out[2] = bits(v[2]);
}

void oracle_camera_vec3s(s16 *current, const s16 *target, const s16 *divisor) {
    approach_vec3s_asymptotic(current, (s16 *)target, divisor[0], divisor[1], divisor[2]);
}

s32 oracle_camera_buttons(u16 current, u16 pressed, u16 down) {
    return find_c_buttons_pressed(current, pressed, down);
}

void oracle_camera_geometry(const f32 *pos, s16 forCamera, s16 intangible, u32 *out) {
    memset(&sMarioGeometry, 0, sizeof(sMarioGeometry));
    vec3f_copy(sMarioCamState->pos, (f32 *)pos);
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    find_mario_floor_and_ceil(&sMarioGeometry);
    out[0] = oracle_surface_index(sMarioGeometry.currFloor);
    out[1] = bits(sMarioGeometry.currFloorHeight);
    out[2] = (u32)(s32)sMarioGeometry.currFloorType;
    out[3] = oracle_surface_index(sMarioGeometry.currCeil);
    out[4] = bits(sMarioGeometry.currCeilHeight);
    out[5] = (u32)(s32)sMarioGeometry.currCeilType;
    out[6] = bits(sMarioGeometry.waterHeight);
    out[7] = gCheckingSurfaceCollisionsForCamera;
    out[8] = gFindFloorIncludeSurfaceIntangible;
}

s32 oracle_camera_collision(s32 which, f32 *pos, f32 offset, f32 radius,
                           s16 forCamera, s16 intangible, u32 *flags) {
    s32 count = 0;
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    if (which == 0) count = collide_with_walls(pos, offset, radius);
    else if (which == 1) resolve_geometry_collisions(pos, pos);
    else abort();
    flags[0] = gCheckingSurfaceCollisionsForCamera;
    flags[1] = gFindFloorIncludeSurfaceIntangible;
    return count;
}

typedef struct {
    f32 mario[3];
    u32 action;
    f32 center[2];
    s32 area;
    s16 offsetYaw, lakituDist, lakituPitch, unused;
    f32 floorHeight;
} RadialSetup;

void oracle_camera_radial(const RadialSetup *input, s16 forCamera, s16 intangible, u32 *out) {
    Vec3f focus, pos;
    s32 yaw;
    if (input->action & ACT_FLAG_ON_POLE) abort();
    vec3f_copy(sMarioCamState->pos, (f32 *)input->mario);
    sMarioCamState->action = input->action;
    sMarioGeometry.currFloorHeight = input->floorHeight;
    sComponentCamera.areaCenX = input->center[0];
    sComponentCamera.areaCenZ = input->center[1];
    gCamera = &sComponentCamera;
    gCurrLevelArea = input->area;
    sModeOffsetYaw = input->offsetYaw;
    sLakituDist = input->lakituDist;
    sLakituPitch = input->lakituPitch;
    gCheckingSurfaceCollisionsForCamera = forCamera;
    gFindFloorIncludeSurfaceIntangible = intangible;
    yaw = update_radial_camera(gCamera, focus, pos);
    for (int i = 0; i < 3; i++) { out[i] = bits(focus[i]); out[3+i] = bits(pos[i]); }
    out[6] = (u32)yaw; out[7] = (u32)(s32)sAreaYaw;
    out[8] = gCheckingSurfaceCollisionsForCamera; out[9] = gFindFloorIncludeSurfaceIntangible;
}

/* ---- Complete camera frames for the tick harness (c/tick.c) ----
 * The camera of one area, created and updated by the original functions:
 * create_camera when the area loads, select_mario_cam_mode, reset_camera at
 * level entry, update_camera after the objects each frame, and the render
 * pass's camera callbacks (geo_camera_fov, geo_camera_main). */
u16 oracle_rng_seed(void);
void oracle_set_rng_seed(u16 seed);

typedef struct {
    s32 mode;      /* the area's GEO_CAMERA node */
    f32 pos[3];
    f32 focus[3];
    s32 actNum;    /* gCurrActNum */
    u32 rngSeed;   /* gRandomSeed16 */
} OracleCameraSetup;

static u8 sFramePoolSpace[sizeof(struct Camera) + 8];
static struct AllocOnlyPool sFramePool;
static struct GraphNodeCamera sFrameGraphCamera;
static struct GraphNodePerspective sFrameGraphPerspective;
static struct Camera *sFrameCamera;

/* The static initial values a fresh boot leaves in every global the camera
 * excerpts define (zero unless camera.c initializes it). */
static void camera_boot_globals(void) {
    memset(&sMarioGeometry, 0, sizeof(sMarioGeometry));
    gCamera = NULL;
    sStatusFlags = 0;
    sAreaYaw = 0;
    sLakituDist = 0;
    sLakituPitch = 0;
    sModeOffsetYaw = 0;
    gCurrLevelArea = 0;
    sMarioCamState = &gPlayerCameraState[0];
    memset(&gLakituState, 0, sizeof(gLakituState));
    memset(&sFOVState, 0, sizeof(sFOVState));
    memset(&sModeTransition, 0, sizeof(sModeTransition));
    memset(&sModeInfo, 0, sizeof(sModeInfo));
    memset(sOldPosition, 0, sizeof(Vec3f));
    memset(sOldFocus, 0, sizeof(Vec3f));
    memset(sPlayer2FocusOffset, 0, sizeof(Vec3f));
    sYawSpeed = 0x400;
    sCUpCameraPitch = 0;
    sAreaYawChange = 0;
    sPanDistance = 0.f;
    sCannonYOffset = 0.f;
    unusedSplinePitch = 0;
    unusedSplineYaw = 0;
    memset(sHandheldShakeSpline, 0, sizeof(sHandheldShakeSpline));
    sHandheldShakeMag = 0;
    sHandheldShakeTimer = 0.f;
    sHandheldShakeInc = 0.f;
    sHandheldShakePitch = 0;
    sHandheldShakeYaw = 0;
    sHandheldShakeRoll = 0;
    s2ndRotateFlags = 0;
    memset(gPlayerCameraState, 0, sizeof(gPlayerCameraState));
    sCreditsPlayer2Pitch = 0;
    sCreditsPlayer2Yaw = 0;
    sFramesPaused = 0;
    unusedFreeRoamWallYaw = 0;
    sAvoidYawVel = 0;
    sCameraYawAfterDoorCutscene = 0;
    memset(sCurCreditsSplinePos, 0, sizeof(sCurCreditsSplinePos));
    memset(sCurCreditsSplineFocus, 0, sizeof(sCurCreditsSplineFocus));
    sCutsceneSplineSegmentProgress = 0.f;
    sCutsceneSplineSegment = 0;
    unused8033B6E8 = 0;
    gCutsceneObjSpawn = 0;
    gObjCutsceneDone = 0;
    unused8033B30C = 0;
    unused8033B310 = 0;
    sSelectionFlags = 0;
    gCameraMovementFlags = 0;
    unused8033B316 = 0;
    unused8033B31A = 0;
    sCameraSoundFlags = 0;
    sCButtonsPressed = 0;
    sCutsceneShot = 0;
    gCutsceneTimer = 0;
    sZoomAmount = 0.f;
    sCSideButtonYaw = 0;
    sBehindMarioSoundTimer = 0;
    sZeroZoomDist = 0.f;
    sSpiralStairsYawOffset = 0;
    s8DirModeBaseYaw = 0;
    s8DirModeYawOffset = 0;
    memset(sCutsceneVars, 0, sizeof(sCutsceneVars));
    memset(sCastleEntranceOffset, 0, sizeof(Vec3f));
    memset(&sCameraStoreCUp, 0, sizeof(sCameraStoreCUp));
    gCutsceneFocus = NULL;
    unused8032CFC8 = 0;
    unused8032CFCC = 0;
    gSecondCameraFocus = NULL;
    gPrevLevel = 0;
    gCameraZoomDist = 800.0f;
    sObjectCutscene = 0;
    gRecentCutscene = 0;
    sFramesSinceCutsceneEnded = 0;
    sLuigiCamState = &gPlayerCameraState[1];
    sFixedModeBasePosition[0] = 646.0f;
    sFixedModeBasePosition[1] = 143.0f;
    sFixedModeBasePosition[2] = -1513.0f;
    gOracleHudCameraStatus = 0;
}

/* A fresh boot's camera globals, select_mario_cam_mode
 * (lvl_init_from_save_file), then create_camera for the area's GEO_CAMERA
 * node when the area loads. Returns the area's camera. The caller must only
 * use levels whose original trigger table is NULL. */
struct Camera *oracle_camera_create(const OracleCameraSetup *s) {
    if (gCurrLevelNum <= 0 || gCurrLevelNum > LEVEL_MAX
        || strcmp(sOriginalCameraTriggers[gCurrLevelNum], "_") != 0) {
        camera_unmodelled("this level's camera triggers");
    }
    camera_boot_globals();
    oracle_set_rng_seed((u16) s->rngSeed);
    gCurrActNum = (s16) s->actNum;
    select_mario_cam_mode();
    memset(sFramePoolSpace, 0, sizeof(sFramePoolSpace));
    sFramePool.totalSpace = sizeof(sFramePoolSpace);
    sFramePool.usedSpace = 0;
    sFramePool.startPtr = sFramePoolSpace;
    sFramePool.freePtr = sFramePoolSpace;
    memset(&sFrameGraphCamera, 0, sizeof(sFrameGraphCamera));
    memset(&sFrameGraphPerspective, 0, sizeof(sFrameGraphPerspective));
    sFrameGraphCamera.config.mode = s->mode;
    vec3f_copy(sFrameGraphCamera.pos, (f32 *) s->pos);
    vec3f_copy(sFrameGraphCamera.focus, (f32 *) s->focus);
    geo_camera_main(GEO_CONTEXT_CREATE, &sFrameGraphCamera.fnNode.node, &sFramePool);
    sFrameCamera = sFrameGraphCamera.config.camera;
    if (sFrameCamera == NULL) {
        camera_unmodelled("a camera allocation failure");
    }
    return sFrameCamera;
}

/* init_level's reset_camera, after init_mario. */
void oracle_camera_reset(void) {
    reset_camera(sFrameCamera);
}

/* update_level's update_camera, after the objects. */
void oracle_camera_update(void) {
    update_camera(sFrameCamera);
}

/* The render pass's camera nodes: the area's perspective node (FOV
 * callback), then its camera node (copies Lakitu into the graph node). */
void oracle_camera_render(void) {
    geo_camera_fov(GEO_CONTEXT_RENDER, &sFrameGraphPerspective.fnNode.node, NULL);
    geo_camera_main(GEO_CONTEXT_RENDER, &sFrameGraphCamera.fnNode.node, NULL);
}

typedef void (*OraclePut)(const char *name, u32 value);
static OraclePut sPut;
static char sNameBuf[64];
static void put_word(const char *name, u32 value) { sPut(name, value); }
static void put_s(const char *name, s32 value) { sPut(name, (u32) value); }
static void put_fl(const char *name, f32 value) { sPut(name, bits(value)); }
static void put_vec(const char *name, const f32 *v) {
    s32 i;
    for (i = 0; i < 3; i++) {
        snprintf(sNameBuf, sizeof(sNameBuf), "%s[%d]", name, i);
        sPut(sNameBuf, bits(v[i]));
    }
}
static void put_vecs(const char *name, const s16 *v) {
    s32 i;
    for (i = 0; i < 3; i++) {
        snprintf(sNameBuf, sizeof(sNameBuf), "%s[%d]", name, i);
        sPut(sNameBuf, (u32) (s32) v[i]);
    }
}
static void put_transition_point(const char *name, const struct LinearTransitionPoint *p) {
    char buf[64];
    snprintf(buf, sizeof(buf), "%s.focus", name);
    put_vec(buf, p->focus);
    snprintf(buf, sizeof(buf), "%s.pos", name);
    put_vec(buf, p->pos);
    snprintf(buf, sizeof(buf), "%s.dist", name);
    put_fl(buf, p->dist);
    snprintf(buf, sizeof(buf), "%s.pitch", name);
    put_s(buf, p->pitch);
    snprintf(buf, sizeof(buf), "%s.yaw", name);
    put_s(buf, p->yaw);
}

/* Every modelled camera value as a named word; src/camera_trace.rs produces
 * the same names from the Rust camera. */
void oracle_camera_snapshot(OraclePut put) {
    struct Camera *c = sFrameCamera;
    s32 i;
    sPut = put;
    put_s("camera.c.mode", c->mode);
    put_s("camera.c.defMode", c->defMode);
    put_s("camera.c.yaw", c->yaw);
    put_vec("camera.c.focus", c->focus);
    put_vec("camera.c.pos", c->pos);
    put_fl("camera.c.areaCenX", c->areaCenX);
    put_fl("camera.c.areaCenY", c->areaCenY);
    put_fl("camera.c.areaCenZ", c->areaCenZ);
    put_s("camera.c.cutscene", c->cutscene);
    put_s("camera.c.nextYaw", c->nextYaw);
    put_s("camera.c.doorStatus", c->doorStatus);
    put_s("camera.gCameraIsC", gCamera == c);
    put_vec("camera.lakitu.curFocus", gLakituState.curFocus);
    put_vec("camera.lakitu.curPos", gLakituState.curPos);
    put_vec("camera.lakitu.goalFocus", gLakituState.goalFocus);
    put_vec("camera.lakitu.goalPos", gLakituState.goalPos);
    put_s("camera.lakitu.mode", gLakituState.mode);
    put_s("camera.lakitu.defMode", gLakituState.defMode);
    put_fl("camera.lakitu.focusDistance", gLakituState.focusDistance);
    put_s("camera.lakitu.oldPitch", gLakituState.oldPitch);
    put_s("camera.lakitu.oldYaw", gLakituState.oldYaw);
    put_s("camera.lakitu.oldRoll", gLakituState.oldRoll);
    put_vecs("camera.lakitu.shakeMagnitude", gLakituState.shakeMagnitude);
    put_s("camera.lakitu.shakePitchPhase", gLakituState.shakePitchPhase);
    put_s("camera.lakitu.shakePitchVel", gLakituState.shakePitchVel);
    put_s("camera.lakitu.shakePitchDecay", gLakituState.shakePitchDecay);
    put_vec("camera.lakitu.unusedVec1", gLakituState.unusedVec1);
    put_vecs("camera.lakitu.unusedVec2", gLakituState.unusedVec2);
    put_s("camera.lakitu.roll", gLakituState.roll);
    put_s("camera.lakitu.yaw", gLakituState.yaw);
    put_s("camera.lakitu.nextYaw", gLakituState.nextYaw);
    put_vec("camera.lakitu.focus", gLakituState.focus);
    put_vec("camera.lakitu.pos", gLakituState.pos);
    put_s("camera.lakitu.shakeRollPhase", gLakituState.shakeRollPhase);
    put_s("camera.lakitu.shakeRollVel", gLakituState.shakeRollVel);
    put_s("camera.lakitu.shakeRollDecay", gLakituState.shakeRollDecay);
    put_s("camera.lakitu.shakeYawPhase", gLakituState.shakeYawPhase);
    put_s("camera.lakitu.shakeYawVel", gLakituState.shakeYawVel);
    put_s("camera.lakitu.shakeYawDecay", gLakituState.shakeYawDecay);
    put_fl("camera.lakitu.focHSpeed", gLakituState.focHSpeed);
    put_fl("camera.lakitu.focVSpeed", gLakituState.focVSpeed);
    put_fl("camera.lakitu.posHSpeed", gLakituState.posHSpeed);
    put_fl("camera.lakitu.posVSpeed", gLakituState.posVSpeed);
    put_s("camera.lakitu.keyDanceRoll", gLakituState.keyDanceRoll);
    put_word("camera.lakitu.lastFrameAction", gLakituState.lastFrameAction);
    put_s("camera.lakitu.unused", gLakituState.unused);
    put_s("camera.transition.posPitch", sModeTransition.posPitch);
    put_s("camera.transition.posYaw", sModeTransition.posYaw);
    put_fl("camera.transition.posDist", sModeTransition.posDist);
    put_s("camera.transition.focPitch", sModeTransition.focPitch);
    put_s("camera.transition.focYaw", sModeTransition.focYaw);
    put_fl("camera.transition.focDist", sModeTransition.focDist);
    put_s("camera.transition.framesLeft", sModeTransition.framesLeft);
    put_vec("camera.transition.marioPos", sModeTransition.marioPos);
    put_s("camera.modeInfo.newMode", sModeInfo.newMode);
    put_s("camera.modeInfo.lastMode", sModeInfo.lastMode);
    put_s("camera.modeInfo.max", sModeInfo.max);
    put_s("camera.modeInfo.frame", sModeInfo.frame);
    put_transition_point("camera.modeInfo.start", &sModeInfo.transitionStart);
    put_transition_point("camera.modeInfo.end", &sModeInfo.transitionEnd);
    put_s("camera.geometry.currFloor", oracle_surface_index(sMarioGeometry.currFloor));
    put_fl("camera.geometry.currFloorHeight", sMarioGeometry.currFloorHeight);
    put_s("camera.geometry.currFloorType", sMarioGeometry.currFloorType);
    put_s("camera.geometry.currCeil", oracle_surface_index(sMarioGeometry.currCeil));
    put_s("camera.geometry.currCeilType", sMarioGeometry.currCeilType);
    put_fl("camera.geometry.currCeilHeight", sMarioGeometry.currCeilHeight);
    put_s("camera.geometry.prevFloor", oracle_surface_index(sMarioGeometry.prevFloor));
    put_fl("camera.geometry.prevFloorHeight", sMarioGeometry.prevFloorHeight);
    put_s("camera.geometry.prevFloorType", sMarioGeometry.prevFloorType);
    put_s("camera.geometry.prevCeil", oracle_surface_index(sMarioGeometry.prevCeil));
    put_fl("camera.geometry.prevCeilHeight", sMarioGeometry.prevCeilHeight);
    put_s("camera.geometry.prevCeilType", sMarioGeometry.prevCeilType);
    put_fl("camera.geometry.waterHeight", sMarioGeometry.waterHeight);
    put_s("camera.fov.fovFunc", sFOVState.fovFunc);
    put_fl("camera.fov.fov", sFOVState.fov);
    put_fl("camera.fov.fovOffset", sFOVState.fovOffset);
    put_word("camera.fov.unusedIsSleeping", sFOVState.unusedIsSleeping);
    put_fl("camera.fov.shakeAmplitude", sFOVState.shakeAmplitude);
    put_s("camera.fov.shakePhase", sFOVState.shakePhase);
    put_s("camera.fov.shakeSpeed", sFOVState.shakeSpeed);
    put_s("camera.fov.decay", sFOVState.decay);
    put_s("camera.statusFlags", sStatusFlags);
    put_s("camera.movementFlags", gCameraMovementFlags);
    put_s("camera.s2ndRotateFlags", s2ndRotateFlags);
    put_s("camera.selectionFlags", sSelectionFlags);
    put_s("camera.soundFlags", sCameraSoundFlags);
    put_s("camera.cButtonsPressed", sCButtonsPressed);
    put_s("camera.yawSpeed", sYawSpeed);
    put_s("camera.areaYaw", sAreaYaw);
    put_s("camera.areaYawChange", sAreaYawChange);
    put_s("camera.lakituDist", sLakituDist);
    put_s("camera.lakituPitch", sLakituPitch);
    put_s("camera.modeOffsetYaw", sModeOffsetYaw);
    put_s("camera.cUpCameraPitch", sCUpCameraPitch);
    put_fl("camera.panDistance", sPanDistance);
    put_fl("camera.cannonYOffset", sCannonYOffset);
    put_fl("camera.zoomAmount", sZoomAmount);
    put_fl("camera.zeroZoomDist", sZeroZoomDist);
    put_fl("camera.zoomDist", gCameraZoomDist);
    put_s("camera.cSideButtonYaw", sCSideButtonYaw);
    put_s("camera.avoidYawVel", sAvoidYawVel);
    put_s("camera.unusedFreeRoamWallYaw", unusedFreeRoamWallYaw);
    put_s("camera.yawAfterDoorCutscene", sCameraYawAfterDoorCutscene);
    put_s("camera.behindMarioSoundTimer", sBehindMarioSoundTimer);
    put_s("camera.spiralStairsYawOffset", sSpiralStairsYawOffset);
    put_s("camera.eightDirBaseYaw", s8DirModeBaseYaw);
    put_s("camera.eightDirYawOffset", s8DirModeYawOffset);
    put_s("camera.framesSinceCutsceneEnded", sFramesSinceCutsceneEnded);
    put_s("camera.recentCutscene", gRecentCutscene);
    put_s("camera.objectCutscene", sObjectCutscene);
    put_s("camera.framesPaused", sFramesPaused);
    put_s("camera.currLevelArea", gCurrLevelArea);
    put_word("camera.prevLevel", gPrevLevel);
    put_s("camera.creditsPlayer2Pitch", sCreditsPlayer2Pitch);
    put_s("camera.creditsPlayer2Yaw", sCreditsPlayer2Yaw);
    put_s("camera.cutsceneSplineSegment", sCutsceneSplineSegment);
    put_fl("camera.cutsceneSplineSegmentProgress", sCutsceneSplineSegmentProgress);
    put_s("camera.cutsceneShot", sCutsceneShot);
    put_s("camera.cutsceneTimer", gCutsceneTimer);
    put_word("camera.cutsceneObjSpawn", gCutsceneObjSpawn);
    put_s("camera.objCutsceneDone", gObjCutsceneDone);
    put_s("camera.cutsceneFocusIsNull", gCutsceneFocus == NULL);
    put_s("camera.secondCameraFocusIsNull", gSecondCameraFocus == NULL);
    put_word("camera.unused8032CFC8", unused8032CFC8);
    put_word("camera.unused8032CFCC", unused8032CFCC);
    put_s("camera.unused8033B316", unused8033B316);
    put_s("camera.unused8033B31A", unused8033B31A);
    put_word("camera.unused8033B30C", unused8033B30C);
    put_word("camera.unused8033B310", unused8033B310);
    put_s("camera.unused8033B6E8", unused8033B6E8);
    put_vec("camera.oldPosition", sOldPosition);
    put_vec("camera.oldFocus", sOldFocus);
    put_vec("camera.player2FocusOffset", sPlayer2FocusOffset);
    put_vec("camera.castleEntranceOffset", sCastleEntranceOffset);
    put_vec("camera.fixedModeBasePosition", sFixedModeBasePosition);
    put_vec("camera.storeCUp.pos", sCameraStoreCUp.pos);
    put_vec("camera.storeCUp.focus", sCameraStoreCUp.focus);
    put_fl("camera.storeCUp.panDist", sCameraStoreCUp.panDist);
    put_fl("camera.storeCUp.cannonYOffset", sCameraStoreCUp.cannonYOffset);
    for (i = 0; i < 4; i++) {
        snprintf(sNameBuf, sizeof(sNameBuf), "camera.handheld.spline[%d].index", i);
        put_s(sNameBuf, sHandheldShakeSpline[i].index);
        snprintf(sNameBuf, sizeof(sNameBuf), "camera.handheld.spline[%d].point", i);
        {
            char buf[64];
            memcpy(buf, sNameBuf, sizeof(buf));
            put_vecs(buf, sHandheldShakeSpline[i].point);
        }
    }
    put_s("camera.handheld.mag", sHandheldShakeMag);
    put_fl("camera.handheld.timer", sHandheldShakeTimer);
    put_fl("camera.handheld.inc", sHandheldShakeInc);
    put_s("camera.handheld.pitch", sHandheldShakePitch);
    put_s("camera.handheld.yaw", sHandheldShakeYaw);
    put_s("camera.handheld.roll", sHandheldShakeRoll);
    for (i = 0; i < 32; i++) {
        snprintf(sNameBuf, sizeof(sNameBuf), "camera.creditsSplinePos[%d].index", i);
        put_s(sNameBuf, sCurCreditsSplinePos[i].index);
        snprintf(sNameBuf, sizeof(sNameBuf), "camera.creditsSplineFocus[%d].index", i);
        put_s(sNameBuf, sCurCreditsSplineFocus[i].index);
    }
    put_s("camera.rngSeed", oracle_rng_seed());
    put_s("camera.hudStatus", gOracleHudCameraStatus);
    put_vec("camera.graph.pos", sFrameGraphCamera.pos);
    put_vec("camera.graph.focus", sFrameGraphCamera.focus);
    put_s("camera.graph.rollScreen", sFrameGraphCamera.rollScreen);
    put_fl("camera.graph.fov", sFrameGraphPerspective.fov);
}
