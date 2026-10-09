/* Authored adapters for verbatim camera.c components; MIT. These do not
 * dispatch modes, run update_lakitu, or change the Mario tick harness. */
#include <stdlib.h>
#include <string.h>
#include "excerpts/camera.c"
#include "engine/surface_load.h"

int oracle_surface_index(struct Surface *surface);
static struct Camera sComponentCamera;
static u32 bits(f32 value) { u32 word; memcpy(&word, &value, sizeof(word)); return word; }

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
