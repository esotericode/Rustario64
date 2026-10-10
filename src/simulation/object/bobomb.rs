//! Black Bob-ombs, translated from pinned CC0
//! src/game/behaviors/bobomb.inc.c: sBobombHitbox, bhv_bobomb_init,
//! bobomb_spawn_coin, bobomb_act_explode, bobomb_check_interactions, the
//! patrol, chase and launched actions, the generic and stationary free
//! loops, the held, dropped and thrown loops, curr_obj_random_blink,
//! bhv_bobomb_loop and bhv_bobomb_fuse_smoke_init. The Bob-omb Buddy's
//! functions in the same file are not ported.
//!
//! Bob-ombs walk with obj_behaviors.c's object_step (`motion`), whose
//! terrain matrix and splash requests apply through
//! [`obj_behaviors::cur_object_step`].
use super::{
    ObjectId,
    animation::cur_obj_init_animation,
    explosion::create_respawner,
    held::cur_obj_get_dropped,
    helpers::{
        ObjectHitbox, cur_obj_enable_rendering, cur_obj_scale, cur_obj_set_pos_relative,
        obj_set_hitbox, obj_turn_toward_object,
    },
    motion::OBJ_COL_FLAG_GROUNDED,
    obj_behaviors::{
        cur_object_step, is_point_within_radius_of_mario, mario_gfx_pos, obj_check_floor_death,
        obj_check_if_facing_toward_angle, obj_lava_death, obj_return_home_if_safe,
        obj_spawn_yellow_coins,
    },
    object, object_mut,
    script::Behavior,
    sound::{cur_obj_play_sound_1, cur_obj_play_sound_2},
    spawn::{set_object_respawn_info_bits, spawn_object},
};
use crate::simulation::mario::{MarioState, StepWorld, constants::*, f32_to_s16, f32_to_s32};

/// sBobombHitbox.
pub const BOBOMB_HITBOX: ObjectHitbox = ObjectHitbox {
    interact_type: INTERACT_GRABBABLE,
    down_offset: 0,
    damage_or_coin_value: 0,
    health: 0,
    num_loot_coins: 0,
    radius: 65,
    height: 113,
    hurtbox_radius: 0,
    hurtbox_height: 0,
};

/// bhv_bobomb_init.
pub fn bhv_bobomb_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
    raw.set_f32(O_GRAVITY, 2.5);
    raw.set_f32(O_FRICTION, 0.8);
    raw.set_f32(O_BUOYANCY, 1.3);
    raw.set_u32(O_INTERACTION_SUBTYPE, INT_SUBTYPE_KICKABLE);
}

/// bobomb_spawn_coin: one moving coin, once per placement (respawn bit 1).
fn bobomb_spawn_coin(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    if (object(&w.objects, &m.obj, id).raw.s32(O_BHV_PARAMS) >> 8) & 0x01 == 0 {
        obj_spawn_yellow_coins(m, w, id, 1);
        let o = object_mut(&mut w.objects, &mut m.obj, id);
        o.raw.set_s32(O_BHV_PARAMS, 0x100);
        let (info_type, info) = (o.respawn_info_type, o.respawn_info);
        set_object_respawn_info_bits(w, info_type, info, 1);
    }
}

/// bobomb_act_explode: swell for five frames, then an explosion, the coin
/// and a respawner replace the Bob-omb.
fn bobomb_act_explode(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let timer = object(&w.objects, &m.obj, id).raw.s32(O_TIMER);
    if timer < 5 {
        // 1.0 + (f32) oTimer / 5.0 in double precision, narrowed by the parameter.
        let scale = (1.0 + f64::from(timer as f32) / 5.0) as f32;
        cur_obj_scale(object_mut(&mut w.objects, &mut m.obj, id), scale);
    } else {
        let behavior = w.behaviors.address(Behavior::Explosion);
        let explosion = spawn_object(m, w, id, MODEL_EXPLOSION, behavior);
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, explosion).raw;
        raw.set_f32(O_GRAPH_Y_OFFSET, raw.f32(O_GRAPH_Y_OFFSET) + 100.0);
        bobomb_spawn_coin(m, w, id);
        let bobomb = w.behaviors.address(Behavior::Bobomb);
        create_respawner(m, w, id, MODEL_BLACK_BOBOMB, bobomb, 3000);
        object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
    }
}

/// obj_attack_collided_from_other_object (object_helpers.c): the first
/// object this one collided with, unless it is Mario, is attacked as if
/// punched and told it touched a Bob-omb.
pub fn obj_attack_collided_from_other_object(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
) -> bool {
    let o = object(&w.objects, &m.obj, id);
    if o.num_collided_objs != 0 {
        let other = o.collided_objs[0].expect("a recorded collision names its object");
        if w.objects.mario != Some(other) {
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, other).raw;
            raw.set_u32(
                O_INTERACT_STATUS,
                raw.u32(O_INTERACT_STATUS)
                    | ATTACK_PUNCH
                    | INT_STATUS_WAS_ATTACKED
                    | INT_STATUS_INTERACTED
                    | INT_STATUS_TOUCHED_BOB_OMB,
            );
            return true;
        }
    }
    false
}

/// bobomb_check_interactions: a kick launches it, another Bob-omb's touch or
/// a collision with any object but Mario explodes it.
fn bobomb_check_interactions(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let mario_yaw = m.obj.gfx.angle[1];
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    obj_set_hitbox(o, &BOBOMB_HITBOX);
    let status = o.raw.u32(O_INTERACT_STATUS);
    if status & INT_STATUS_INTERACTED != 0 {
        if status & INT_STATUS_MARIO_KNOCKBACK_DMG != 0 {
            o.raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(mario_yaw));
            o.raw.set_f32(O_FORWARD_VEL, 25.0);
            o.raw.set_f32(O_VEL_Y, 30.0);
            o.raw.set_s32(O_ACTION, BOBOMB_ACT_LAUNCHED);
        }
        if status & INT_STATUS_TOUCHED_BOB_OMB != 0 {
            o.raw.set_s32(O_ACTION, BOBOMB_ACT_EXPLODE);
        }
        o.raw.set_s32(O_INTERACT_STATUS, 0);
    }
    if obj_attack_collided_from_other_object(m, w, id) {
        object_mut(&mut w.objects, &mut m.obj, id)
            .raw
            .set_s32(O_ACTION, BOBOMB_ACT_EXPLODE);
    }
}

/// bobomb_act_patrol: walk home; light the fuse when Mario is near home and
/// in front.
fn bobomb_act_patrol(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    object_mut(&mut w.objects, &mut m.obj, id)
        .raw
        .set_f32(O_FORWARD_VEL, 5.0);
    let (collision_flags, floor) = cur_object_step(m, w, id);
    let mario = mario_gfx_pos(m);
    let trig = w.trig;
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    let home = [
        o.raw.f32(O_HOME_X),
        o.raw.f32(O_HOME_Y),
        o.raw.f32(O_HOME_Z),
    ];
    if obj_return_home_if_safe(o, trig, mario, home, 400)
        && obj_check_if_facing_toward_angle(
            trig,
            o.raw.u32(O_MOVE_ANGLE_YAW),
            o.raw.u32(O_ANGLE_TO_MARIO),
            0x2000,
        )
    {
        o.raw.set_s32(O_BOBOMB_FUSE_LIT, 1);
        o.raw.set_s32(O_ACTION, BOBOMB_ACT_CHASE_MARIO);
    }
    let collision = w.collision;
    obj_check_floor_death(
        object_mut(&mut w.objects, &mut m.obj, id),
        collision,
        collision_flags,
        floor,
    );
}

/// bobomb_act_chase_mario: the animation steps one extra frame per update.
fn bobomb_act_chase_mario(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.gfx.anim.anim_frame = o.gfx.anim.anim_frame.wrapping_add(1);
    let anim_frame = o.gfx.anim.anim_frame;
    o.raw.set_f32(O_FORWARD_VEL, 20.0);
    let (collision_flags, floor) = cur_object_step(m, w, id);
    if anim_frame == 5 || anim_frame == 16 {
        cur_obj_play_sound_2(m, w, id, SOUND_OBJ_BOBOMB_WALK);
    }
    let target = m.obj.pos();
    let trig = w.trig;
    obj_turn_toward_object(
        object_mut(&mut w.objects, &mut m.obj, id),
        trig,
        target,
        O_MOVE_ANGLE_YAW,
        0x800,
    );
    let collision = w.collision;
    obj_check_floor_death(
        object_mut(&mut w.objects, &mut m.obj, id),
        collision,
        collision_flags,
        floor,
    );
}

/// bobomb_act_launched: explode on landing.
fn bobomb_act_launched(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let (collision_flags, _) = cur_object_step(m, w, id);
    if collision_flags & OBJ_COL_FLAG_GROUNDED == OBJ_COL_FLAG_GROUNDED {
        object_mut(&mut w.objects, &mut m.obj, id)
            .raw
            .set_s32(O_ACTION, BOBOMB_ACT_EXPLODE);
    }
}

/// The death actions both free loops share.
fn bobomb_death_actions(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, action: i32) {
    let bobomb = w.behaviors.address(Behavior::Bobomb);
    match action {
        BOBOMB_ACT_EXPLODE => bobomb_act_explode(m, w, id),
        BOBOMB_ACT_LAVA_DEATH => {
            if obj_lava_death(m, w, id) {
                create_respawner(m, w, id, MODEL_BLACK_BOBOMB, bobomb, 3000);
            }
        }
        BOBOMB_ACT_DEATH_PLANE_DEATH => {
            object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
            create_respawner(m, w, id, MODEL_BLACK_BOBOMB, bobomb, 3000);
        }
        _ => {}
    }
}

/// generic_bobomb_free_loop and stationary_bobomb_free_loop: the stationary
/// variant neither patrols nor chases.
fn bobomb_free_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object(&w.objects, &m.obj, id);
    let generic = o.raw.s32(O_BHV_PARAMS2ND_BYTE) == BOBOMB_BP_STYPE_GENERIC;
    match o.raw.s32(O_ACTION) {
        BOBOMB_ACT_PATROL if generic => bobomb_act_patrol(m, w, id),
        BOBOMB_ACT_LAUNCHED => bobomb_act_launched(m, w, id),
        BOBOMB_ACT_CHASE_MARIO if generic => bobomb_act_chase_mario(m, w, id),
        action => bobomb_death_actions(m, w, id, action),
    }
    bobomb_check_interactions(m, w, id);
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
    if raw.s32(O_BOBOMB_FUSE_TIMER) > 150 {
        raw.set_s32(O_ACTION, 3);
    }
}

/// bobomb_held_loop: drawn through Mario's hand; an expired fuse makes him
/// drop it. (The action is set to explode, but that runs only once the held
/// state changes: the original's regrab quirk.)
fn bobomb_held_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let anims = w.object_anims;
    let trig = w.trig;
    let mario = m.obj.clone();
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.gfx.node_flags |= GRAPH_RENDER_INVISIBLE;
    cur_obj_init_animation(o, anims, 1);
    cur_obj_set_pos_relative(o, trig, &mario, 0.0, 60.0, 100.0);
    o.raw.set_s32(O_BOBOMB_FUSE_LIT, 1);
    if o.raw.s32(O_BOBOMB_FUSE_TIMER) > 150 {
        let status = m.obj.raw.u32(O_INTERACT_STATUS);
        m.obj
            .raw
            .set_u32(O_INTERACT_STATUS, status | INT_STATUS_MARIO_DROP_OBJECT);
        object_mut(&mut w.objects, &mut m.obj, id)
            .raw
            .set_s32(O_ACTION, BOBOMB_ACT_EXPLODE);
    }
}

/// bobomb_dropped_loop.
fn bobomb_dropped_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    cur_obj_get_dropped(m, w, id);
    let anims = w.object_anims;
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    cur_obj_init_animation(o, anims, 0);
    o.raw.set_s32(O_HELD_STATE, 0);
    o.raw.set_s32(O_ACTION, BOBOMB_ACT_PATROL);
}

/// bobomb_thrown_loop.
fn bobomb_thrown_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    cur_obj_enable_rendering(o);
    o.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    o.raw.set_s32(O_HELD_STATE, 0);
    o.raw.set_u32(
        O_FLAGS,
        o.raw.u32(O_FLAGS) & !OBJ_FLAG_SET_FACE_YAW_TO_MOVE_YAW,
    );
    o.raw.set_f32(O_FORWARD_VEL, 25.0);
    o.raw.set_f32(O_VEL_Y, 20.0);
    o.raw.set_s32(O_ACTION, BOBOMB_ACT_LAUNCHED);
}

/// curr_obj_random_blink for the timer at raw index `timer`: oAnimState 1
/// (eyes closed) in two short blinks.
pub fn curr_obj_random_blink(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    timer: usize,
) {
    if object(&w.objects, &m.obj, id).raw.s32(timer) == 0 {
        // (s16)(random_float() * 100.0f)
        if f32_to_s16(w.rng.random_float() * 100.0) == 0 {
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
            raw.set_s32(O_ANIM_STATE, 1);
            raw.set_s32(timer, 1);
        }
    } else {
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
        let blink = raw.s32(timer).wrapping_add(1);
        raw.set_s32(timer, blink);
        if blink > 5 {
            raw.set_s32(O_ANIM_STATE, 0);
        }
        if blink > 10 {
            raw.set_s32(O_ANIM_STATE, 1);
        }
        if blink > 15 {
            raw.set_s32(O_ANIM_STATE, 0);
            raw.set_s32(timer, 0);
        }
    }
}

/// bhv_bobomb_loop: only within 4000 units of Mario.
pub fn bhv_bobomb_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let pos = object(&w.objects, &m.obj, id).pos();
    if !is_point_within_radius_of_mario(mario_gfx_pos(m), pos[0], pos[1], pos[2], 4000) {
        return;
    }
    match object(&w.objects, &m.obj, id).raw.s32(O_HELD_STATE) {
        HELD_FREE => bobomb_free_loop(m, w, id),
        HELD_HELD => bobomb_held_loop(m, w, id),
        HELD_THROWN => bobomb_thrown_loop(m, w, id),
        HELD_DROPPED => bobomb_dropped_loop(m, w, id),
        _ => {}
    }
    curr_obj_random_blink(m, w, id, O_BOBOMB_BLINK_TIMER);
    let o = object(&w.objects, &m.obj, id);
    if o.raw.s32(O_BOBOMB_FUSE_LIT) == 1 {
        let fuse_timer = o.raw.s32(O_BOBOMB_FUSE_TIMER);
        let dust_period_minus_1: i32 = if fuse_timer > 120 { 1 } else { 7 };
        if dust_period_minus_1 & fuse_timer == 0 {
            let smoke = w.behaviors.address(Behavior::BobombFuseSmoke);
            spawn_object(m, w, id, MODEL_SMOKE, smoke);
        }
        cur_obj_play_sound_1(m, w, id, SOUND_AIR_BOBOMB_LIT_FUSE);
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
        raw.set_s32(
            O_BOBOMB_FUSE_TIMER,
            raw.s32(O_BOBOMB_FUSE_TIMER).wrapping_add(1),
        );
    }
}

/// bhv_bobomb_fuse_smoke_init: a puff at a random offset above the fuse.
pub fn bhv_bobomb_fuse_smoke_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    for (field, offset) in [(O_POS_X, -40), (O_POS_Y, 60), (O_POS_Z, -40)] {
        let random = f32_to_s32(w.rng.random_float() * 80.0);
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
        raw.set_f32(field, raw.f32(field) + random.wrapping_add(offset) as f32);
    }
    cur_obj_scale(object_mut(&mut w.objects, &mut m.obj, id), 1.2);
}
