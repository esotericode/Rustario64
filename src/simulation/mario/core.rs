//! Mario's core update, translated from pinned CC0 src/game/mario.c: audio
//! helpers, floor queries, the action setters and common transitions, health,
//! caps, the per-tick action loop (execute_mario_action), and init_mario. The
//! input stage lives in `inputs`, animation helpers in `animation`, and the
//! step helpers this file defines in the original live in `step`.
//!
//! f32 operation order, s16 wraparound and C promotions follow the original.
//! Calls into sound, the camera and the level runtime are recorded as events.
use super::{
    Event, MarioState, StepWorld, SurfaceRef, Unsupported, airborne, automatic,
    constants::*,
    f32_to_s16, f32_to_s32, interaction, moving, object, stationary,
    step::{self, mario_set_forward_vel},
};

/// play_sound_if_no_flag.
pub fn play_sound_if_no_flag(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    sound_bits: u32,
    flags: u32,
) {
    if m.flags & flags == 0 {
        w.play_sound(sound_bits);
        m.flags |= flags;
    }
}

/// play_mario_jump_sound (US: the triple jump has its own voice set).
pub fn play_mario_jump_sound(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.flags & MARIO_MARIO_SOUND_PLAYED == 0 {
        if m.action == ACT_TRIPLE_JUMP {
            w.play_sound(SOUND_MARIO_YAHOO_WAHA_YIPPEE.wrapping_add((w.audio_random % 5) << 16));
        } else {
            w.play_sound(SOUND_MARIO_YAH_WAH_HOO.wrapping_add((w.audio_random % 3) << 16));
        }
        m.flags |= MARIO_MARIO_SOUND_PLAYED;
    }
}

/// adjust_sound_for_speed.
pub fn adjust_sound_for_speed(m: &MarioState, w: &mut StepWorld<'_>) {
    let abs_forward_vel = f32_to_s32(if m.forward_vel > 0.0 {
        m.forward_vel
    } else {
        -m.forward_vel
    });
    let speed = if abs_forward_vel > 100 {
        100
    } else {
        abs_forward_vel
    };
    w.event(Event::MovingSpeed {
        bank: SOUND_BANK_MOVING,
        speed: speed as u8,
    });
}

/// play_sound_and_spawn_particles.
pub fn play_sound_and_spawn_particles(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    sound_bits: u32,
    wave_particle_type: u32,
) {
    if m.terrain_sound_addend == (SOUND_TERRAIN_WATER as u32) << 16 {
        if wave_particle_type != 0 {
            m.particle_flags |= PARTICLE_SHALLOW_WATER_SPLASH;
        } else {
            m.particle_flags |= PARTICLE_SHALLOW_WATER_WAVE;
        }
    } else if m.terrain_sound_addend == (SOUND_TERRAIN_SAND as u32) << 16 {
        m.particle_flags |= PARTICLE_DIRT;
    } else if m.terrain_sound_addend == (SOUND_TERRAIN_SNOW as u32) << 16 {
        m.particle_flags |= PARTICLE_SNOW;
    }
    if m.flags & MARIO_METAL_CAP != 0
        || sound_bits == SOUND_ACTION_UNSTUCK_FROM_GROUND
        || sound_bits == SOUND_MARIO_PUNCH_HOO
    {
        w.play_sound(sound_bits);
    } else {
        w.play_sound(m.terrain_sound_addend.wrapping_add(sound_bits));
    }
}

/// play_mario_action_sound: once per action.
pub fn play_mario_action_sound(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    sound_bits: u32,
    wave_particle_type: u32,
) {
    if m.flags & MARIO_ACTION_SOUND_PLAYED == 0 {
        play_sound_and_spawn_particles(m, w, sound_bits, wave_particle_type);
        m.flags |= MARIO_ACTION_SOUND_PLAYED;
    }
}

fn metal_or(m: &MarioState, metal: u32, sound_bits: u32) -> u32 {
    if m.flags & MARIO_METAL_CAP != 0 {
        metal
    } else {
        sound_bits
    }
}

/// play_mario_landing_sound.
pub fn play_mario_landing_sound(m: &mut MarioState, w: &mut StepWorld<'_>, sound_bits: u32) {
    let bits = metal_or(m, SOUND_ACTION_METAL_LANDING, sound_bits);
    play_sound_and_spawn_particles(m, w, bits, 1);
}

/// play_mario_landing_sound_once.
pub fn play_mario_landing_sound_once(m: &mut MarioState, w: &mut StepWorld<'_>, sound_bits: u32) {
    let bits = metal_or(m, SOUND_ACTION_METAL_LANDING, sound_bits);
    play_mario_action_sound(m, w, bits, 1);
}

/// play_mario_heavy_landing_sound.
pub fn play_mario_heavy_landing_sound(m: &mut MarioState, w: &mut StepWorld<'_>, sound_bits: u32) {
    let bits = metal_or(m, SOUND_ACTION_METAL_HEAVY_LANDING, sound_bits);
    play_sound_and_spawn_particles(m, w, bits, 1);
}

/// play_mario_heavy_landing_sound_once.
pub fn play_mario_heavy_landing_sound_once(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    sound_bits: u32,
) {
    let bits = metal_or(m, SOUND_ACTION_METAL_HEAVY_LANDING, sound_bits);
    play_mario_action_sound(m, w, bits, 1);
}

/// play_mario_sound: `mario_sound` 0 plays a jump voice, -1 none.
pub fn play_mario_sound(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    action_sound: i32,
    mario_sound: i32,
) {
    if action_sound == SOUND_ACTION_TERRAIN_JUMP as i32 {
        let bits = metal_or(m, SOUND_ACTION_METAL_JUMP, SOUND_ACTION_TERRAIN_JUMP);
        play_mario_action_sound(m, w, bits, 1);
    } else {
        play_sound_if_no_flag(m, w, action_sound as u32, MARIO_ACTION_SOUND_PLAYED);
    }
    if mario_sound == 0 {
        play_mario_jump_sound(m, w);
    }
    if mario_sound != -1 {
        play_sound_if_no_flag(m, w, mario_sound as u32, MARIO_MARIO_SOUND_PLAYED);
    }
}

fn floor_normal_y(m: &MarioState, w: &StepWorld<'_>) -> f32 {
    w.surface(
        m.floor
            .expect("Mario has no floor (the original dereferences NULL)"),
    )
    .normal[1]
}

/// mario_facing_downhill.
pub fn mario_facing_downhill(m: &MarioState, turn_yaw: bool) -> bool {
    let mut face_angle_yaw = m.face_angle[1];
    // Never true in practice: callers pass FALSE.
    if turn_yaw && m.forward_vel < 0.0 {
        face_angle_yaw = face_angle_yaw.wrapping_add(i16::MIN);
    }
    let face_angle_yaw = m.floor_angle.wrapping_sub(face_angle_yaw);
    -0x4000 < face_angle_yaw && face_angle_yaw < 0x4000
}

/// mario_floor_is_slope.
pub fn mario_floor_is_slope(m: &MarioState, w: &StepWorld<'_>) -> bool {
    let normal_y = floor_normal_y(m, w);
    if w.area_terrain_type & TERRAIN_MASK == TERRAIN_SLIDE && normal_y < 0.9998477 {
        return true;
    }
    let threshold = match super::inputs::mario_get_floor_class(m, w) {
        SURFACE_VERY_SLIPPERY => 0.9961947,
        SURFACE_SLIPPERY => 0.9848077,
        SURFACE_NOT_SLIPPERY => 0.9396926,
        _ => 0.9659258,
    };
    normal_y <= threshold
}

/// mario_floor_is_steep. Unlike the other checks it ignores slide terrain.
pub fn mario_floor_is_steep(m: &MarioState, w: &StepWorld<'_>) -> bool {
    if mario_facing_downhill(m, false) {
        return false;
    }
    let threshold = match super::inputs::mario_get_floor_class(m, w) {
        SURFACE_VERY_SLIPPERY => 0.9659258,
        SURFACE_SLIPPERY => 0.9396926,
        _ => 0.8660254,
    };
    floor_normal_y(m, w) <= threshold
}

/// find_floor_height_relative_polar.
pub fn find_floor_height_relative_polar(
    m: &MarioState,
    w: &mut StepWorld<'_>,
    angle_from_mario: i16,
    dist_from_mario: f32,
) -> f32 {
    let angle = i32::from(m.face_angle[1]) + i32::from(angle_from_mario);
    let y = w.trig.sins(angle) * dist_from_mario;
    let x = w.trig.coss(angle) * dist_from_mario;
    w.collision
        .find_floor(
            m.pos[0] + y,
            m.pos[1] + 100.0,
            m.pos[2] + x,
            &mut w.collision_flags,
        )
        .0
}

/// find_floor_slope.
pub fn find_floor_slope(m: &MarioState, w: &mut StepWorld<'_>, yaw_offset: i16) -> i16 {
    let angle = i32::from(m.face_angle[1]) + i32::from(yaw_offset);
    let x = w.trig.sins(angle) * 5.0;
    let z = w.trig.coss(angle) * 5.0;
    let forward_floor_y = w
        .collision
        .find_floor(
            m.pos[0] + x,
            m.pos[1] + 100.0,
            m.pos[2] + z,
            &mut w.collision_flags,
        )
        .0;
    let backward_floor_y = w
        .collision
        .find_floor(
            m.pos[0] - x,
            m.pos[1] + 100.0,
            m.pos[2] - z,
            &mut w.collision_flags,
        )
        .0;
    let forward_y_delta = forward_floor_y - m.pos[1];
    let backward_y_delta = m.pos[1] - backward_floor_y;
    if forward_y_delta * forward_y_delta < backward_y_delta * backward_y_delta {
        w.trig.atan2s(5.0, forward_y_delta)
    } else {
        w.trig.atan2s(5.0, backward_y_delta)
    }
}

/// update_mario_sound_and_camera: camera calls are recorded requests.
pub fn update_mario_sound_and_camera(m: &MarioState, w: &mut StepWorld<'_>) {
    let action = m.action;
    let cam_preset = i32::from(w.camera.mode);
    if action == ACT_FIRST_PERSON {
        w.event(Event::RaiseBackgroundNoise(2));
        w.camera_movement_flags &= !(CAM_MOVE_C_UP_MODE as i16);
        w.event(Event::CameraMode {
            mode: -1,
            frames: 1,
        });
    } else if action == ACT_SLEEPING {
        w.event(Event::RaiseBackgroundNoise(2));
    }
    if action & (ACT_FLAG_SWIMMING | ACT_FLAG_METAL_WATER) == 0
        && (cam_preset == i32::from(CAMERA_MODE_BEHIND_MARIO)
            || cam_preset == i32::from(CAMERA_MODE_WATER_SURFACE))
    {
        w.event(Event::CameraMode {
            mode: i16::from(w.camera.def_mode),
            frames: 1,
        });
    }
}

/// set_steep_jump_action.
pub fn set_steep_jump_action(m: &mut MarioState, w: &mut StepWorld<'_>) {
    m.obj
        .raw
        .set_s32(O_MARIO_STEEP_JUMP_YAW, i32::from(m.face_angle[1]));
    if m.forward_vel > 0.0 {
        let angle_temp = m.floor_angle.wrapping_add(i16::MIN);
        let face_angle_temp = m.face_angle[1].wrapping_sub(angle_temp);
        let y = w.trig.sins(i32::from(face_angle_temp)) * m.forward_vel;
        let x = w.trig.coss(i32::from(face_angle_temp)) * m.forward_vel * 0.75;
        m.forward_vel = (y * y + x * x).sqrt();
        m.face_angle[1] = w.trig.atan2s(x, y).wrapping_add(angle_temp);
    }
    drop_and_set_mario_action(m, w, ACT_STEEP_JUMP, 0);
}

/// set_mario_y_vel_based_on_fspeed (get_additive_y_vel_for_jumps is 0).
fn set_mario_y_vel_based_on_fspeed(m: &mut MarioState, initial_vel_y: f32, multiplier: f32) {
    m.vel[1] = initial_vel_y + 0.0 + m.forward_vel * multiplier;
    if m.squish_timer != 0 || m.quicksand_depth > 1.0 {
        m.vel[1] *= 0.5;
    }
}

fn set_mario_action_airborne(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    mut action: u32,
    action_arg: u32,
) -> u32 {
    if (m.squish_timer != 0 || m.quicksand_depth >= 1.0)
        && (action == ACT_DOUBLE_JUMP || action == ACT_TWIRLING)
    {
        action = ACT_JUMP;
    }
    match action {
        ACT_DOUBLE_JUMP => {
            set_mario_y_vel_based_on_fspeed(m, 52.0, 0.25);
            m.forward_vel *= 0.8;
        }
        ACT_BACKFLIP => {
            m.obj.gfx.anim.anim_id = -1;
            m.forward_vel = -16.0;
            set_mario_y_vel_based_on_fspeed(m, 62.0, 0.0);
        }
        ACT_TRIPLE_JUMP => {
            set_mario_y_vel_based_on_fspeed(m, 69.0, 0.0);
            m.forward_vel *= 0.8;
        }
        ACT_FLYING_TRIPLE_JUMP => set_mario_y_vel_based_on_fspeed(m, 82.0, 0.0),
        ACT_WATER_JUMP | ACT_HOLD_WATER_JUMP => {
            if action_arg == 0 {
                set_mario_y_vel_based_on_fspeed(m, 42.0, 0.0);
            }
        }
        ACT_BURNING_JUMP => {
            m.vel[1] = 31.5;
            m.forward_vel = 8.0;
        }
        ACT_RIDING_SHELL_JUMP => set_mario_y_vel_based_on_fspeed(m, 42.0, 0.25),
        ACT_JUMP | ACT_HOLD_JUMP => {
            m.obj.gfx.anim.anim_id = -1;
            set_mario_y_vel_based_on_fspeed(m, 42.0, 0.25);
            m.forward_vel *= 0.8;
        }
        ACT_WALL_KICK_AIR | ACT_TOP_OF_POLE_JUMP => {
            set_mario_y_vel_based_on_fspeed(m, 62.0, 0.0);
            if m.forward_vel < 24.0 {
                m.forward_vel = 24.0;
            }
            m.wall_kick_timer = 0;
        }
        ACT_SIDE_FLIP => {
            set_mario_y_vel_based_on_fspeed(m, 62.0, 0.0);
            m.forward_vel = 8.0;
            m.face_angle[1] = m.intended_yaw;
        }
        ACT_STEEP_JUMP => {
            m.obj.gfx.anim.anim_id = -1;
            set_mario_y_vel_based_on_fspeed(m, 42.0, 0.25);
            m.face_angle[0] = -0x2000;
        }
        ACT_LAVA_BOOST => {
            m.vel[1] = 84.0;
            if action_arg == 0 {
                m.forward_vel = 0.0;
            }
        }
        ACT_DIVE => {
            let mut forward_vel = m.forward_vel + 15.0;
            if forward_vel > 48.0 {
                forward_vel = 48.0;
            }
            mario_set_forward_vel(m, w, forward_vel);
        }
        ACT_LONG_JUMP => {
            m.obj.gfx.anim.anim_id = -1;
            set_mario_y_vel_based_on_fspeed(m, 30.0, 0.0);
            m.obj
                .raw
                .set_s32(O_MARIO_LONG_JUMP_IS_SLOW, i32::from(m.forward_vel <= 16.0));
            // (BLJ) Backwards long jumps are not capped.
            m.forward_vel *= 1.5;
            if m.forward_vel > 48.0 {
                m.forward_vel = 48.0;
            }
        }
        ACT_SLIDE_KICK => {
            m.vel[1] = 12.0;
            if m.forward_vel < 32.0 {
                m.forward_vel = 32.0;
            }
        }
        ACT_JUMP_KICK => m.vel[1] = 20.0,
        _ => {}
    }
    m.peak_height = m.pos[1];
    m.flags |= MARIO_UNKNOWN_08;
    action
}

fn set_mario_action_moving(m: &mut MarioState, w: &StepWorld<'_>, mut action: u32) -> u32 {
    let floor_class = super::inputs::mario_get_floor_class(m, w);
    let forward_vel = m.forward_vel;
    // min(a, b) is ((a) <= (b) ? (a) : (b)).
    let mag = if m.intended_mag <= 8.0 {
        m.intended_mag
    } else {
        8.0
    };
    match action {
        ACT_WALKING => {
            if floor_class != SURFACE_CLASS_VERY_SLIPPERY && 0.0 <= forward_vel && forward_vel < mag
            {
                m.forward_vel = mag;
            }
            m.obj.raw.set_s32(O_MARIO_WALKING_PITCH, 0);
        }
        ACT_HOLD_WALKING => {
            if 0.0 <= forward_vel && forward_vel < mag / 2.0 {
                m.forward_vel = mag / 2.0;
            }
        }
        ACT_BEGIN_SLIDING => {
            action = if mario_facing_downhill(m, false) {
                ACT_BUTT_SLIDE
            } else {
                ACT_STOMACH_SLIDE
            };
        }
        ACT_HOLD_BEGIN_SLIDING => {
            action = if mario_facing_downhill(m, false) {
                ACT_HOLD_BUTT_SLIDE
            } else {
                ACT_HOLD_STOMACH_SLIDE
            };
        }
        _ => {}
    }
    action
}

fn set_mario_action_submerged(m: &mut MarioState, action: u32) -> u32 {
    if action == ACT_METAL_WATER_JUMP || action == ACT_HOLD_METAL_WATER_JUMP {
        m.vel[1] = 32.0;
    }
    action
}

fn set_mario_action_cutscene(m: &mut MarioState, w: &StepWorld<'_>, action: u32) -> u32 {
    match action {
        ACT_EMERGE_FROM_PIPE => m.vel[1] = 52.0,
        ACT_FALL_AFTER_STAR_GRAB => mario_set_forward_vel(m, w, 0.0),
        ACT_SPAWN_SPIN_AIRBORNE => mario_set_forward_vel(m, w, 2.0),
        ACT_SPECIAL_EXIT_AIRBORNE | ACT_SPECIAL_DEATH_EXIT => m.vel[1] = 64.0,
        _ => {}
    }
    action
}

/// set_mario_action. Always returns TRUE.
pub fn set_mario_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    action: u32,
    action_arg: u32,
) -> i32 {
    let action = match action & ACT_GROUP_MASK {
        ACT_GROUP_MOVING => set_mario_action_moving(m, w, action),
        ACT_GROUP_AIRBORNE => set_mario_action_airborne(m, w, action, action_arg),
        ACT_GROUP_SUBMERGED => set_mario_action_submerged(m, action),
        ACT_GROUP_CUTSCENE => set_mario_action_cutscene(m, w, action),
        _ => action,
    };
    m.flags &= !(MARIO_ACTION_SOUND_PLAYED | MARIO_MARIO_SOUND_PLAYED);
    if m.action & ACT_FLAG_AIR == 0 {
        m.flags &= !MARIO_UNKNOWN_18;
    }
    m.prev_action = m.action;
    m.action = action;
    m.action_arg = action_arg;
    m.action_state = 0;
    m.action_timer = 0;
    1
}

/// set_jump_from_landing.
pub fn set_jump_from_landing(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.quicksand_depth >= 11.0 {
        let action = if m.held_obj.is_none() {
            ACT_QUICKSAND_JUMP_LAND
        } else {
            ACT_HOLD_QUICKSAND_JUMP_LAND
        };
        return set_mario_action(m, w, action, 0);
    }
    if mario_floor_is_steep(m, w) {
        set_steep_jump_action(m, w);
    } else if m.double_jump_timer == 0 || m.squish_timer != 0 {
        set_mario_action(m, w, ACT_JUMP, 0);
    } else {
        match m.prev_action {
            ACT_JUMP_LAND | ACT_FREEFALL_LAND | ACT_SIDE_FLIP_LAND_STOP => {
                set_mario_action(m, w, ACT_DOUBLE_JUMP, 0);
            }
            ACT_DOUBLE_JUMP_LAND => {
                // The wing cap skips the speed requirement for a triple jump.
                if m.flags & MARIO_WING_CAP != 0 {
                    set_mario_action(m, w, ACT_FLYING_TRIPLE_JUMP, 0);
                } else if m.forward_vel > 20.0 {
                    set_mario_action(m, w, ACT_TRIPLE_JUMP, 0);
                } else {
                    set_mario_action(m, w, ACT_JUMP, 0);
                }
            }
            _ => {
                set_mario_action(m, w, ACT_JUMP, 0);
            }
        }
    }
    m.double_jump_timer = 0;
    1
}

/// set_jumping_action: quicksand and steep floors override `action`.
pub fn set_jumping_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    action: u32,
    action_arg: u32,
) -> i32 {
    if m.quicksand_depth >= 11.0 {
        let action = if m.held_obj.is_none() {
            ACT_QUICKSAND_JUMP_LAND
        } else {
            ACT_HOLD_QUICKSAND_JUMP_LAND
        };
        return set_mario_action(m, w, action, 0);
    }
    if mario_floor_is_steep(m, w) {
        set_steep_jump_action(m, w);
    } else {
        set_mario_action(m, w, action, action_arg);
    }
    1
}

/// drop_and_set_mario_action.
pub fn drop_and_set_mario_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    action: u32,
    action_arg: u32,
) -> i32 {
    interaction::mario_stop_riding_and_holding(m, w);
    set_mario_action(m, w, action, action_arg)
}

/// hurt_and_set_mario_action.
pub fn hurt_and_set_mario_action(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    action: u32,
    action_arg: u32,
    hurt_counter: i16,
) -> i32 {
    m.hurt_counter = hurt_counter as u8;
    set_mario_action(m, w, action, action_arg)
}

/// check_common_action_exits.
pub fn check_common_action_exits(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_A_PRESSED != 0 {
        return set_mario_action(m, w, ACT_JUMP, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_FREEFALL, 0);
    }
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        return set_mario_action(m, w, ACT_WALKING, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_BEGIN_SLIDING, 0);
    }
    0
}

/// check_common_hold_action_exits.
pub fn check_common_hold_action_exits(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if m.input & INPUT_A_PRESSED != 0 {
        return set_mario_action(m, w, ACT_HOLD_JUMP, 0);
    }
    if m.input & INPUT_OFF_FLOOR != 0 {
        return set_mario_action(m, w, ACT_HOLD_FREEFALL, 0);
    }
    if m.input & INPUT_NONZERO_ANALOG != 0 {
        return set_mario_action(m, w, ACT_HOLD_WALKING, 0);
    }
    if m.input & INPUT_ABOVE_SLIDE != 0 {
        return set_mario_action(m, w, ACT_HOLD_BEGIN_SLIDING, 0);
    }
    0
}

/// transition_submerged_to_walking.
pub fn transition_submerged_to_walking(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    w.event(Event::CameraMode {
        mode: i16::from(w.camera.def_mode),
        frames: 1,
    });
    m.angle_vel = [0, 0, 0];
    let action = if m.held_obj.is_none() {
        ACT_WALKING
    } else {
        ACT_HOLD_WALKING
    };
    set_mario_action(m, w, action, 0)
}

/// set_water_plunge_action.
pub fn set_water_plunge_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    m.forward_vel /= 4.0;
    m.vel[1] /= 2.0;
    m.pos[1] = (i32::from(m.water_level) - 100) as f32;
    m.face_angle[2] = 0;
    m.angle_vel = [0, 0, 0];
    if m.action & ACT_FLAG_DIVING == 0 {
        m.face_angle[0] = 0;
    }
    if w.camera.mode != CAMERA_MODE_WATER_SURFACE as u8 {
        w.event(Event::CameraMode {
            mode: CAMERA_MODE_WATER_SURFACE,
            frames: 1,
        });
    }
    set_mario_action(m, w, ACT_WATER_PLUNGE, 0)
}

const SQUISH_SCALE_OVER_TIME: [u8; 16] = [
    0x46, 0x32, 0x32, 0x3C, 0x46, 0x50, 0x50, 0x3C, 0x28, 0x14, 0x14, 0x1E, 0x32, 0x3C, 0x3C, 0x28,
];

/// squish_mario_model: the timer counts down here.
pub fn squish_mario_model(m: &mut MarioState) {
    if m.squish_timer != 0xFF {
        if m.squish_timer == 0 {
            m.obj.gfx.scale = [1.0, 1.0, 1.0];
        } else if m.squish_timer <= 16 {
            m.squish_timer -= 1;
            let step = f32::from(SQUISH_SCALE_OVER_TIME[15 - usize::from(m.squish_timer)]);
            m.obj.gfx.scale[1] = 1.0 - ((step * 0.6) / 100.0);
            m.obj.gfx.scale[0] = ((step * 0.4) / 100.0) + 1.0;
            m.obj.gfx.scale[2] = m.obj.gfx.scale[0];
        } else {
            m.squish_timer -= 1;
            m.obj.gfx.scale = [1.4, 0.4, 1.4];
        }
    }
}

/// set_submerged_cam_preset_and_spawn_bubbles.
pub fn set_submerged_cam_preset_and_spawn_bubbles(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.action & ACT_GROUP_MASK == ACT_GROUP_SUBMERGED {
        let height_below_water = (i32::from(m.water_level) - 80) as f32 - m.pos[1];
        let cam_preset = i16::from(w.camera.mode);
        if m.action & ACT_FLAG_METAL_WATER != 0 {
            if cam_preset != CAMERA_MODE_CLOSE {
                w.event(Event::CameraMode {
                    mode: CAMERA_MODE_CLOSE,
                    frames: 1,
                });
            }
        } else {
            if height_below_water > 800.0 && cam_preset != CAMERA_MODE_BEHIND_MARIO {
                w.event(Event::CameraMode {
                    mode: CAMERA_MODE_BEHIND_MARIO,
                    frames: 1,
                });
            }
            if height_below_water < 400.0 && cam_preset != CAMERA_MODE_WATER_SURFACE {
                w.event(Event::CameraMode {
                    mode: CAMERA_MODE_WATER_SURFACE,
                    frames: 1,
                });
            }
            if m.action & ACT_FLAG_INTANGIBLE == 0
                && (m.pos[1] < (i32::from(m.water_level) - 160) as f32 || m.face_angle[0] < -0x800)
            {
                m.particle_flags |= PARTICLE_BUBBLE;
            }
        }
    }
}

/// update_mario_health.
pub fn update_mario_health(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.health >= 0x100 {
        if (u32::from(m.heal_counter) | u32::from(m.hurt_counter)) == 0 {
            if m.input & INPUT_IN_POISON_GAS != 0 && m.action & ACT_FLAG_INTANGIBLE == 0 {
                if m.flags & MARIO_METAL_CAP == 0 && w.debug_level_select == 0 {
                    m.health = m.health.wrapping_sub(4);
                }
            } else if m.action & ACT_FLAG_SWIMMING != 0 && m.action & ACT_FLAG_INTANGIBLE == 0 {
                let terrain_is_snow = w.area_terrain_type & TERRAIN_MASK == TERRAIN_SNOW;
                if m.pos[1] >= (i32::from(m.water_level) - 140) as f32 && !terrain_is_snow {
                    m.health = m.health.wrapping_add(0x1A);
                } else if w.debug_level_select == 0 {
                    m.health = m.health.wrapping_sub(if terrain_is_snow { 3 } else { 1 });
                }
            }
        }
        if m.heal_counter > 0 {
            m.health = m.health.wrapping_add(0x40);
            m.heal_counter -= 1;
        }
        if m.hurt_counter > 0 {
            m.health = m.health.wrapping_sub(0x40);
            m.hurt_counter -= 1;
        }
        if m.health > 0x880 {
            m.health = 0x880;
        }
        if m.health < 0x100 {
            m.health = 0xFF;
        }
        if m.action & ACT_GROUP_MASK == ACT_GROUP_SUBMERGED && m.health < 0x300 {
            w.play_sound(SOUND_MOVING_ALMOST_DROWNING);
        }
    }
}

/// update_mario_info_for_cam.
pub fn update_mario_info_for_cam(m: &mut MarioState) {
    m.body.action = m.action;
    m.camera_status.action = m.action;
    m.camera_status.face_angle = m.face_angle;
    if m.flags & MARIO_UNKNOWN_25 == 0 {
        m.camera_status.pos = m.pos;
    }
}

/// mario_reset_bodystate.
pub fn mario_reset_bodystate(m: &mut MarioState) {
    m.body.cap_state = MARIO_HAS_DEFAULT_CAP_OFF;
    m.body.eye_state = MARIO_EYES_BLINK;
    m.body.hand_state = MARIO_HAND_FISTS;
    m.body.model_state = 0;
    m.body.wing_flutter = 0;
    m.flags &= !MARIO_METAL_SHOCK;
}

/// sink_mario_in_quicksand: lowers the drawn position only.
pub fn sink_mario_in_quicksand(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if let Some(matrix) = m.obj.gfx.throw_matrix {
        w.floor_align_matrix[matrix][3][1] -= m.quicksand_depth;
    }
    m.obj.gfx.pos[1] -= m.quicksand_depth;
}

/// Bit n set: the special cap flickers off when capTimer is n.
const CAP_FLICKER_FRAMES: u64 = 0x4444449249255555;

/// update_and_return_cap_flags.
pub fn update_and_return_cap_flags(m: &mut MarioState, w: &mut StepWorld<'_>) -> u32 {
    let mut flags = m.flags;
    if m.cap_timer > 0 {
        let action = m.action;
        if m.cap_timer <= 60
            || (action != ACT_READING_AUTOMATIC_DIALOG
                && action != ACT_READING_NPC_DIALOG
                && action != ACT_READING_SIGN
                && action != ACT_IN_CANNON)
        {
            m.cap_timer -= 1;
        }
        if m.cap_timer == 0 {
            w.event(Event::StopCapMusic);
            m.flags &= !MARIO_SPECIAL_CAPS;
            if m.flags & MARIO_CAPS == 0 {
                m.flags &= !MARIO_CAP_ON_HEAD;
            }
        }
        if m.cap_timer == 60 {
            w.event(Event::FadeoutCapMusic);
        }
        if m.cap_timer < 64 && (1u64 << m.cap_timer) & CAP_FLICKER_FRAMES != 0 {
            flags &= !MARIO_SPECIAL_CAPS;
            if flags & MARIO_CAPS == 0 {
                flags &= !MARIO_CAP_ON_HEAD;
            }
        }
    }
    flags
}

/// mario_update_hitbox_and_cap_model.
pub fn mario_update_hitbox_and_cap_model(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let flags = update_and_return_cap_flags(m, w);
    if flags & MARIO_VANISH_CAP != 0 {
        m.body.model_state = MODEL_STATE_NOISE_ALPHA;
    }
    if flags & MARIO_METAL_CAP != 0 {
        m.body.model_state |= MODEL_STATE_METAL;
    }
    if flags & MARIO_METAL_SHOCK != 0 {
        m.body.model_state |= MODEL_STATE_METAL;
    }
    // (Pause buffered hitstun) gGlobalTimer also runs while paused.
    if m.invinc_timer >= 3 && w.global_timer & 1 != 0 {
        m.obj.gfx.node_flags |= GRAPH_RENDER_INVISIBLE;
    }
    if flags & MARIO_CAP_IN_HAND != 0 {
        m.body.hand_state = if flags & MARIO_WING_CAP != 0 {
            MARIO_HAND_HOLDING_WING_CAP
        } else {
            MARIO_HAND_HOLDING_CAP
        };
    }
    if flags & MARIO_CAP_ON_HEAD != 0 {
        m.body.cap_state = if flags & MARIO_WING_CAP != 0 {
            MARIO_HAS_WING_CAP_ON
        } else {
            MARIO_HAS_DEFAULT_CAP_ON
        };
    }
    m.obj.hitbox_height = if m.action & ACT_FLAG_SHORT_HITBOX != 0 {
        100.0
    } else {
        160.0
    };
    if m.flags & MARIO_TELEPORTING != 0 && m.fade_warp_opacity != 0xFF {
        m.body.model_state &= !0xFF;
        m.body.model_state |= 0x100 | i16::from(m.fade_warp_opacity);
    }
}

/// One iteration of execute_mario_action's group dispatch.
fn execute_group(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    match m.action & ACT_GROUP_MASK {
        ACT_GROUP_STATIONARY => stationary::mario_execute_stationary_action(m, w),
        ACT_GROUP_MOVING => moving::mario_execute_moving_action(m, w),
        ACT_GROUP_AIRBORNE => airborne::mario_execute_airborne_action(m, w),
        ACT_GROUP_SUBMERGED => {
            w.event(Event::Unsupported(Unsupported::SubmergedGroup(m.action)));
            0
        }
        ACT_GROUP_CUTSCENE => {
            w.event(Event::Unsupported(Unsupported::CutsceneGroup(m.action)));
            0
        }
        ACT_GROUP_AUTOMATIC => automatic::mario_execute_automatic_action(m, w),
        ACT_GROUP_OBJECT => object::mario_execute_object_action(m, w),
        // An unknown group leaves the loop variable unchanged (TRUE).
        _ => 1,
    }
}

/// execute_mario_action: one tick of Mario's behavior. Returns the particle
/// flags. Groups that are not ported yet (cutscene, submerged) record an
/// `Unsupported` event and end the loop, like the oracle's boundary.
pub fn execute_mario_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> u32 {
    if m.action == 0 {
        return 0;
    }
    m.obj.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    mario_reset_bodystate(m);
    // A missing floor records its death-warp request inside the input stage.
    let _ = super::inputs::update_mario_inputs(m, w);
    interaction::mario_handle_special_floors(m, w);
    interaction::mario_process_interactions(m, w);
    // Out of bounds: stop executing actions.
    if m.floor.is_none() {
        return 0;
    }
    let mut in_loop = 1;
    while in_loop != 0 {
        in_loop = execute_group(m, w);
    }
    sink_mario_in_quicksand(m, w);
    squish_mario_model(m);
    set_submerged_cam_preset_and_spawn_bubbles(m, w);
    update_mario_health(m, w);
    update_mario_info_for_cam(m);
    mario_update_hitbox_and_cap_model(m, w);
    let floor = w.surface(m.floor.expect("floor checked above"));
    if floor.surface_type == SURFACE_HORIZONTAL_WIND {
        w.event(Event::WindParticles {
            pitch: 0,
            yaw: (i32::from(floor.force) << 8) as i16,
        });
        w.play_sound(SOUND_ENV_WIND2);
    }
    if floor.surface_type == SURFACE_VERTICAL_WIND {
        w.event(Event::WindParticles { pitch: 1, yaw: 0 });
        w.play_sound(SOUND_ENV_WIND2);
    }
    // play_infinite_stairs_music only acts in the castle.
    if w.level_num == LEVEL_CASTLE {
        w.event(Event::Unsupported(Unsupported::InfiniteStairs));
    }
    m.obj.raw.set_s32(O_INTERACT_STATUS, 0);
    m.particle_flags
}

/// Where Mario spawns: gMarioSpawnInfo (struct SpawnInfo).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpawnPoint {
    pub start_pos: [i16; 3],
    pub start_angle: [i16; 3],
    pub area_index: i8,
    pub active_area_index: i8,
    pub behavior_arg: u32,
}

impl SpawnPoint {
    /// What a level script's MARIO and MARIO_POS commands set
    /// (level_cmd_init_mario and level_cmd_set_mario_start_pos in
    /// level_script.c): `yaw` is in degrees, as in the script.
    pub fn from_level_script(behavior_arg: u32, area: u8, yaw: i16, pos: [i16; 3]) -> Self {
        Self {
            start_pos: pos,
            start_angle: [0, (i32::from(yaw) * 0x8000 / 180) as i16, 0],
            area_index: area as i8,
            active_area_index: -1,
            behavior_arg,
        }
    }
}

/// init_mario_from_save_file. Coins, lives, health and the animation Y
/// translation get their file-load values; the HUD is outside the simulation.
pub fn init_mario_from_save_file(m: &mut MarioState, w: &StepWorld<'_>) {
    m.unk00 = 0;
    m.flags = 0;
    m.action = 0;
    m.num_coins = 0;
    m.num_stars = w.save.total_star_count as i16;
    m.num_keys = 0;
    m.num_lives = 4;
    m.health = 0x880;
    m.prev_num_stars_for_dialog = m.num_stars;
    m.unk_b0 = 0xBD;
}

/// init_mario: place Mario at the spawn point on the floor and start idle.
/// The cap object a saved cap position would spawn is not supported yet.
pub fn init_mario(m: &mut MarioState, w: &mut StepWorld<'_>, spawn: SpawnPoint) {
    m.action_timer = 0;
    m.frames_since_a = 0xFF;
    m.frames_since_b = 0xFF;
    m.invinc_timer = 0;
    if w.save.flags
        & (SAVE_FLAG_CAP_ON_GROUND
            | SAVE_FLAG_CAP_ON_KLEPTO
            | SAVE_FLAG_CAP_ON_UKIKI
            | SAVE_FLAG_CAP_ON_MR_BLIZZARD)
        != 0
    {
        m.flags = 0;
    } else {
        m.flags = MARIO_NORMAL_CAP | MARIO_CAP_ON_HEAD;
    }
    m.forward_vel = 0.0;
    m.squish_timer = 0;
    m.hurt_counter = 0;
    m.heal_counter = 0;
    m.cap_timer = 0;
    m.quicksand_depth = 0.0;
    m.held_obj = None;
    m.ridden_obj = None;
    m.used_obj = None;
    let start = spawn.start_pos.map(f32::from);
    m.water_level = f32_to_s16(w.collision.find_water_level(start[0], start[2]));
    m.obj.gfx.anim.anim_id = -1;
    m.face_angle = spawn.start_angle;
    m.angle_vel = [0, 0, 0];
    m.pos = start;
    m.vel = [0.0; 3];
    let (floor_height, floor) =
        w.collision
            .find_floor(m.pos[0], m.pos[1], m.pos[2], &mut w.collision_flags);
    m.floor_height = floor_height;
    m.floor = floor.map(SurfaceRef::Collision);
    if m.pos[1] < m.floor_height {
        m.pos[1] = m.floor_height;
    }
    m.obj.gfx.pos[1] = m.pos[1];
    m.action = if m.pos[1] <= (i32::from(m.water_level) - 100) as f32 {
        ACT_WATER_IDLE
    } else {
        ACT_IDLE
    };
    mario_reset_bodystate(m);
    update_mario_info_for_cam(m);
    m.body.punch_state = 0;
    m.obj.raw.set_f32(O_POS_X, m.pos[0]);
    m.obj.raw.set_f32(O_POS_Y, m.pos[1]);
    m.obj.raw.set_f32(O_POS_Z, m.pos[2]);
    m.obj
        .raw
        .set_s32(O_MOVE_ANGLE_PITCH, i32::from(m.face_angle[0]));
    m.obj
        .raw
        .set_s32(O_MOVE_ANGLE_YAW, i32::from(m.face_angle[1]));
    m.obj
        .raw
        .set_s32(O_MOVE_ANGLE_ROLL, i32::from(m.face_angle[2]));
    m.obj.gfx.pos = m.pos;
    m.obj.gfx.angle = [0, m.face_angle[1], 0];
    assert!(
        w.save.cap_pos.is_none(),
        "a saved cap position spawns a cap object; objects are not supported yet"
    );
}

/// step::mario_update_quicksand needs these setters; re-exported for clarity.
pub use step::{mario_push_off_steep_floor, mario_update_quicksand};
