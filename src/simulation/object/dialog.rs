//! Object-side dialog helpers translated from pinned CC0
//! src/game/object_helpers.c. `o` (gCurrentObject) is an explicit argument;
//! gMarioObject and gMarioStates[0] are `m`. The dialog state machines
//! (cur_obj_update_dialog and cur_obj_update_dialog_with_cutscene) need the
//! dialog box and the camera's dialog cutscenes and are not ported yet.
use super::{Object, helpers::lateral_dist_between_objects};
use crate::simulation::mario::{MarioState, constants::*, cutscene::mario_ready_to_speak};

/// cur_obj_can_mario_activate_textbox. The original also computes Mario's
/// angle to the object and ignores it; that pure calculation is omitted.
pub fn cur_obj_can_mario_activate_textbox(
    o: &Object,
    m: &MarioState,
    radius: f32,
    height: f32,
) -> bool {
    if o.raw.f32(O_DISTANCE_TO_MARIO) < 1500.0 {
        let lat_dist_to_mario = lateral_dist_between_objects(o, &m.obj);
        if lat_dist_to_mario < radius
            && o.raw.f32(O_POS_Y) < m.obj.raw.f32(O_POS_Y) + 160.0
            && m.obj.raw.f32(O_POS_Y) < o.raw.f32(O_POS_Y) + height
            && m.action & ACT_FLAG_AIR == 0
            && mario_ready_to_speak(m)
        {
            return true;
        }
    }
    false
}

/// cur_obj_can_mario_activate_textbox_2.
pub fn cur_obj_can_mario_activate_textbox_2(
    o: &Object,
    m: &MarioState,
    radius: f32,
    height: f32,
) -> bool {
    cur_obj_can_mario_activate_textbox(o, m, radius, height)
}
