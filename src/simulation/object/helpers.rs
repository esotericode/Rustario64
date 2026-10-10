//! Object helpers translated from pinned CC0 src/game/object_helpers.c and
//! behavior_script.c (obj_update_gfx_pos_and_angle), with original f32
//! operation order. `o` (gCurrentObject) is an explicit argument.
use super::Object;
use crate::simulation::{
    mario::{StepWorld, constants::*},
    math::{TrigTables, mtxf_rotate_zxy_and_translate},
};

/// struct ObjectHitbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectHitbox {
    pub interact_type: u32,
    pub down_offset: u8,
    pub damage_or_coin_value: i8,
    pub health: i8,
    pub num_loot_coins: i8,
    pub radius: i16,
    pub height: i16,
    pub hurtbox_radius: i16,
    pub hurtbox_height: i16,
}

/// dist_between_objects.
pub fn dist_between_objects(obj1: &Object, obj2: &Object) -> f32 {
    let dx = obj1.raw.f32(O_POS_X) - obj2.raw.f32(O_POS_X);
    let dy = obj1.raw.f32(O_POS_Y) - obj2.raw.f32(O_POS_Y);
    let dz = obj1.raw.f32(O_POS_Z) - obj2.raw.f32(O_POS_Z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// obj_angle_to_object.
pub fn obj_angle_to_object(trig: &TrigTables, obj1: &Object, obj2: &Object) -> i16 {
    let (z1, z2) = (obj1.raw.f32(O_POS_Z), obj2.raw.f32(O_POS_Z));
    let (x1, x2) = (obj1.raw.f32(O_POS_X), obj2.raw.f32(O_POS_X));
    trig.atan2s(z2 - z1, x2 - x1)
}

/// cur_obj_move_xz_using_fvel_and_yaw.
pub fn cur_obj_move_xz_using_fvel_and_yaw(o: &mut Object, trig: &TrigTables) {
    let yaw = o.raw.s32(O_MOVE_ANGLE_YAW);
    let forward = o.raw.f32(O_FORWARD_VEL);
    o.raw.set_f32(O_VEL_X, forward * trig.sins(yaw));
    o.raw.set_f32(O_VEL_Z, forward * trig.coss(yaw));
    o.raw
        .set_f32(O_POS_X, o.raw.f32(O_POS_X) + o.raw.f32(O_VEL_X));
    o.raw
        .set_f32(O_POS_Z, o.raw.f32(O_POS_Z) + o.raw.f32(O_VEL_Z));
}

/// cur_obj_move_y_with_terminal_vel.
pub fn cur_obj_move_y_with_terminal_vel(o: &mut Object) {
    if o.raw.f32(O_VEL_Y) < -70.0 {
        o.raw.set_f32(O_VEL_Y, -70.0);
    }
    o.raw
        .set_f32(O_POS_Y, o.raw.f32(O_POS_Y) + o.raw.f32(O_VEL_Y));
}

/// obj_update_gfx_pos_and_angle (behavior_script.c).
pub fn obj_update_gfx_pos_and_angle(o: &mut Object) {
    o.gfx.pos = [
        o.raw.f32(O_POS_X),
        o.raw.f32(O_POS_Y) + o.raw.f32(O_GRAPH_Y_OFFSET),
        o.raw.f32(O_POS_Z),
    ];
    o.gfx.angle = [
        (o.raw.s32(O_FACE_ANGLE_PITCH) & 0xFFFF) as i16,
        (o.raw.s32(O_FACE_ANGLE_YAW) & 0xFFFF) as i16,
        (o.raw.s32(O_FACE_ANGLE_ROLL) & 0xFFFF) as i16,
    ];
}

/// cur_obj_enable_rendering_if_mario_in_room. gMarioCurrentRoom is 0
/// outside the levels with rooms (BBH, the castle and HMC), which the port
/// does not load, so the original changes nothing.
pub fn cur_obj_enable_rendering_if_mario_in_room(o: &Object) {
    let _ = o;
}

/// The levels whose floors carry rooms (sLevelsWithRooms).
const LEVELS_WITH_ROOMS: [i16; 3] = [LEVEL_BBH, LEVEL_CASTLE, LEVEL_HMC];

/// bhv_init_room.
pub fn bhv_init_room(o: &mut Object, level_num: i16) {
    // is_item_in_array compares the level as an s8.
    if LEVELS_WITH_ROOMS.contains(&i16::from(level_num as i8)) {
        panic!("room visibility needs the room system, which is not ported");
    }
    o.raw.set_s32(O_ROOM, -1);
}

/// obj_copy_pos.
pub fn obj_copy_pos(dst: &mut Object, src: &Object) {
    for field in [O_POS_X, O_POS_Y, O_POS_Z] {
        dst.raw.set_u32(field, src.raw.u32(field));
    }
}

/// obj_copy_angle.
pub fn obj_copy_angle(dst: &mut Object, src: &Object) {
    for field in [
        O_MOVE_ANGLE_PITCH,
        O_MOVE_ANGLE_YAW,
        O_MOVE_ANGLE_ROLL,
        O_FACE_ANGLE_PITCH,
        O_FACE_ANGLE_YAW,
        O_FACE_ANGLE_ROLL,
    ] {
        dst.raw.set_u32(field, src.raw.u32(field));
    }
}

/// obj_copy_pos_and_angle.
pub fn obj_copy_pos_and_angle(dst: &mut Object, src: &Object) {
    obj_copy_pos(dst, src);
    obj_copy_angle(dst, src);
}

/// obj_set_pos: s16 coordinates.
pub fn obj_set_pos(o: &mut Object, x: i16, y: i16, z: i16) {
    o.raw.set_f32(O_POS_X, f32::from(x));
    o.raw.set_f32(O_POS_Y, f32::from(y));
    o.raw.set_f32(O_POS_Z, f32::from(z));
}

/// obj_set_angle.
pub fn obj_set_angle(o: &mut Object, pitch: i16, yaw: i16, roll: i16) {
    for (field, value) in [
        (O_FACE_ANGLE_PITCH, pitch),
        (O_FACE_ANGLE_YAW, yaw),
        (O_FACE_ANGLE_ROLL, roll),
        (O_MOVE_ANGLE_PITCH, pitch),
        (O_MOVE_ANGLE_YAW, yaw),
        (O_MOVE_ANGLE_ROLL, roll),
    ] {
        o.raw.set_s32(field, i32::from(value));
    }
}

/// obj_set_parent_relative_pos.
pub fn obj_set_parent_relative_pos(o: &mut Object, x: i16, y: i16, z: i16) {
    o.raw.set_f32(O_PARENT_RELATIVE_POS_X, f32::from(x));
    o.raw.set_f32(O_PARENT_RELATIVE_POS_Y, f32::from(y));
    o.raw.set_f32(O_PARENT_RELATIVE_POS_Z, f32::from(z));
}

/// obj_build_transform_from_pos_and_angle: the angles are read as s32 and
/// narrowed to s16, as the original's `s16 rotation[3]`.
pub fn obj_build_transform_from_pos_and_angle(
    o: &mut Object,
    trig: &TrigTables,
    pos_index: usize,
    angle_index: usize,
) {
    let translate = [
        o.raw.f32(pos_index),
        o.raw.f32(pos_index + 1),
        o.raw.f32(pos_index + 2),
    ];
    let rotation = [
        o.raw.s32(angle_index) as i16,
        o.raw.s32(angle_index + 1) as i16,
        o.raw.s32(angle_index + 2) as i16,
    ];
    o.transform = mtxf_rotate_zxy_and_translate(trig, translate, rotation);
}

/// obj_translate_local: add the transform's rotation of the local offset.
pub fn obj_translate_local(o: &mut Object, pos_index: usize, local_index: usize) {
    let dx = o.raw.f32(local_index);
    let dy = o.raw.f32(local_index + 1);
    let dz = o.raw.f32(local_index + 2);
    let [t0, t1, t2, _] = o.transform;
    for axis in 0..3 {
        let value = o.raw.f32(pos_index + axis) + (t0[axis] * dx + t1[axis] * dy + t2[axis] * dz);
        o.raw.set_f32(pos_index + axis, value);
    }
}

/// obj_scale.
pub fn obj_scale(o: &mut Object, scale: f32) {
    o.gfx.scale = [scale; 3];
}

/// cur_obj_become_tangible.
pub fn cur_obj_become_tangible(o: &mut Object) {
    o.raw.set_s32(O_INTANGIBLE_TIMER, 0);
}

/// cur_obj_become_intangible.
pub fn cur_obj_become_intangible(o: &mut Object) {
    o.raw.set_s32(O_INTANGIBLE_TIMER, -1);
}

/// obj_set_hitbox.
pub fn obj_set_hitbox(o: &mut Object, hitbox: &ObjectHitbox) {
    if o.raw.u32(O_FLAGS) & OBJ_FLAG_30 == 0 {
        o.raw.set_u32(O_FLAGS, o.raw.u32(O_FLAGS) | OBJ_FLAG_30);
        o.raw.set_u32(O_INTERACT_TYPE, hitbox.interact_type);
        o.raw.set_s32(
            O_DAMAGE_OR_COIN_VALUE,
            i32::from(hitbox.damage_or_coin_value),
        );
        o.raw.set_s32(O_HEALTH, i32::from(hitbox.health));
        o.raw
            .set_s32(O_NUM_LOOT_COINS, i32::from(hitbox.num_loot_coins));
        cur_obj_become_tangible(o);
    }
    o.hitbox_radius = o.gfx.scale[0] * f32::from(hitbox.radius);
    o.hitbox_height = o.gfx.scale[1] * f32::from(hitbox.height);
    o.hurtbox_radius = o.gfx.scale[0] * f32::from(hitbox.hurtbox_radius);
    o.hurtbox_height = o.gfx.scale[1] * f32::from(hitbox.hurtbox_height);
    o.hitbox_down_offset = o.gfx.scale[1] * f32::from(hitbox.down_offset);
}

/// cur_obj_update_floor_height.
pub fn cur_obj_update_floor_height(o: &mut Object, w: &mut StepWorld<'_>) {
    let pos = o.pos();
    let height = w.find_floor_height(pos[0], pos[1], pos[2]);
    o.raw.set_f32(O_FLOOR_HEIGHT, height);
}

/// obj_mark_for_deletion: clears every active flag.
pub fn obj_mark_for_deletion(o: &mut Object) {
    o.active_flags = ACTIVE_FLAG_DEACTIVATED;
}

/// bit_shift_left: sPowersOfTwo, an eight-entry s16 table.
pub fn bit_shift_left(a0: i32) -> i32 {
    const POWERS_OF_TWO: [i16; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];
    let index = usize::try_from(a0)
        .ok()
        .filter(|i| *i < POWERS_OF_TWO.len())
        .unwrap_or_else(|| panic!("bit_shift_left({a0}) reads past sPowersOfTwo"));
    i32::from(POWERS_OF_TWO[index])
}

/// cur_obj_scale.
pub fn cur_obj_scale(o: &mut Object, scale: f32) {
    obj_scale(o, scale);
}
