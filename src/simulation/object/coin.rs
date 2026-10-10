//! Yellow coins, coin formations and their sparkles, translated from pinned
//! CC0 src/game/behaviors/coin.inc.c (bhv_coin_sparkles_init,
//! bhv_yellow_coin_init/loop, bhv_coin_formation_spawn_loop,
//! spawn_coin_in_formation, bhv_coin_formation_init/loop,
//! bhv_coin_sparkles_loop, bhv_golden_coin_sparkles_loop) and the hitbox
//! they share (sYellowCoinHitbox, a constant of the same file).
use super::{
    ObjectId,
    helpers::{
        ObjectHitbox, bhv_init_room, bit_shift_left, cur_obj_scale, cur_obj_update_floor_height,
        obj_mark_for_deletion, obj_set_hitbox,
    },
    object, object_mut,
    script::Behavior,
    spawn::{set_object_respawn_info_bits, spawn_object, spawn_object_relative},
};
use crate::simulation::mario::{MarioState, StepWorld, constants::*, f32_to_s32};

/// sYellowCoinHitbox.
pub const YELLOW_COIN_HITBOX: ObjectHitbox = ObjectHitbox {
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

/// absf (object_helpers.c).
fn absf(x: f32) -> f32 {
    if x >= 0.0 { x } else { -x }
}

/// bhv_coin_sparkles_init: once Mario collected the coin, spawn the golden
/// sparkles and delete it.
pub fn bhv_coin_sparkles_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) -> bool {
    let status = object(&w.objects, &m.obj, id).raw.u32(O_INTERACT_STATUS);
    if status & INT_STATUS_INTERACTED != 0 && status & INT_STATUS_TOUCHED_BOB_OMB == 0 {
        let sparkles = w.behaviors.address(Behavior::GoldenCoinSparkles);
        spawn_object(m, w, id, MODEL_SPARKLES, sparkles);
        obj_mark_for_deletion(object_mut(&mut w.objects, &mut m.obj, id));
        return true;
    }
    object_mut(&mut w.objects, &mut m.obj, id)
        .raw
        .set_s32(O_INTERACT_STATUS, 0);
    false
}

/// bhv_yellow_coin_init.
pub fn bhv_yellow_coin_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let yellow_coin = w.behaviors.address(Behavior::YellowCoin);
    let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
    o.behavior = yellow_coin;
    obj_set_hitbox(&mut o, &YELLOW_COIN_HITBOX);
    bhv_init_room(&mut o, w.level_num);
    cur_obj_update_floor_height(&mut o, w);
    if 500.0 < absf(o.raw.f32(O_POS_Y) - o.raw.f32(O_FLOOR_HEIGHT)) {
        o.gfx.shared_child = w.loaded_model(MODEL_YELLOW_COIN_NO_SHADOW);
    }
    if o.raw.f32(O_FLOOR_HEIGHT) < FLOOR_LOWER_LIMIT_MISC as f32 {
        obj_mark_for_deletion(&mut o);
    }
    *object_mut(&mut w.objects, &mut m.obj, id) = o;
}

/// bhv_yellow_coin_loop.
pub fn bhv_yellow_coin_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    bhv_coin_sparkles_init(m, w, id);
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
    raw.set_s32(O_ANIM_STATE, raw.s32(O_ANIM_STATE).wrapping_add(1));
}

/// bhv_coin_formation_spawn_loop: one coin of a formation.
pub fn bhv_coin_formation_spawn_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    if object(&w.objects, &m.obj, id).raw.s32(O_TIMER) == 0 {
        let yellow_coin = w.behaviors.address(Behavior::YellowCoin);
        let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
        o.behavior = yellow_coin;
        obj_set_hitbox(&mut o, &YELLOW_COIN_HITBOX);
        bhv_init_room(&mut o, w.level_num);
        if o.raw.s32(O_COIN_ON_GROUND) != 0 {
            o.raw.set_f32(O_POS_Y, o.raw.f32(O_POS_Y) + 300.0);
            cur_obj_update_floor_height(&mut o, w);
            let floor = o.raw.f32(O_FLOOR_HEIGHT);
            if o.raw.f32(O_POS_Y) < floor || floor < FLOOR_LOWER_LIMIT_MISC as f32 {
                obj_mark_for_deletion(&mut o);
            } else {
                o.raw.set_f32(O_POS_Y, floor);
            }
        } else {
            cur_obj_update_floor_height(&mut o, w);
            if absf(o.raw.f32(O_POS_Y) - o.raw.f32(O_FLOOR_HEIGHT)) > 250.0 {
                o.gfx.shared_child = w.loaded_model(MODEL_YELLOW_COIN_NO_SHADOW);
            }
        }
        *object_mut(&mut w.objects, &mut m.obj, id) = o;
    } else {
        if bhv_coin_sparkles_init(m, w, id) {
            let o = object(&w.objects, &m.obj, id);
            let bit = bit_shift_left(o.raw.s32(O_BHV_PARAMS2ND_BYTE));
            let parent = o.parent.expect("parentObj is NULL");
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, parent).raw;
            raw.set_s32(
                O_COIN_COLLECTED_FLAGS,
                raw.s32(O_COIN_COLLECTED_FLAGS) | bit,
            );
        }
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
        raw.set_s32(O_ANIM_STATE, raw.s32(O_ANIM_STATE).wrapping_add(1));
    }
    let parent = object(&w.objects, &m.obj, id)
        .parent
        .expect("parentObj is NULL");
    if object(&w.objects, &m.obj, parent).raw.s32(O_ACTION) == COIN_FORMATION_ACT_RESPAWN_COINS {
        obj_mark_for_deletion(object_mut(&mut w.objects, &mut m.obj, id));
    }
}

/// sCoinArrowPositions.
const COIN_ARROW_POSITIONS: [[i16; 2]; 8] = [
    [0, -150],
    [0, -50],
    [0, 50],
    [0, 150],
    [-50, 100],
    [-100, 50],
    [50, 100],
    [100, 50],
];

/// spawn_coin_in_formation: coin `coin_index` of the formation `id`.
fn spawn_coin_in_formation(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    coin_index: i32,
    flags: i32,
) {
    let mut pos = [0i32; 3];
    let mut set_spawner = true;
    let mut on_ground = true;
    let trig = w.trig;
    match flags & COIN_FORMATION_BP_FLAG_MASK {
        COIN_FORMATION_BP_LINE_HORIZONTAL => {
            pos[2] = 160 * (coin_index - 2);
            if coin_index > 4 {
                set_spawner = false;
            }
        }
        COIN_FORMATION_BP_LINE_VERTICAL => {
            on_ground = false;
            // 160 * coinIndex * 0.8: int, then double, truncated to s32.
            pos[1] = (f64::from(160 * coin_index) * 0.8) as i32;
            if coin_index > 4 {
                set_spawner = false;
            }
        }
        COIN_FORMATION_BP_RING_HORIZONTAL => {
            pos[0] = f32_to_s32(trig.sins(coin_index << 13) * 300.0);
            pos[2] = f32_to_s32(trig.coss(coin_index << 13) * 300.0);
        }
        COIN_FORMATION_BP_RING_VERTICAL => {
            on_ground = false;
            pos[0] = f32_to_s32(trig.coss(coin_index << 13) * 200.0);
            pos[1] = f32_to_s32(trig.sins(coin_index << 13) * 200.0 + 200.0);
        }
        COIN_FORMATION_BP_ARROW => {
            let arrow = COIN_ARROW_POSITIONS[coin_index as usize];
            pos[0] = i32::from(arrow[0]);
            pos[2] = i32::from(arrow[1]);
        }
        _ => {}
    }
    if flags & COIN_FORMATION_BP_FLAG_FLYING != 0 {
        on_ground = false;
    }
    if set_spawner {
        let behavior = w.behaviors.address(Behavior::CoinFormationSpawn);
        let spawner = spawn_object_relative(
            m,
            w,
            coin_index as i16,
            pos.map(|v| v as i16),
            id,
            MODEL_YELLOW_COIN,
            behavior,
        );
        object_mut(&mut w.objects, &mut m.obj, spawner)
            .raw
            .set_s32(O_COIN_ON_GROUND, i32::from(on_ground));
    }
}

/// bhv_coin_formation_init: the collected coins recorded in the respawn bits.
pub fn bhv_coin_formation_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
    raw.set_s32(O_COIN_COLLECTED_FLAGS, (raw.s32(O_BHV_PARAMS) >> 8) & 0xFF);
}

/// bhv_coin_formation_loop: spawn the uncollected coins when Mario is near,
/// respawn them after he leaves.
pub fn bhv_coin_formation_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object(&w.objects, &m.obj, id);
    let (action, distance) = (o.raw.s32(O_ACTION), o.raw.f32(O_DISTANCE_TO_MARIO));
    match action {
        COIN_FORMATION_ACT_SPAWN_COINS => {
            if distance < 2000.0 {
                for coin_index in 0..=7 {
                    let o = object(&w.objects, &m.obj, id);
                    let (collected, flags) = (
                        o.raw.s32(O_COIN_COLLECTED_FLAGS),
                        o.raw.s32(O_BHV_PARAMS2ND_BYTE),
                    );
                    if collected & (1 << coin_index) == 0 {
                        spawn_coin_in_formation(m, w, id, coin_index, flags);
                    }
                }
                let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
                raw.set_s32(O_ACTION, raw.s32(O_ACTION).wrapping_add(1));
            }
        }
        COIN_FORMATION_ACT_IDLE => {
            if distance > 2100.0 {
                let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
                raw.set_s32(O_ACTION, raw.s32(O_ACTION).wrapping_add(1));
            }
        }
        COIN_FORMATION_ACT_RESPAWN_COINS => {
            object_mut(&mut w.objects, &mut m.obj, id)
                .raw
                .set_s32(O_ACTION, COIN_FORMATION_ACT_SPAWN_COINS);
        }
        _ => {}
    }
    let o = object(&w.objects, &m.obj, id);
    let (info_type, info) = (o.respawn_info_type, o.respawn_info);
    let bits = (o.raw.s32(O_COIN_COLLECTED_FLAGS) & 0xFF) as u8;
    set_object_respawn_info_bits(w, info_type, info, bits);
}

/// bhv_coin_sparkles_loop.
pub fn bhv_coin_sparkles_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    cur_obj_scale(object_mut(&mut w.objects, &mut m.obj, id), 0.6);
}

/// bhv_golden_coin_sparkles_loop: one sparkle at a random offset.
pub fn bhv_golden_coin_sparkles_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let spread = 30.0f32;
    let behavior = w.behaviors.address(Behavior::CoinSparkles);
    let sparkles = spawn_object(m, w, id, MODEL_SPARKLES, behavior);
    let dx = w.rng.random_float() * spread - spread / 2.0;
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, sparkles).raw;
    raw.set_f32(O_POS_X, raw.f32(O_POS_X) + dx);
    let dz = w.rng.random_float() * spread - spread / 2.0;
    let raw = &mut object_mut(&mut w.objects, &mut m.obj, sparkles).raw;
    raw.set_f32(O_POS_Z, raw.f32(O_POS_Z) + dz);
}
