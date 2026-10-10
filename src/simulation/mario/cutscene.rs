//! Cutscene actions, translated from pinned CC0
//! src/game/mario_actions_cutscene.c: the group dispatcher's instant-quicksand
//! check, Mario's side of NPC dialog (mario_ready_to_speak,
//! set_mario_npc_dialog) and the reading/waiting NPC dialog actions. Every
//! other cutscene action is still a recorded `Unsupported` boundary that ends
//! the action loop, as in the oracle.
use super::{
    Event, MarioState, StepWorld, Unsupported,
    animation::set_mario_animation,
    constants::*,
    core::{drop_and_set_mario_action, set_mario_action, update_mario_sound_and_camera},
    interaction::mario_obj_angle_to_object,
};
use crate::simulation::math::approach_s32;

/// mario_ready_to_speak.
pub fn mario_ready_to_speak(m: &MarioState) -> bool {
    let action_group = m.action & ACT_GROUP_MASK;
    (m.action == ACT_WAITING_FOR_DIALOG
        || action_group == ACT_GROUP_STATIONARY
        || action_group == ACT_GROUP_MOVING)
        && (m.action & (ACT_FLAG_RIDING_SHELL | ACT_FLAG_INVULNERABLE) == 0
            && m.action != ACT_FIRST_PERSON)
}

/// set_mario_npc_dialog, called by the current object (gCurrentObject).
/// Returns MARIO_DIALOG_STATUS_NONE, _START or _SPEAK.
pub fn set_mario_npc_dialog(m: &mut MarioState, w: &mut StepWorld<'_>, action_arg: u32) -> u32 {
    let mut dialog_state = MARIO_DIALOG_STATUS_NONE;
    if m.action == ACT_READING_NPC_DIALOG {
        if m.action_state < 8 {
            dialog_state = MARIO_DIALOG_STATUS_START;
        }
        if m.action_state == 8 {
            if action_arg == MARIO_DIALOG_STOP {
                m.action_state = m.action_state.wrapping_add(1);
            } else {
                dialog_state = MARIO_DIALOG_STATUS_SPEAK;
            }
        }
    } else if action_arg != 0 && mario_ready_to_speak(m) {
        m.used_obj = w.objects.current;
        set_mario_action(m, w, ACT_READING_NPC_DIALOG, action_arg);
        dialog_state = MARIO_DIALOG_STATUS_START;
    }
    dialog_state
}

/// act_reading_npc_dialog. Action states 0–7 turn toward the NPC, 8 holds
/// the dialog, 9–22 look back and 23 ends the action.
fn act_reading_npc_dialog(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let mut head_turn_amount: i32 = 0;
    if m.action_arg == MARIO_DIALOG_LOOK_UP {
        head_turn_amount = -1024;
    }
    if m.action_arg == MARIO_DIALOG_LOOK_DOWN {
        head_turn_amount = 384;
    }
    if m.action_state < 8 {
        let used_obj = m
            .used_obj
            .expect("Mario's usedObj is NULL (the original crashes)");
        let angle_to_npc = mario_obj_angle_to_object(m, w, used_obj);
        let delta = (i32::from(angle_to_npc) - i32::from(m.face_angle[1])).wrapping_shl(16) >> 16;
        m.face_angle[1] = (i32::from(angle_to_npc) - approach_s32(delta, 0, 2048, 2048)) as i16;
        m.action_timer = (i32::from(m.action_timer) + head_turn_amount) as u16;
        let anim = if m.held_obj.is_none() {
            MARIO_ANIM_FIRST_PERSON
        } else {
            MARIO_ANIM_IDLE_WITH_LIGHT_OBJ
        };
        set_mario_animation(m, w, anim);
    } else if m.action_state >= 9 && m.action_state < 17 {
        m.action_timer = (i32::from(m.action_timer) - head_turn_amount) as u16;
    } else if m.action_state == 23 {
        if m.flags & MARIO_CAP_IN_HAND != 0 {
            set_mario_action(m, w, ACT_PUTTING_ON_CAP, 0);
        } else {
            let action = if m.held_obj.is_none() {
                ACT_IDLE
            } else {
                ACT_HOLD_IDLE
            };
            set_mario_action(m, w, action, 0);
        }
    }
    m.obj.gfx.pos = m.pos;
    m.obj.gfx.angle = [0, m.face_angle[1], 0];
    m.body.head_angle = [m.action_timer as i16, 0, 0];
    if m.action_state != 8 {
        m.action_state = m.action_state.wrapping_add(1);
    }
    0
}

/// act_waiting_for_dialog.
fn act_waiting_for_dialog(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let anim = if m.held_obj.is_none() {
        MARIO_ANIM_FIRST_PERSON
    } else {
        MARIO_ANIM_IDLE_WITH_LIGHT_OBJ
    };
    set_mario_animation(m, w, anim);
    m.obj.gfx.pos = m.pos;
    m.obj.gfx.angle = [0, m.face_angle[1], 0];
    0
}

/// check_for_instant_quicksand. A NULL floor crashes the original.
fn check_for_instant_quicksand(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let floor = m
        .floor
        .expect("Mario's floor is NULL (the original crashes)");
    let floor_type = w.surface(floor).surface_type;
    if floor_type == SURFACE_INSTANT_QUICKSAND
        && m.action & ACT_FLAG_INVULNERABLE != 0
        && m.action != ACT_QUICKSAND_DEATH
    {
        update_mario_sound_and_camera(m, w);
        return drop_and_set_mario_action(m, w, ACT_QUICKSAND_DEATH, 0);
    }
    0
}

/// mario_execute_cutscene_action for the ported actions. Other cutscene
/// actions record `Unsupported::CutsceneGroup` and end the loop (FALSE).
pub fn mario_execute_cutscene_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    if check_for_instant_quicksand(m, w) != 0 {
        return 1;
    }
    let cancel = match m.action {
        ACT_READING_NPC_DIALOG => act_reading_npc_dialog(m, w),
        ACT_WAITING_FOR_DIALOG => act_waiting_for_dialog(m, w),
        action => {
            w.event(Event::Unsupported(Unsupported::CutsceneGroup(action)));
            return 0;
        }
    };
    if cancel == 0 && m.input & INPUT_IN_WATER != 0 {
        m.particle_flags |= PARTICLE_IDLE_WATER_WAVE;
    }
    cancel
}
