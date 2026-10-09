//! Object-group actions, translated from pinned CC0
//! src/game/mario_actions_object.c. Punching and the stomach-slide stop need
//! no object; picking up, placing, throwing and the Bowser swing hold one and
//! panic until objects are simulated.
use super::{
    MarioState, StepWorld,
    animation::{is_anim_at_end, is_anim_past_end, set_mario_animation},
    constants::*,
    core::{
        check_common_action_exits, drop_and_set_mario_action, play_mario_action_sound,
        set_mario_action, set_water_plunge_action,
    },
    interaction::mario_check_object_grab,
    step::{
        mario_set_forward_vel, mario_update_quicksand, perform_ground_step, stationary_ground_step,
    },
};

/// sPunchingForwardVelocities.
const PUNCHING_FORWARD_VELOCITIES: [i8; 8] = [0, 1, 1, 2, 3, 5, 7, 10];

/// animated_stationary_ground_step.
pub fn animated_stationary_ground_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    animation: i32,
    end_action: u32,
) {
    stationary_ground_step(m, w);
    set_mario_animation(m, w, animation);
    if is_anim_at_end(m, w) {
        set_mario_action(m, w, end_action, 0);
    }
}

/// mario_update_punch_sequence: TRUE when a punch grabbed an object.
pub fn mario_update_punch_sequence(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let (end_action, crouch_end_action) = if m.action & ACT_FLAG_MOVING != 0 {
        (ACT_WALKING, ACT_CROUCH_SLIDE)
    } else {
        (ACT_IDLE, ACT_CROUCHING)
    };
    match m.action_arg {
        0 | 1 => {
            if m.action_arg == 0 {
                w.play_sound(SOUND_MARIO_PUNCH_YAH);
            }
            set_mario_animation(m, w, MARIO_ANIM_FIRST_PUNCH);
            m.action_arg = if is_anim_past_end(m, w) { 2 } else { 1 };
            if m.obj.gfx.anim.anim_frame >= 2 {
                if mario_check_object_grab(m) {
                    return 1;
                }
                m.flags |= MARIO_PUNCHING;
            }
            if m.action_arg == 2 {
                m.body.punch_state = 4;
            }
        }
        2 => {
            set_mario_animation(m, w, MARIO_ANIM_FIRST_PUNCH_FAST);
            if m.obj.gfx.anim.anim_frame <= 0 {
                m.flags |= MARIO_PUNCHING;
            }
            if m.input & INPUT_B_PRESSED != 0 {
                m.action_arg = 3;
            }
            if is_anim_at_end(m, w) {
                set_mario_action(m, w, end_action, 0);
            }
        }
        3 | 4 => {
            if m.action_arg == 3 {
                w.play_sound(SOUND_MARIO_PUNCH_WAH);
            }
            set_mario_animation(m, w, MARIO_ANIM_SECOND_PUNCH);
            m.action_arg = if is_anim_past_end(m, w) { 5 } else { 4 };
            if m.obj.gfx.anim.anim_frame > 0 {
                m.flags |= MARIO_PUNCHING;
            }
            if m.action_arg == 5 {
                m.body.punch_state = (1 << 6) | 4;
            }
        }
        5 => {
            set_mario_animation(m, w, MARIO_ANIM_SECOND_PUNCH_FAST);
            if m.obj.gfx.anim.anim_frame <= 0 {
                m.flags |= MARIO_PUNCHING;
            }
            if m.input & INPUT_B_PRESSED != 0 {
                m.action_arg = 6;
            }
            if is_anim_at_end(m, w) {
                set_mario_action(m, w, end_action, 0);
            }
        }
        6 => {
            play_mario_action_sound(m, w, SOUND_MARIO_PUNCH_HOO, 1);
            let anim_frame = i32::from(set_mario_animation(m, w, MARIO_ANIM_GROUND_KICK));
            if anim_frame == 0 {
                m.body.punch_state = (2 << 6) | 6;
            }
            if (0..8).contains(&anim_frame) {
                m.flags |= MARIO_KICKING;
            }
            if is_anim_at_end(m, w) {
                set_mario_action(m, w, end_action, 0);
            }
        }
        9 => {
            play_mario_action_sound(m, w, SOUND_MARIO_PUNCH_HOO, 1);
            set_mario_animation(m, w, MARIO_ANIM_BREAKDANCE);
            let anim_frame = i32::from(m.obj.gfx.anim.anim_frame);
            if (2..8).contains(&anim_frame) {
                m.flags |= MARIO_TRIPPING;
            }
            if is_anim_at_end(m, w) {
                set_mario_action(m, w, crouch_end_action, 0);
            }
        }
        _ => {}
    }
    0
}

fn act_punching(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE) != 0
    {
        return check_common_action_exits(m, w);
    }
    if m.action_state == 0 && m.input & INPUT_A_DOWN != 0 {
        return set_mario_action(m, w, ACT_JUMP_KICK, 0);
    }
    m.action_state = 1;
    if m.action_arg == 0 {
        m.action_timer = 7;
    }
    let velocity = *PUNCHING_FORWARD_VELOCITIES
        .get(usize::from(m.action_timer))
        .expect("punch timer reads past sPunchingForwardVelocities");
    mario_set_forward_vel(m, w, f32::from(velocity));
    if m.action_timer > 0 {
        m.action_timer -= 1;
    }
    mario_update_punch_sequence(m, w);
    perform_ground_step(m, w);
    0
}

fn act_stomach_slide_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    animated_stationary_ground_step(m, w, MARIO_ANIM_SLOW_LAND_FROM_DIVE, ACT_IDLE);
    0
}

/// check_common_object_cancels.
fn check_common_object_cancels(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let water_surface = (i32::from(m.water_level) - 100) as f32;
    if m.pos[1] < water_surface {
        return set_water_plunge_action(m, w);
    }
    if m.input & INPUT_SQUISHED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SQUISHED, 0);
    }
    if m.health < 0x100 {
        return drop_and_set_mario_action(m, w, ACT_STANDING_DEATH, 0);
    }
    0
}

/// mario_execute_object_action.
pub fn mario_execute_object_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_object_cancels(m, w) != 0 {
        return 1;
    }
    if mario_update_quicksand(m, w, 0.5) != 0 {
        return 1;
    }
    let cancel = match m.action {
        ACT_PUNCHING => act_punching(m, w),
        ACT_STOMACH_SLIDE_STOP => act_stomach_slide_stop(m, w),
        ACT_PICKING_UP
        | ACT_DIVE_PICKING_UP
        | ACT_PLACING_DOWN
        | ACT_THROWING
        | ACT_HEAVY_THROW
        | ACT_PICKING_UP_BOWSER
        | ACT_HOLDING_BOWSER
        | ACT_RELEASING_BOWSER => panic!(
            "object action {:#X} holds or uses an object; objects are not simulated yet",
            m.action
        ),
        action => panic!("object action {action:#X} is not in the original table"),
    };
    if cancel == 0 && m.input & INPUT_IN_WATER != 0 {
        m.particle_flags |= PARTICLE_IDLE_WATER_WAVE;
    }
    cancel
}
