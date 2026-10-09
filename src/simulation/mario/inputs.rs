//! Pre-action input update translated from pinned CC0 `mario.c`.
//! This is one stage of a tick, not an action dispatcher or a playable runtime.
use super::{MarioState, StepWorld, SurfaceRef, constants::*, step};
use crate::simulation::{
    collision::SURFACE_FLAG_DYNAMIC,
    controller::{A_BUTTON, B_BUTTON, Controller, Z_TRIG},
};

/// Reference camera and Mario-object inputs. Camera movement flags are mutable
/// authoritative state. Camera yaw must never come from an interpolated pose.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputContext {
    pub camera_yaw: i16,
    pub camera_movement_flags: u16,
    pub object_interact_status: u32,
    pub object_collided_interact_types: u32,
}

/// The future level runtime must handle a missing-floor death request. This
/// component does not implement warp timers, lives, or saves.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[must_use = "a missing-floor death warp request must be handled by the caller"]
pub enum InputOutcome {
    #[default]
    Continue,
    DeathWarpRequested,
}

pub fn update_mario_button_inputs(m: &mut MarioState, controller: &Controller) {
    if controller.button_pressed & A_BUTTON != 0 {
        m.input |= INPUT_A_PRESSED;
    }
    if controller.button_down & A_BUTTON != 0 {
        m.input |= INPUT_A_DOWN;
    }
    if m.squish_timer == 0 {
        if controller.button_pressed & B_BUTTON != 0 {
            m.input |= INPUT_B_PRESSED;
        }
        if controller.button_down & Z_TRIG != 0 {
            m.input |= INPUT_Z_DOWN;
        }
        if controller.button_pressed & Z_TRIG != 0 {
            m.input |= INPUT_Z_PRESSED;
        }
    }
    m.frames_since_a = if m.input & INPUT_A_PRESSED != 0 {
        0
    } else {
        m.frames_since_a.saturating_add(1)
    };
    m.frames_since_b = if m.input & INPUT_B_PRESSED != 0 {
        0
    } else {
        m.frames_since_b.saturating_add(1)
    };
}

pub fn update_mario_joystick_inputs(
    m: &mut MarioState,
    w: &StepWorld<'_>,
    controller: &Controller,
    camera_yaw: i16,
) {
    let mag = ((controller.stick_mag / 64.0) * (controller.stick_mag / 64.0)) * 64.0;
    m.intended_mag = mag / if m.squish_timer == 0 { 2.0 } else { 8.0 };
    if m.intended_mag > 0.0 {
        m.intended_yaw = w
            .trig
            .atan2s(-controller.stick_y, controller.stick_x)
            .wrapping_add(camera_yaw);
        m.input |= INPUT_NONZERO_ANALOG;
    } else {
        m.intended_yaw = m.face_angle[1];
    }
}

pub fn mario_get_floor_class(m: &MarioState, w: &StepWorld<'_>) -> i16 {
    let mut class = if w.area_terrain_type & TERRAIN_MASK == TERRAIN_SLIDE {
        SURFACE_CLASS_VERY_SLIPPERY
    } else {
        SURFACE_CLASS_DEFAULT
    };
    if let Some(floor) = m.floor {
        class = match w.surface(floor).surface_type {
            SURFACE_NOT_SLIPPERY | SURFACE_HARD_NOT_SLIPPERY | SURFACE_SWITCH => {
                SURFACE_CLASS_NOT_SLIPPERY
            }
            SURFACE_SLIPPERY
            | SURFACE_NOISE_SLIPPERY
            | SURFACE_HARD_SLIPPERY
            | SURFACE_NO_CAM_COL_SLIPPERY => SURFACE_CLASS_SLIPPERY,
            SURFACE_VERY_SLIPPERY
            | SURFACE_ICE
            | SURFACE_HARD_VERY_SLIPPERY
            | SURFACE_NOISE_VERY_SLIPPERY_73
            | SURFACE_NOISE_VERY_SLIPPERY_74
            | SURFACE_NOISE_VERY_SLIPPERY
            | SURFACE_NO_CAM_COL_VERY_SLIPPERY => SURFACE_CLASS_VERY_SLIPPERY,
            _ => class,
        };
    }
    if m.action == ACT_CRAWLING
        && w.surface(
            m.floor
                .expect("crawling floor class dereferences NULL in the original"),
        )
        .normal[1]
            > 0.5
        && class == SURFACE_CLASS_DEFAULT
    {
        class = SURFACE_CLASS_NOT_SLIPPERY;
    }
    class
}

pub fn mario_floor_is_slippery(m: &MarioState, w: &StepWorld<'_>) -> bool {
    let normal_y = w
        .surface(m.floor.expect("slipperiness requires a floor"))
        .normal[1];
    if w.area_terrain_type & TERRAIN_MASK == TERRAIN_SLIDE && normal_y < 0.9998477 {
        return true;
    }
    let threshold = match mario_get_floor_class(m, w) {
        SURFACE_VERY_SLIPPERY => 0.9848077,
        SURFACE_SLIPPERY => 0.9396926,
        SURFACE_NOT_SLIPPERY => 0.0,
        _ => 0.7880108,
    };
    normal_y <= threshold
}

pub fn update_mario_geometry_inputs(m: &mut MarioState, w: &mut StepWorld<'_>) -> InputOutcome {
    for (offset, radius) in [(60.0, 50.0), (30.0, 24.0)] {
        w.collision.f32_find_wall_collision(
            &mut m.pos,
            offset,
            radius,
            w.collision_flags,
            m.flags & MARIO_VANISH_CAP != 0,
        );
    }
    let find_floor = |pos: [f32; 3], w: &mut StepWorld<'_>| {
        let (height, surface) =
            w.collision
                .find_floor(pos[0], pos[1], pos[2], &mut w.collision_flags);
        (height, surface.map(SurfaceRef::Collision))
    };
    (m.floor_height, m.floor) = find_floor(m.pos, w);
    if m.floor.is_none() {
        // Preserve the graphical-position fallback quirk. This is the last
        // authoritative gfx position, never an interpolated presentation pose.
        m.pos = m.gfx_pos;
        (m.floor_height, m.floor) = find_floor(m.pos, w);
    }
    let (ceil_height, ceil) = step::vec3f_find_ceil(w, m.pos, m.floor_height);
    m.ceil_height = ceil_height;
    m.ceil = ceil.map(SurfaceRef::Collision);
    let gas_level = w.collision.find_poison_gas_level(m.pos[0], m.pos[2]);
    m.water_level = w.collision.find_water_level(m.pos[0], m.pos[2]) as i16;
    let Some(floor) = m.floor.map(|f| w.surface(f)) else {
        return InputOutcome::DeathWarpRequested;
    };
    m.floor_angle = w.trig.atan2s(floor.normal[2], floor.normal[0]);
    m.terrain_sound_addend = step::mario_get_terrain_sound_addend(m, w);
    if m.pos[1] > f32::from(m.water_level) - 40.0 && mario_floor_is_slippery(m, w) {
        m.input |= INPUT_ABOVE_SLIDE;
    }
    if floor.flags & SURFACE_FLAG_DYNAMIC != 0
        || m.ceil
            .is_some_and(|c| w.surface(c).flags & SURFACE_FLAG_DYNAMIC != 0)
    {
        let gap = m.ceil_height - m.floor_height;
        if (0.0..=150.0).contains(&gap) {
            m.input |= INPUT_SQUISHED;
        }
    }
    if m.pos[1] > m.floor_height + 100.0 {
        m.input |= INPUT_OFF_FLOOR;
    }
    if m.pos[1] < f32::from(m.water_level) - 10.0 {
        m.input |= INPUT_IN_WATER;
    }
    if m.pos[1] < gas_level - 100.0 {
        m.input |= INPUT_IN_POISON_GAS;
    }
    InputOutcome::Continue
}

/// Run once before action dispatch. Debug text is presentation-only and the
/// original stub_mario_step_1 is empty. Warp execution is an explicit boundary.
pub fn update_mario_inputs(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    controller: &Controller,
    context: &mut InputContext,
) -> InputOutcome {
    m.particle_flags = 0;
    m.input = 0;
    m.collided_obj_interact_types = context.object_collided_interact_types;
    m.flags &= 0x00ff_ffff;
    update_mario_button_inputs(m, controller);
    update_mario_joystick_inputs(m, w, controller, context.camera_yaw);
    let outcome = update_mario_geometry_inputs(m, w);
    if context.camera_movement_flags & CAM_MOVE_C_UP_MODE != 0 {
        if m.action & ACT_FLAG_ALLOW_FIRST_PERSON != 0 {
            m.input |= INPUT_FIRST_PERSON;
        } else {
            context.camera_movement_flags &= !CAM_MOVE_C_UP_MODE;
        }
    }
    if m.input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED) == 0 {
        m.input |= INPUT_UNKNOWN_5;
    }
    if context.object_interact_status
        & (INT_STATUS_MARIO_STUNNED | INT_STATUS_MARIO_KNOCKBACK_DMG | INT_STATUS_MARIO_SHOCKWAVE)
        != 0
    {
        m.input |= INPUT_STOMPED;
    }
    m.wall_kick_timer = m.wall_kick_timer.saturating_sub(1);
    m.double_jump_timer = m.double_jump_timer.saturating_sub(1);
    outcome
}
