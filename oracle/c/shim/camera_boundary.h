/* Authored prototypes absent from camera.h; MIT, development only. */
#ifndef ORACLE_CAMERA_BOUNDARY_H
#define ORACLE_CAMERA_BOUNDARY_H
void approach_vec3s_asymptotic(Vec3s current, Vec3s target, s16 x, s16 y, s16 z);
s32 camera_approach_s16_symmetric(s16 current, s16 target, s16 increment);
s32 is_pos_in_bounds(Vec3f pos, Vec3f center, Vec3f bounds, s16 yaw);
s16 find_in_bounds_yaw_wdw_bob_thi(Vec3f pos, Vec3f origin, s16 yaw);
s16 look_down_slopes(s16 yaw);
BAD_RETURN(f32) calc_y_to_curr_floor(f32 *posOff, f32 posMul, f32 posBound,
                                   f32 *focOff, f32 focMul, f32 focBound);
void focus_on_mario(Vec3f focus, Vec3f pos, f32 posOff, f32 focOff,
                    f32 dist, s16 pitch, s16 yaw);
s32 update_radial_camera(struct Camera *camera, Vec3f focus, Vec3f pos);
s32 is_behind_surface(Vec3f pos, struct Surface *surface);
s32 is_surf_within_bounding_box(struct Surface *surface, f32 x, f32 y, f32 z);
s32 is_mario_behind_surface(struct Camera *camera, struct Surface *surface);
void radial_camera_move(struct Camera *camera);
void lakitu_zoom(f32 rangeDist, s16 rangePitch);
#endif
