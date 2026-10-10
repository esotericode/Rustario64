//! Airborne actions, translated from pinned CC0
//! src/game/mario_actions_airborne.c (US: rumble calls are not compiled).
//! Actions that hold, ride or use an object panic until objects are simulated.
use super::{
    Event, MarioState, StepWorld,
    animation::{
        cur_anim, is_anim_at_end, is_anim_past_end, set_anim_to_frame, set_mario_animation,
    },
    constants::*,
    core::{
        adjust_sound_for_speed, drop_and_set_mario_action, play_mario_heavy_landing_sound,
        play_mario_jump_sound, play_mario_landing_sound, play_mario_sound, play_sound_if_no_flag,
        set_mario_action, set_water_plunge_action, update_mario_sound_and_camera,
    },
    f32_to_s16,
    inputs::mario_floor_is_slippery,
    interaction::{
        level_trigger_warp, mario_blow_off_cap, mario_check_object_grab, mario_drop_held_object,
        mario_grab_used_object, mario_throw_held_object,
    },
    step::{mario_bonk_reflection, mario_set_forward_vel, perform_air_step},
};
use crate::simulation::{
    collision::Surface,
    math::{approach_f32, approach_s32},
};

use super::stationary::told_to_drop;
use crate::simulation::object::object;

const TERRAIN_JUMP: i32 = SOUND_ACTION_TERRAIN_JUMP as i32;

fn floor(m: &MarioState, w: &StepWorld<'_>) -> Surface {
    w.surface(
        m.floor
            .expect("Mario has no floor (the original dereferences NULL)"),
    )
}

fn camera_mode(w: &StepWorld<'_>) -> i16 {
    i16::from(w.camera.mode)
}

/// set_camera_mode: a recorded request to the camera.
fn set_camera_mode(w: &mut StepWorld<'_>, mode: i16, frames: i16) {
    w.set_camera_mode(mode, frames);
}

/// set_camera_mode(m->area->camera, m->area->camera->defMode, 1).
fn reset_camera_mode(w: &mut StepWorld<'_>) {
    let mode = i16::from(w.camera.def_mode);
    set_camera_mode(w, mode, 1);
}

fn cap_damage(m: &MarioState, on_head: u8, off_head: u8) -> u8 {
    if m.flags & MARIO_CAP_ON_HEAD != 0 {
        on_head
    } else {
        off_head
    }
}

/// play_flip_sounds.
fn play_flip_sounds(m: &MarioState, w: &mut StepWorld<'_>, frame1: i16, frame2: i16, frame3: i16) {
    let anim_frame = m.obj.gfx.anim.anim_frame;
    if anim_frame == frame1 || anim_frame == frame2 || anim_frame == frame3 {
        w.play_sound(SOUND_ACTION_SPIN);
    }
}

/// play_far_fall_sound.
fn play_far_fall_sound(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let action = m.action;
    if action & ACT_FLAG_INVULNERABLE == 0
        && action != ACT_TWIRLING
        && action != ACT_FLYING
        && m.flags & MARIO_UNKNOWN_18 == 0
        && m.peak_height - m.pos[1] > 1150.0
    {
        w.play_sound(SOUND_MARIO_WAAAOOOW);
        m.flags |= MARIO_UNKNOWN_18;
    }
}

/// play_knockback_sound (not in JP).
fn play_knockback_sound(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.action_arg == 0 && (m.forward_vel <= -28.0 || m.forward_vel >= 28.0) {
        play_sound_if_no_flag(m, w, SOUND_MARIO_DOH, MARIO_MARIO_SOUND_PLAYED);
    } else {
        play_sound_if_no_flag(m, w, SOUND_MARIO_UH, MARIO_MARIO_SOUND_PLAYED);
    }
}

/// lava_boost_on_wall.
fn lava_boost_on_wall(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let wall = w.surface(
        m.wall
            .expect("lava wall is NULL (the original dereferences it)"),
    );
    m.face_angle[1] = w.trig.atan2s(wall.normal[2], wall.normal[0]);
    if m.forward_vel < 24.0 {
        m.forward_vel = 24.0;
    }
    if m.flags & MARIO_METAL_CAP == 0 {
        m.hurt_counter = m.hurt_counter.wrapping_add(cap_damage(m, 12, 18));
    }
    w.play_sound(SOUND_MARIO_ON_FIRE);
    update_mario_sound_and_camera(m, w);
    drop_and_set_mario_action(m, w, ACT_LAVA_BOOST, 1)
}

/// check_fall_damage.
fn check_fall_damage(m: &mut MarioState, w: &mut StepWorld<'_>, hard_fall_action: u32) -> i32 {
    let fall_height = m.peak_height - m.pos[1];
    // Never true: actionState is a u16.
    let damage_height = if u32::from(m.action_state) == ACT_GROUND_POUND {
        600.0
    } else {
        1150.0
    };
    if m.action != ACT_TWIRLING && floor(m, w).surface_type != SURFACE_BURNING && m.vel[1] < -55.0 {
        if fall_height > 3000.0 {
            m.hurt_counter = m.hurt_counter.wrapping_add(cap_damage(m, 16, 24));
            w.event(Event::CameraShake(SHAKE_FALL_DAMAGE));
            w.play_sound(SOUND_MARIO_ATTACKED);
            return drop_and_set_mario_action(m, w, hard_fall_action, 4);
        } else if fall_height > damage_height && !mario_floor_is_slippery(m, w) {
            m.hurt_counter = m.hurt_counter.wrapping_add(cap_damage(m, 8, 12));
            m.squish_timer = 30;
            w.event(Event::CameraShake(SHAKE_FALL_DAMAGE));
            w.play_sound(SOUND_MARIO_ATTACKED);
        }
    }
    0
}

/// check_kick_or_dive_in_air.
fn check_kick_or_dive_in_air(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        let action = if m.forward_vel > 28.0 {
            ACT_DIVE
        } else {
            ACT_JUMP_KICK
        };
        return set_mario_action(m, w, action, 0);
    }
    0
}

/// SURFACE_IS_NOT_HARD.
fn surface_is_not_hard(surface_type: i16) -> bool {
    surface_type != SURFACE_HARD && !(0x35..=0x37).contains(&surface_type)
}

/// should_get_stuck_in_ground. The original reads the floor's fields before
/// its NULL check.
fn should_get_stuck_in_ground(m: &MarioState, w: &StepWorld<'_>) -> bool {
    let terrain_type = w.area_terrain_type & TERRAIN_MASK;
    let floor = floor(m, w);
    let flags = i32::from(floor.flags);
    let surface_type = floor.surface_type;
    (terrain_type == TERRAIN_SNOW || terrain_type == TERRAIN_SAND)
        && surface_type != SURFACE_BURNING
        && surface_is_not_hard(surface_type)
        && flags & 0x01 == 0
        && m.peak_height - m.pos[1] > 1000.0
        && floor.normal[1] >= 0.8660254
}

/// check_fall_damage_or_get_stuck.
fn check_fall_damage_or_get_stuck(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    hard_fall_action: u32,
) -> i32 {
    if should_get_stuck_in_ground(m, w) {
        w.play_sound(SOUND_MARIO_OOOF2);
        m.particle_flags |= PARTICLE_MIST_CIRCLE;
        drop_and_set_mario_action(m, w, ACT_FEET_STUCK_IN_GROUND, 0);
        return 1;
    }
    check_fall_damage(m, w, hard_fall_action)
}

/// check_horizontal_wind (US: no wind sound here).
fn check_horizontal_wind(m: &mut MarioState, w: &StepWorld<'_>) -> bool {
    let floor = floor(m, w);
    if floor.surface_type == SURFACE_HORIZONTAL_WIND {
        let push_angle = i32::from((i32::from(floor.force) << 8) as i16);
        m.slide_vel_x += 1.2 * w.trig.sins(push_angle);
        m.slide_vel_z += 1.2 * w.trig.coss(push_angle);
        let mut speed = (m.slide_vel_x * m.slide_vel_x + m.slide_vel_z * m.slide_vel_z).sqrt();
        if speed > 48.0 {
            m.slide_vel_x = m.slide_vel_x * 48.0 / speed;
            m.slide_vel_z = m.slide_vel_z * 48.0 / speed;
            // This was meant to be 48?
            speed = 32.0;
        } else if speed > 32.0 {
            speed = 32.0;
        }
        m.vel[0] = m.slide_vel_x;
        m.vel[2] = m.slide_vel_z;
        m.slide_yaw = w.trig.atan2s(m.slide_vel_z, m.slide_vel_x);
        m.forward_vel = speed
            * w.trig
                .coss(i32::from(m.face_angle[1]) - i32::from(m.slide_yaw));
        return true;
    }
    false
}

/// The s16 difference intendedYaw - faceAngle[1], widened for sins/coss.
fn intended_d_yaw(m: &MarioState) -> i32 {
    i32::from(m.intended_yaw.wrapping_sub(m.face_angle[1]))
}

/// update_air_with_turn.
fn update_air_with_turn(m: &mut MarioState, w: &StepWorld<'_>) {
    if !check_horizontal_wind(m, w) {
        let drag_threshold = if m.action == ACT_LONG_JUMP {
            48.0
        } else {
            32.0
        };
        m.forward_vel = approach_f32(m.forward_vel, 0.0, 0.35, 0.35);
        if m.input & INPUT_NONZERO_ANALOG != 0 {
            let d_yaw = intended_d_yaw(m);
            let intended_mag = m.intended_mag / 32.0;
            m.forward_vel += 1.5 * w.trig.coss(d_yaw) * intended_mag;
            m.face_angle[1] =
                f32_to_s16(f32::from(m.face_angle[1]) + 512.0 * w.trig.sins(d_yaw) * intended_mag);
        }
        // Uncapped air speed. Net positive when moving forward.
        if m.forward_vel > drag_threshold {
            m.forward_vel -= 1.0;
        }
        if m.forward_vel < -16.0 {
            m.forward_vel += 2.0;
        }
        let yaw = i32::from(m.face_angle[1]);
        m.slide_vel_x = m.forward_vel * w.trig.sins(yaw);
        m.vel[0] = m.slide_vel_x;
        m.slide_vel_z = m.forward_vel * w.trig.coss(yaw);
        m.vel[2] = m.slide_vel_z;
    }
}

/// update_air_without_turn.
fn update_air_without_turn(m: &mut MarioState, w: &StepWorld<'_>) {
    let mut sideways_speed = 0.0f32;
    if !check_horizontal_wind(m, w) {
        let drag_threshold = if m.action == ACT_LONG_JUMP {
            48.0
        } else {
            32.0
        };
        m.forward_vel = approach_f32(m.forward_vel, 0.0, 0.35, 0.35);
        if m.input & INPUT_NONZERO_ANALOG != 0 {
            let d_yaw = intended_d_yaw(m);
            let intended_mag = m.intended_mag / 32.0;
            m.forward_vel += intended_mag * w.trig.coss(d_yaw) * 1.5;
            sideways_speed = intended_mag * w.trig.sins(d_yaw) * 10.0;
        }
        // Uncapped air speed. Net positive when moving forward.
        if m.forward_vel > drag_threshold {
            m.forward_vel -= 1.0;
        }
        if m.forward_vel < -16.0 {
            m.forward_vel += 2.0;
        }
        let yaw = i32::from(m.face_angle[1]);
        m.slide_vel_x = m.forward_vel * w.trig.sins(yaw);
        m.slide_vel_z = m.forward_vel * w.trig.coss(yaw);
        m.slide_vel_x += sideways_speed * w.trig.sins(yaw + 0x4000);
        m.slide_vel_z += sideways_speed * w.trig.coss(yaw + 0x4000);
        m.vel[0] = m.slide_vel_x;
        m.vel[2] = m.slide_vel_z;
    }
}

/// update_lava_boost_or_twirling.
fn update_lava_boost_or_twirling(m: &mut MarioState, w: &StepWorld<'_>) {
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        let d_yaw = intended_d_yaw(m);
        let intended_mag = m.intended_mag / 32.0;
        m.forward_vel += w.trig.coss(d_yaw) * intended_mag;
        m.face_angle[1] =
            f32_to_s16(f32::from(m.face_angle[1]) + w.trig.sins(d_yaw) * intended_mag * 1024.0);
        if m.forward_vel < 0.0 {
            m.face_angle[1] = m.face_angle[1].wrapping_add(i16::MIN);
            m.forward_vel *= -1.0;
        }
        if m.forward_vel > 32.0 {
            m.forward_vel -= 2.0;
        }
    }
    let yaw = i32::from(m.face_angle[1]);
    m.slide_vel_x = m.forward_vel * w.trig.sins(yaw);
    m.vel[0] = m.slide_vel_x;
    m.slide_vel_z = m.forward_vel * w.trig.coss(yaw);
    m.vel[2] = m.slide_vel_z;
}

/// The angular-velocity update shared by update_flying_yaw and
/// update_flying_pitch.
fn approach_flying_angle_vel(
    vel: i16,
    target: i16,
    reverse_limit: i16,
    toward: (i32, i32),
    away: (i32, i32),
) -> i16 {
    if target > 0 {
        if vel < 0 {
            let vel = vel.wrapping_add(0x40);
            if vel > reverse_limit {
                reverse_limit
            } else {
                vel
            }
        } else {
            approach_s32(i32::from(vel), i32::from(target), toward.0, toward.1) as i16
        }
    } else if target < 0 {
        if vel > 0 {
            let vel = vel.wrapping_sub(0x40);
            if vel < -reverse_limit {
                -reverse_limit
            } else {
                vel
            }
        } else {
            approach_s32(i32::from(vel), i32::from(target), away.0, away.1) as i16
        }
    } else {
        approach_s32(i32::from(vel), 0, 0x40, 0x40) as i16
    }
}

/// update_flying_yaw.
fn update_flying_yaw(m: &mut MarioState, w: &StepWorld<'_>) {
    let target_yaw_vel =
        (-i32::from(f32_to_s16(w.controller.stick_x * (m.forward_vel / 4.0)))) as i16;
    m.angle_vel[1] = approach_flying_angle_vel(
        m.angle_vel[1],
        target_yaw_vel,
        0x10,
        (0x10, 0x20),
        (0x20, 0x10),
    );
    m.face_angle[1] = m.face_angle[1].wrapping_add(m.angle_vel[1]);
    m.face_angle[2] = (20 * -i32::from(m.angle_vel[1])) as i16;
}

/// update_flying_pitch.
fn update_flying_pitch(m: &mut MarioState, w: &StepWorld<'_>) {
    let target_pitch_vel =
        (-i32::from(f32_to_s16(w.controller.stick_y * (m.forward_vel / 5.0)))) as i16;
    m.angle_vel[0] = approach_flying_angle_vel(
        m.angle_vel[0],
        target_pitch_vel,
        0x20,
        (0x20, 0x40),
        (0x40, 0x20),
    );
}

/// update_flying.
fn update_flying(m: &mut MarioState, w: &StepWorld<'_>) {
    update_flying_pitch(m, w);
    update_flying_yaw(m, w);
    m.forward_vel -= 2.0 * (f32::from(m.face_angle[0]) / 16384.0) + 0.1;
    m.forward_vel -= 0.5 * (1.0 - w.trig.coss(i32::from(m.angle_vel[1])));
    if m.forward_vel < 0.0 {
        m.forward_vel = 0.0;
    }
    if m.forward_vel > 16.0 {
        m.face_angle[0] = f32_to_s16(f32::from(m.face_angle[0]) + (m.forward_vel - 32.0) * 6.0);
    } else if m.forward_vel > 4.0 {
        m.face_angle[0] = f32_to_s16(f32::from(m.face_angle[0]) + (m.forward_vel - 32.0) * 10.0);
    } else {
        m.face_angle[0] = m.face_angle[0].wrapping_sub(0x400);
    }
    m.face_angle[0] = m.face_angle[0].wrapping_add(m.angle_vel[0]);
    if m.face_angle[0] > 0x2AAA {
        m.face_angle[0] = 0x2AAA;
    }
    if m.face_angle[0] < -0x2AAA {
        m.face_angle[0] = -0x2AAA;
    }
    let (pitch, yaw) = (i32::from(m.face_angle[0]), i32::from(m.face_angle[1]));
    m.vel[0] = m.forward_vel * w.trig.coss(pitch) * w.trig.sins(yaw);
    m.vel[1] = m.forward_vel * w.trig.sins(pitch);
    m.vel[2] = m.forward_vel * w.trig.coss(pitch) * w.trig.coss(yaw);
    m.slide_vel_x = m.vel[0];
    m.slide_vel_z = m.vel[2];
}

/// common_air_action_step.
fn common_air_action_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    land_action: u32,
    animation: i32,
    step_arg: u32,
) -> u32 {
    update_air_without_turn(m, w);
    let step_result = perform_air_step(m, w, step_arg);
    match step_result {
        AIR_STEP_NONE => {
            set_mario_animation(m, w, animation);
        }
        AIR_STEP_LANDED => {
            if check_fall_damage_or_get_stuck(m, w, ACT_HARD_BACKWARD_GROUND_KB) == 0 {
                set_mario_action(m, w, land_action, 0);
            }
        }
        AIR_STEP_HIT_WALL => {
            set_mario_animation(m, w, animation);
            if m.forward_vel > 16.0 {
                mario_bonk_reflection(m, w, false);
                m.face_angle[1] = m.face_angle[1].wrapping_add(i16::MIN);
                if m.wall.is_some() {
                    set_mario_action(m, w, ACT_AIR_HIT_WALL, 0);
                } else {
                    if m.vel[1] > 0.0 {
                        m.vel[1] = 0.0;
                    }
                    // Hands-free holding: bonking with no referenced wall
                    // changes to a non-holding action without dropping.
                    if m.forward_vel >= 38.0 {
                        m.particle_flags |= PARTICLE_VERTICAL_STAR;
                        set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
                    } else {
                        if m.forward_vel > 8.0 {
                            mario_set_forward_vel(m, w, -8.0);
                        }
                        return set_mario_action(m, w, ACT_SOFT_BONK, 0) as u32;
                    }
                }
            } else {
                mario_set_forward_vel(m, w, 0.0);
            }
        }
        AIR_STEP_GRABBED_LEDGE => {
            set_mario_animation(m, w, MARIO_ANIM_IDLE_ON_LEDGE);
            drop_and_set_mario_action(m, w, ACT_LEDGE_GRAB, 0);
        }
        AIR_STEP_GRABBED_CEILING => {
            set_mario_action(m, w, ACT_START_HANGING, 0);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    step_result
}

fn act_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_kick_or_dive_in_air(m, w) != 0 {
        return 1;
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    common_air_action_step(
        m,
        w,
        ACT_JUMP_LAND,
        MARIO_ANIM_SINGLE_JUMP,
        AIR_STEP_CHECK_LEDGE_GRAB | AIR_STEP_CHECK_HANG,
    );
    0
}

fn act_double_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let animation = if m.vel[1] >= 0.0 {
        MARIO_ANIM_DOUBLE_JUMP_RISE
    } else {
        MARIO_ANIM_DOUBLE_JUMP_FALL
    };
    if check_kick_or_dive_in_air(m, w) != 0 {
        return 1;
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, SOUND_MARIO_HOOHOO as i32);
    common_air_action_step(
        m,
        w,
        ACT_DOUBLE_JUMP_LAND,
        animation,
        AIR_STEP_CHECK_LEDGE_GRAB | AIR_STEP_CHECK_HANG,
    );
    0
}

fn act_triple_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if w.special_triple_jump != 0 {
        return set_mario_action(m, w, ACT_SPECIAL_TRIPLE_JUMP, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_DIVE, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    common_air_action_step(m, w, ACT_TRIPLE_JUMP_LAND, MARIO_ANIM_TRIPLE_JUMP, 0);
    play_flip_sounds(m, w, 2, 8, 20);
    0
}

fn act_backflip(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, SOUND_MARIO_YAH_WAH_HOO as i32);
    common_air_action_step(m, w, ACT_BACKFLIP_LAND, MARIO_ANIM_BACKFLIP, 0);
    play_flip_sounds(m, w, 2, 3, 17);
    0
}

fn act_freefall(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_DIVE, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    let animation = match m.action_arg {
        0 => MARIO_ANIM_GENERAL_FALL,
        1 => MARIO_ANIM_FALL_FROM_SLIDE,
        2 => MARIO_ANIM_FALL_FROM_SLIDE_KICK,
        arg => panic!("act_freefall reads an uninitialized animation for argument {arg}"),
    };
    common_air_action_step(
        m,
        w,
        ACT_FREEFALL_LAND,
        animation,
        AIR_STEP_CHECK_LEDGE_GRAB,
    );
    0
}

/// The held object's oInteractionSubtype & INT_SUBTYPE_HOLDABLE_NPC (the
/// original dereferences heldObj).
fn holding_npc(m: &MarioState, w: &StepWorld<'_>) -> bool {
    let held = m
        .held_obj
        .expect("Mario holds no object (the original dereferences NULL)");
    object(&w.objects, &m.obj, held)
        .raw
        .u32(O_INTERACTION_SUBTYPE)
        & INT_SUBTYPE_HOLDABLE_NPC
        != 0
}

fn act_hold_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if told_to_drop(m) {
        return drop_and_set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 && !holding_npc(m, w) {
        return set_mario_action(m, w, ACT_AIR_THROW, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return drop_and_set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    common_air_action_step(
        m,
        w,
        ACT_HOLD_JUMP_LAND,
        MARIO_ANIM_JUMP_WITH_LIGHT_OBJ,
        AIR_STEP_CHECK_LEDGE_GRAB,
    );
    0
}

fn act_hold_freefall(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let animation = if m.action_arg == 0 {
        MARIO_ANIM_FALL_WITH_LIGHT_OBJ
    } else {
        MARIO_ANIM_FALL_FROM_SLIDING_WITH_LIGHT_OBJ
    };
    if told_to_drop(m) {
        return drop_and_set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_B_PRESSED != 0 && !holding_npc(m, w) {
        return set_mario_action(m, w, ACT_AIR_THROW, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return drop_and_set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    common_air_action_step(
        m,
        w,
        ACT_HOLD_FREEFALL_LAND,
        animation,
        AIR_STEP_CHECK_LEDGE_GRAB,
    );
    0
}

fn act_side_flip(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_DIVE, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    if common_air_action_step(
        m,
        w,
        ACT_SIDE_FLIP_LAND,
        MARIO_ANIM_SLIDEFLIP,
        AIR_STEP_CHECK_LEDGE_GRAB,
    ) != AIR_STEP_GRABBED_LEDGE
    {
        m.obj.gfx.angle[1] = m.obj.gfx.angle[1].wrapping_add(i16::MIN);
    }
    if m.obj.gfx.anim.anim_frame == 6 {
        w.play_sound(SOUND_ACTION_SIDE_FLIP_UNK);
    }
    0
}

fn act_wall_kick_air(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_DIVE, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_jump_sound(m, w);
    common_air_action_step(
        m,
        w,
        ACT_JUMP_LAND,
        MARIO_ANIM_SLIDEJUMP,
        AIR_STEP_CHECK_LEDGE_GRAB,
    );
    0
}

fn act_long_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let animation = if m.obj.raw.s32(O_MARIO_LONG_JUMP_IS_SLOW) == 0 {
        MARIO_ANIM_FAST_LONGJUMP
    } else {
        MARIO_ANIM_SLOW_LONGJUMP
    };
    play_mario_sound(m, w, TERRAIN_JUMP, SOUND_MARIO_YAHOO as i32);
    if floor(m, w).surface_type == SURFACE_VERTICAL_WIND && m.action_state == 0 {
        w.play_sound(SOUND_MARIO_HERE_WE_GO);
        m.action_state = 1;
    }
    common_air_action_step(
        m,
        w,
        ACT_LONG_JUMP_LAND,
        animation,
        AIR_STEP_CHECK_LEDGE_GRAB,
    );
    0
}

fn act_twirling(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let start_twirl_yaw = m.twirl_yaw;
    let yaw_vel_target = if m.input & INPUT_A_DOWN != 0 {
        0x2000
    } else {
        0x1800
    };
    m.angle_vel[1] = approach_s32(i32::from(m.angle_vel[1]), yaw_vel_target, 0x200, 0x200) as i16;
    m.twirl_yaw = m.twirl_yaw.wrapping_add(m.angle_vel[1]);
    let animation = if m.action_arg == 0 {
        MARIO_ANIM_START_TWIRL
    } else {
        MARIO_ANIM_TWIRL
    };
    set_mario_animation(m, w, animation);
    if is_anim_past_end(m, w) {
        m.action_arg = 1;
    }
    if start_twirl_yaw > m.twirl_yaw {
        w.play_sound(SOUND_ACTION_TWIRL);
    }
    update_lava_boost_or_twirling(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_TWIRL_LAND, 0);
        }
        AIR_STEP_HIT_WALL => mario_bonk_reflection(m, w, false),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    m.obj.gfx.angle[1] = m.obj.gfx.angle[1].wrapping_add(m.twirl_yaw);
    0
}

fn act_dive(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.action_arg == 0 {
        play_mario_sound(m, w, SOUND_ACTION_THROW as i32, SOUND_MARIO_HOOHOO as i32);
    } else {
        play_mario_sound(m, w, TERRAIN_JUMP, 0);
    }
    set_mario_animation(m, w, MARIO_ANIM_DIVE);
    if mario_check_object_grab(m, w) {
        mario_grab_used_object(m, w);
        m.body.grab_pos = GRAB_POS_LIGHT_OBJ;
        if m.action != ACT_DIVE {
            return 1;
        }
    }
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_NONE => {
            if m.vel[1] < 0.0 && m.face_angle[0] > -0x2AAA {
                m.face_angle[0] = m.face_angle[0].wrapping_sub(0x200);
                if m.face_angle[0] < -0x2AAA {
                    m.face_angle[0] = -0x2AAA;
                }
            }
            m.obj.gfx.angle[0] = m.face_angle[0].wrapping_neg();
        }
        AIR_STEP_LANDED => {
            if should_get_stuck_in_ground(m, w) && m.face_angle[0] == -0x2AAA {
                w.play_sound(SOUND_MARIO_OOOF2);
                m.particle_flags |= PARTICLE_MIST_CIRCLE;
                drop_and_set_mario_action(m, w, ACT_HEAD_STUCK_IN_GROUND, 0);
            } else if check_fall_damage(m, w, ACT_HARD_FORWARD_GROUND_KB) == 0 {
                if m.held_obj.is_none() {
                    set_mario_action(m, w, ACT_DIVE_SLIDE, 0);
                } else {
                    set_mario_action(m, w, ACT_DIVE_PICKING_UP, 0);
                }
            }
            m.face_angle[0] = 0;
        }
        AIR_STEP_HIT_WALL => {
            mario_bonk_reflection(m, w, true);
            m.face_angle[0] = 0;
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            drop_and_set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    0
}

fn act_air_throw(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer == 4 {
        mario_throw_held_object(m, w);
    }
    play_sound_if_no_flag(m, w, SOUND_MARIO_WAH2, MARIO_MARIO_SOUND_PLAYED);
    set_mario_animation(m, w, MARIO_ANIM_THROW_LIGHT_OBJECT);
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if check_fall_damage_or_get_stuck(m, w, ACT_HARD_BACKWARD_GROUND_KB) == 0 {
                m.action = ACT_AIR_THROW_LAND;
            }
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, 0.0),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    0
}

fn act_water_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.forward_vel < 15.0 {
        mario_set_forward_vel(m, w, 15.0);
    }
    play_mario_sound(m, w, SOUND_ACTION_UNKNOWN432 as i32, 0);
    set_mario_animation(m, w, MARIO_ANIM_SINGLE_JUMP);
    match perform_air_step(m, w, AIR_STEP_CHECK_LEDGE_GRAB) {
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_JUMP_LAND, 0);
            reset_camera_mode(w);
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, 15.0),
        AIR_STEP_GRABBED_LEDGE => {
            set_mario_animation(m, w, MARIO_ANIM_IDLE_ON_LEDGE);
            set_mario_action(m, w, ACT_LEDGE_GRAB, 0);
            reset_camera_mode(w);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    0
}

fn act_hold_water_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if told_to_drop(m) {
        return drop_and_set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.forward_vel < 15.0 {
        mario_set_forward_vel(m, w, 15.0);
    }
    play_mario_sound(m, w, SOUND_ACTION_UNKNOWN432 as i32, 0);
    set_mario_animation(m, w, MARIO_ANIM_JUMP_WITH_LIGHT_OBJ);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_HOLD_JUMP_LAND, 0);
            reset_camera_mode(w);
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, 15.0),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    0
}

fn act_steep_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_DIVE, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    let forward_vel = 0.98 * m.forward_vel;
    mario_set_forward_vel(m, w, forward_vel);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if check_fall_damage_or_get_stuck(m, w, ACT_HARD_BACKWARD_GROUND_KB) == 0 {
                m.face_angle[0] = 0;
                let action = if m.forward_vel < 0.0 {
                    ACT_BEGIN_SLIDING
                } else {
                    ACT_JUMP_LAND
                };
                set_mario_action(m, w, action, 0);
            }
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, 0.0),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    set_mario_animation(m, w, MARIO_ANIM_SINGLE_JUMP);
    m.obj.gfx.angle[1] = m.obj.raw.s32(O_MARIO_STEEP_JUMP_YAW) as i16;
    0
}

fn act_ground_pound(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    play_sound_if_no_flag(m, w, SOUND_ACTION_THROW, MARIO_ACTION_SOUND_PLAYED);
    if m.action_state == 0 {
        if m.action_timer < 10 {
            let y_offset = (20 - 2 * i32::from(m.action_timer)) as f32;
            if m.pos[1] + y_offset + 160.0 < m.ceil_height {
                m.pos[1] += y_offset;
                m.peak_height = m.pos[1];
                m.obj.gfx.pos = m.pos;
            }
        }
        m.vel[1] = -50.0;
        mario_set_forward_vel(m, w, 0.0);
        let animation = if m.action_arg == 0 {
            MARIO_ANIM_START_GROUND_POUND
        } else {
            MARIO_ANIM_TRIPLE_JUMP_GROUND_POUND
        };
        set_mario_animation(m, w, animation);
        if m.action_timer == 0 {
            w.play_sound(SOUND_ACTION_SPIN);
        }
        m.action_timer = m.action_timer.wrapping_add(1);
        if i32::from(m.action_timer) >= i32::from(cur_anim(&m.obj.gfx.anim, w).loop_end) + 4 {
            w.play_sound(SOUND_MARIO_GROUND_POUND_WAH);
            m.action_state = 1;
        }
    } else {
        set_mario_animation(m, w, MARIO_ANIM_GROUND_POUND);
        let step_result = perform_air_step(m, w, 0);
        if step_result == AIR_STEP_LANDED {
            if should_get_stuck_in_ground(m, w) {
                w.play_sound(SOUND_MARIO_OOOF2);
                m.particle_flags |= PARTICLE_MIST_CIRCLE;
                set_mario_action(m, w, ACT_BUTT_STUCK_IN_GROUND, 0);
            } else {
                play_mario_heavy_landing_sound(m, w, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
                if check_fall_damage(m, w, ACT_HARD_BACKWARD_GROUND_KB) == 0 {
                    m.particle_flags |= PARTICLE_MIST_CIRCLE | PARTICLE_HORIZONTAL_STAR;
                    set_mario_action(m, w, ACT_GROUND_POUND_LAND, 0);
                }
            }
            w.event(Event::CameraShake(SHAKE_GROUND_POUND));
        } else if step_result == AIR_STEP_HIT_WALL {
            mario_set_forward_vel(m, w, -16.0);
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
        }
    }
    0
}

/// The burn timer and health loss shared by the burning actions.
fn burn_health(m: &mut MarioState) {
    let timer = m.obj.raw.s32(O_MARIO_BURN_TIMER).wrapping_add(3);
    m.obj.raw.set_s32(O_MARIO_BURN_TIMER, timer);
    m.health = m.health.wrapping_sub(10);
    if m.health < 0x100 {
        m.health = 0xFF;
    }
}

fn act_burning_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    play_mario_sound(m, w, TERRAIN_JUMP, if m.action_arg == 0 { 0 } else { -1 });
    let forward_vel = m.forward_vel;
    mario_set_forward_vel(m, w, forward_vel);
    if perform_air_step(m, w, 0) == AIR_STEP_LANDED {
        play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        set_mario_action(m, w, ACT_BURNING_GROUND, 0);
    }
    let animation = if m.action_arg == 0 {
        MARIO_ANIM_SINGLE_JUMP
    } else {
        MARIO_ANIM_FIRE_LAVA_BURN
    };
    set_mario_animation(m, w, animation);
    m.particle_flags |= PARTICLE_FIRE;
    w.play_sound(SOUND_MOVING_LAVA_BURN);
    burn_health(m);
    0
}

fn act_burning_fall(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let forward_vel = m.forward_vel;
    mario_set_forward_vel(m, w, forward_vel);
    if perform_air_step(m, w, 0) == AIR_STEP_LANDED {
        play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        set_mario_action(m, w, ACT_BURNING_GROUND, 0);
    }
    set_mario_animation(m, w, MARIO_ANIM_GENERAL_FALL);
    m.particle_flags |= PARTICLE_FIRE;
    burn_health(m);
    0
}

fn act_crazy_box_bounce(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.action_timer == 0 {
        let min_speed = match m.action_arg {
            0 => {
                m.vel[1] = 45.0;
                32.0
            }
            1 => {
                m.vel[1] = 60.0;
                36.0
            }
            2 => {
                m.vel[1] = 100.0;
                48.0
            }
            arg => panic!("act_crazy_box_bounce reads an uninitialized speed for argument {arg}"),
        };
        w.play_sound(if min_speed < 40.0 {
            SOUND_GENERAL_BOING1
        } else {
            SOUND_GENERAL_BOING2
        });
        if m.forward_vel < min_speed {
            mario_set_forward_vel(m, w, min_speed);
        }
        m.action_timer = 1;
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    set_mario_animation(m, w, MARIO_ANIM_DIVE);
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if m.action_arg < 2 {
                let arg = m.action_arg + 1;
                set_mario_action(m, w, ACT_CRAZY_BOX_BOUNCE, arg);
            } else {
                panic!(
                    "the last crazy box bounce releases the held box; objects are not simulated yet"
                );
            }
            m.particle_flags |= PARTICLE_MIST_CIRCLE;
        }
        AIR_STEP_HIT_WALL => mario_bonk_reflection(m, w, false),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    m.obj.gfx.angle[0] = w.trig.atan2s(m.forward_vel, -m.vel[1]);
    0
}

/// common_air_knockback_step.
fn common_air_knockback_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    land_action: u32,
    hard_fall_action: u32,
    animation: i32,
    speed: f32,
) -> u32 {
    mario_set_forward_vel(m, w, speed);
    let step_result = perform_air_step(m, w, 0);
    match step_result {
        AIR_STEP_NONE => {
            set_mario_animation(m, w, animation);
        }
        AIR_STEP_LANDED => {
            if check_fall_damage_or_get_stuck(m, w, hard_fall_action) == 0 {
                let arg = if m.action == ACT_THROWN_FORWARD || m.action == ACT_THROWN_BACKWARD {
                    u32::from(m.hurt_counter)
                } else {
                    m.action_arg
                };
                set_mario_action(m, w, land_action, arg);
            }
        }
        AIR_STEP_HIT_WALL => {
            set_mario_animation(m, w, MARIO_ANIM_BACKWARD_AIR_KB);
            mario_bonk_reflection(m, w, false);
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            mario_set_forward_vel(m, w, -speed);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    step_result
}

/// check_wall_kick.
fn check_wall_kick(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_A_PRESSED != 0 && m.wall_kick_timer != 0 && m.prev_action == ACT_AIR_HIT_WALL
    {
        m.face_angle[1] = m.face_angle[1].wrapping_add(i16::MIN);
        return set_mario_action(m, w, ACT_WALL_KICK_AIR, 0);
    }
    0
}

/// The knockback actions pass animation IDs as literals in the original.
const ANIM_0X02: i32 = 0x0002;
const ANIM_0X2D: i32 = 0x002D;
const ANIM_0X56: i32 = 0x0056;

fn act_backward_air_kb(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_wall_kick(m, w) != 0 {
        return 1;
    }
    play_knockback_sound(m, w);
    common_air_knockback_step(
        m,
        w,
        ACT_BACKWARD_GROUND_KB,
        ACT_HARD_BACKWARD_GROUND_KB,
        ANIM_0X02,
        -16.0,
    );
    0
}

fn act_forward_air_kb(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_wall_kick(m, w) != 0 {
        return 1;
    }
    play_knockback_sound(m, w);
    common_air_knockback_step(
        m,
        w,
        ACT_FORWARD_GROUND_KB,
        ACT_HARD_FORWARD_GROUND_KB,
        ANIM_0X2D,
        16.0,
    );
    0
}

fn act_hard_backward_air_kb(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    play_knockback_sound(m, w);
    common_air_knockback_step(
        m,
        w,
        ACT_HARD_BACKWARD_GROUND_KB,
        ACT_HARD_BACKWARD_GROUND_KB,
        ANIM_0X02,
        -16.0,
    );
    0
}

fn act_hard_forward_air_kb(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    play_knockback_sound(m, w);
    common_air_knockback_step(
        m,
        w,
        ACT_HARD_FORWARD_GROUND_KB,
        ACT_HARD_FORWARD_GROUND_KB,
        ANIM_0X2D,
        16.0,
    );
    0
}

fn act_thrown_backward(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let land_action = if m.action_arg != 0 {
        ACT_HARD_BACKWARD_GROUND_KB
    } else {
        ACT_BACKWARD_GROUND_KB
    };
    play_sound_if_no_flag(m, w, SOUND_MARIO_WAAAOOOW, MARIO_MARIO_SOUND_PLAYED);
    let speed = m.forward_vel;
    common_air_knockback_step(
        m,
        w,
        land_action,
        ACT_HARD_BACKWARD_GROUND_KB,
        ANIM_0X02,
        speed,
    );
    m.forward_vel *= 0.98;
    0
}

fn act_thrown_forward(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let land_action = if m.action_arg != 0 {
        ACT_HARD_FORWARD_GROUND_KB
    } else {
        ACT_FORWARD_GROUND_KB
    };
    play_sound_if_no_flag(m, w, SOUND_MARIO_WAAAOOOW, MARIO_MARIO_SOUND_PLAYED);
    let speed = m.forward_vel;
    if common_air_knockback_step(
        m,
        w,
        land_action,
        ACT_HARD_FORWARD_GROUND_KB,
        ANIM_0X2D,
        speed,
    ) == AIR_STEP_NONE
    {
        let mut pitch = w.trig.atan2s(m.forward_vel, -m.vel[1]);
        if pitch > 0x1800 {
            pitch = 0x1800;
        }
        m.obj.gfx.angle[0] = pitch.wrapping_add(0x1800);
    }
    m.forward_vel *= 0.98;
    0
}

fn act_soft_bonk(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_wall_kick(m, w) != 0 {
        return 1;
    }
    play_knockback_sound(m, w);
    let speed = m.forward_vel;
    common_air_knockback_step(
        m,
        w,
        ACT_FREEFALL_LAND,
        ACT_HARD_BACKWARD_GROUND_KB,
        ANIM_0X56,
        speed,
    );
    0
}

fn act_getting_blown(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.action_state == 0 {
        if m.forward_vel > -60.0 {
            m.forward_vel -= 6.0;
        } else {
            m.action_state = 1;
        }
    } else {
        if m.forward_vel < -16.0 {
            m.forward_vel += 0.8;
        }
        if m.vel[1] < 0.0 && m.getting_blown_gravity < 4.0 {
            m.getting_blown_gravity += 0.05;
        }
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer == 20 {
        mario_blow_off_cap(m, 50.0);
    }
    let forward_vel = m.forward_vel;
    mario_set_forward_vel(m, w, forward_vel);
    set_mario_animation(m, w, MARIO_ANIM_BACKWARD_AIR_KB);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_HARD_BACKWARD_AIR_KB, 0);
        }
        AIR_STEP_HIT_WALL => {
            set_mario_animation(m, w, MARIO_ANIM_AIR_FORWARD_KB);
            mario_bonk_reflection(m, w, false);
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            let forward_vel = -m.forward_vel;
            mario_set_forward_vel(m, w, forward_vel);
        }
        _ => {}
    }
    0
}

fn act_air_hit_wall(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.held_obj.is_some() {
        mario_drop_held_object(m, w);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer <= 2 {
        if m.input & INPUT_A_PRESSED != 0 {
            m.vel[1] = 52.0;
            m.face_angle[1] = m.face_angle[1].wrapping_add(i16::MIN);
            return set_mario_action(m, w, ACT_WALL_KICK_AIR, 0);
        }
    } else if m.forward_vel >= 38.0 {
        m.wall_kick_timer = 5;
        if m.vel[1] > 0.0 {
            m.vel[1] = 0.0;
        }
        m.particle_flags |= PARTICLE_VERTICAL_STAR;
        return set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
    } else {
        m.wall_kick_timer = 5;
        if m.vel[1] > 0.0 {
            m.vel[1] = 0.0;
        }
        if m.forward_vel > 8.0 {
            mario_set_forward_vel(m, w, -8.0);
        }
        return set_mario_action(m, w, ACT_SOFT_BONK, 0);
    }
    // Missing return statement: the result of set_mario_animation is
    // returned (AVOID_UB makes this explicit). A nonzero frame makes the
    // action run again in the same tick.
    i32::from(set_mario_animation(m, w, MARIO_ANIM_START_WALLKICK))
}

fn act_rollout(m: &mut MarioState, w: &mut StepWorld<'_>, spinning: i32) {
    if m.action_state == 0 {
        m.vel[1] = 30.0;
        m.action_state = 1;
    }
    play_mario_sound(m, w, TERRAIN_JUMP, 0);
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_NONE => {
            if m.action_state == 1 {
                if set_mario_animation(m, w, spinning) == 4 {
                    w.play_sound(SOUND_ACTION_SPIN);
                }
            } else {
                set_mario_animation(m, w, MARIO_ANIM_GENERAL_FALL);
            }
        }
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_FREEFALL_LAND_STOP, 0);
            play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, 0.0),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
}

fn act_forward_rollout(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    act_rollout(m, w, MARIO_ANIM_FORWARD_SPINNING);
    if m.action_state == 1 && is_anim_past_end(m, w) {
        m.action_state = 2;
    }
    0
}

fn act_backward_rollout(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    act_rollout(m, w, MARIO_ANIM_BACKWARD_SPINNING);
    if m.action_state == 1 && m.obj.gfx.anim.anim_frame == 2 {
        m.action_state = 2;
    }
    0
}

fn act_hold_butt_slide_air(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if told_to_drop(m) {
        return drop_and_set_mario_action(m, w, ACT_HOLD_FREEFALL, 1);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer > 30 && m.pos[1] - m.floor_height > 500.0 {
        return set_mario_action(m, w, ACT_HOLD_FREEFALL, 1);
    }
    update_air_with_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if m.action_state == 0 && m.vel[1] < 0.0 && floor(m, w).normal[1] >= 0.9848077 {
                m.vel[1] = -m.vel[1] / 2.0;
                m.action_state = 1;
            } else {
                set_mario_action(m, w, ACT_HOLD_BUTT_SLIDE, 0);
            }
            play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        }
        AIR_STEP_HIT_WALL => {
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            mario_drop_held_object(m, w);
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    set_mario_animation(m, w, MARIO_ANIM_SLIDING_ON_BOTTOM_WITH_LIGHT_OBJ);
    0
}

fn act_butt_slide_air(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer > 30 && m.pos[1] - m.floor_height > 500.0 {
        return set_mario_action(m, w, ACT_FREEFALL, 1);
    }
    update_air_with_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if m.action_state == 0 && m.vel[1] < 0.0 && floor(m, w).normal[1] >= 0.9848077 {
                m.vel[1] = -m.vel[1] / 2.0;
                m.action_state = 1;
            } else {
                set_mario_action(m, w, ACT_BUTT_SLIDE, 0);
            }
            play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        }
        AIR_STEP_HIT_WALL => {
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    set_mario_animation(m, w, MARIO_ANIM_SLIDE);
    0
}

fn act_lava_boost(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    play_sound_if_no_flag(m, w, SOUND_MARIO_ON_FIRE, MARIO_MARIO_SOUND_PLAYED);
    if m.input & INPUT_NONZERO_ANALOG == 0 {
        m.forward_vel = approach_f32(m.forward_vel, 0.0, 0.35, 0.35);
    }
    update_lava_boost_or_twirling(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if floor(m, w).surface_type == SURFACE_BURNING {
                m.action_state = 0;
                if m.flags & MARIO_METAL_CAP == 0 {
                    m.hurt_counter = m.hurt_counter.wrapping_add(cap_damage(m, 12, 18));
                }
                m.vel[1] = 84.0;
                w.play_sound(SOUND_MARIO_ON_FIRE);
            } else {
                play_mario_heavy_landing_sound(m, w, SOUND_ACTION_TERRAIN_BODY_HIT_GROUND);
                if m.action_state < 2 && m.vel[1] < 0.0 {
                    m.vel[1] = -m.vel[1] * 0.4;
                    let forward_vel = m.forward_vel * 0.5;
                    mario_set_forward_vel(m, w, forward_vel);
                    m.action_state += 1;
                } else {
                    set_mario_action(m, w, ACT_LAVA_BOOST_LAND, 0);
                }
            }
        }
        AIR_STEP_HIT_WALL => mario_bonk_reflection(m, w, false),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    set_mario_animation(m, w, MARIO_ANIM_FIRE_LAVA_BURN);
    if w.area_terrain_type & TERRAIN_MASK != TERRAIN_SNOW
        && m.flags & MARIO_METAL_CAP == 0
        && m.vel[1] > 0.0
    {
        m.particle_flags |= PARTICLE_FIRE;
        if m.action_state == 0 {
            w.play_sound(SOUND_MOVING_LAVA_BURN);
        }
    }
    if m.health < 0x100 {
        level_trigger_warp(w, WARP_OP_DEATH);
    }
    m.body.eye_state = MARIO_EYES_DEAD;
    0
}

fn act_slide_kick(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.action_state == 0 && m.action_timer == 0 {
        play_mario_sound(m, w, TERRAIN_JUMP, SOUND_MARIO_HOOHOO as i32);
        set_mario_animation(m, w, MARIO_ANIM_SLIDE_KICK);
    }
    m.action_timer = m.action_timer.wrapping_add(1);
    if m.action_timer > 30 && m.pos[1] - m.floor_height > 500.0 {
        return set_mario_action(m, w, ACT_FREEFALL, 2);
    }
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_NONE => {
            if m.action_state == 0 {
                m.obj.gfx.angle[0] = w.trig.atan2s(m.forward_vel, -m.vel[1]);
                if m.obj.gfx.angle[0] > 0x1800 {
                    m.obj.gfx.angle[0] = 0x1800;
                }
            }
        }
        AIR_STEP_LANDED => {
            if m.action_state == 0 && m.vel[1] < 0.0 {
                m.vel[1] = -m.vel[1] / 2.0;
                m.action_state = 1;
                m.action_timer = 0;
            } else {
                set_mario_action(m, w, ACT_SLIDE_KICK_SLIDE, 0);
            }
            play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        }
        AIR_STEP_HIT_WALL => {
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    0
}

fn act_jump_kick(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.action_state == 0 {
        play_sound_if_no_flag(m, w, SOUND_MARIO_PUNCH_HOO, MARIO_ACTION_SOUND_PLAYED);
        m.obj.gfx.anim.anim_id = -1;
        set_mario_animation(m, w, MARIO_ANIM_AIR_KICK);
        m.action_state = 1;
    }
    let anim_frame = i32::from(m.obj.gfx.anim.anim_frame);
    if anim_frame == 0 {
        m.body.punch_state = (2 << 6) | 6;
    }
    if (0..8).contains(&anim_frame) {
        m.flags |= MARIO_KICKING;
    }
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if check_fall_damage_or_get_stuck(m, w, ACT_HARD_BACKWARD_GROUND_KB) == 0 {
                set_mario_action(m, w, ACT_FREEFALL_LAND, 0);
            }
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, 0.0),
        _ => {}
    }
    0
}

fn act_shot_from_cannon(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if camera_mode(w) != CAMERA_MODE_BEHIND_MARIO {
        m.camera_status.camera_event = CAM_EVENT_SHOT_FROM_CANNON;
    }
    let forward_vel = m.forward_vel;
    mario_set_forward_vel(m, w, forward_vel);
    play_sound_if_no_flag(m, w, SOUND_MARIO_YAHOO, MARIO_MARIO_SOUND_PLAYED);
    match perform_air_step(m, w, 0) {
        AIR_STEP_NONE => {
            set_mario_animation(m, w, MARIO_ANIM_AIRBORNE_ON_STOMACH);
            m.face_angle[0] = w.trig.atan2s(m.forward_vel, m.vel[1]);
            m.obj.gfx.angle[0] = m.face_angle[0].wrapping_neg();
        }
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_DIVE_SLIDE, 0);
            m.face_angle[0] = 0;
            reset_camera_mode(w);
        }
        AIR_STEP_HIT_WALL => {
            mario_set_forward_vel(m, w, -16.0);
            m.face_angle[0] = 0;
            if m.vel[1] > 0.0 {
                m.vel[1] = 0.0;
            }
            m.particle_flags |= PARTICLE_VERTICAL_STAR;
            set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
            reset_camera_mode(w);
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    if m.flags & MARIO_WING_CAP != 0 && m.vel[1] < 0.0 {
        set_mario_action(m, w, ACT_FLYING, 0);
    }
    // 0.05 is a double constant: the subtraction happens in double.
    m.forward_vel = (f64::from(m.forward_vel) - 0.05) as f32;
    if m.forward_vel < 10.0 {
        mario_set_forward_vel(m, w, 10.0);
    }
    if m.vel[1] > 0.0 {
        m.particle_flags |= PARTICLE_DUST;
    }
    0
}

fn act_flying(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let start_pitch = m.face_angle[0];
    if m.input & INPUT_Z_PRESSED != 0 {
        if camera_mode(w) == CAMERA_MODE_BEHIND_MARIO {
            reset_camera_mode(w);
        }
        return set_mario_action(m, w, ACT_GROUND_POUND, 1);
    }
    if m.flags & MARIO_WING_CAP == 0 {
        if camera_mode(w) == CAMERA_MODE_BEHIND_MARIO {
            reset_camera_mode(w);
        }
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if camera_mode(w) != CAMERA_MODE_BEHIND_MARIO {
        set_camera_mode(w, CAMERA_MODE_BEHIND_MARIO, 1);
    }
    if m.action_state == 0 {
        if m.action_arg == 0 {
            set_mario_animation(m, w, MARIO_ANIM_FLY_FROM_CANNON);
        } else {
            set_mario_animation(m, w, MARIO_ANIM_FORWARD_SPINNING_FLIP);
            if m.obj.gfx.anim.anim_frame == 1 {
                w.play_sound(SOUND_ACTION_SPIN);
            }
        }
        if is_anim_at_end(m, w) {
            if m.action_arg == 2 {
                w.event(Event::LevelInitText(0));
                m.action_arg = 1;
            }
            set_mario_animation(m, w, MARIO_ANIM_WING_CAP_FLY);
            m.action_state = 1;
        }
    }
    update_flying(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_NONE => {
            m.obj.gfx.angle[0] = m.face_angle[0].wrapping_neg();
            m.obj.gfx.angle[2] = m.face_angle[2];
            m.action_timer = 0;
        }
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_DIVE_SLIDE, 0);
            set_mario_animation(m, w, MARIO_ANIM_DIVE);
            set_anim_to_frame(m, w, 7);
            m.face_angle[0] = 0;
            reset_camera_mode(w);
        }
        AIR_STEP_HIT_WALL => {
            if m.wall.is_some() {
                mario_set_forward_vel(m, w, -16.0);
                m.face_angle[0] = 0;
                if m.vel[1] > 0.0 {
                    m.vel[1] = 0.0;
                }
                w.play_sound(if m.flags & MARIO_METAL_CAP != 0 {
                    SOUND_ACTION_METAL_BONK
                } else {
                    SOUND_ACTION_BONK
                });
                m.particle_flags |= PARTICLE_VERTICAL_STAR;
                set_mario_action(m, w, ACT_BACKWARD_AIR_KB, 0);
                reset_camera_mode(w);
            } else {
                let timer = m.action_timer;
                m.action_timer = m.action_timer.wrapping_add(1);
                if timer == 0 {
                    w.play_sound(SOUND_ACTION_HIT);
                }
                if m.action_timer == 30 {
                    m.action_timer = 0;
                }
                m.face_angle[0] = m.face_angle[0].wrapping_sub(0x200);
                if m.face_angle[0] < -0x2AAA {
                    m.face_angle[0] = -0x2AAA;
                }
                m.obj.gfx.angle[0] = m.face_angle[0].wrapping_neg();
                m.obj.gfx.angle[2] = m.face_angle[2];
            }
        }
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    if m.face_angle[0] > 0x800 && m.forward_vel >= 48.0 {
        m.particle_flags |= PARTICLE_DUST;
    }
    if start_pitch <= 0 && m.face_angle[0] > 0 && m.forward_vel >= 48.0 {
        w.play_sound(SOUND_ACTION_FLYING_FAST);
        w.play_sound(SOUND_MARIO_YAHOO_WAHA_YIPPEE.wrapping_add((w.audio_random % 5) << 16));
    }
    w.play_sound(SOUND_MOVING_FLYING);
    adjust_sound_for_speed(m, w);
    0
}

fn act_flying_triple_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & (INPUT_B_PRESSED | INPUT_Z_PRESSED) != 0 {
        if camera_mode(w) == CAMERA_MODE_BEHIND_MARIO {
            reset_camera_mode(w);
        }
        let action = if m.input & INPUT_B_PRESSED != 0 {
            ACT_DIVE
        } else {
            ACT_GROUND_POUND
        };
        return set_mario_action(m, w, action, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, SOUND_MARIO_YAHOO as i32);
    if m.action_state == 0 {
        set_mario_animation(m, w, MARIO_ANIM_TRIPLE_JUMP_FLY);
        if m.obj.gfx.anim.anim_frame == 7 {
            w.play_sound(SOUND_ACTION_SPIN);
        }
        if is_anim_past_end(m, w) {
            set_mario_animation(m, w, MARIO_ANIM_FORWARD_SPINNING);
            m.action_state = 1;
        }
    }
    if m.action_state == 1 && m.obj.gfx.anim.anim_frame == 1 {
        w.play_sound(SOUND_ACTION_SPIN);
    }
    if m.vel[1] < 4.0 {
        if camera_mode(w) != CAMERA_MODE_BEHIND_MARIO {
            set_camera_mode(w, CAMERA_MODE_BEHIND_MARIO, 1);
        }
        if m.forward_vel < 32.0 {
            mario_set_forward_vel(m, w, 32.0);
        }
        // The action continues this tick as ACT_FLYING (no gravity below).
        set_mario_action(m, w, ACT_FLYING, 1);
    }
    let timer = m.action_timer;
    m.action_timer = m.action_timer.wrapping_add(1);
    if timer == 10 && camera_mode(w) != CAMERA_MODE_BEHIND_MARIO {
        set_camera_mode(w, CAMERA_MODE_BEHIND_MARIO, 1);
    }
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            if check_fall_damage_or_get_stuck(m, w, ACT_HARD_BACKWARD_GROUND_KB) == 0 {
                set_mario_action(m, w, ACT_DOUBLE_JUMP_LAND, 0);
            }
        }
        AIR_STEP_HIT_WALL => mario_bonk_reflection(m, w, false),
        AIR_STEP_HIT_LAVA_WALL => {
            lava_boost_on_wall(m, w);
        }
        _ => {}
    }
    0
}

fn act_top_of_pole_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    play_mario_jump_sound(m, w);
    common_air_action_step(
        m,
        w,
        ACT_FREEFALL_LAND,
        MARIO_ANIM_HANDSTAND_JUMP,
        AIR_STEP_CHECK_LEDGE_GRAB,
    );
    0
}

fn act_vertical_wind(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let d_yaw = intended_d_yaw(m);
    let intended_mag = m.intended_mag / 32.0;
    play_sound_if_no_flag(m, w, SOUND_MARIO_HERE_WE_GO, MARIO_MARIO_SOUND_PLAYED);
    if m.action_state == 0 {
        set_mario_animation(m, w, MARIO_ANIM_FORWARD_SPINNING_FLIP);
        if m.obj.gfx.anim.anim_frame == 1 {
            w.play_sound(SOUND_ACTION_SPIN);
        }
        if is_anim_past_end(m, w) {
            m.action_state = 1;
        }
    } else {
        set_mario_animation(m, w, MARIO_ANIM_AIRBORNE_ON_STOMACH);
    }
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            set_mario_action(m, w, ACT_DIVE_SLIDE, 0);
        }
        AIR_STEP_HIT_WALL => mario_set_forward_vel(m, w, -16.0),
        _ => {}
    }
    m.obj.gfx.angle[0] = f32_to_s16(6144.0 * intended_mag * w.trig.coss(d_yaw));
    m.obj.gfx.angle[2] = f32_to_s16(-4096.0 * intended_mag * w.trig.sins(d_yaw));
    0
}

fn act_special_triple_jump(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_B_PRESSED != 0 {
        return set_mario_action(m, w, ACT_DIVE, 0);
    }
    if m.input & INPUT_Z_PRESSED != 0 {
        return set_mario_action(m, w, ACT_GROUND_POUND, 0);
    }
    play_mario_sound(m, w, TERRAIN_JUMP, SOUND_MARIO_YAHOO as i32);
    update_air_without_turn(m, w);
    match perform_air_step(m, w, 0) {
        AIR_STEP_LANDED => {
            let state = m.action_state;
            m.action_state = m.action_state.wrapping_add(1);
            if state == 0 {
                m.vel[1] = 42.0;
            } else {
                set_mario_action(m, w, ACT_FREEFALL_LAND_STOP, 0);
            }
            play_mario_landing_sound(m, w, SOUND_ACTION_TERRAIN_LANDING);
        }
        AIR_STEP_HIT_WALL => mario_bonk_reflection(m, w, true),
        _ => {}
    }
    if m.action_state == 0 || m.vel[1] > 0.0 {
        if set_mario_animation(m, w, MARIO_ANIM_FORWARD_SPINNING) == 0 {
            w.play_sound(SOUND_ACTION_SPIN);
        }
    } else {
        set_mario_animation(m, w, MARIO_ANIM_GENERAL_FALL);
    }
    m.particle_flags |= PARTICLE_SPARKLES;
    0
}

/// check_common_airborne_cancels.
fn check_common_airborne_cancels(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.pos[1] < (i32::from(m.water_level) - 100) as f32 {
        return set_water_plunge_action(m, w);
    }
    if m.input & INPUT_SQUISHED != 0 {
        return drop_and_set_mario_action(m, w, ACT_SQUISHED, 0);
    }
    if floor(m, w).surface_type == SURFACE_VERTICAL_WIND
        && m.action & ACT_FLAG_ALLOW_VERTICAL_WIND_ACTION != 0
    {
        return drop_and_set_mario_action(m, w, ACT_VERTICAL_WIND, 0);
    }
    m.quicksand_depth = 0.0;
    0
}

/// mario_execute_airborne_action.
pub fn mario_execute_airborne_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_common_airborne_cancels(m, w) != 0 {
        return 1;
    }
    play_far_fall_sound(m, w);
    match m.action {
        ACT_JUMP => act_jump(m, w),
        ACT_DOUBLE_JUMP => act_double_jump(m, w),
        ACT_FREEFALL => act_freefall(m, w),
        ACT_SIDE_FLIP => act_side_flip(m, w),
        ACT_WALL_KICK_AIR => act_wall_kick_air(m, w),
        ACT_TWIRLING => act_twirling(m, w),
        ACT_WATER_JUMP => act_water_jump(m, w),
        ACT_STEEP_JUMP => act_steep_jump(m, w),
        ACT_BURNING_JUMP => act_burning_jump(m, w),
        ACT_BURNING_FALL => act_burning_fall(m, w),
        ACT_TRIPLE_JUMP => act_triple_jump(m, w),
        ACT_BACKFLIP => act_backflip(m, w),
        ACT_LONG_JUMP => act_long_jump(m, w),
        ACT_DIVE => act_dive(m, w),
        ACT_AIR_THROW => act_air_throw(m, w),
        ACT_BACKWARD_AIR_KB => act_backward_air_kb(m, w),
        ACT_FORWARD_AIR_KB => act_forward_air_kb(m, w),
        ACT_HARD_FORWARD_AIR_KB => act_hard_forward_air_kb(m, w),
        ACT_HARD_BACKWARD_AIR_KB => act_hard_backward_air_kb(m, w),
        ACT_SOFT_BONK => act_soft_bonk(m, w),
        ACT_AIR_HIT_WALL => act_air_hit_wall(m, w),
        ACT_FORWARD_ROLLOUT => act_forward_rollout(m, w),
        ACT_SHOT_FROM_CANNON => act_shot_from_cannon(m, w),
        ACT_BUTT_SLIDE_AIR => act_butt_slide_air(m, w),
        ACT_LAVA_BOOST => act_lava_boost(m, w),
        ACT_GETTING_BLOWN => act_getting_blown(m, w),
        ACT_BACKWARD_ROLLOUT => act_backward_rollout(m, w),
        ACT_CRAZY_BOX_BOUNCE => act_crazy_box_bounce(m, w),
        ACT_SPECIAL_TRIPLE_JUMP => act_special_triple_jump(m, w),
        ACT_GROUND_POUND => act_ground_pound(m, w),
        ACT_THROWN_FORWARD => act_thrown_forward(m, w),
        ACT_THROWN_BACKWARD => act_thrown_backward(m, w),
        ACT_FLYING_TRIPLE_JUMP => act_flying_triple_jump(m, w),
        ACT_SLIDE_KICK => act_slide_kick(m, w),
        ACT_JUMP_KICK => act_jump_kick(m, w),
        ACT_FLYING => act_flying(m, w),
        ACT_TOP_OF_POLE_JUMP => act_top_of_pole_jump(m, w),
        ACT_VERTICAL_WIND => act_vertical_wind(m, w),
        ACT_HOLD_JUMP => act_hold_jump(m, w),
        ACT_HOLD_FREEFALL => act_hold_freefall(m, w),
        ACT_HOLD_WATER_JUMP => act_hold_water_jump(m, w),
        ACT_HOLD_BUTT_SLIDE_AIR => act_hold_butt_slide_air(m, w),
        ACT_RIDING_SHELL_JUMP | ACT_RIDING_SHELL_FALL | ACT_RIDING_HOOT => panic!(
            "airborne action {:#X} rides an object that is not ported",
            m.action
        ),
        action => panic!("airborne action {action:#X} is not in the original table"),
    }
}
