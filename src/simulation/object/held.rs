//! Held-object helpers, translated from pinned CC0 src/game/object_helpers.c:
//! cur_obj_move_after_thrown_or_dropped, cur_obj_get_dropped,
//! cur_obj_get_thrown_or_placed and the shared Mario grabbing helpers.
use super::{
    Object, ObjectId,
    helpers::{cur_obj_become_tangible, cur_obj_enable_rendering, obj_copy_pos},
    object_mut,
};
use crate::simulation::controller::A_BUTTON;
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// cur_obj_get_thrown_or_placed for the supported non-Bowser actors.
/// Bowser's special parent-relative release is outside the spawned subset.
pub fn cur_obj_get_thrown_or_placed(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    forward_vel: f32,
    vel_y: f32,
    thrown_action: i32,
) {
    let mario = m.obj.clone();
    let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
    cur_obj_become_tangible(&mut o);
    cur_obj_enable_rendering(&mut o);
    o.raw.set_s32(O_HELD_STATE, HELD_FREE);
    if o.raw.u32(O_INTERACTION_SUBTYPE) & INT_SUBTYPE_HOLDABLE_NPC != 0 || forward_vel == 0.0 {
        cur_obj_move_after_thrown_or_dropped(&mut o, w, &mario, 0.0, 0.0);
    } else {
        o.raw.set_s32(O_ACTION, thrown_action);
        cur_obj_move_after_thrown_or_dropped(&mut o, w, &mario, forward_vel, vel_y);
    }
    *object_mut(&mut w.objects, &mut m.obj, id) = o;
}

/// cur_obj_check_grabbed_mario: acknowledge the interaction without clearing it.
pub fn cur_obj_check_grabbed_mario(o: &mut Object) -> bool {
    if o.raw.u32(O_INTERACT_STATUS) & INT_STATUS_GRABBED_MARIO != 0 {
        o.raw.set_s32(O_KING_BOBOMB_UNK88, 1);
        super::helpers::cur_obj_become_intangible(o);
        true
    } else {
        false
    }
}

/// player_performed_grab_escape_action: strict stick hysteresis, or a new A
/// press. Both inputs in one sample still count as one escape action.
pub fn player_performed_grab_escape_action(w: &mut StepWorld<'_>) -> bool {
    if w.controller.stick_mag < 30.0 {
        w.grab_release_state = 0;
    }
    let mut result = false;
    if w.grab_release_state == 0 && w.controller.stick_mag > 40.0 {
        w.grab_release_state = 1;
        result = true;
    }
    if w.controller.button_pressed & A_BUTTON != 0 {
        result = true;
    }
    result
}

/// common_anchor_mario_behavior (chuckya.inc.c), shared by King Bob-omb.
/// State 1 places Mario's graphics at the animated anchor; 2 and 3 release
/// him on his next action update. The boss and anchor remain unspawned.
pub fn common_anchor_mario_behavior(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    forward_vel: f32,
    vel_y: f32,
    status: u32,
) {
    let anchor = w.objects.slot(id).clone();
    let parent = anchor
        .parent
        .expect("Mario anchor dereferences a NULL parentObj");
    let o = w.objects.slot_mut(parent);
    match o.raw.s32(O_KING_BOBOMB_UNK88) {
        1 => {
            m.obj.gfx.pos = [
                anchor.raw.f32(O_POS_X),
                anchor.raw.f32(O_POS_Y) + anchor.raw.f32(O_GRAPH_Y_OFFSET),
                anchor.raw.f32(O_POS_Z),
            ];
            m.obj.gfx.angle = [
                anchor.raw.s32(O_MOVE_ANGLE_PITCH) as i16,
                anchor.raw.s32(O_MOVE_ANGLE_YAW) as i16,
                anchor.raw.s32(O_MOVE_ANGLE_ROLL) as i16,
            ];
        }
        state @ (2 | 3) => {
            let flags = if state == 2 {
                INT_STATUS_MARIO_UNK2.wrapping_add(status)
            } else {
                INT_STATUS_MARIO_UNK2 | INT_STATUS_MARIO_UNK6
            };
            m.obj
                .raw
                .set_u32(O_INTERACT_STATUS, m.obj.raw.u32(O_INTERACT_STATUS) | flags);
            m.forward_vel = if state == 2 { forward_vel } else { 10.0 };
            m.vel[1] = if state == 2 { vel_y } else { 10.0 };
            o.raw.set_s32(O_KING_BOBOMB_UNK88, 0);
        }
        _ => {}
    }
    let yaw = o.raw.s32(O_MOVE_ANGLE_YAW);
    let dead = o.active_flags == ACTIVE_FLAG_DEACTIVATED;
    let anchor = w.objects.slot_mut(id);
    anchor.raw.set_s32(O_MOVE_ANGLE_YAW, yaw);
    if dead {
        super::helpers::obj_mark_for_deletion(anchor);
    }
}

/// cur_obj_move_after_thrown_or_dropped: back onto the floor under it (or,
/// out of bounds, at Mario), with the new speeds. A moving release runs
/// cur_obj_move_y, the standard movement family.
pub fn cur_obj_move_after_thrown_or_dropped(
    o: &mut Object,
    w: &mut StepWorld<'_>,
    mario: &Object,
    forward_vel: f32,
    vel_y: f32,
) {
    o.raw.set_u32(O_MOVE_FLAGS, 0);
    let [x, y, z] = o.pos();
    let floor = w.find_floor_height(x, y + 160.0, z);
    o.raw.set_f32(O_FLOOR_HEIGHT, floor);
    if floor > y {
        o.raw.set_f32(O_POS_Y, floor);
    } else if floor < FLOOR_LOWER_LIMIT_MISC as f32 {
        obj_copy_pos(o, mario);
        let [x, y, z] = o.pos();
        let floor = w.find_floor_height(x, y, z);
        o.raw.set_f32(O_FLOOR_HEIGHT, floor);
    }
    o.raw.set_f32(O_FORWARD_VEL, forward_vel);
    o.raw.set_f32(O_VEL_Y, vel_y);
    if o.raw.f32(O_FORWARD_VEL) != 0.0 {
        super::standard_motion::cur_obj_move_y(o, w, -4.0, -0.1, 2.0);
    }
}

/// cur_obj_get_dropped.
pub fn cur_obj_get_dropped(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let mario = m.obj.clone();
    let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
    cur_obj_become_tangible(&mut o);
    cur_obj_enable_rendering(&mut o);
    o.raw.set_s32(O_HELD_STATE, HELD_FREE);
    cur_obj_move_after_thrown_or_dropped(&mut o, w, &mario, 0.0, 0.0);
    *object_mut(&mut w.objects, &mut m.obj, id) = o;
}
