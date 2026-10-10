//! Object-side dialog helpers translated from pinned CC0
//! src/game/object_helpers.c. `o` (gCurrentObject) is an explicit argument;
//! gMarioObject and gMarioStates[0] are `m`. cur_obj_update_dialog_with_cutscene
//! (US: BUGFIX_DIALOG_TIME_STOP) drives the camera's dialog cutscene; the
//! cutscene-free cur_obj_update_dialog is not ported yet.
use super::{
    Object, ObjectId,
    helpers::{cur_obj_rotate_yaw_toward, lateral_dist_between_objects, obj_angle_to_object},
    object, object_mut,
};
use crate::simulation::mario::{
    MarioState, StepWorld,
    constants::*,
    cutscene::{mario_ready_to_speak, set_mario_npc_dialog},
};

/// oDialogResponse and oDialogState: the two halves of one word.
fn o_dialog_response(o: &Object) -> i16 {
    o.raw.s16(O_DIALOG_RESPONSE, 0)
}
fn o_dialog_state(o: &Object) -> i16 {
    o.raw.s16(O_DIALOG_STATE, 1)
}

/// cur_obj_update_dialog_with_cutscene: time stop, Mario's speaking pose
/// (turning the object toward him if asked), the camera's dialog cutscene
/// and the release; returns the dialog response once, when it ends.
pub fn cur_obj_update_dialog_with_cutscene(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    action_arg: u32,
    dialog_flags: i32,
    cutscene_table: u8,
    dialog_id: i16,
) -> i32 {
    let mut dialog_response = DIALOG_RESPONSE_NONE;
    let mut done_turning = true;
    let state = i32::from(o_dialog_state(object(&w.objects, &m.obj, id)));
    match state {
        DIALOG_STATUS_ENABLE_TIME_STOP | DIALOG_STATUS_INTERRUPT => {
            let mut interrupt = state == DIALOG_STATUS_INTERRUPT;
            if !interrupt {
                // Wait for Mario to be ready to speak, then stop time and
                // fall through so his action is interrupted at once.
                if mario_ready_to_speak(m) || m.action == ACT_READING_NPC_DIALOG {
                    w.time_stop_state |= TIME_STOP_ENABLED;
                    let o = object_mut(&mut w.objects, &mut m.obj, id);
                    o.active_flags |= ACTIVE_FLAG_INITIATED_TIME_STOP;
                    o.raw.set_s16(O_DIALOG_STATE, 1, (state + 1) as i16);
                    o.raw
                        .set_s16(O_DIALOG_RESPONSE, 0, DIALOG_RESPONSE_NONE as i16);
                    interrupt = true;
                }
            }
            if interrupt {
                if dialog_flags & DIALOG_FLAG_TURN_TO_MARIO != 0 {
                    let trig = w.trig;
                    let mario = w.objects.mario.expect("gMarioObject is NULL");
                    let target = obj_angle_to_object(
                        trig,
                        object(&w.objects, &m.obj, id),
                        object(&w.objects, &m.obj, mario),
                    );
                    let o = object_mut(&mut w.objects, &mut m.obj, id);
                    done_turning = cur_obj_rotate_yaw_toward(o, target, 0x800);
                    if o_dialog_response(o) >= 33 {
                        done_turning = true;
                    }
                }
                let speaking = set_mario_npc_dialog(m, w, action_arg) == MARIO_DIALOG_STATUS_SPEAK;
                let o = object_mut(&mut w.objects, &mut m.obj, id);
                if speaking && done_turning {
                    o.raw.set_s16(O_DIALOG_RESPONSE, 0, 0);
                    let next = o_dialog_state(o).wrapping_add(1);
                    o.raw.set_s16(O_DIALOG_STATE, 1, next);
                } else {
                    let timer = o_dialog_response(o).wrapping_add(1);
                    o.raw.set_s16(O_DIALOG_RESPONSE, 0, timer);
                }
            }
        }
        DIALOG_STATUS_START_DIALOG => {
            let response = if cutscene_table == CUTSCENE_CAP_SWITCH_PRESS {
                w.cutscene
                    .cutscene_object_with_dialog(cutscene_table, id, DIALOG_NONE)
            } else {
                w.cutscene
                    .cutscene_object_with_dialog(cutscene_table, id, dialog_id)
            };
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            o.raw.set_s16(O_DIALOG_RESPONSE, 0, response);
            if response != 0 {
                let next = o_dialog_state(o).wrapping_add(1);
                o.raw.set_s16(O_DIALOG_STATE, 1, next);
            }
        }
        DIALOG_STATUS_STOP_DIALOG => {
            if dialog_flags & DIALOG_FLAG_TIME_STOP_ENABLED != 0 {
                let o = object_mut(&mut w.objects, &mut m.obj, id);
                dialog_response = i32::from(o_dialog_response(o));
                o.raw
                    .set_s16(O_DIALOG_STATE, 1, DIALOG_STATUS_ENABLE_TIME_STOP as i16);
            } else if m.action != ACT_READING_NPC_DIALOG {
                w.time_stop_state &= !TIME_STOP_ENABLED;
                let o = object_mut(&mut w.objects, &mut m.obj, id);
                o.active_flags &= !ACTIVE_FLAG_INITIATED_TIME_STOP;
                dialog_response = i32::from(o_dialog_response(o));
                o.raw
                    .set_s16(O_DIALOG_STATE, 1, DIALOG_STATUS_ENABLE_TIME_STOP as i16);
            } else {
                set_mario_npc_dialog(m, w, MARIO_DIALOG_STOP);
            }
        }
        _ => {}
    }
    dialog_response
}

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
