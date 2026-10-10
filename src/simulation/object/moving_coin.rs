//! Moving yellow coins (enemy loot), translated from pinned CC0
//! src/game/behaviors/moving_coin.inc.c: sMovingYellowCoinHitbox, coin_step,
//! moving_coin_flicker, coin_collected and bhv_moving_yellow_coin_init/loop.
//! Blue coins in the same file are not ported.
use super::{
    ObjectId,
    helpers::{ObjectHitbox, cur_obj_become_intangible, cur_obj_become_tangible, obj_set_hitbox},
    motion::{OBJ_COL_FLAG_GROUNDED, OBJ_COL_FLAG_NO_Y_VEL},
    obj_behaviors::{cur_object_step, obj_check_floor_death, obj_flicker_and_disappear},
    object, object_mut,
    script::Behavior,
    sound::cur_obj_play_sound_2,
    spawn::spawn_object,
};
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// sMovingYellowCoinHitbox.
pub const MOVING_YELLOW_COIN_HITBOX: ObjectHitbox = ObjectHitbox {
    interact_type: INTERACT_COIN,
    down_offset: 0,
    damage_or_coin_value: 1,
    health: 0,
    num_loot_coins: 0,
    radius: 100,
    height: 64,
    hurtbox_radius: 0,
    hurtbox_height: 0,
};

/// coin_step: object_step with the floor death check; true (and the drop
/// sound) when it lands with vertical speed.
fn coin_step(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) -> bool {
    let (collision_flags, floor) = cur_object_step(m, w, id);
    let collision = w.collision;
    obj_check_floor_death(
        object_mut(&mut w.objects, &mut m.obj, id),
        collision,
        collision_flags,
        floor,
    );
    if collision_flags & OBJ_COL_FLAG_GROUNDED != 0 && collision_flags & OBJ_COL_FLAG_NO_Y_VEL == 0
    {
        cur_obj_play_sound_2(m, w, id, SOUND_GENERAL_COIN_DROP);
        return true;
    }
    false
}

/// moving_coin_flicker.
fn moving_coin_flicker(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    coin_step(m, w, id);
    obj_flicker_and_disappear(object_mut(&mut w.objects, &mut m.obj, id), 0);
}

/// coin_collected.
fn coin_collected(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let sparkles = w.behaviors.address(Behavior::GoldenCoinSparkles);
    spawn_object(m, w, id, MODEL_SPARKLES, sparkles);
    object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
}

/// bhv_moving_yellow_coin_init.
pub fn bhv_moving_yellow_coin_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.raw.set_f32(O_GRAVITY, 3.0);
    o.raw.set_f32(O_FRICTION, 1.0);
    o.raw.set_f32(O_BUOYANCY, 1.5);
    obj_set_hitbox(o, &MOVING_YELLOW_COIN_HITBOX);
}

/// bhv_moving_yellow_coin_loop: intangible for ten frames, blinking away
/// after 300.
pub fn bhv_moving_yellow_coin_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    match object(&w.objects, &m.obj, id).raw.s32(O_ACTION) {
        MOV_YCOIN_ACT_IDLE => {
            coin_step(m, w, id);
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            if o.raw.s32(O_TIMER) < 10 {
                cur_obj_become_intangible(o);
            } else {
                cur_obj_become_tangible(o);
            }
            if o.raw.s32(O_TIMER) > 300 {
                o.raw.set_s32(O_ACTION, MOV_YCOIN_ACT_BLINKING);
            }
        }
        MOV_YCOIN_ACT_BLINKING => moving_coin_flicker(m, w, id),
        MOV_YCOIN_ACT_LAVA_DEATH | MOV_YCOIN_ACT_DEATH_PLANE_DEATH => {
            object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
        }
        _ => {}
    }
    if object(&w.objects, &m.obj, id).raw.u32(O_INTERACT_STATUS) & INT_STATUS_INTERACTED != 0 {
        coin_collected(m, w, id);
        object_mut(&mut w.objects, &mut m.obj, id)
            .raw
            .set_s32(O_INTERACT_STATUS, 0);
    }
}
