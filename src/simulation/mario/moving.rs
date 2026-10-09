//! Moving actions, translated from pinned CC0
//! src/game/mario_actions_moving.c. Actions that hold or ride an object panic
//! until objects are simulated.
use super::{
    MarioState, Mat4, StepWorld,
    animation::{
        is_anim_at_end, is_anim_past_frame, set_mario_anim_with_accel, set_mario_animation,
    },
    constants::*,
    core::{
        adjust_sound_for_speed, check_common_action_exits, drop_and_set_mario_action,
        find_floor_slope, mario_facing_downhill, mario_floor_is_slope,
        play_mario_heavy_landing_sound_once, play_mario_jump_sound, play_mario_landing_sound,
        play_mario_landing_sound_once, play_sound_and_spawn_particles, play_sound_if_no_flag,
        set_jump_from_landing, set_jumping_action, set_mario_action, set_water_plunge_action,
    },
    f32_to_s16, f32_to_s32,
    inputs::{mario_floor_is_slippery, mario_get_floor_class},
    interaction::{mario_check_object_grab, mario_drop_held_object},
    object::mario_update_punch_sequence,
    step::{
        mario_bonk_reflection, mario_push_off_steep_floor, mario_set_forward_vel,
        mario_update_moving_sand, mario_update_quicksand, mario_update_windy_ground,
        perform_ground_step,
    },
};
use crate::simulation::{
    collision::WallCollisionData,
    math::{approach_f32, approach_s32},
};

/// struct LandingAction.
struct LandingAction {
    num_frames: i16,
    unk02: i16,
    very_steep_action: u32,
    end_action: u32,
    a_pressed_action: u32,
    off_floor_action: u32,
    slide_action: u32,
}

const JUMP_LAND: LandingAction = LandingAction {
    num_frames: 4,
    unk02: 5,
    very_steep_action: ACT_FREEFALL,
    end_action: ACT_JUMP_LAND_STOP,
    a_pressed_action: ACT_DOUBLE_JUMP,
    off_floor_action: ACT_FREEFALL,
    slide_action: ACT_BEGIN_SLIDING,
};
const FREEFALL_LAND: LandingAction = LandingAction {
    end_action: ACT_FREEFALL_LAND_STOP,
    ..JUMP_LAND
};
const SIDE_FLIP_LAND: LandingAction = LandingAction {
    end_action: ACT_SIDE_FLIP_LAND_STOP,
    ..JUMP_LAND
};
const LONG_JUMP_LAND: LandingAction = LandingAction {
    num_frames: 6,
    end_action: ACT_LONG_JUMP_LAND_STOP,
    a_pressed_action: ACT_LONG_JUMP,
    ..JUMP_LAND
};
const DOUBLE_JUMP_LAND: LandingAction = LandingAction {
    end_action: ACT_DOUBLE_JUMP_LAND_STOP,
    a_pressed_action: ACT_JUMP,
    ..JUMP_LAND
};
const TRIPLE_JUMP_LAND: LandingAction = LandingAction {
    unk02: 0,
    end_action: ACT_TRIPLE_JUMP_LAND_STOP,
    a_pressed_action: ACT_UNINITIALIZED,
    ..JUMP_LAND
};
const BACKFLIP_LAND: LandingAction = LandingAction {
    unk02: 0,
    end_action: ACT_BACKFLIP_LAND_STOP,
    a_pressed_action: ACT_BACKFLIP,
    ..JUMP_LAND
};

fn floor_normal(m: &MarioState, w: &StepWorld<'_>) -> [f32; 3] {
    w.surface(
        m.floor
            .expect("Mario has no floor (the original dereferences NULL)"),
    )
    .normal
}

/// tilt_body_running.
fn tilt_body_running(m: &MarioState, w: &mut StepWorld<'_>) -> i16 {
    let pitch = find_floor_slope(m, w, 0);
    let pitch = f32_to_s16(f32::from(pitch) * m.forward_vel / 40.0);
    (-i32::from(pitch)) as i16
}

/// play_step_sound.
pub fn play_step_sound(m: &mut MarioState, w: &mut StepWorld<'_>, frame1: i16, frame2: i16) {
    if is_anim_past_frame(m, w, frame1) || is_anim_past_frame(m, w, frame2) {
        let tiptoe = i32::from(m.obj.gfx.anim.anim_id) == MARIO_ANIM_TIPTOE;
        if m.flags & MARIO_METAL_CAP != 0 {
            let sound = if tiptoe {
                SOUND_ACTION_METAL_STEP_TIPTOE
            } else {
                SOUND_ACTION_METAL_STEP
            };
            play_sound_and_spawn_particles(m, w, sound, 0);
        } else if m.quicksand_depth > 50.0 {
            w.play_sound(SOUND_ACTION_QUICKSAND_STEP);
        } else if tiptoe {
            play_sound_and_spawn_particles(m, w, SOUND_ACTION_TERRAIN_STEP_TIPTOE, 0);
        } else {
            play_sound_and_spawn_particles(m, w, SOUND_ACTION_TERRAIN_STEP, 0);
        }
    }
}

/// find_vector_perpendicular_to_plane (math_util.c).
fn find_vector_perpendicular_to_plane(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    [
        (b[1] - a[1]) * (c[2] - b[2]) - (c[1] - b[1]) * (b[2] - a[2]),
        (b[2] - a[2]) * (c[0] - b[0]) - (c[2] - b[2]) * (b[0] - a[0]),
        (b[0] - a[0]) * (c[1] - b[1]) - (c[0] - b[0]) * (b[1] - a[1]),
    ]
}

/// vec3f_cross (math_util.c).
fn vec3f_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - b[1] * a[2],
        a[2] * b[0] - b[2] * a[0],
        a[0] * b[1] - b[0] * a[1],
    ]
}

/// vec3f_normalize (math_util.c), including its unguarded division.
fn vec3f_normalize(v: [f32; 3]) -> [f32; 3] {
    let inv = 1.0 / (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

/// mtxf_align_terrain_triangle (math_util.c): a presentation matrix, but its
/// floor queries run inside the tick and touch collision flags.
fn mtxf_align_terrain_triangle(
    w: &mut StepWorld<'_>,
    pos: [f32; 3],
    yaw: i16,
    radius: f32,
) -> Mat4 {
    let yaw32 = i32::from(yaw);
    let min_y = -radius * 3.0;
    let floor_at = |angle: i32, w: &mut StepWorld<'_>| {
        let x = pos[0] + radius * w.trig.sins(yaw32 + angle);
        let z = pos[2] + radius * w.trig.coss(yaw32 + angle);
        [x, 0.0, z]
    };
    let mut point0 = floor_at(0x2AAA, w);
    let mut point1 = floor_at(0x8000, w);
    let mut point2 = floor_at(0xD555, w);
    for point in [&mut point0, &mut point1, &mut point2] {
        point[1] = w
            .collision
            .find_floor(point[0], pos[1] + 150.0, point[2], &mut w.collision_flags)
            .0;
    }
    for point in [&mut point0, &mut point1, &mut point2] {
        if point[1] - pos[1] < min_y {
            point[1] = pos[1];
        }
    }
    let avg_y = (point0[1] + point1[1] + point2[1]) / 3.0;
    let forward = [w.trig.sins(yaw32), 0.0, w.trig.coss(yaw32)];
    let y_column = vec3f_normalize(find_vector_perpendicular_to_plane(point0, point1, point2));
    let x_column = vec3f_normalize(vec3f_cross(y_column, forward));
    let z_column = vec3f_normalize(vec3f_cross(x_column, y_column));
    [
        [x_column[0], x_column[1], x_column[2], 0.0],
        [y_column[0], y_column[1], y_column[2], 0.0],
        [z_column[0], z_column[1], z_column[2], 0.0],
        [
            pos[0],
            if avg_y < pos[1] { pos[1] } else { avg_y },
            pos[2],
            1.0,
        ],
    ]
}

/// align_with_floor.
pub fn align_with_floor(m: &mut MarioState, w: &mut StepWorld<'_>) {
    m.pos[1] = m.floor_height;
    let index = usize::from(m.unk00);
    w.floor_align_matrix[index] = mtxf_align_terrain_triangle(w, m.pos, m.face_angle[1], 40.0);
    m.obj.gfx.throw_matrix = Some(index);
}

/// begin_walking_action.
pub fn begin_walking_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    forward_vel: f32,
    action: u32,
    action_arg: u32,
) -> i32 {
    m.face_angle[1] = m.intended_yaw;
    mario_set_forward_vel(m, w, forward_vel);
    set_mario_action(m, w, action, action_arg)
}

/// check_ledge_climb_down.
fn check_ledge_climb_down(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.forward_vel < 10.0 {
        let mut cols = WallCollisionData::new(m.pos, -10.0, 10.0);
        let pass_vanish_walls = m.flags & MARIO_VANISH_CAP != 0;
        if w.collision
            .find_wall_collisions(&mut cols, w.collision_flags, pass_vanish_walls)
            != 0
        {
            let (floor_height, floor) =
                w.collision
                    .find_floor(cols.x, cols.y, cols.z, &mut w.collision_flags);
            if floor.is_some() && cols.y - floor_height > 160.0 {
                let wall = w
                    .collision
                    .surface(cols.walls[cols.num_walls as usize - 1].expect("counted wall"));
                let wall_angle = w.trig.atan2s(wall.normal[2], wall.normal[0]);
                let wall_d_yaw = wall_angle.wrapping_sub(m.face_angle[1]);
                if wall_d_yaw > -0x4000 && wall_d_yaw < 0x4000 {
                    m.pos[0] = cols.x - 20.0 * wall.normal[0];
                    m.pos[2] = cols.z - 20.0 * wall.normal[2];
                    m.face_angle[0] = 0;
                    m.face_angle[1] = wall_angle.wrapping_add(i16::MIN);
                    set_mario_action(m, w, ACT_LEDGE_CLIMB_DOWN, 0);
                    set_mario_animation(m, w, MARIO_ANIM_CLIMB_DOWN_LEDGE);
                }
            }
        }
    }
}

/// slide_bonk.
fn slide_bonk(m: &mut MarioState, w: &mut StepWorld<'_>, fast_action: u32, slow_action: u32) {
    if m.forward_vel > 16.0 {
        mario_bonk_reflection(m, w, true);
        drop_and_set_mario_action(m, w, fast_action, 0);
    } else {
        mario_set_forward_vel(m, w, 0.0);
        set_mario_action(m, w, slow_action, 0);
    }
}

/// set_triple_jump_action.
fn set_triple_jump_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    _action: u32,
    _arg: u32,
) -> i32 {
    if m.flags & MARIO_WING_CAP != 0 {
        set_mario_action(m, w, ACT_FLYING_TRIPLE_JUMP, 0)
    } else if m.forward_vel > 20.0 {
        set_mario_action(m, w, ACT_TRIPLE_JUMP, 0)
    } else {
        set_mario_action(m, w, ACT_JUMP, 0)
    }
}

/// update_sliding_angle.
fn update_sliding_angle(m: &mut MarioState, w: &mut StepWorld<'_>, accel: f32, loss_factor: f32) {
    let normal = floor_normal(m, w);
    let slope_angle = w.trig.atan2s(normal[2], normal[0]);
    let steepness = (normal[0] * normal[0] + normal[2] * normal[2]).sqrt();
    m.slide_vel_x += accel * steepness * w.trig.sins(i32::from(slope_angle));
    m.slide_vel_z += accel * steepness * w.trig.coss(i32::from(slope_angle));
    m.slide_vel_x *= loss_factor;
    m.slide_vel_z *= loss_factor;
    m.slide_yaw = w.trig.atan2s(m.slide_vel_z, m.slide_vel_x);
    let facing_d_yaw = m.face_angle[1].wrapping_sub(m.slide_yaw);
    let mut new_facing_d_yaw = i32::from(facing_d_yaw);
    // -0x4000 is not handled: Mario can slide while facing perpendicular.
    if new_facing_d_yaw > 0 && new_facing_d_yaw <= 0x4000 {
        new_facing_d_yaw -= 0x200;
        if new_facing_d_yaw < 0 {
            new_facing_d_yaw = 0;
        }
    } else if new_facing_d_yaw > -0x4000 && new_facing_d_yaw < 0 {
        new_facing_d_yaw += 0x200;
        if new_facing_d_yaw > 0 {
            new_facing_d_yaw = 0;
        }
    } else if new_facing_d_yaw > 0x4000 && new_facing_d_yaw < 0x8000 {
        new_facing_d_yaw += 0x200;
        if new_facing_d_yaw > 0x8000 {
            new_facing_d_yaw = 0x8000;
        }
    } else if new_facing_d_yaw > -0x8000 && new_facing_d_yaw < -0x4000 {
        new_facing_d_yaw -= 0x200;
        if new_facing_d_yaw < -0x8000 {
            new_facing_d_yaw = -0x8000;
        }
    }
    m.face_angle[1] = (i32::from(m.slide_yaw) + new_facing_d_yaw) as i16;
    m.vel = [m.slide_vel_x, 0.0, m.slide_vel_z];
    mario_update_moving_sand(m, w);
    mario_update_windy_ground(m, w);
    // (Butt slide HSG) Speed is capped a frame late.
    m.forward_vel = (m.slide_vel_x * m.slide_vel_x + m.slide_vel_z * m.slide_vel_z).sqrt();
    if m.forward_vel > 100.0 {
        m.slide_vel_x = m.slide_vel_x * 100.0 / m.forward_vel;
        m.slide_vel_z = m.slide_vel_z * 100.0 / m.forward_vel;
    }
    if new_facing_d_yaw < -0x4000 || new_facing_d_yaw > 0x4000 {
        m.forward_vel *= -1.0;
    }
}

/// update_sliding: TRUE when Mario stopped.
fn update_sliding(m: &mut MarioState, w: &mut StepWorld<'_>, stop_speed: f32) -> bool {
    let intended_d_yaw = m.intended_yaw.wrapping_sub(m.slide_yaw);
    let mut forward = w.trig.coss(i32::from(intended_d_yaw));
    let sideward = w.trig.sins(i32::from(intended_d_yaw));
    // 10k glitch
    if forward < 0.0 && m.forward_vel >= 0.0 {
        forward *= 0.5 + 0.5 * m.forward_vel / 100.0;
    }
    let mag = m.intended_mag;
    let (accel, loss_factor) = match mario_get_floor_class(m, w) {
        SURFACE_CLASS_VERY_SLIPPERY => (10.0, mag / 32.0 * forward * 0.02 + 0.98),
        SURFACE_CLASS_SLIPPERY => (8.0, mag / 32.0 * forward * 0.02 + 0.96),
        SURFACE_CLASS_NOT_SLIPPERY => (5.0, mag / 32.0 * forward * 0.02 + 0.92),
        _ => (7.0, mag / 32.0 * forward * 0.02 + 0.92),
    };
    let old_speed = (m.slide_vel_x * m.slide_vel_x + m.slide_vel_z * m.slide_vel_z).sqrt();
    // Rotates with the new X speed but the old Z speed.
    m.slide_vel_x += m.slide_vel_z * (mag / 32.0) * sideward * 0.05;
    m.slide_vel_z -= m.slide_vel_x * (mag / 32.0) * sideward * 0.05;
    let new_speed = (m.slide_vel_x * m.slide_vel_x + m.slide_vel_z * m.slide_vel_z).sqrt();
    if old_speed > 0.0 && new_speed > 0.0 {
        m.slide_vel_x = m.slide_vel_x * old_speed / new_speed;
        m.slide_vel_z = m.slide_vel_z * old_speed / new_speed;
    }
    update_sliding_angle(m, w, accel, loss_factor);
    if !mario_floor_is_slope(m, w) && m.forward_vel * m.forward_vel < stop_speed * stop_speed {
        mario_set_forward_vel(m, w, 0.0);
        return true;
    }
    false
}

/// apply_slope_accel.
fn apply_slope_accel(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let normal = floor_normal(m, w);
    let steepness = (normal[0] * normal[0] + normal[2] * normal[2]).sqrt();
    let floor_d_yaw = m.floor_angle.wrapping_sub(m.face_angle[1]);
    if mario_floor_is_slope(m, w) {
        let slope_class =
            if m.action != ACT_SOFT_BACKWARD_GROUND_KB && m.action != ACT_SOFT_FORWARD_GROUND_KB {
                mario_get_floor_class(m, w)
            } else {
                0
            };
        let slope_accel = match slope_class {
            SURFACE_CLASS_VERY_SLIPPERY => 5.3,
            SURFACE_CLASS_SLIPPERY => 2.7,
            SURFACE_CLASS_NOT_SLIPPERY => 0.0,
            _ => 1.7,
        };
        if floor_d_yaw > -0x4000 && floor_d_yaw < 0x4000 {
            m.forward_vel += slope_accel * steepness;
        } else {
            m.forward_vel -= slope_accel * steepness;
        }
    }
    m.slide_yaw = m.face_angle[1];
    m.slide_vel_x = m.forward_vel * w.trig.sins(i32::from(m.face_angle[1]));
    m.slide_vel_z = m.forward_vel * w.trig.coss(i32::from(m.face_angle[1]));
    m.vel = [m.slide_vel_x, 0.0, m.slide_vel_z];
    mario_update_moving_sand(m, w);
    mario_update_windy_ground(m, w);
}

/// apply_landing_accel: TRUE when Mario stopped.
fn apply_landing_accel(m: &mut MarioState, w: &mut StepWorld<'_>, friction_factor: f32) -> bool {
    apply_slope_accel(m, w);
    if !mario_floor_is_slope(m, w) {
        m.forward_vel *= friction_factor;
        if m.forward_vel * m.forward_vel < 1.0 {
            mario_set_forward_vel(m, w, 0.0);
            return true;
        }
    }
    false
}

/// apply_slope_decel: TRUE when Mario stopped.
fn apply_slope_decel(m: &mut MarioState, w: &mut StepWorld<'_>, decel_coef: f32) -> bool {
    let decel = match mario_get_floor_class(m, w) {
        SURFACE_CLASS_VERY_SLIPPERY => decel_coef * 0.2,
        SURFACE_CLASS_SLIPPERY => decel_coef * 0.7,
        SURFACE_CLASS_NOT_SLIPPERY => decel_coef * 3.0,
        _ => decel_coef * 2.0,
    };
    m.forward_vel = approach_f32(m.forward_vel, 0.0, decel, decel);
    let stopped = m.forward_vel == 0.0;
    apply_slope_accel(m, w);
    stopped
}

/// update_decelerating_speed: TRUE when Mario stopped.
fn update_decelerating_speed(m: &mut MarioState, w: &mut StepWorld<'_>) -> bool {
    m.forward_vel = approach_f32(m.forward_vel, 0.0, 1.0, 1.0);
    let stopped = m.forward_vel == 0.0;
    let forward_vel = m.forward_vel;
    mario_set_forward_vel(m, w, forward_vel);
    mario_update_moving_sand(m, w);
    mario_update_windy_ground(m, w);
    stopped
}

/// update_walking_speed.
fn update_walking_speed(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let slow_floor = m
        .floor
        .is_some_and(|floor| w.surface(floor).surface_type == SURFACE_SLOW);
    let max_target_speed = if slow_floor { 24.0 } else { 32.0 };
    let mut target_speed = if m.intended_mag < max_target_speed {
        m.intended_mag
    } else {
        max_target_speed
    };
    if m.quicksand_depth > 10.0 {
        // 6.25 is a double constant: the product is computed in double.
        target_speed = (f64::from(target_speed) * (6.25 / f64::from(m.quicksand_depth))) as f32;
    }
    if m.forward_vel <= 0.0 {
        m.forward_vel += 1.1;
    } else if m.forward_vel <= target_speed {
        m.forward_vel += 1.1 - m.forward_vel / 43.0;
    } else if floor_normal(m, w)[1] >= 0.95 {
        m.forward_vel -= 1.0;
    }
    if m.forward_vel > 48.0 {
        m.forward_vel = 48.0;
    }
    let d_yaw = i32::from(m.intended_yaw.wrapping_sub(m.face_angle[1]));
    m.face_angle[1] = m
        .intended_yaw
        .wrapping_sub(approach_s32(d_yaw, 0, 0x800, 0x800) as i16);
    apply_slope_accel(m, w);
}

/// should_begin_sliding.
fn should_begin_sliding(m: &MarioState, w: &StepWorld<'_>) -> bool {
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        let slide_level = w.area_terrain_type & TERRAIN_MASK == TERRAIN_SLIDE;
        let moving_backward = m.forward_vel <= -1.0;
        if slide_level || moving_backward || mario_facing_downhill(m, false) {
            return true;
        }
    }
    false
}

/// analog_stick_held_back.
fn analog_stick_held_back(m: &MarioState) -> bool {
    let intended_d_yaw = m.intended_yaw.wrapping_sub(m.face_angle[1]);
    !(-0x471C..=0x471C).contains(&intended_d_yaw)
}

/// check_ground_dive_or_punch.
fn check_ground_dive_or_punch(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        // Speed kick.
        if m.forward_vel >= 29.0 && w.controller.stick_mag > 48.0 {
            m.vel[1] = 20.0;
            return set_mario_action(m, w, ACT_DIVE, 1);
        }
        return set_mario_action(m, w, ACT_MOVE_PUNCHING, 0);
    }
    0
}

/// begin_braking_action.
fn begin_braking_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    mario_drop_held_object(m);
    if m.action_state == 1 {
        m.face_angle[1] = m.action_arg as i16;
        return set_mario_action(m, w, ACT_STANDING_AGAINST_WALL, 0);
    }
    if m.forward_vel >= 16.0 && floor_normal(m, w)[1] >= 0.17364818 {
        return set_mario_action(m, w, ACT_BRAKING, 0);
    }
    set_mario_action(m, w, ACT_DECELERATING, 0)
}

/// anim_and_audio_for_walk.
fn anim_and_audio_for_walk(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let mut target_pitch: i16 = 0;
    let mut speed = if m.intended_mag > m.forward_vel {
        m.intended_mag
    } else {
        m.forward_vel
    };
    if speed < 4.0 {
        speed = 4.0;
    }
    if m.quicksand_depth > 50.0 {
        let accel = f32_to_s32(speed / 4.0 * 65536.0);
        set_mario_anim_with_accel(m, w, MARIO_ANIM_MOVE_IN_QUICKSAND, accel);
        play_step_sound(m, w, 19, 93);
        m.action_timer = 0;
    } else {
        loop {
            match m.action_timer {
                0 => {
                    if speed > 8.0 {
                        m.action_timer = 2;
                    } else {
                        // (Speed crash) above 2^17 speed.
                        let mut accel = f32_to_s32(speed / 4.0 * 65536.0);
                        if accel < 0x1000 {
                            accel = 0x1000;
                        }
                        set_mario_anim_with_accel(m, w, MARIO_ANIM_START_TIPTOE, accel);
                        play_step_sound(m, w, 7, 22);
                        if is_anim_past_frame(m, w, 23) {
                            m.action_timer = 2;
                        }
                        break;
                    }
                }
                1 => {
                    if speed > 8.0 {
                        m.action_timer = 2;
                    } else {
                        let mut accel = f32_to_s32(speed * 65536.0);
                        if accel < 0x1000 {
                            accel = 0x1000;
                        }
                        set_mario_anim_with_accel(m, w, MARIO_ANIM_TIPTOE, accel);
                        play_step_sound(m, w, 14, 72);
                        break;
                    }
                }
                2 => {
                    if speed < 5.0 {
                        m.action_timer = 1;
                    } else if speed > 22.0 {
                        m.action_timer = 3;
                    } else {
                        let accel = f32_to_s32(speed / 4.0 * 65536.0);
                        set_mario_anim_with_accel(m, w, MARIO_ANIM_WALKING, accel);
                        play_step_sound(m, w, 10, 49);
                        break;
                    }
                }
                3 => {
                    if speed < 18.0 {
                        m.action_timer = 2;
                    } else {
                        let accel = f32_to_s32(speed / 4.0 * 65536.0);
                        set_mario_anim_with_accel(m, w, MARIO_ANIM_RUNNING, accel);
                        play_step_sound(m, w, 9, 45);
                        target_pitch = tilt_body_running(m, w);
                        break;
                    }
                }
                // The original loops forever on any other timer value.
                timer => panic!("walking animation timer {timer} never ends the original loop"),
            }
        }
    }
    let pitch = approach_s32(
        m.obj.raw.s32(O_MARIO_WALKING_PITCH),
        i32::from(target_pitch),
        0x800,
        0x800,
    ) as i16;
    m.obj.raw.set_s32(O_MARIO_WALKING_PITCH, i32::from(pitch));
    m.obj.gfx.angle[0] = m.obj.raw.s32(O_MARIO_WALKING_PITCH) as i16;
}

/// push_or_sidle_wall.
fn push_or_sidle_wall(m: &mut MarioState, w: &mut StepWorld<'_>, start_pos: [f32; 3]) {
    let dx = m.pos[0] - start_pos[0];
    let dz = m.pos[2] - start_pos[2];
    let moved_distance = (dx * dx + dz * dz).sqrt();
    // (Speed crash) if a wall is hit after moving 16384 units.
    let accel = f32_to_s32(moved_distance * 2.0 * 65536.0);
    if m.forward_vel > 6.0 {
        mario_set_forward_vel(m, w, 6.0);
    }
    let wall = m.wall.map(|wall| {
        let normal = w.surface(wall).normal;
        let wall_angle = w.trig.atan2s(normal[2], normal[0]);
        (wall_angle, wall_angle.wrapping_sub(m.face_angle[1]))
    });
    match wall {
        Some((wall_angle, d_wall_angle)) if d_wall_angle > -0x71C8 && d_wall_angle < 0x71C8 => {
            if d_wall_angle < 0 {
                set_mario_anim_with_accel(m, w, MARIO_ANIM_SIDESTEP_RIGHT, accel);
            } else {
                set_mario_anim_with_accel(m, w, MARIO_ANIM_SIDESTEP_LEFT, accel);
            }
            if m.obj.gfx.anim.anim_frame < 20 {
                w.play_sound(SOUND_MOVING_TERRAIN_SLIDE.wrapping_add(m.terrain_sound_addend));
                m.particle_flags |= PARTICLE_DUST;
            }
            m.action_state = 1;
            m.action_arg = (i32::from(wall_angle) + 0x8000) as u32;
            m.obj.gfx.angle[1] = wall_angle.wrapping_add(i16::MIN);
            m.obj.gfx.angle[2] = find_floor_slope(m, w, 0x4000);
        }
        _ => {
            m.flags |= MARIO_UNKNOWN_31;
            set_mario_animation(m, w, MARIO_ANIM_PUSHING);
            play_step_sound(m, w, 6, 18);
        }
    }
}

/// tilt_body_walking.
fn tilt_body_walking(m: &mut MarioState, start_yaw: i16) {
    let anim_id = i32::from(m.obj.gfx.anim.anim_id);
    if anim_id == MARIO_ANIM_WALKING || anim_id == MARIO_ANIM_RUNNING {
        let d_yaw = m.face_angle[1].wrapping_sub(start_yaw);
        // (Speed crash) these casts crash beyond 2^31.
        let mut roll = (-i32::from(f32_to_s16(f32::from(d_yaw) * m.forward_vel / 12.0))) as i16;
        let mut pitch = f32_to_s16(m.forward_vel * 170.0);
        roll = roll.clamp(-0x1555, 0x1555);
        if pitch > 0x1555 {
            pitch = 0x1555;
        }
        if pitch < 0 {
            pitch = 0;
        }
        let torso = &mut m.body.torso_angle;
        torso[2] = approach_s32(i32::from(torso[2]), i32::from(roll), 0x400, 0x400) as i16;
        torso[0] = approach_s32(i32::from(torso[0]), i32::from(pitch), 0x400, 0x400) as i16;
    } else {
        m.body.torso_angle[2] = 0;
        m.body.torso_angle[0] = 0;
    }
}

fn act_walking(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let start_yaw = m.face_angle[1];
    mario_drop_held_object(m);
    if should_begin_sliding(m, w) {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return begin_braking_action(m, w);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jump_from_landing(m, w);
    }
    if check_ground_dive_or_punch(m, w) != 0 {
        return 1;
    }
    if m.input & INPUT_UNKNOWN_5 != 0 {
        return begin_braking_action(m, w);
    }
    if analog_stick_held_back(m) && m.forward_vel >= 16.0 {
        return set_mario_action(m, w, ACT_TURNING_AROUND, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_CROUCH_SLIDE, 0);
    }
    m.action_state = 0;
    let start_pos = m.pos;
    update_walking_speed(m, w);
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 0);
            set_mario_animation(m, w, MARIO_ANIM_GENERAL_FALL);
        }
        GROUND_STEP_NONE => {
            anim_and_audio_for_walk(m, w);
            if m.intended_mag - m.forward_vel > 16.0 {
                m.particle_flags |= PARTICLE_DUST;
            }
        }
        GROUND_STEP_HIT_WALL => {
            push_or_sidle_wall(m, w, start_pos);
            m.action_timer = 0;
        }
        _ => {}
    }
    check_ledge_climb_down(m, w);
    tilt_body_walking(m, start_yaw);
    0
}

fn act_move_punching(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if should_begin_sliding(m, w) {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.action_state == 0 && m.input & INPUT_A_DOWN != 0 {
        return set_mario_action(m, w, ACT_JUMP_KICK, 0);
    }
    m.action_state = 1;
    mario_update_punch_sequence(m, w);
    if m.forward_vel >= 0.0 {
        apply_slope_decel(m, w, 0.5);
    } else {
        m.forward_vel += 8.0;
        if m.forward_vel >= 0.0 {
            m.forward_vel = 0.0;
        }
        apply_slope_accel(m, w);
    }
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 0);
        }
        GROUND_STEP_NONE => m.particle_flags |= PARTICLE_DUST,
        _ => {}
    }
    0
}

fn act_turning_around(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_SIDE_FLIP, 0);
    }
    if m.input & INPUT_UNKNOWN_5 != 0 {
        return set_mario_action(m, w, ACT_BRAKING, 0);
    }
    if !analog_stick_held_back(m) {
        return set_mario_action(m, w, ACT_WALKING, 0);
    }
    if apply_slope_decel(m, w, 2.0) {
        return begin_walking_action(m, w, 8.0, ACT_FINISH_TURNING_AROUND, 0);
    }
    w.play_sound(SOUND_MOVING_TERRAIN_SLIDE.wrapping_add(m.terrain_sound_addend));
    adjust_sound_for_speed(m, w);
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 0);
        }
        GROUND_STEP_NONE => m.particle_flags |= PARTICLE_DUST,
        _ => {}
    }
    if m.forward_vel >= 18.0 {
        set_mario_animation(m, w, MARIO_ANIM_TURNING_PART1);
    } else {
        set_mario_animation(m, w, MARIO_ANIM_TURNING_PART2);
        if is_anim_at_end(m, w) {
            if m.forward_vel > 0.0 {
                let reverse = -m.forward_vel;
                begin_walking_action(m, w, reverse, ACT_WALKING, 0);
            } else {
                begin_walking_action(m, w, 8.0, ACT_WALKING, 0);
            }
        }
    }
    0
}

fn act_finish_turning_around(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_SIDE_FLIP, 0);
    }
    update_walking_speed(m, w);
    set_mario_animation(m, w, MARIO_ANIM_TURNING_PART2);
    if perform_ground_step(m, w) == GROUND_STEP_LEFT_GROUND {
        set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if is_anim_at_end(m, w) {
        set_mario_action(m, w, ACT_WALKING, 0);
    }
    m.obj.gfx.angle[1] = m.obj.gfx.angle[1].wrapping_add(i16::MIN);
    0
}

fn act_braking(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_FIRST_PERSON == 0
        && m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE)
            != 0
    {
        return check_common_action_exits(m, w);
    }
    if apply_slope_decel(m, w, 2.0) {
        return set_mario_action(m, w, ACT_BRAKING_STOP, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_MOVE_PUNCHING, 0);
    }
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 0);
        }
        GROUND_STEP_NONE => m.particle_flags |= PARTICLE_DUST,
        GROUND_STEP_HIT_WALL => slide_bonk(m, w, ACT_BACKWARD_GROUND_KB, ACT_BRAKING_STOP),
        _ => {}
    }
    w.play_sound(SOUND_MOVING_TERRAIN_SLIDE.wrapping_add(m.terrain_sound_addend));
    adjust_sound_for_speed(m, w);
    set_mario_animation(m, w, MARIO_ANIM_SKID_ON_GROUND);
    0
}

fn act_decelerating(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let slope_class = mario_get_floor_class(m, w);
    if m.input & INPUT_FIRST_PERSON == 0 {
        if should_begin_sliding(m, w) {
            return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
        }
        if m.input & INPUT_A_PRESSED != 0 {
            return set_jump_from_landing(m, w);
        }
        if check_ground_dive_or_punch(m, w) != 0 {
            return 1;
        }
        if m.input & INPUT_NONZERO_ANALOG != 0 {
            return set_mario_action(m, w, ACT_WALKING, 0);
        }
        if m.input & INPUT_Z_PRESSED != 0 {
            return set_mario_action(m, w, ACT_CROUCH_SLIDE, 0);
        }
    }
    if update_decelerating_speed(m, w) {
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 0);
        }
        GROUND_STEP_HIT_WALL => {
            if slope_class == SURFACE_CLASS_VERY_SLIPPERY {
                mario_bonk_reflection(m, w, true);
            } else {
                mario_set_forward_vel(m, w, 0.0);
            }
        }
        _ => {}
    }
    if slope_class == SURFACE_CLASS_VERY_SLIPPERY {
        set_mario_animation(m, w, MARIO_ANIM_IDLE_HEAD_LEFT);
        w.play_sound(SOUND_MOVING_TERRAIN_SLIDE.wrapping_add(m.terrain_sound_addend));
        adjust_sound_for_speed(m, w);
        m.particle_flags |= PARTICLE_DUST;
    } else {
        // (Speed crash) above 2^17 speed.
        let mut accel = f32_to_s32(m.forward_vel / 4.0 * 65536.0);
        if accel < 0x1000 {
            accel = 0x1000;
        }
        set_mario_anim_with_accel(m, w, MARIO_ANIM_WALKING, accel);
        play_step_sound(m, w, 10, 49);
    }
    0
}

fn act_crawling(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if should_begin_sliding(m, w) {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_STOP_CRAWLING, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_JUMP, 0);
    }
    if check_ground_dive_or_punch(m, w) != 0 {
        return 1;
    }
    if m.input & INPUT_UNKNOWN_5 != 0 {
        return set_mario_action(m, w, ACT_STOP_CRAWLING, 0);
    }
    if m.input & INPUT_Z_DOWN == 0 {
        return set_mario_action(m, w, ACT_STOP_CRAWLING, 0);
    }
    m.intended_mag *= 0.1;
    update_walking_speed(m, w);
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 0);
        }
        GROUND_STEP_HIT_WALL => {
            if m.forward_vel > 10.0 {
                mario_set_forward_vel(m, w, 10.0);
            }
            // The original falls through into the no-wall case.
            align_with_floor(m, w);
        }
        GROUND_STEP_NONE => align_with_floor(m, w),
        _ => {}
    }
    let accel = f32_to_s32(m.intended_mag * 2.0 * 65536.0);
    set_mario_anim_with_accel(m, w, MARIO_ANIM_CRAWLING, accel);
    play_step_sound(m, w, 26, 79);
    0
}

fn act_burning_ground(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_A_PRESSED != 0 {
        return set_mario_action(m, w, ACT_BURNING_JUMP, 0);
    }
    let burn = m.obj.raw.s32(O_MARIO_BURN_TIMER).wrapping_add(2);
    m.obj.raw.set_s32(O_MARIO_BURN_TIMER, burn);
    if burn > 160 {
        return set_mario_action(m, w, ACT_WALKING, 0);
    }
    if f32::from(m.water_level) - m.floor_height > 50.0 {
        w.play_sound(SOUND_GENERAL_FLAME_OUT);
        return set_mario_action(m, w, ACT_WALKING, 0);
    }
    if m.forward_vel < 8.0 {
        m.forward_vel = 8.0;
    }
    if m.forward_vel > 48.0 {
        m.forward_vel = 48.0;
    }
    m.forward_vel = approach_f32(m.forward_vel, 32.0, 4.0, 1.0);
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        let d_yaw = i32::from(m.intended_yaw.wrapping_sub(m.face_angle[1]));
        m.face_angle[1] = m
            .intended_yaw
            .wrapping_sub(approach_s32(d_yaw, 0, 0x600, 0x600) as i16);
    }
    apply_slope_accel(m, w);
    if perform_ground_step(m, w) == GROUND_STEP_LEFT_GROUND {
        set_mario_action(m, w, ACT_BURNING_FALL, 0);
    }
    let accel = f32_to_s32(m.forward_vel / 2.0 * 65536.0);
    set_mario_anim_with_accel(m, w, MARIO_ANIM_RUNNING, accel);
    play_step_sound(m, w, 9, 45);
    m.particle_flags |= PARTICLE_FIRE;
    w.play_sound(SOUND_MOVING_LAVA_BURN);
    m.health = m.health.wrapping_sub(10);
    if m.health < 0x100 {
        set_mario_action(m, w, ACT_STANDING_DEATH, 0);
    }
    m.body.eye_state = MARIO_EYES_DEAD;
    0
}

/// tilt_body_butt_slide.
fn tilt_body_butt_slide(m: &mut MarioState, w: &StepWorld<'_>) {
    let intended_d_yaw = i32::from(m.intended_yaw.wrapping_sub(m.face_angle[1]));
    m.body.torso_angle[0] =
        f32_to_s32(5461.3335 * m.intended_mag / 32.0 * w.trig.coss(intended_d_yaw)) as i16;
    m.body.torso_angle[2] =
        f32_to_s32(-(5461.3335 * m.intended_mag / 32.0 * w.trig.sins(intended_d_yaw))) as i16;
}

/// common_slide_action.
fn common_slide_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    end_action: u32,
    air_action: u32,
    animation: i32,
) {
    w.play_sound(SOUND_MOVING_TERRAIN_SLIDE.wrapping_add(m.terrain_sound_addend));
    adjust_sound_for_speed(m, w);
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, air_action, 0);
            if m.forward_vel < -50.0 || 50.0 < m.forward_vel {
                w.play_sound(SOUND_MARIO_HOOHOO);
            }
        }
        GROUND_STEP_NONE => {
            set_mario_animation(m, w, animation);
            align_with_floor(m, w);
            m.particle_flags |= PARTICLE_DUST;
        }
        GROUND_STEP_HIT_WALL => {
            if !mario_floor_is_slippery(m, w) {
                if m.forward_vel > 16.0 {
                    m.particle_flags |= PARTICLE_VERTICAL_STAR;
                }
                slide_bonk(m, w, ACT_GROUND_BONK, end_action);
            } else if let Some(wall) = m.wall {
                let normal = w.surface(wall).normal;
                let wall_angle = w.trig.atan2s(normal[2], normal[0]);
                let mut slide_speed =
                    (m.slide_vel_x * m.slide_vel_x + m.slide_vel_z * m.slide_vel_z).sqrt();
                // 0.9 is a double constant.
                slide_speed = (f64::from(slide_speed) * 0.9) as f32;
                if slide_speed < 4.0 {
                    slide_speed = 4.0;
                }
                m.slide_yaw = (i32::from(wall_angle)
                    - i32::from(m.slide_yaw.wrapping_sub(wall_angle))
                    + 0x8000) as i16;
                m.slide_vel_x = slide_speed * w.trig.sins(i32::from(m.slide_yaw));
                m.vel[0] = m.slide_vel_x;
                m.slide_vel_z = slide_speed * w.trig.coss(i32::from(m.slide_yaw));
                m.vel[2] = m.slide_vel_z;
            }
            align_with_floor(m, w);
        }
        _ => {}
    }
}

/// common_slide_action_with_jump.
fn common_slide_action_with_jump(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    stop_action: u32,
    jump_action: u32,
    air_action: u32,
    animation: i32,
) -> i32 {
    if m.action_timer == 5 {
        if m.input & INPUT_A_PRESSED != 0 {
            return set_jumping_action(m, w, jump_action, 0);
        }
    } else {
        m.action_timer = m.action_timer.wrapping_add(1);
    }
    if update_sliding(m, w, 4.0) {
        return set_mario_action(m, w, stop_action, 0);
    }
    common_slide_action(m, w, stop_action, air_action, animation);
    0
}

fn act_butt_slide(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let cancel = common_slide_action_with_jump(
        m,
        w,
        ACT_BUTT_SLIDE_STOP,
        ACT_JUMP,
        ACT_BUTT_SLIDE_AIR,
        MARIO_ANIM_SLIDE,
    );
    tilt_body_butt_slide(m, w);
    cancel
}

fn act_crouch_slide(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BUTT_SLIDE, 0);
    }
    if m.action_timer < 30 {
        m.action_timer += 1;
        if m.input & INPUT_A_PRESSED != 0 && m.forward_vel > 10.0 {
            return set_jumping_action(m, w, ACT_LONG_JUMP, 0);
        }
    }
    if m.input & INPUT_B_PRESSED != 0 {
        if m.forward_vel >= 10.0 {
            return set_mario_action(m, w, ACT_SLIDE_KICK, 0);
        }
        return set_mario_action(m, w, ACT_MOVE_PUNCHING, 0x0009);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_JUMP, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_BRAKING, 0);
    }
    common_slide_action_with_jump(
        m,
        w,
        ACT_CROUCHING,
        ACT_JUMP,
        ACT_FREEFALL,
        MARIO_ANIM_START_CROUCHING,
    )
}

fn act_slide_kick_slide(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_FORWARD_ROLLOUT, 0);
    }
    set_mario_animation(m, w, MARIO_ANIM_SLIDE_KICK);
    if is_anim_at_end(m, w) && m.forward_vel < 1.0 {
        return set_mario_action(m, w, ACT_SLIDE_KICK_SLIDE_STOP, 0);
    }
    update_sliding(m, w, 1.0);
    match perform_ground_step(m, w) {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, ACT_FREEFALL, 2);
        }
        GROUND_STEP_HIT_WALL => {
            mario_bonk_reflection(m, w, true);
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            set_mario_action(m, w, ACT_BACKWARD_GROUND_KB, 0);
        }
        _ => {}
    }
    w.play_sound(SOUND_MOVING_TERRAIN_SLIDE.wrapping_add(m.terrain_sound_addend));
    m.particle_flags |= PARTICLE_DUST;
    0
}

/// stomach_slide_action.
fn stomach_slide_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    stop_action: u32,
    air_action: u32,
    animation: i32,
) -> i32 {
    if m.action_timer == 5 {
        if m.input & INPUT_ABOVE_SLIDE == 0 && m.input & (INPUT_A_PRESSED | INPUT_B_PRESSED) != 0 {
            let rollout = if m.forward_vel >= 0.0 {
                ACT_FORWARD_ROLLOUT
            } else {
                ACT_BACKWARD_ROLLOUT
            };
            return drop_and_set_mario_action(m, w, rollout, 0);
        }
    } else {
        m.action_timer = m.action_timer.wrapping_add(1);
    }
    if update_sliding(m, w, 4.0) {
        return set_mario_action(m, w, stop_action, 0);
    }
    common_slide_action(m, w, stop_action, air_action, animation);
    0
}

fn act_dive_slide(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_ABOVE_SLIDE == 0 && m.input & (INPUT_A_PRESSED | INPUT_B_PRESSED) != 0 {
        let rollout = if m.forward_vel > 0.0 {
            ACT_FORWARD_ROLLOUT
        } else {
            ACT_BACKWARD_ROLLOUT
        };
        return set_mario_action(m, w, rollout, 0);
    }
    play_mario_landing_sound_once(m, w, SOUND_ACTION_TERRAIN_BODY_HIT_GROUND);
    // A dive slide ending on the frame an object is grabbed skips the dive
    // pickup action.
    if update_sliding(m, w, 8.0) && is_anim_at_end(m, w) {
        mario_set_forward_vel(m, w, 0.0);
        set_mario_action(m, w, ACT_STOMACH_SLIDE_STOP, 0);
    }
    if mario_check_object_grab(m) {
        unreachable!("object grabs need objects");
    }
    common_slide_action(m, w, ACT_STOMACH_SLIDE_STOP, ACT_FREEFALL, MARIO_ANIM_DIVE);
    0
}

/// common_ground_knockback_action: returns the animation frame.
fn common_ground_knockback_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    animation: i32,
    arg2: i32,
    arg3: bool,
    arg4: i32,
) -> i32 {
    if arg3 {
        play_mario_heavy_landing_sound_once(m, w, SOUND_ACTION_TERRAIN_BODY_HIT_GROUND);
    }
    if arg4 > 0 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_ATTACKED, MARIO_MARIO_SOUND_PLAYED);
    } else {
        play_sound_if_no_flag(m, w, SOUND_MARIO_OOOF2, MARIO_MARIO_SOUND_PLAYED);
    }
    if m.forward_vel > 32.0 {
        m.forward_vel = 32.0;
    }
    if m.forward_vel < -32.0 {
        m.forward_vel = -32.0;
    }
    let anim_frame = i32::from(set_mario_animation(m, w, animation));
    if anim_frame < arg2 {
        apply_landing_accel(m, w, 0.9);
    } else if m.forward_vel >= 0.0 {
        mario_set_forward_vel(m, w, 0.1);
    } else {
        mario_set_forward_vel(m, w, -0.1);
    }
    if perform_ground_step(m, w) == GROUND_STEP_LEFT_GROUND {
        let action = if m.forward_vel >= 0.0 {
            ACT_FORWARD_AIR_KB
        } else {
            ACT_BACKWARD_AIR_KB
        };
        set_mario_action(m, w, action, arg4 as u32);
    } else if is_anim_at_end(m, w) {
        if m.health < 0x100 {
            set_mario_action(m, w, ACT_STANDING_DEATH, 0);
        } else {
            if arg4 > 0 {
                m.invinc_timer = 30;
            }
            set_mario_action(m, w, ACT_IDLE, 0);
        }
    }
    anim_frame
}

fn act_hard_backward_ground_kb(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let arg = m.action_arg as i32;
    let anim_frame =
        common_ground_knockback_action(m, w, MARIO_ANIM_FALL_OVER_BACKWARDS, 43, true, arg);
    if anim_frame == 43 && m.health < 0x100 {
        set_mario_action(m, w, ACT_DEATH_ON_BACK, 0);
    }
    if anim_frame == 54 && m.prev_action == ACT_SPECIAL_DEATH_EXIT {
        w.play_sound(SOUND_MARIO_MAMA_MIA);
    }
    if anim_frame == 69 {
        play_mario_landing_sound_once(m, w, SOUND_ACTION_TERRAIN_LANDING);
    }
    0
}

fn act_hard_forward_ground_kb(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let arg = m.action_arg as i32;
    let anim_frame =
        common_ground_knockback_action(m, w, MARIO_ANIM_LAND_ON_STOMACH, 21, true, arg);
    if anim_frame == 23 && m.health < 0x100 {
        set_mario_action(m, w, ACT_DEATH_ON_STOMACH, 0);
    }
    0
}

fn act_ground_bonk(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let arg = m.action_arg as i32;
    let anim_frame = common_ground_knockback_action(m, w, MARIO_ANIM_GROUND_BONK, 32, true, arg);
    if anim_frame == 32 {
        play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
    }
    0
}

fn act_death_exit_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    apply_landing_accel(m, w, 0.9);
    play_mario_heavy_landing_sound_once(m, w, SOUND_ACTION_TERRAIN_BODY_HIT_GROUND);
    let anim_frame = set_mario_animation(m, w, MARIO_ANIM_FALL_OVER_BACKWARDS);
    if anim_frame == 54 {
        w.play_sound(SOUND_MARIO_MAMA_MIA);
    }
    if anim_frame == 68 {
        play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
    }
    if is_anim_at_end(m, w) {
        set_mario_action(m, w, ACT_IDLE, 0);
    }
    0
}

/// common_landing_action: returns the ground step result.
fn common_landing_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    animation: i16,
    air_action: u32,
) -> u32 {
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        apply_landing_accel(m, w, 0.98);
    } else if m.forward_vel >= 16.0 {
        apply_slope_decel(m, w, 2.0);
    } else {
        m.vel[1] = 0.0;
    }
    let step_result = perform_ground_step(m, w);
    match step_result {
        GROUND_STEP_LEFT_GROUND => {
            set_mario_action(m, w, air_action, 0);
        }
        GROUND_STEP_HIT_WALL => {
            set_mario_animation(m, w, MARIO_ANIM_PUSHING);
        }
        _ => {}
    }
    if m.forward_vel > 16.0 {
        m.particle_flags |= PARTICLE_DUST;
    }
    set_mario_animation(m, w, i32::from(animation));
    play_mario_landing_sound_once(m, w, SOUND_ACTION_TERRAIN_LANDING);
    let floor_type = w.surface(m.floor.expect("landing floor")).surface_type;
    if (SURFACE_SHALLOW_QUICKSAND..=SURFACE_MOVING_QUICKSAND).contains(&floor_type) {
        m.quicksand_depth += (4 - i32::from(m.action_timer)) as f32 * 3.5 - 0.5;
    }
    step_result
}

type APressAction = fn(&mut MarioState, &mut StepWorld<'_>, u32, u32) -> i32;

/// common_landing_cancels. Checks run before Mario is known to be grounded
/// (remote sliding).
fn common_landing_cancels(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    landing: &LandingAction,
    set_a_press_action: APressAction,
) -> i32 {
    if floor_normal(m, w)[1] < 0.2923717 {
        return mario_push_off_steep_floor(m, w, landing.very_steep_action, 0);
    }
    m.double_jump_timer = landing.unk02 as u8;
    if should_begin_sliding(m, w) {
        return set_mario_action(m, w, landing.slide_action, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, landing.end_action, 0);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if i32::from(m.action_timer) >= i32::from(landing.num_frames) {
        return set_mario_action(m, w, landing.end_action, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_a_press_action(m, w, landing.a_pressed_action, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, landing.off_floor_action, 0);
    }
    0
}

fn landing(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    action: &LandingAction,
    a_press: APressAction,
    animation: i32,
) -> i32 {
    if common_landing_cancels(m, w, action, a_press) != 0 {
        return 1;
    }
    common_landing_action(m, w, animation as i16, ACT_FREEFALL);
    0
}

fn act_side_flip_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if common_landing_cancels(m, w, &SIDE_FLIP_LAND, set_jumping_action) != 0 {
        return 1;
    }
    if common_landing_action(m, w, MARIO_ANIM_SLIDEFLIP_LAND as i16, ACT_FREEFALL)
        != GROUND_STEP_HIT_WALL
    {
        m.obj.gfx.angle[1] = m.obj.gfx.angle[1].wrapping_add(i16::MIN);
    }
    0
}

fn act_long_jump_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    // US: no BLJ fix.
    if m.input & INPUT_Z_DOWN == 0 {
        m.input &= !INPUT_A_PRESSED;
    }
    if common_landing_cancels(m, w, &LONG_JUMP_LAND, set_jumping_action) != 0 {
        return 1;
    }
    if m.input & INPUT_NONZERO_ANALOG == 0 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_UH2_2, MARIO_MARIO_SOUND_PLAYED);
    }
    let anim = if m.obj.raw.s32(O_MARIO_LONG_JUMP_IS_SLOW) == 0 {
        MARIO_ANIM_CROUCH_FROM_FAST_LONGJUMP
    } else {
        MARIO_ANIM_CROUCH_FROM_SLOW_LONGJUMP
    };
    common_landing_action(m, w, anim as i16, ACT_FREEFALL);
    0
}

fn act_triple_jump_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.input &= !INPUT_A_PRESSED;
    if common_landing_cancels(m, w, &TRIPLE_JUMP_LAND, set_jumping_action) != 0 {
        return 1;
    }
    if m.input & INPUT_NONZERO_ANALOG == 0 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_HAHA, MARIO_MARIO_SOUND_PLAYED);
    }
    common_landing_action(m, w, MARIO_ANIM_TRIPLE_JUMP_LAND as i16, ACT_FREEFALL);
    0
}

fn act_backflip_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_Z_DOWN == 0 {
        m.input &= !INPUT_A_PRESSED;
    }
    if common_landing_cancels(m, w, &BACKFLIP_LAND, set_jumping_action) != 0 {
        return 1;
    }
    if m.input & INPUT_NONZERO_ANALOG == 0 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_HAHA, MARIO_MARIO_SOUND_PLAYED);
    }
    common_landing_action(m, w, MARIO_ANIM_TRIPLE_JUMP_LAND as i16, ACT_FREEFALL);
    0
}

/// quicksand_jump_land_action.
fn quicksand_jump_land_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    animation1: i32,
    animation2: i32,
    end_action: u32,
    air_action: u32,
) -> i32 {
    let timer = m.action_timer;
    m.action_timer = m.action_timer.wrapping_add(1);
    if timer < 6 {
        m.quicksand_depth -= (7 - i32::from(m.action_timer)) as f32 * 0.8;
        if m.quicksand_depth < 1.0 {
            m.quicksand_depth = 1.1;
        }
        play_mario_jump_sound(m, w);
        set_mario_animation(m, w, animation1);
    } else {
        if m.action_timer >= 13 {
            return set_mario_action(m, w, end_action, 0);
        }
        set_mario_animation(m, w, animation2);
    }
    apply_landing_accel(m, w, 0.95);
    if perform_ground_step(m, w) == GROUND_STEP_LEFT_GROUND {
        set_mario_action(m, w, air_action, 0);
    }
    0
}

/// check_common_moving_cancels.
fn check_common_moving_cancels(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.pos[1] < (i32::from(m.water_level) - 100) as f32 {
        return set_water_plunge_action(m, w);
    }
    if m.action & ACT_FLAG_INVULNERABLE == 0 && m.input & INPUT_STOMPED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_SQUISHED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SQUISHED, 0);
    }
    if m.action & ACT_FLAG_INVULNERABLE == 0 && m.health < 0x100 {
        return drop_and_set_mario_action(m, w, ACT_STANDING_DEATH, 0);
    }
    0
}

/// mario_execute_moving_action.
pub fn mario_execute_moving_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_moving_cancels(m, w) != 0 {
        return 1;
    }
    if mario_update_quicksand(m, w, 0.25) != 0 {
        return 1;
    }
    let arg = m.action_arg as i32;
    let cancel = match m.action {
        ACT_WALKING => act_walking(m, w),
        ACT_TURNING_AROUND => act_turning_around(m, w),
        ACT_FINISH_TURNING_AROUND => act_finish_turning_around(m, w),
        ACT_BRAKING => act_braking(m, w),
        ACT_CRAWLING => act_crawling(m, w),
        ACT_BURNING_GROUND => act_burning_ground(m, w),
        ACT_DECELERATING => act_decelerating(m, w),
        ACT_BUTT_SLIDE => act_butt_slide(m, w),
        ACT_STOMACH_SLIDE => stomach_slide_action(
            m,
            w,
            ACT_STOMACH_SLIDE_STOP,
            ACT_FREEFALL,
            MARIO_ANIM_SLIDE_DIVE,
        ),
        ACT_DIVE_SLIDE => act_dive_slide(m, w),
        ACT_MOVE_PUNCHING => act_move_punching(m, w),
        ACT_CROUCH_SLIDE => act_crouch_slide(m, w),
        ACT_SLIDE_KICK_SLIDE => act_slide_kick_slide(m, w),
        ACT_HARD_BACKWARD_GROUND_KB => act_hard_backward_ground_kb(m, w),
        ACT_HARD_FORWARD_GROUND_KB => act_hard_forward_ground_kb(m, w),
        ACT_BACKWARD_GROUND_KB => {
            common_ground_knockback_action(m, w, MARIO_ANIM_BACKWARD_KB, 22, true, arg);
            0
        }
        ACT_FORWARD_GROUND_KB => {
            common_ground_knockback_action(m, w, MARIO_ANIM_FORWARD_KB, 20, true, arg);
            0
        }
        ACT_SOFT_BACKWARD_GROUND_KB => {
            common_ground_knockback_action(m, w, MARIO_ANIM_SOFT_BACK_KB, 100, false, arg);
            0
        }
        ACT_SOFT_FORWARD_GROUND_KB => {
            common_ground_knockback_action(m, w, MARIO_ANIM_SOFT_FRONT_KB, 100, false, arg);
            0
        }
        ACT_GROUND_BONK => act_ground_bonk(m, w),
        ACT_DEATH_EXIT_LAND => act_death_exit_land(m, w),
        ACT_JUMP_LAND => landing(
            m,
            w,
            &JUMP_LAND,
            set_jumping_action,
            MARIO_ANIM_LAND_FROM_SINGLE_JUMP,
        ),
        ACT_FREEFALL_LAND => landing(
            m,
            w,
            &FREEFALL_LAND,
            set_jumping_action,
            MARIO_ANIM_GENERAL_LAND,
        ),
        ACT_DOUBLE_JUMP_LAND => landing(
            m,
            w,
            &DOUBLE_JUMP_LAND,
            set_triple_jump_action,
            MARIO_ANIM_LAND_FROM_DOUBLE_JUMP,
        ),
        ACT_SIDE_FLIP_LAND => act_side_flip_land(m, w),
        ACT_TRIPLE_JUMP_LAND => act_triple_jump_land(m, w),
        ACT_BACKFLIP_LAND => act_backflip_land(m, w),
        ACT_QUICKSAND_JUMP_LAND => quicksand_jump_land_action(
            m,
            w,
            MARIO_ANIM_SINGLE_JUMP,
            MARIO_ANIM_LAND_FROM_SINGLE_JUMP,
            ACT_JUMP_LAND_STOP,
            ACT_FREEFALL,
        ),
        ACT_LONG_JUMP_LAND => act_long_jump_land(m, w),
        ACT_HOLD_WALKING
        | ACT_HOLD_HEAVY_WALKING
        | ACT_HOLD_DECELERATING
        | ACT_RIDING_SHELL_GROUND
        | ACT_HOLD_BUTT_SLIDE
        | ACT_HOLD_STOMACH_SLIDE
        | ACT_HOLD_JUMP_LAND
        | ACT_HOLD_FREEFALL_LAND
        | ACT_HOLD_QUICKSAND_JUMP_LAND => {
            panic!(
                "moving action {:#X} holds or rides an object; objects are not simulated yet",
                m.action
            )
        }
        action => panic!("moving action {action:#X} is not in the original table"),
    };
    if cancel == 0 && m.input & INPUT_IN_WATER != 0 {
        m.particle_flags |= PARTICLE_WAVE_TRAIL;
        m.particle_flags &= !PARTICLE_DUST;
    }
    cancel
}
