//! Stationary actions, translated from pinned CC0
//! src/game/mario_actions_stationary.c. Actions that hold an object (the
//! ACT_HOLD_* family) need objects and panic until objects are simulated.
use super::{
    Event, MarioState, StepWorld,
    animation::{is_anim_at_end, is_anim_past_end, set_mario_animation},
    constants::*,
    core::{
        check_common_action_exits, drop_and_set_mario_action, find_floor_height_relative_polar,
        hurt_and_set_mario_action, play_mario_heavy_landing_sound, play_mario_landing_sound,
        play_sound_if_no_flag, set_jump_from_landing, set_jumping_action, set_mario_action,
        set_water_plunge_action, update_mario_sound_and_camera,
    },
    interaction::{mario_drop_held_object, mario_throw_held_object},
    step::{
        mario_push_off_steep_floor, mario_set_forward_vel, mario_update_quicksand,
        stationary_ground_step,
    },
};
use crate::simulation::collision::SURFACE_FLAG_DYNAMIC;

fn floor_normal_y(m: &MarioState, w: &StepWorld<'_>) -> f32 {
    w.surface(
        m.floor
            .expect("Mario has no floor (the original dereferences NULL)"),
    )
    .normal[1]
}

/// check_common_idle_cancels.
pub fn check_common_idle_cancels(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    mario_drop_held_object(m);
    if floor_normal_y(m, w) < 0.29237169 {
        return mario_push_off_steep_floor(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_JUMP, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_FIRST_PERSON, 0);
    }
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        m.face_angle[1] = m.intended_yaw;
        return set_mario_action(m, w, ACT_WALKING, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_PUNCHING, 0);
    }
    if m.input & INPUT_Z_DOWN != 0 {
        return set_mario_action(m, w, ACT_START_CROUCHING, 0);
    }
    0
}

fn act_idle(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.quicksand_depth > 30.0 {
        return set_mario_action(m, w, ACT_IN_QUICKSAND, 0);
    }
    if m.input & INPUT_IN_POISON_GAS != 0 {
        return set_mario_action(m, w, ACT_COUGHING, 0);
    }
    if m.action_arg & 1 == 0 && m.health < 0x300 {
        return set_mario_action(m, w, ACT_PANTING, 0);
    }
    if check_common_idle_cancels(m, w) != 0 {
        return 1;
    }
    if m.action_state == 3 {
        return if w.area_terrain_type & TERRAIN_MASK == TERRAIN_SNOW {
            set_mario_action(m, w, ACT_SHIVERING, 0)
        } else {
            set_mario_action(m, w, ACT_START_SLEEPING, 0)
        };
    }
    if m.action_arg & 1 != 0 {
        set_mario_animation(m, w, MARIO_ANIM_STAND_AGAINST_WALL);
    } else {
        match m.action_state {
            0 => {
                set_mario_animation(m, w, MARIO_ANIM_IDLE_HEAD_LEFT);
            }
            1 => {
                set_mario_animation(m, w, MARIO_ANIM_IDLE_HEAD_RIGHT);
            }
            2 => {
                set_mario_animation(m, w, MARIO_ANIM_IDLE_HEAD_CENTER);
            }
            _ => {}
        }
        if is_anim_at_end(m, w) {
            // Sleep after ten head-turn cycles on level, static ground.
            m.action_state = m.action_state.wrapping_add(1);
            if m.action_state == 3 {
                let delta_y = m.pos[1] - find_floor_height_relative_polar(m, w, i16::MIN, 60.0);
                let dynamic =
                    w.surface(m.floor.expect("idle floor")).flags & SURFACE_FLAG_DYNAMIC != 0;
                if delta_y < -24.0 || 24.0 < delta_y || dynamic {
                    m.action_state = 0;
                } else {
                    m.action_timer = m.action_timer.wrapping_add(1);
                    if m.action_timer < 10 {
                        m.action_state = 0;
                    }
                }
            }
        }
    }
    stationary_ground_step(m, w);
    0
}

fn play_anim_sound(
    m: &MarioState,
    w: &mut StepWorld<'_>,
    action_state: u16,
    anim_frame: i16,
    sound: u32,
) {
    if m.action_state == action_state && m.obj.gfx.anim.anim_frame == anim_frame {
        w.play_sound(sound);
    }
}

fn act_start_sleeping(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_idle_cancels(m, w) != 0 {
        return 1;
    }
    if m.quicksand_depth > 30.0 {
        return set_mario_action(m, w, ACT_IN_QUICKSAND, 0);
    }
    if m.action_state == 4 {
        return set_mario_action(m, w, ACT_SLEEPING, 0);
    }
    let anim_frame = match m.action_state {
        0 => set_mario_animation(m, w, MARIO_ANIM_START_SLEEP_IDLE),
        1 => set_mario_animation(m, w, MARIO_ANIM_START_SLEEP_SCRATCH),
        2 => {
            let frame = set_mario_animation(m, w, MARIO_ANIM_START_SLEEP_YAWN);
            m.body.eye_state = MARIO_EYES_HALF_CLOSED;
            frame
        }
        3 => {
            let frame = set_mario_animation(m, w, MARIO_ANIM_START_SLEEP_SITTING);
            m.body.eye_state = MARIO_EYES_HALF_CLOSED;
            frame
        }
        state => panic!("start-sleeping state {state} leaves animFrame uninitialized"),
    };
    play_anim_sound(m, w, 1, 41, SOUND_ACTION_PAT_BACK);
    play_anim_sound(m, w, 1, 49, SOUND_ACTION_PAT_BACK);
    let body_hit = m
        .terrain_sound_addend
        .wrapping_add(SOUND_ACTION_TERRAIN_BODY_HIT_GROUND);
    play_anim_sound(m, w, 3, 15, body_hit);
    if is_anim_at_end(m, w) {
        m.action_state = m.action_state.wrapping_add(1);
    }
    if m.action_state == 2 && anim_frame == -1 {
        w.play_sound(SOUND_MARIO_YAWNING);
    }
    if m.action_state == 1 && anim_frame == -1 {
        w.play_sound(SOUND_MARIO_IMA_TIRED);
    }
    stationary_ground_step(m, w);
    0
}

const WAKING_INPUTS: u16 = INPUT_NONZERO_ANALOG
    | INPUT_A_PRESSED
    | INPUT_OFF_FLOOR
    | INPUT_ABOVE_SLIDE
    | INPUT_FIRST_PERSON
    | INPUT_STOMPED
    | INPUT_B_PRESSED
    | INPUT_Z_PRESSED;

fn act_sleeping(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & WAKING_INPUTS != 0 {
        let state = u32::from(m.action_state);
        return set_mario_action(m, w, ACT_WAKING_UP, state);
    }
    if m.quicksand_depth > 30.0 {
        let state = u32::from(m.action_state);
        return set_mario_action(m, w, ACT_WAKING_UP, state);
    }
    if m.pos[1] - find_floor_height_relative_polar(m, w, i16::MIN, 60.0) > 24.0 {
        let state = u32::from(m.action_state);
        return set_mario_action(m, w, ACT_WAKING_UP, state);
    }
    m.body.eye_state = MARIO_EYES_CLOSED;
    stationary_ground_step(m, w);
    match m.action_state {
        0 => {
            let anim_frame = set_mario_animation(m, w, MARIO_ANIM_SLEEP_IDLE);
            if anim_frame == -1 && m.action_timer == 0 {
                w.event(Event::LowerBackgroundNoise(2));
            }
            if anim_frame == 2 {
                w.play_sound(SOUND_MARIO_SNORING1);
            }
            if anim_frame == 20 {
                w.play_sound(SOUND_MARIO_SNORING2);
            }
            if is_anim_at_end(m, w) {
                m.action_timer = m.action_timer.wrapping_add(1);
                if m.action_timer > 45 {
                    m.action_state = m.action_state.wrapping_add(1);
                }
            }
        }
        1 => {
            if set_mario_animation(m, w, MARIO_ANIM_SLEEP_START_LYING) == 18 {
                play_mario_heavy_landing_sound(m, w, SOUND_ACTION_TERRAIN_BODY_HIT_GROUND);
            }
            if is_anim_at_end(m, w) {
                m.action_state = m.action_state.wrapping_add(1);
            }
        }
        2 => {
            set_mario_animation(m, w, MARIO_ANIM_SLEEP_LYING);
            play_sound_if_no_flag(m, w, SOUND_MARIO_SNORING3, MARIO_ACTION_SOUND_PLAYED);
        }
        _ => {}
    }
    0
}

fn act_waking_up(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.action_timer == 0 {
        w.event(Event::StopSound(SOUND_MARIO_SNORING1));
        w.event(Event::StopSound(SOUND_MARIO_SNORING2));
        w.event(Event::StopSound(SOUND_MARIO_SNORING3));
        w.event(Event::RaiseBackgroundNoise(2));
    }
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer > 20 {
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    stationary_ground_step(m, w);
    let anim = if m.action_arg == 0 {
        MARIO_ANIM_WAKE_FROM_SLEEP
    } else {
        MARIO_ANIM_WAKE_FROM_LYING
    };
    set_mario_animation(m, w, anim);
    0
}

fn act_shivering(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & WAKING_INPUTS != 0 {
        m.action_state = 2;
    }
    stationary_ground_step(m, w);
    match m.action_state {
        0 => {
            let anim_frame = set_mario_animation(m, w, MARIO_ANIM_SHIVERING_WARMING_HAND);
            if anim_frame == 49 {
                m.particle_flags |= PARTICLE_BREATH;
                w.play_sound(SOUND_MARIO_PANTING_COLD);
            }
            if anim_frame == 7 || anim_frame == 81 {
                w.play_sound(SOUND_ACTION_CLAP_HANDS_COLD);
            }
            if is_anim_past_end(m, w) {
                m.action_state = 1;
            }
        }
        1 => {
            let anim_frame = set_mario_animation(m, w, MARIO_ANIM_SHIVERING);
            if anim_frame == 9 || anim_frame == 25 || anim_frame == 44 {
                w.play_sound(SOUND_ACTION_CLAP_HANDS_COLD);
            }
        }
        2 => {
            set_mario_animation(m, w, MARIO_ANIM_SHIVERING_RETURN_TO_IDLE);
            if is_anim_past_end(m, w) {
                set_mario_action(m, w, ACT_IDLE, 0);
            }
        }
        _ => {}
    }
    0
}

fn act_coughing(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_idle_cancels(m, w) != 0 {
        return 1;
    }
    stationary_ground_step(m, w);
    let anim_frame = set_mario_animation(m, w, MARIO_ANIM_COUGHING);
    if anim_frame == 25 || anim_frame == 35 {
        w.play_sound(SOUND_MARIO_COUGHING3);
    }
    if anim_frame == 50 || anim_frame == 58 {
        w.play_sound(SOUND_MARIO_COUGHING2);
    }
    if anim_frame == 71 || anim_frame == 80 {
        w.play_sound(SOUND_MARIO_COUGHING1);
    }
    0
}

fn act_standing_against_wall(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE) != 0
    {
        return check_common_action_exits(m, w);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_FIRST_PERSON, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_PUNCHING, 0);
    }
    set_mario_animation(m, w, MARIO_ANIM_STAND_AGAINST_WALL);
    stationary_ground_step(m, w);
    0
}

fn act_in_quicksand(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.quicksand_depth < 30.0 {
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    if check_common_idle_cancels(m, w) != 0 {
        return 1;
    }
    if m.quicksand_depth > 70.0 {
        set_mario_animation(m, w, MARIO_ANIM_DYING_IN_QUICKSAND);
    } else {
        set_mario_animation(m, w, MARIO_ANIM_IDLE_IN_QUICKSAND);
    }
    stationary_ground_step(m, w);
    0
}

fn act_crouching(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_BACKFLIP, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_STOP_CROUCHING, 0);
    }
    if m.input & INPUT_Z_DOWN == 0 {
        return set_mario_action(m, w, ACT_STOP_CROUCHING, 0);
    }
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        return set_mario_action(m, w, ACT_START_CRAWLING, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_PUNCHING, 9);
    }
    stationary_ground_step(m, w);
    set_mario_animation(m, w, MARIO_ANIM_CROUCHING);
    0
}

fn act_panting(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.health >= 0x500 {
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    if check_common_idle_cancels(m, w) != 0 {
        return 1;
    }
    if set_mario_animation(m, w, MARIO_ANIM_WALK_PANTING) == 1 {
        w.play_sound(SOUND_MARIO_PANTING.wrapping_add((w.audio_random % 3) << 0x10));
    }
    stationary_ground_step(m, w);
    m.body.eye_state = MARIO_EYES_HALF_CLOSED;
    0
}

/// stopping_step.
fn stopping_step(m: &mut MarioState, w: &mut StepWorld<'_>, anim_id: i32, action: u32) {
    stationary_ground_step(m, w);
    set_mario_animation(m, w, anim_id);
    if is_anim_at_end(m, w) {
        set_mario_action(m, w, action, 0);
    }
}

fn act_braking_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_PUNCHING, 0);
    }
    if m.input & INPUT_FIRST_PERSON == 0
        && m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE)
            != 0
    {
        return check_common_action_exits(m, w);
    }
    stopping_step(m, w, MARIO_ANIM_STOP_SKID, ACT_IDLE);
    0
}

fn act_butt_slide_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE) != 0
    {
        return check_common_action_exits(m, w);
    }
    stopping_step(m, w, MARIO_ANIM_STOP_SLIDE, ACT_IDLE);
    if m.obj.gfx.anim.anim_frame == 6 {
        play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
    }
    0
}

fn act_slide_kick_slide_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return drop_and_set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    stopping_step(m, w, MARIO_ANIM_CROUCH_FROM_SLIDE_KICK, ACT_CROUCHING);
    0
}

fn crouch_transition(m: &mut MarioState, w: &mut StepWorld<'_>, anim: i32, end: u32) -> i32 {
    stationary_ground_step(m, w);
    set_mario_animation(m, w, anim);
    if is_anim_past_end(m, w) {
        set_mario_action(m, w, end, 0);
    }
    0
}

fn act_start_crouching(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_BACKFLIP, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    crouch_transition(m, w, MARIO_ANIM_START_CROUCHING, ACT_CROUCHING)
}

fn act_stop_crouching(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return set_jumping_action(m, w, ACT_BACKFLIP, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    crouch_transition(m, w, MARIO_ANIM_STOP_CROUCHING, ACT_IDLE)
}

fn act_start_crawling(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_STOP_CROUCHING, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    crouch_transition(m, w, MARIO_ANIM_START_CRAWLING, ACT_CRAWLING)
}

fn act_stop_crawling(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    crouch_transition(m, w, MARIO_ANIM_STOP_CRAWLING, ACT_CROUCHING)
}

fn act_shockwave_bounce(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let interact_status = m.obj.raw.u32(O_INTERACT_STATUS);
    if interact_status & INT_STATUS_MARIO_SHOCKWAVE != 0 {
        return hurt_and_set_mario_action(m, w, ACT_SHOCKED, 0, 4);
    }
    if m.action_timer == 0 && interact_status & INT_STATUS_MARIO_KNOCKBACK_DMG != 0 {
        return hurt_and_set_mario_action(m, w, ACT_BACKWARD_GROUND_KB, 0, 0xC);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer == 48 {
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    let timer = i32::from(m.action_timer);
    let angle = ((timer % 16) << 12) as i16;
    let height = ((6 - timer / 8) as f32 * 8.0) + 4.0;
    mario_set_forward_vel(m, w, 0.0);
    m.vel = [0.0, 0.0, 0.0];
    let s = w.trig.sins(i32::from(angle));
    m.pos[1] = if s >= 0.0 {
        s * height + m.floor_height
    } else {
        m.floor_height - s * height
    };
    m.obj.gfx.pos = m.pos;
    m.obj.gfx.angle = [0, m.face_angle[1], 0];
    set_mario_animation(m, w, MARIO_ANIM_A_POSE);
    0
}

/// landing_step.
fn landing_step(m: &mut MarioState, w: &mut StepWorld<'_>, anim: i32, action: u32) -> i32 {
    stationary_ground_step(m, w);
    set_mario_animation(m, w, anim);
    if is_anim_at_end(m, w) {
        return set_mario_action(m, w, action, 0);
    }
    0
}

/// check_common_landing_cancels: `action` 0 means jump from landing.
fn check_common_landing_cancels(m: &mut MarioState, w: &mut StepWorld<'_>, action: u32) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_FIRST_PERSON != 0 {
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    if m.input & INPUT_A_PRESSED != 0 {
        return if action == 0 {
            set_jump_from_landing(m, w)
        } else {
            set_jumping_action(m, w, action, 0)
        };
    }
    if m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED | INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE) != 0
    {
        return check_common_action_exits(m, w);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_PUNCHING, 0);
    }
    0
}

fn land_stop(m: &mut MarioState, w: &mut StepWorld<'_>, jump: u32, anim: i32) -> i32 {
    if check_common_landing_cancels(m, w, jump) != 0 {
        return 1;
    }
    landing_step(m, w, anim, ACT_IDLE);
    0
}

fn act_side_flip_land_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_landing_cancels(m, w, 0) != 0 {
        return 1;
    }
    landing_step(m, w, MARIO_ANIM_SLIDEFLIP_LAND, ACT_IDLE);
    m.obj.gfx.angle[1] = m.obj.gfx.angle[1].wrapping_add(i16::MIN);
    0
}

fn act_backflip_land_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_Z_DOWN == 0 || m.obj.gfx.anim.anim_frame >= 6 {
        m.input &= !INPUT_A_PRESSED;
    }
    land_stop(m, w, ACT_BACKFLIP, MARIO_ANIM_TRIPLE_JUMP_LAND)
}

fn act_lava_boost_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.input &= !(INPUT_FIRST_PERSON | INPUT_B_PRESSED);
    land_stop(m, w, 0, MARIO_ANIM_STAND_UP_FROM_LAVA_BOOST)
}

fn act_long_jump_land_stop(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.input &= !INPUT_B_PRESSED;
    if check_common_landing_cancels(m, w, ACT_JUMP) != 0 {
        return 1;
    }
    let anim = if m.obj.raw.s32(O_MARIO_LONG_JUMP_IS_SLOW) == 0 {
        MARIO_ANIM_CROUCH_FROM_FAST_LONGJUMP
    } else {
        MARIO_ANIM_CROUCH_FROM_SLOW_LONGJUMP
    };
    landing_step(m, w, anim, ACT_CROUCHING);
    0
}

fn act_air_throw_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer == 4 {
        mario_throw_held_object(m);
    }
    landing_step(m, w, MARIO_ANIM_THROW_LIGHT_OBJECT, ACT_IDLE);
    0
}

fn act_twirl_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.action_state = 1;
    if m.input & INPUT_STOMPED != 0 {
        return set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    stationary_ground_step(m, w);
    set_mario_animation(m, w, MARIO_ANIM_TWIRL_LAND);
    if m.angle_vel[1] > 0 {
        m.angle_vel[1] = m.angle_vel[1].wrapping_sub(0x400);
        if m.angle_vel[1] < 0 {
            m.angle_vel[1] = 0;
        }
        m.twirl_yaw = m.twirl_yaw.wrapping_add(m.angle_vel[1]);
    }
    m.obj.gfx.angle[1] = m.obj.gfx.angle[1].wrapping_add(m.twirl_yaw);
    if is_anim_at_end(m, w) && m.angle_vel[1] == 0 {
        m.face_angle[1] = m.face_angle[1].wrapping_add(m.twirl_yaw);
        set_mario_action(m, w, ACT_IDLE, 0);
    }
    0
}

fn act_ground_pound_land(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.action_state = 1;
    if m.input & INPUT_STOMPED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SHOCKWAVE_BOUNCE, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BUTT_SLIDE, 0);
    }
    landing_step(m, w, MARIO_ANIM_GROUND_POUND_LANDING, ACT_BUTT_SLIDE_STOP);
    0
}

fn act_first_person(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let leave = m.input & (INPUT_OFF_FLOOR | INPUT_ABOVE_SLIDE | INPUT_STOMPED) != 0;
    if m.action_state == 0 {
        w.event(Event::LowerBackgroundNoise(2));
        w.set_camera_mode(CAMERA_MODE_C_UP, 0x10);
        m.action_state = 1;
    } else if m.input & INPUT_FIRST_PERSON == 0 || leave {
        w.event(Event::RaiseBackgroundNoise(2));
        w.set_camera_mode(-1, 1);
        return set_mario_action(m, w, ACT_IDLE, 0);
    }
    let floor = w.surface(m.floor.expect("first-person floor"));
    if floor.surface_type == SURFACE_LOOK_UP_WARP && w.save.total_star_count >= 10 {
        let pitch = m.camera_status.head_rotation[0];
        let yaw = ((i32::from(m.camera_status.head_rotation[1]) * 4) / 3
            + i32::from(m.face_angle[1])) as i16;
        if pitch == -0x1800 && (yaw < -0x6FFF || yaw >= 0x7000) {
            w.event(Event::Warp(WARP_OP_UNKNOWN_01));
        }
    }
    stationary_ground_step(m, w);
    set_mario_animation(m, w, MARIO_ANIM_FIRST_PERSON);
    0
}

/// check_common_stationary_cancels.
fn check_common_stationary_cancels(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.pos[1] < (i32::from(m.water_level) - 100) as f32 {
        if m.action == ACT_SPAWN_SPIN_LANDING {
            w.event(Event::LevelInitText(0));
        }
        update_mario_sound_and_camera(m, w);
        return set_water_plunge_action(m, w);
    }
    if m.input & INPUT_SQUISHED != 0 {
        update_mario_sound_and_camera(m, w);
        return drop_and_set_mario_action(m, w, ACT_SQUISHED, 0);
    }
    if m.action != ACT_UNKNOWN_0002020E && m.health < 0x100 {
        update_mario_sound_and_camera(m, w);
        return drop_and_set_mario_action(m, w, ACT_STANDING_DEATH, 0);
    }
    0
}

/// mario_execute_stationary_action.
pub fn mario_execute_stationary_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_stationary_cancels(m, w) != 0 {
        return 1;
    }
    if mario_update_quicksand(m, w, 0.5) != 0 {
        return 1;
    }
    let cancel = match m.action {
        ACT_IDLE => act_idle(m, w),
        ACT_START_SLEEPING => act_start_sleeping(m, w),
        ACT_SLEEPING => act_sleeping(m, w),
        ACT_WAKING_UP => act_waking_up(m, w),
        ACT_PANTING => act_panting(m, w),
        ACT_IN_QUICKSAND => act_in_quicksand(m, w),
        ACT_STANDING_AGAINST_WALL => act_standing_against_wall(m, w),
        ACT_COUGHING => act_coughing(m, w),
        ACT_SHIVERING => act_shivering(m, w),
        ACT_CROUCHING => act_crouching(m, w),
        ACT_START_CROUCHING => act_start_crouching(m, w),
        ACT_STOP_CROUCHING => act_stop_crouching(m, w),
        ACT_START_CRAWLING => act_start_crawling(m, w),
        ACT_STOP_CRAWLING => act_stop_crawling(m, w),
        ACT_SLIDE_KICK_SLIDE_STOP => act_slide_kick_slide_stop(m, w),
        ACT_SHOCKWAVE_BOUNCE => act_shockwave_bounce(m, w),
        ACT_FIRST_PERSON => act_first_person(m, w),
        ACT_JUMP_LAND_STOP => land_stop(m, w, 0, MARIO_ANIM_LAND_FROM_SINGLE_JUMP),
        ACT_DOUBLE_JUMP_LAND_STOP => land_stop(m, w, 0, MARIO_ANIM_LAND_FROM_DOUBLE_JUMP),
        ACT_FREEFALL_LAND_STOP => land_stop(m, w, 0, MARIO_ANIM_GENERAL_LAND),
        ACT_SIDE_FLIP_LAND_STOP => act_side_flip_land_stop(m, w),
        ACT_AIR_THROW_LAND => act_air_throw_land(m, w),
        ACT_LAVA_BOOST_LAND => act_lava_boost_land(m, w),
        ACT_TWIRL_LAND => act_twirl_land(m, w),
        ACT_TRIPLE_JUMP_LAND_STOP => land_stop(m, w, ACT_JUMP, MARIO_ANIM_TRIPLE_JUMP_LAND),
        ACT_BACKFLIP_LAND_STOP => act_backflip_land_stop(m, w),
        ACT_LONG_JUMP_LAND_STOP => act_long_jump_land_stop(m, w),
        ACT_GROUND_POUND_LAND => act_ground_pound_land(m, w),
        ACT_BRAKING_STOP => act_braking_stop(m, w),
        ACT_BUTT_SLIDE_STOP => act_butt_slide_stop(m, w),
        ACT_HOLD_PANTING_UNUSED
        | ACT_HOLD_IDLE
        | ACT_HOLD_HEAVY_IDLE
        | ACT_HOLD_JUMP_LAND_STOP
        | ACT_HOLD_FREEFALL_LAND_STOP
        | ACT_HOLD_BUTT_SLIDE_STOP => {
            panic!(
                "stationary action {:#X} holds an object; objects are not simulated yet",
                m.action
            )
        }
        action => panic!("stationary action {action:#X} is not in the original table"),
    };
    if cancel == 0 && m.input & INPUT_IN_WATER != 0 {
        m.particle_flags |= PARTICLE_IDLE_WATER_WAVE;
    }
    cancel
}
