//! Object particle bursts, translated from pinned CC0 sources:
//! object_helpers.c (cur_obj_spawn_particles, random_f32_around_zero),
//! behavior_actions.c (sMistParticles, spawn_mist_particles_variable),
//! behaviors/white_puff_explode.inc.c (bhv_white_puff_exploding_loop) and
//! behaviors/break_particles.inc.c (spawn_triangle_break_particles). The
//! particles are ordinary objects: they take pool slots, draw from the shared
//! random sequence and run their own scripts, as in the original.
use super::{
    ObjectId,
    helpers::{obj_mark_for_deletion, obj_scale, obj_scale_xyz},
    object_mut,
    script::Behavior,
    spawn::spawn_object,
    standard_motion::{
        cur_obj_apply_drag_xz, cur_obj_compute_vel_xz, cur_obj_move_using_vel_and_gravity,
    },
};
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// struct SpawnParticlesInfo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnParticlesInfo {
    pub bhv_param: i8,
    pub count: i8,
    pub model: u8,
    pub offset_y: i8,
    pub forward_vel_base: i8,
    pub forward_vel_range: i8,
    pub vel_y_base: i8,
    pub vel_y_range: i8,
    pub gravity: i8,
    pub drag_strength: i8,
    pub size_base: f32,
    pub size_range: f32,
}

/// sMistParticles' initializer. spawn_mist_particles_variable rewrites its
/// count, offset and sizes before every use.
const MIST_PARTICLES: SpawnParticlesInfo = SpawnParticlesInfo {
    bhv_param: 2,
    count: 20,
    model: MODEL_MIST as u8,
    offset_y: 0,
    forward_vel_base: 40,
    forward_vel_range: 5,
    vel_y_base: 30,
    vel_y_range: 20,
    gravity: 252u8 as i8,
    drag_strength: 30,
    size_base: 330.0,
    size_range: 10.0,
};

/// random_f32_around_zero.
pub fn random_f32_around_zero(w: &mut StepWorld<'_>, diameter: f32) -> f32 {
    w.rng.random_float() * diameter - diameter / 2.0
}

/// cur_obj_spawn_particles: fewer (or no) particles when the last frame's
/// update visited many objects.
pub fn cur_obj_spawn_particles(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    info: &SpawnParticlesInfo,
) {
    let mut num_particles = i32::from(info.count);
    let capacity = OBJECT_POOL_CAPACITY as i32;
    if i32::from(w.prev_frame_object_count) > capacity - 90 && num_particles > 10 {
        num_particles = 10;
    }
    if i32::from(w.prev_frame_object_count) > capacity - 30 {
        num_particles = 0;
    }
    let behavior = w.behaviors.address(Behavior::WhitePuffExplosion);
    for _ in 0..num_particles {
        let scale = w.rng.random_float() * (info.size_range * 0.1) + info.size_base * 0.1;
        let particle = spawn_object(m, w, id, i32::from(info.model), behavior);
        let yaw = w.rng.random_u16();
        let p = object_mut(&mut w.objects, &mut m.obj, particle);
        p.raw
            .set_s32(O_BHV_PARAMS2ND_BYTE, i32::from(info.bhv_param));
        p.raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
        p.raw.set_f32(O_GRAVITY, f32::from(info.gravity));
        p.raw
            .set_f32(O_DRAG_STRENGTH, f32::from(info.drag_strength));
        p.raw
            .set_f32(O_POS_Y, p.raw.f32(O_POS_Y) + f32::from(info.offset_y));
        let forward = w.rng.random_float() * f32::from(info.forward_vel_range)
            + f32::from(info.forward_vel_base);
        let vel_y = w.rng.random_float() * f32::from(info.vel_y_range) + f32::from(info.vel_y_base);
        let p = object_mut(&mut w.objects, &mut m.obj, particle);
        p.raw.set_f32(O_FORWARD_VEL, forward);
        p.raw.set_f32(O_VEL_Y, vel_y);
        obj_scale_xyz(p, scale, scale, scale);
    }
}

/// spawn_mist_particles_variable.
pub fn spawn_mist_particles_variable(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    count: i32,
    offset_y: i32,
    size: f32,
) {
    let mut info = MIST_PARTICLES;
    info.size_base = size;
    info.size_range = (f64::from(size) / 20.0) as f32;
    info.offset_y = offset_y as i8;
    info.count = if count == 0 {
        20
    } else if count > 20 {
        count as i8
    } else {
        4
    };
    cur_obj_spawn_particles(m, w, id, &info);
}

/// bhv_white_puff_exploding_loop: drift, slow, fade and shrink or grow.
pub fn bhv_white_puff_exploding_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let trig = w.trig;
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    if o.raw.s32(O_TIMER) == 0 {
        cur_obj_compute_vel_xz(o, trig);
        o.raw.set_f32(O_WHITE_PUFF_UNK_F4, o.gfx.scale[0]);
        match o.raw.s32(O_BHV_PARAMS2ND_BYTE) {
            2 => {
                o.raw.set_s32(O_OPACITY, 254);
                o.raw.set_s32(O_WHITE_PUFF_UNK_F8, -21);
                o.raw.set_s32(O_WHITE_PUFF_UNK_F_C, 0);
            }
            3 => {
                o.raw.set_s32(O_OPACITY, 254);
                o.raw.set_s32(O_WHITE_PUFF_UNK_F8, -13);
                o.raw.set_s32(O_WHITE_PUFF_UNK_F_C, 1);
            }
            _ => {}
        }
    }
    cur_obj_move_using_vel_and_gravity(o);
    let drag = o.raw.f32(O_DRAG_STRENGTH);
    cur_obj_apply_drag_xz(o, drag);
    if o.raw.f32(O_VEL_Y) > 100.0 {
        o.raw.set_f32(O_VEL_Y, 100.0);
    }
    if o.raw.s32(O_TIMER) > 20 {
        obj_mark_for_deletion(o);
    }
    if o.raw.s32(O_OPACITY) != 0 {
        let opacity = o
            .raw
            .s32(O_OPACITY)
            .wrapping_add(o.raw.s32(O_WHITE_PUFF_UNK_F8));
        o.raw.set_s32(O_OPACITY, opacity);
        if opacity < 2 {
            obj_mark_for_deletion(o);
        }
        let base = f64::from(o.raw.f32(O_WHITE_PUFF_UNK_F4));
        let scale = if o.raw.s32(O_WHITE_PUFF_UNK_F_C) != 0 {
            (base * (f64::from(254 - opacity) / 254.0)) as f32
        } else {
            (base * (f64::from(opacity) / 254.0)) as f32
        };
        obj_scale(o, scale);
    }
}

/// spawn_triangle_break_particles: tumbling triangles thrown out of the
/// object.
pub fn spawn_triangle_break_particles(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    num_tris: i16,
    tri_model: i16,
    tri_size: f32,
    tri_anim_state: i16,
) {
    let behavior = w.behaviors.address(Behavior::BreakBoxTriangle);
    for _ in 0..num_tris {
        let triangle = spawn_object(m, w, id, i32::from(tri_model), behavior);
        let yaw = w.rng.random_u16();
        let pitch = w.rng.random_u16();
        let vel_y = random_f32_around_zero(w, 50.0);
        let t = object_mut(&mut w.objects, &mut m.obj, triangle);
        t.raw.set_s32(O_ANIM_STATE, i32::from(tri_anim_state));
        t.raw.set_f32(O_POS_Y, t.raw.f32(O_POS_Y) + 100.0);
        t.raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
        t.raw.set_s32(O_FACE_ANGLE_YAW, t.raw.s32(O_MOVE_ANGLE_YAW));
        t.raw.set_s32(O_FACE_ANGLE_PITCH, i32::from(pitch));
        t.raw.set_f32(O_VEL_Y, vel_y);
        if i32::from(tri_model) == MODEL_DIRT_ANIMATION
            || i32::from(tri_model) == MODEL_SL_CRACKED_ICE_CHUNK
        {
            t.raw.set_s32(O_ANGLE_VEL_PITCH, 0xF00);
            t.raw.set_s32(O_ANGLE_VEL_YAW, 0x500);
            t.raw.set_f32(O_FORWARD_VEL, 30.0);
        } else {
            let random = w.rng.random_float();
            let t = object_mut(&mut w.objects, &mut m.obj, triangle);
            t.raw.set_s32(
                O_ANGLE_VEL_PITCH,
                0x80 * crate::simulation::mario::f32_to_s32(random + 50.0),
            );
            t.raw.set_f32(O_FORWARD_VEL, 30.0);
        }
        obj_scale(object_mut(&mut w.objects, &mut m.obj, triangle), tri_size);
    }
}
