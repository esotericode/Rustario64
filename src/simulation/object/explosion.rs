//! Explosions and what they leave behind, translated from pinned CC0
//! src/game/behaviors/explosion.inc.c (bhv_explosion_init,
//! bhv_explosion_loop) and src/game/behaviors/corkbox.inc.c
//! (bhv_bobomb_bully_death_smoke_init, bhv_respawner_loop,
//! create_respawner). The explosion bubbles of that file (underwater
//! explosions) are not ported: an explosion below a water surface stops with
//! a panic instead of spawning them.
use super::{
    ObjectId,
    helpers::cur_obj_scale,
    obj_behaviors::{is_point_within_radius_of_mario, mario_gfx_pos},
    object, object_mut,
    script::Behavior,
    sound::create_sound_spawner,
    spawn::{spawn_object, spawn_object_abs_with_rot},
};
use crate::simulation::mario::{
    Event, MarioState, StepWorld, constants::*, f32_to_s16, f32_to_s32,
};

/// bhv_explosion_init: the explosion sound (through a sound spawner) and the
/// environmental camera shake, which the linked camera performs after the
/// object update in call order (`simulation::game`).
pub fn bhv_explosion_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    create_sound_spawner(m, w, id, SOUND_GENERAL2_BOBOMB_EXPLOSION);
    w.event(Event::EnvironmentalCameraShake(SHAKE_ENV_EXPLOSION));
    object_mut(&mut w.objects, &mut m.obj, id)
        .raw
        .set_s32(O_OPACITY, 255);
}

/// bhv_explosion_loop: grow and fade for nine frames, then leave smoke.
pub fn bhv_explosion_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object(&w.objects, &m.obj, id);
    if o.raw.s32(O_TIMER) == 9 {
        let [x, y, z] = o.pos();
        if w.collision.find_water_level(x, z) > y {
            panic!("an underwater explosion spawns bhvBobombExplosionBubble, which is not ported");
        }
        let smoke = w.behaviors.address(Behavior::BobombBullyDeathSmoke);
        spawn_object(m, w, id, MODEL_SMOKE, smoke);
        object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
    }
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.raw
        .set_s32(O_OPACITY, o.raw.s32(O_OPACITY).wrapping_sub(14));
    // (f32) oTimer / 9.0f + 1.0: float division, double sum, f32 parameter.
    let scale = (f64::from(o.raw.s32(O_TIMER) as f32 / 9.0) + 1.0) as f32;
    cur_obj_scale(o, scale);
}

/// bhv_bobomb_bully_death_smoke_init.
pub fn bhv_bobomb_bully_death_smoke_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.raw.set_f32(O_POS_Y, o.raw.f32(O_POS_Y) - 300.0);
    cur_obj_scale(o, 10.0);
}

/// bhv_respawner_loop: once Mario is farther than the minimum distance,
/// spawn the remembered behavior and model with the same parameters.
pub fn bhv_respawner_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object(&w.objects, &m.obj, id);
    let [x, y, z] = o.pos();
    // The f32 distance converts to the s32 parameter.
    let dist = f32_to_s32(o.raw.f32(O_RESPAWNER_MIN_SPAWN_DIST));
    if !is_point_within_radius_of_mario(mario_gfx_pos(m), x, y, z, dist) {
        let (model, behavior, params) = (
            o.raw.s32(O_RESPAWNER_MODEL_TO_RESPAWN),
            o.raw.u32(O_RESPAWNER_BEHAVIOR_TO_RESPAWN),
            o.raw.s32(O_BHV_PARAMS),
        );
        let spawned = spawn_object(m, w, id, model, behavior);
        object_mut(&mut w.objects, &mut m.obj, spawned)
            .raw
            .set_s32(O_BHV_PARAMS, params);
        object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
    }
}

/// create_respawner: a respawner at the current object's home (as s16
/// coordinates) that remembers its parameters, model and behavior.
pub fn create_respawner(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    model: i32,
    behavior: u32,
    min_spawn_dist: i32,
) {
    let o = object(&w.objects, &m.obj, id);
    let home = [O_HOME_X, O_HOME_Y, O_HOME_Z].map(|field| f32_to_s16(o.raw.f32(field)));
    let params = o.raw.s32(O_BHV_PARAMS);
    let respawner_script = w.behaviors.address(Behavior::Respawner);
    let respawner =
        spawn_object_abs_with_rot(m, w, id, MODEL_NONE, respawner_script, home, [0, 0, 0]);
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, respawner).raw;
    raw.set_s32(O_BHV_PARAMS, params);
    raw.set_s32(O_RESPAWNER_MODEL_TO_RESPAWN, model);
    raw.set_f32(O_RESPAWNER_MIN_SPAWN_DIST, min_spawn_dist as f32);
    raw.set_u32(O_RESPAWNER_BEHAVIOR_TO_RESPAWN, behavior);
}
