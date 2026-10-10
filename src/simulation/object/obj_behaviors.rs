//! Object behavior helpers translated from pinned CC0
//! src/game/obj_behaviors.c (the parts other than the `object_step` family in
//! `motion`): Mario-distance and home checks, facing tests, floor deaths, the
//! lava death, yellow-coin loot and flickering. `o` (gCurrentObject) is
//! explicit; `mario_gfx_pos` is gMarioObject->header.gfx.pos.
use super::{
    ObjectId,
    helpers::{approach_s16_symmetric, obj_mark_for_deletion},
    motion::{OBJ_COL_FLAG_GROUNDED, ObjectStepEffect, ObjectStepResult},
    object, object_mut,
    script::Behavior,
    sound::cur_obj_play_sound_2,
    spawn::spawn_object,
};
use crate::simulation::{
    collision::{CollisionWorld, SurfaceIndex},
    mario::{MarioState, StepWorld, constants::*},
    math::TrigTables,
};

/// is_point_within_radius_of_mario: against Mario's graphics position.
/// `dist * dist` is int arithmetic (wrapping like the oracle's -fwrapv).
pub fn is_point_within_radius_of_mario(
    mario_gfx_pos: [f32; 3],
    x: f32,
    y: f32,
    z: f32,
    dist: i32,
) -> bool {
    let [mx, my, mz] = mario_gfx_pos;
    (x - mx) * (x - mx) + (y - my) * (y - my) + (z - mz) * (z - mz) < dist.wrapping_mul(dist) as f32
}

/// gMarioObject->header.gfx.pos.
pub fn mario_gfx_pos(m: &MarioState) -> [f32; 3] {
    m.obj.gfx.pos
}

/// obj_return_home_if_safe: true while Mario is within `dist` of home;
/// otherwise the move yaw turns 320 toward home.
pub fn obj_return_home_if_safe(
    obj: &mut super::Object,
    trig: &TrigTables,
    mario_gfx_pos: [f32; 3],
    home: [f32; 3],
    dist: i32,
) -> bool {
    let home_dist_x = home[0] - obj.raw.f32(O_POS_X);
    let home_dist_z = home[2] - obj.raw.f32(O_POS_Z);
    let angle_towards_home = trig.atan2s(home_dist_z, home_dist_x);
    if is_point_within_radius_of_mario(mario_gfx_pos, home[0], home[1], home[2], dist) {
        return true;
    }
    let yaw = approach_s16_symmetric(
        obj.raw.s32(O_MOVE_ANGLE_YAW) as i16,
        angle_towards_home,
        320,
    );
    obj.raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
    false
}

/// obj_check_if_facing_toward_angle: `base` and `goal` as u32 parameters,
/// their difference taken as u16 and narrowed to s16.
pub fn obj_check_if_facing_toward_angle(
    trig: &TrigTables,
    base: u32,
    goal: u32,
    range: i16,
) -> bool {
    let d_angle = (i32::from(goal as u16) - i32::from(base as u16)) as i16;
    let d = i32::from(d_angle);
    trig.sins(-i32::from(range)) < trig.sins(d)
        && trig.sins(d) < trig.sins(i32::from(range))
        && trig.coss(d) > 0.0
}

/// obj_check_floor_death: a grounded object on a burning or death-plane
/// floor starts its death action (the vertical-wind death floor is missed,
/// as in the original).
pub fn obj_check_floor_death(
    o: &mut super::Object,
    collision: &CollisionWorld,
    collision_flags: i16,
    floor: Option<SurfaceIndex>,
) {
    let Some(floor) = floor else {
        return;
    };
    if collision_flags & OBJ_COL_FLAG_GROUNDED == OBJ_COL_FLAG_GROUNDED {
        match collision.surface(floor).surface_type {
            SURFACE_BURNING => o.raw.set_s32(O_ACTION, OBJ_ACT_LAVA_DEATH),
            SURFACE_DEATH_PLANE => o.raw.set_s32(O_ACTION, OBJ_ACT_DEATH_PLANE_DEATH),
            _ => {}
        }
    }
}

/// obj_lava_death: sink for 30 frames with smoke every eighth, then
/// deactivate (true).
pub fn obj_lava_death(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) -> bool {
    let timer = object(&w.objects, &m.obj, id).raw.s32(O_TIMER);
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    if timer > 30 {
        o.active_flags = ACTIVE_FLAG_DEACTIVATED;
        return true;
    }
    o.raw.set_f32(O_POS_Y, o.raw.f32(O_POS_Y) - 10.0);
    if timer % 8 == 0 {
        cur_obj_play_sound_2(m, w, id, SOUND_OBJ_BULLY_EXPLODE_2);
        let smoke = w.behaviors.address(Behavior::BobombBullyDeathSmoke);
        let smoke = spawn_object(m, w, id, MODEL_SMOKE, smoke);
        for field in [O_POS_X, O_POS_Y, O_POS_Z] {
            let random = w.rng.random_float();
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, smoke).raw;
            raw.set_f32(field, raw.f32(field) + random * 20.0);
        }
        let random = w.rng.random_float();
        object_mut(&mut w.objects, &mut m.obj, smoke)
            .raw
            .set_f32(O_FORWARD_VEL, random * 10.0);
    }
    false
}

/// obj_spawn_yellow_coins: `n_coins` moving yellow coins thrown upward in a
/// random direction. The move yaw takes random_u16 zero-extended.
pub fn obj_spawn_yellow_coins(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    obj: ObjectId,
    n_coins: i8,
) {
    let behavior = w.behaviors.address(Behavior::MovingYellowCoin);
    for _ in 0..n_coins {
        let coin = spawn_object(m, w, obj, MODEL_YELLOW_COIN, behavior);
        let forward = w.rng.random_float() * 20.0;
        let vel_y = w.rng.random_float() * 40.0 + 20.0;
        let yaw = w.rng.random_u16();
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, coin).raw;
        raw.set_f32(O_FORWARD_VEL, forward);
        raw.set_f32(O_VEL_Y, vel_y);
        raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
    }
}

/// obj_flicker_and_disappear: invisible on odd frames for 40 frames after
/// `life_span`, then deactivated (true).
pub fn obj_flicker_and_disappear(o: &mut super::Object, life_span: i16) -> bool {
    let timer = o.raw.s32(O_TIMER);
    let life_span = i32::from(life_span);
    if timer < life_span {
        return false;
    }
    if timer < life_span + 40 {
        if timer % 2 != 0 {
            o.gfx.node_flags |= GRAPH_RENDER_INVISIBLE;
        } else {
            o.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
        }
    } else {
        obj_mark_for_deletion(o);
        return true;
    }
    false
}

/// What an actor does with object_step's result: install the terrain matrix
/// obj_orient_graph built, and perform obj_splash's requests in order. Water
/// waves and bubbles spawn particle behaviors the port does not run yet;
/// reaching one (an object at a water surface) stops with a panic.
pub fn apply_object_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    result: &ObjectStepResult,
) {
    if let Some(matrix) = result.terrain_matrix {
        object_mut(&mut w.objects, &mut m.obj, id).gfx.throw_matrix =
            Some(super::ThrowMatrix::Terrain(matrix));
    }
    for effect in &result.effects {
        match effect {
            ObjectStepEffect::Sound(sound) => cur_obj_play_sound_2(m, w, id, *sound),
            ObjectStepEffect::WaterWave | ObjectStepEffect::SmallBubble => panic!(
                "object_step's water {effect:?} spawns particle behaviors, which are not ported"
            ),
        }
    }
}

/// object_step for the current object, applied (see [`apply_object_step`]):
/// the collision flags and sObjFloor.
pub fn cur_object_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
) -> (i16, Option<SurfaceIndex>) {
    let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
    let result = super::motion::object_step(&mut o, w);
    *object_mut(&mut w.objects, &mut m.obj, id) = o;
    apply_object_step(m, w, id, &result);
    (result.collision_flags, result.floor)
}
