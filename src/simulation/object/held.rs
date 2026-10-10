//! Held-object helpers, translated from pinned CC0 src/game/object_helpers.c:
//! cur_obj_move_after_thrown_or_dropped and cur_obj_get_dropped.
use super::{
    ObjectId,
    helpers::{cur_obj_become_tangible, cur_obj_enable_rendering, obj_copy_pos},
    object_mut,
};
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// cur_obj_move_after_thrown_or_dropped: back onto the floor under it (or,
/// out of bounds, at Mario), with the new speeds. A moving release runs
/// cur_obj_move_y, the standard movement family, which no ported caller
/// reaches.
fn cur_obj_move_after_thrown_or_dropped(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    forward_vel: f32,
    vel_y: f32,
) {
    let mario = m.obj.clone();
    let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
    o.raw.set_u32(O_MOVE_FLAGS, 0);
    let [x, y, z] = o.pos();
    let floor = w.find_floor_height(x, y + 160.0, z);
    o.raw.set_f32(O_FLOOR_HEIGHT, floor);
    if floor > y {
        o.raw.set_f32(O_POS_Y, floor);
    } else if floor < FLOOR_LOWER_LIMIT_MISC as f32 {
        obj_copy_pos(&mut o, &mario);
        let [x, y, z] = o.pos();
        let floor = w.find_floor_height(x, y, z);
        o.raw.set_f32(O_FLOOR_HEIGHT, floor);
    }
    o.raw.set_f32(O_FORWARD_VEL, forward_vel);
    o.raw.set_f32(O_VEL_Y, vel_y);
    assert!(
        o.raw.f32(O_FORWARD_VEL) == 0.0,
        "cur_obj_move_y (a thrown or placed object's standard movement) is not ported"
    );
    *object_mut(&mut w.objects, &mut m.obj, id) = o;
}

/// cur_obj_get_dropped.
pub fn cur_obj_get_dropped(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    cur_obj_become_tangible(o);
    cur_obj_enable_rendering(o);
    o.raw.set_s32(O_HELD_STATE, HELD_FREE);
    cur_obj_move_after_thrown_or_dropped(m, w, id, 0.0, 0.0);
}
