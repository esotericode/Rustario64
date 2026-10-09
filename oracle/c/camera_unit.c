/* Authored adapters for verbatim camera.c components; MIT. These do not
 * dispatch modes or change the Mario tick harness. */
#include <stdlib.h>
#include <string.h>
/* Preserve Mario's existing event boundary. Only these verbatim camera
 * excerpts call the real camera shake implementation. Random paths remain
 * explicitly unavailable until the reference RNG is ported. */
static float oracle_camera_random_unavailable(void);
#define random_float oracle_camera_random_unavailable
#define set_camera_shake_from_hit oracle_camera_native_hit
#include "excerpts/camera.c"
#undef set_camera_shake_from_hit
#undef random_float
#include "engine/surface_load.h"

int oracle_surface_index(struct Surface *surface);
static struct Camera sComponentCamera;
static u32 bits(f32 value) { u32 word; memcpy(&word, &value, sizeof(word)); return word; }
static f32 oracle_camera_random_unavailable(void) { abort(); }
#include "lakitu_state.inc.c"

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
        case 2:
            if (value == SHAKE_SHOCK) abort();
            oracle_camera_native_hit(value); break;
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
