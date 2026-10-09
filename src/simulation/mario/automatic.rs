//! Automatic actions, translated from pinned CC0
//! src/game/mario_actions_automatic.c: hanging and ledge actions. Poles,
//! cannons, tornadoes and being grabbed use an object and panic where the
//! original reads it, until objects are simulated.
use super::{
    MarioState, StepWorld, SurfaceRef,
    animation::{is_anim_at_end, is_anim_past_end, set_mario_animation},
    constants::*,
    core::{
        check_common_action_exits, find_floor_height_relative_polar, play_mario_landing_sound,
        play_sound_if_no_flag, set_mario_action, set_water_plunge_action,
    },
    step::{resolve_and_return_wall_collisions, stop_and_set_height_to_floor, vec3f_find_ceil},
};
use crate::simulation::math::approach_s32;

const HANG_NONE: i32 = 0;
const HANG_HIT_CEIL_OR_OOB: i32 = 1;
const HANG_LEFT_CEIL: i32 = 2;

/// The referenced ceiling's type. A NULL ceiling crashes the original.
fn ceil_type(m: &MarioState, w: &StepWorld<'_>) -> i16 {
    w.surface(
        m.ceil
            .expect("Mario's referenced ceiling is NULL (the original crashes)"),
    )
    .surface_type
}

/// perform_hanging_step.
fn perform_hanging_step(m: &mut MarioState, w: &mut StepWorld<'_>, next_pos: &mut [f32; 3]) -> i32 {
    m.wall = resolve_and_return_wall_collisions(m, w, next_pos, 50.0, 50.0);
    let (floor_height, floor) = w.collision.find_floor(
        next_pos[0],
        next_pos[1],
        next_pos[2],
        &mut w.collision_flags,
    );
    let (ceil_height, ceil) = vec3f_find_ceil(w, *next_pos, floor_height);
    let Some(floor) = floor else {
        return HANG_HIT_CEIL_OR_OOB;
    };
    let Some(ceil) = ceil else {
        return HANG_LEFT_CEIL;
    };
    if ceil_height - floor_height <= 160.0 {
        return HANG_HIT_CEIL_OR_OOB;
    }
    if w.collision.surface(ceil).surface_type != SURFACE_HANGABLE {
        return HANG_LEFT_CEIL;
    }
    let ceil_offset = ceil_height - (next_pos[1] + 160.0);
    if ceil_offset < -30.0 {
        return HANG_HIT_CEIL_OR_OOB;
    }
    if ceil_offset > 30.0 {
        return HANG_LEFT_CEIL;
    }
    // Uses the previous ceiling height, not the one just found.
    next_pos[1] = m.ceil_height - 160.0;
    m.pos = *next_pos;
    m.floor = Some(SurfaceRef::Collision(floor));
    m.floor_height = floor_height;
    m.ceil = Some(SurfaceRef::Collision(ceil));
    m.ceil_height = ceil_height;
    HANG_NONE
}

/// update_hang_moving.
fn update_hang_moving(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let max_speed = 4.0;
    m.forward_vel += 1.0;
    if m.forward_vel > max_speed {
        m.forward_vel = max_speed;
    }
    let d_yaw = i32::from(m.intended_yaw.wrapping_sub(m.face_angle[1]));
    m.face_angle[1] = (i32::from(m.intended_yaw) - approach_s32(d_yaw, 0, 0x800, 0x800)) as i16;
    m.slide_yaw = m.face_angle[1];
    let yaw = i32::from(m.face_angle[1]);
    m.slide_vel_x = m.forward_vel * w.trig.sins(yaw);
    m.slide_vel_z = m.forward_vel * w.trig.coss(yaw);
    m.vel = [m.slide_vel_x, 0.0, m.slide_vel_z];
    let ceil_normal_y = w
        .surface(
            m.ceil
                .expect("Mario's referenced ceiling is NULL (the original crashes)"),
        )
        .normal[1];
    let mut next_pos = [
        m.pos[0] - ceil_normal_y * m.vel[0],
        m.pos[1],
        m.pos[2] - ceil_normal_y * m.vel[2],
    ];
    let step_result = perform_hanging_step(m, w, &mut next_pos);
    m.obj.gfx.pos = m.pos;
    m.obj.gfx.angle = [0, m.face_angle[1], 0];
    step_result
}

/// update_hang_stationary.
fn update_hang_stationary(m: &mut MarioState) {
    m.forward_vel = 0.0;
    m.slide_vel_x = 0.0;
    m.slide_vel_z = 0.0;
    m.pos[1] = m.ceil_height - 160.0;
    m.vel = [0.0; 3];
    m.obj.gfx.pos = m.pos;
}

fn act_start_hanging(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.input & INPUT_NONZERO_ANALOG != 0 && m.action_timer >= 31 {
        return set_mario_action(m, w, ACT_HANGING, 0);
    }
    if m.input & INPUT_A_DOWN == 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    if ceil_type(m, w) != SURFACE_HANGABLE {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    set_mario_animation(m, w, MARIO_ANIM_HANG_ON_CEILING);
    play_sound_if_no_flag(m, w, SOUND_ACTION_HANGING_STEP, MARIO_ACTION_SOUND_PLAYED);
    update_hang_stationary(m);
    if is_anim_at_end(m, w) {
        set_mario_action(m, w, ACT_HANGING, 0);
    }
    0
}

fn act_hanging(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        let arg = m.action_arg;
        return set_mario_action(m, w, ACT_HANG_MOVING, arg);
    }
    if m.input & INPUT_A_DOWN == 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    if ceil_type(m, w) != SURFACE_HANGABLE {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    let animation = if m.action_arg & 1 != 0 {
        MARIO_ANIM_HANDSTAND_LEFT
    } else {
        MARIO_ANIM_HANDSTAND_RIGHT
    };
    set_mario_animation(m, w, animation);
    update_hang_stationary(m);
    0
}

fn act_hang_moving(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_A_DOWN == 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    if ceil_type(m, w) != SURFACE_HANGABLE {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    let animation = if m.action_arg & 1 != 0 {
        MARIO_ANIM_MOVE_ON_WIRE_NET_RIGHT
    } else {
        MARIO_ANIM_MOVE_ON_WIRE_NET_LEFT
    };
    set_mario_animation(m, w, animation);
    if m.obj.gfx.anim.anim_frame == 12 {
        w.play_sound(SOUND_ACTION_HANGING_STEP);
    }
    if is_anim_past_end(m, w) {
        m.action_arg ^= 1;
        if m.input & INPUT_UNKNOWN_5 != 0 {
            let arg = m.action_arg;
            return set_mario_action(m, w, ACT_HANGING, arg);
        }
    }
    if update_hang_moving(m, w) == HANG_LEFT_CEIL {
        set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    0
}

/// let_go_of_ledge.
fn let_go_of_ledge(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.vel[1] = 0.0;
    m.forward_vel = -8.0;
    let yaw = i32::from(m.face_angle[1]);
    m.pos[0] -= 60.0 * w.trig.sins(yaw);
    m.pos[2] -= 60.0 * w.trig.coss(yaw);
    let (floor_height, _floor) =
        w.collision
            .find_floor(m.pos[0], m.pos[1], m.pos[2], &mut w.collision_flags);
    if floor_height < m.pos[1] - 100.0 {
        m.pos[1] -= 100.0;
    } else {
        m.pos[1] = floor_height;
    }
    set_mario_action(m, w, ACT_SOFT_BONK, 0)
}

/// climb_up_ledge.
fn climb_up_ledge(m: &mut MarioState, w: &mut StepWorld<'_>) {
    set_mario_animation(m, w, MARIO_ANIM_IDLE_HEAD_LEFT);
    let yaw = i32::from(m.face_angle[1]);
    m.pos[0] += 14.0 * w.trig.sins(yaw);
    m.pos[2] += 14.0 * w.trig.coss(yaw);
    m.obj.gfx.pos = m.pos;
}

/// update_ledge_climb_camera.
fn update_ledge_climb_camera(m: &mut MarioState, w: &StepWorld<'_>) {
    let distance = if m.action_timer < 14 {
        f32::from(m.action_timer)
    } else {
        14.0
    };
    let yaw = i32::from(m.face_angle[1]);
    m.camera_status.pos[0] = m.pos[0] + distance * w.trig.sins(yaw);
    m.camera_status.pos[2] = m.pos[2] + distance * w.trig.coss(yaw);
    m.camera_status.pos[1] = m.pos[1];
    m.action_timer = m.action_timer.wrapping_add(1);
    m.flags |= MARIO_UNKNOWN_25;
}

/// update_ledge_climb.
fn update_ledge_climb(m: &mut MarioState, w: &mut StepWorld<'_>, animation: i32, end_action: u32) {
    stop_and_set_height_to_floor(m, w);
    set_mario_animation(m, w, animation);
    if is_anim_at_end(m, w) {
        set_mario_action(m, w, end_action, 0);
        if end_action == ACT_IDLE {
            climb_up_ledge(m, w);
        }
    }
}

fn act_ledge_grab(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let intended_d_yaw = m.intended_yaw.wrapping_sub(m.face_angle[1]);
    let has_space_for_mario = m.ceil_height - m.floor_height >= 160.0;
    if m.action_timer < 10 {
        m.action_timer += 1;
    }
    let floor = m
        .floor
        .expect("Mario has no floor (the original dereferences NULL)");
    if w.surface(floor).normal[1] < 0.9063078 {
        return let_go_of_ledge(m, w);
    }
    if m.input & (INPUT_Z_PRESSED | INPUT_OFF_FLOOR) != 0 {
        return let_go_of_ledge(m, w);
    }
    if m.input & INPUT_A_PRESSED != 0 && has_space_for_mario {
        return set_mario_action(m, w, ACT_LEDGE_CLIMB_FAST, 0);
    }
    if m.input & INPUT_STOMPED != 0 {
        if m.obj.raw.u32(O_INTERACT_STATUS) & INT_STATUS_MARIO_KNOCKBACK_DMG != 0 {
            let damage = if m.flags & MARIO_CAP_ON_HEAD != 0 {
                12
            } else {
                18
            };
            m.hurt_counter = m.hurt_counter.wrapping_add(damage);
        }
        return let_go_of_ledge(m, w);
    }
    if m.action_timer == 10 && m.input & INPUT_NONZERO_ANALOG != 0 {
        if (-0x4000..=0x4000).contains(&intended_d_yaw) {
            if has_space_for_mario {
                return set_mario_action(m, w, ACT_LEDGE_CLIMB_SLOW_1, 0);
            }
        } else {
            return let_go_of_ledge(m, w);
        }
    }
    let height_above_floor = m.pos[1] - find_floor_height_relative_polar(m, w, i16::MIN, 30.0);
    if has_space_for_mario && height_above_floor < 100.0 {
        return set_mario_action(m, w, ACT_LEDGE_CLIMB_FAST, 0);
    }
    if m.action_arg == 0 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_WHOA, MARIO_MARIO_SOUND_PLAYED);
    }
    stop_and_set_height_to_floor(m, w);
    set_mario_animation(m, w, MARIO_ANIM_IDLE_ON_LEDGE);
    0
}

fn act_ledge_climb_slow(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_OFF_FLOOR != 0 {
        return let_go_of_ledge(m, w);
    }
    if m.action_timer >= 28
        && m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE)
            != 0
    {
        climb_up_ledge(m, w);
        return check_common_action_exits(m, w);
    }
    if m.action_timer == 10 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_EEUH, MARIO_MARIO_SOUND_PLAYED);
    }
    update_ledge_climb(m, w, MARIO_ANIM_SLOW_LEDGE_GRAB, ACT_IDLE);
    update_ledge_climb_camera(m, w);
    if m.obj.gfx.anim.anim_frame == 17 {
        m.action = ACT_LEDGE_CLIMB_SLOW_2;
    }
    0
}

fn act_ledge_climb_down(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_OFF_FLOOR != 0 {
        return let_go_of_ledge(m, w);
    }
    play_sound_if_no_flag(m, w, SOUND_MARIO_WHOA, MARIO_MARIO_SOUND_PLAYED);
    update_ledge_climb(m, w, MARIO_ANIM_CLIMB_DOWN_LEDGE, ACT_LEDGE_GRAB);
    m.action_arg = 1;
    0
}

fn act_ledge_climb_fast(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_OFF_FLOOR != 0 {
        return let_go_of_ledge(m, w);
    }
    play_sound_if_no_flag(m, w, SOUND_MARIO_UH2, MARIO_MARIO_SOUND_PLAYED);
    update_ledge_climb(m, w, MARIO_ANIM_FAST_LEDGE_GRAB, ACT_IDLE);
    if m.obj.gfx.anim.anim_frame == 8 {
        play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
    }
    update_ledge_climb_camera(m, w);
    0
}

fn act_grabbed(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.obj.raw.u32(O_INTERACT_STATUS) & INT_STATUS_MARIO_UNK2 != 0 {
        panic!("being thrown reads the grabbing object; objects are not simulated yet");
    }
    set_mario_animation(m, w, MARIO_ANIM_BEING_GRABBED);
    0
}

/// check_common_automatic_cancels.
fn check_common_automatic_cancels(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.pos[1] < (i32::from(m.water_level) - 100) as f32 {
        return set_water_plunge_action(m, w);
    }
    0
}

/// mario_execute_automatic_action.
pub fn mario_execute_automatic_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_automatic_cancels(m, w) != 0 {
        return 1;
    }
    m.quicksand_depth = 0.0;
    match m.action {
        ACT_START_HANGING => act_start_hanging(m, w),
        ACT_HANGING => act_hanging(m, w),
        ACT_HANG_MOVING => act_hang_moving(m, w),
        ACT_LEDGE_GRAB => act_ledge_grab(m, w),
        ACT_LEDGE_CLIMB_SLOW_1 | ACT_LEDGE_CLIMB_SLOW_2 => act_ledge_climb_slow(m, w),
        ACT_LEDGE_CLIMB_DOWN => act_ledge_climb_down(m, w),
        ACT_LEDGE_CLIMB_FAST => act_ledge_climb_fast(m, w),
        ACT_GRABBED => act_grabbed(m, w),
        ACT_HOLDING_POLE
        | ACT_GRAB_POLE_SLOW
        | ACT_GRAB_POLE_FAST
        | ACT_CLIMBING_POLE
        | ACT_TOP_OF_POLE_TRANSITION
        | ACT_TOP_OF_POLE
        | ACT_IN_CANNON
        | ACT_TORNADO_TWIRLING => {
            panic!(
                "automatic action {:#X} uses an object; objects are not simulated yet",
                m.action
            )
        }
        action => panic!("automatic action {action:#X} is not in the original table"),
    }
}
