//! King Bob-omb, translated from pinned CC0
//! src/game/behaviors/king_bobomb.inc.c: bhv_bobomb_anchor_mario_loop, the
//! nine actions (waiting for Mario and his first dialog, approaching,
//! walking and grabbing, holding and throwing Mario, being thrown, returning
//! to the summit after being thrown off it, being hit, his defeat dialog and
//! his last action), sKingBobombActions' dispatch, sKingBobombSoundStates,
//! king_bobomb_move and bhv_king_bobomb_loop. US version (non-JP branches).
//!
//! His geo layout's geo_update_held_mario_pos acts only through prevObj,
//! which nothing sets for him, so the render pass leaves it a no-op (see
//! `render`). The star he spawns at his defeat (spawn_default_star) is a
//! recorded `Unsupported::StarSpawn` stop: the star object, its cutscene and
//! collection are not ported.
use super::{
    Object, ObjectId,
    animation::{
        cur_obj_check_anim_frame, cur_obj_init_animation_and_anim_frame,
        cur_obj_init_animation_and_check_if_near_end, cur_obj_init_animation_and_extend_if_at_end,
        cur_obj_init_animation_with_sound,
    },
    dialog::{cur_obj_can_mario_activate_textbox_2, cur_obj_update_dialog_with_cutscene},
    held::{
        common_anchor_mario_behavior, cur_obj_check_grabbed_mario, cur_obj_get_thrown_or_placed,
        player_performed_grab_escape_action,
    },
    helpers::{
        approach_s16_symmetric, arc_to_goal_pos, cur_obj_angle_to_home, cur_obj_become_intangible,
        cur_obj_become_tangible, cur_obj_disable_rendering, cur_obj_enable_rendering, cur_obj_hide,
        cur_obj_rotate_yaw_toward, cur_obj_set_pos_to_home, cur_obj_shake_screen,
    },
    object, object_mut,
    particles::{spawn_mist_particles_variable, spawn_triangle_break_particles},
    sound::{SoundState, create_sound_spawner, cur_obj_play_sound_2, exec_anim_sound_state},
    standard_motion::{
        cur_obj_move_standard, cur_obj_move_using_fvel_and_gravity, cur_obj_update_floor_and_walls,
    },
};
use crate::simulation::{
    dialog::SEQUENCE_ARGS_4_SEQ_EVENT_BOSS,
    mario::{Event, MarioState, StepWorld, Unsupported, constants::*},
};

/// The object, taken out of the pool for helpers that also need the world
/// (they read collision and tables only), then put back.
fn with_object<R>(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    f: impl FnOnce(&mut Object, &mut StepWorld<'_>) -> R,
) -> R {
    let mut o = std::mem::take(object_mut(&mut w.objects, &mut m.obj, id));
    let result = f(&mut o, w);
    *object_mut(&mut w.objects, &mut m.obj, id) = o;
    result
}

fn o<'x>(m: &'x mut MarioState, w: &'x mut StepWorld<'_>, id: ObjectId) -> &'x mut Object {
    object_mut(&mut w.objects, &mut m.obj, id)
}

fn init_anim(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, index: i32) {
    let anims = w.object_anims;
    cur_obj_init_animation_with_sound(o(m, w, id), anims, index);
}

fn init_anim_near_end(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, index: i32) -> bool {
    let anims = w.object_anims;
    cur_obj_init_animation_and_check_if_near_end(o(m, w, id), anims, index)
}

fn shake(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let obj = object(&w.objects, &m.obj, id).clone();
    cur_obj_shake_screen(&obj, &mut w.events, SHAKE_POS_SMALL);
}

fn stop_boss_music(w: &mut StepWorld<'_>) {
    w.event(Event::StopBackgroundMusic(SEQUENCE_ARGS_4_SEQ_EVENT_BOSS));
}

/// bhv_bobomb_anchor_mario_loop.
pub fn bhv_bobomb_anchor_mario_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    common_anchor_mario_behavior(m, w, id, 50.0, 50.0, INT_STATUS_MARIO_UNK6);
}

/// king_bobomb_act_0: wait on the summit until Mario comes to talk, then
/// introduce himself.
fn king_bobomb_act_0(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let k = o(m, w, id);
    k.raw.set_f32(O_FORWARD_VEL, 0.0);
    k.raw.set_f32(O_VEL_Y, 0.0);
    if k.raw.s32(O_SUB_ACTION) == 0 {
        cur_obj_become_intangible(k);
        w.cutscene.second_focus = Some(id);
        init_anim(m, w, id, 5);
        let k = o(m, w, id);
        cur_obj_set_pos_to_home(k);
        k.raw.set_s32(O_HEALTH, 3);
        if cur_obj_can_mario_activate_textbox_2(object(&w.objects, &m.obj, id), m, 500.0, 100.0) {
            let k = o(m, w, id);
            k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
            w.event(Event::SeqLowerVolume {
                player: SEQ_PLAYER_LEVEL,
                duration: 60,
                percentage: 40,
            });
        }
    } else if cur_obj_update_dialog_with_cutscene(
        m,
        w,
        id,
        MARIO_DIALOG_LOOK_UP,
        DIALOG_FLAG_TURN_TO_MARIO,
        CUTSCENE_DIALOG,
        DIALOG_017,
    ) != 0
    {
        let k = o(m, w, id);
        k.raw.set_s32(O_ACTION, 2);
        k.raw
            .set_u32(O_FLAGS, k.raw.u32(O_FLAGS) | OBJ_FLAG_HOLDABLE);
    }
}

/// mario_is_far_below_object.
fn mario_is_far_below_object(m: &MarioState, w: &StepWorld<'_>, id: ObjectId, dist: f32) -> bool {
    let k = object(&w.objects, &m.obj, id);
    dist < k.raw.f32(O_POS_Y) - m.obj.raw.f32(O_POS_Y)
}

/// king_bobomb_act_2: stomp toward Mario; grab him on contact; go back to
/// waiting if he leaves the summit.
fn king_bobomb_act_2(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let k = o(m, w, id);
    cur_obj_become_tangible(k);
    if k.raw.f32(O_POS_Y) - k.raw.f32(O_HOME_Y) < -100.0 {
        // Thrown off the hill.
        k.raw.set_s32(O_ACTION, 5);
        cur_obj_become_intangible(k);
    }
    if k.raw.s32(O_KING_BOBOMB_UNK100) == 0 {
        if cur_obj_check_anim_frame(k, 15) {
            shake(m, w, id);
        }
        if init_anim_near_end(m, w, id, 4) {
            let k = o(m, w, id);
            k.raw
                .set_s32(O_KING_BOBOMB_UNK100, k.raw.s32(O_KING_BOBOMB_UNK100) + 1);
        }
    } else {
        if k.raw.s32(O_KING_BOBOMB_UNK100) == 1 {
            let anims = w.object_anims;
            let k = o(m, w, id);
            cur_obj_init_animation_and_anim_frame(k, anims, 11, 7);
            k.raw.set_s32(O_KING_BOBOMB_UNK100, 2);
        } else {
            init_anim(m, w, id, 11);
        }
        let k = o(m, w, id);
        if k.raw.s32(O_KING_BOBOMB_UNK108) == 0 {
            k.raw.set_f32(O_FORWARD_VEL, 3.0);
            let target = k.raw.s32(O_ANGLE_TO_MARIO) as i16;
            cur_obj_rotate_yaw_toward(k, target, 0x100);
        } else {
            k.raw.set_f32(O_FORWARD_VEL, 0.0);
            k.raw
                .set_s32(O_KING_BOBOMB_UNK108, k.raw.s32(O_KING_BOBOMB_UNK108) - 1);
        }
    }
    if cur_obj_check_grabbed_mario(o(m, w, id)) {
        o(m, w, id).raw.set_s32(O_ACTION, 3);
    }
    if mario_is_far_below_object(m, w, id, 1200.0) {
        o(m, w, id).raw.set_s32(O_ACTION, 0);
        stop_boss_music(w);
    }
}

/// king_bobomb_act_3: hold Mario, let him wriggle free, or turn and throw
/// him.
fn king_bobomb_act_3(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let k = o(m, w, id);
    match k.raw.s32(O_SUB_ACTION) {
        0 => {
            k.raw.set_f32(O_FORWARD_VEL, 0.0);
            k.raw.set_s32(O_KING_BOBOMB_UNK104, 0);
            k.raw.set_s32(O_KING_BOBOMB_UNK_F_C, 0);
            if k.raw.s32(O_TIMER) == 0 {
                cur_obj_play_sound_2(m, w, id, SOUND_OBJ_UNKNOWN3);
            }
            if init_anim_near_end(m, w, id, 0) {
                let anims = w.object_anims;
                let k = o(m, w, id);
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
                cur_obj_init_animation_and_anim_frame(k, anims, 1, 0);
            }
        }
        1 => {
            init_anim(m, w, id, 1);
            let escaped = i32::from(player_performed_grab_escape_action(w));
            let k = o(m, w, id);
            k.raw.set_s32(
                O_KING_BOBOMB_UNK_F_C,
                k.raw.s32(O_KING_BOBOMB_UNK_F_C).wrapping_add(escaped),
            );
            // print_debug_bottom_up prints only on a debug page.
            if k.raw.s32(O_KING_BOBOMB_UNK_F_C) > 10 {
                k.raw.set_s32(O_KING_BOBOMB_UNK88, 3);
                k.raw.set_s32(O_ACTION, 2);
                k.raw.set_s32(O_KING_BOBOMB_UNK108, 35);
                k.raw.set_u32(
                    O_INTERACT_STATUS,
                    k.raw.u32(O_INTERACT_STATUS) & !INT_STATUS_GRABBED_MARIO,
                );
            } else {
                k.raw.set_f32(O_FORWARD_VEL, 3.0);
                if k.raw.s32(O_KING_BOBOMB_UNK104) > 20 && cur_obj_rotate_yaw_toward(k, 0, 0x400) {
                    let anims = w.object_anims;
                    let k = o(m, w, id);
                    k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
                    cur_obj_init_animation_and_anim_frame(k, anims, 9, 22);
                }
            }
            let k = o(m, w, id);
            k.raw
                .set_s32(O_KING_BOBOMB_UNK104, k.raw.s32(O_KING_BOBOMB_UNK104) + 1);
        }
        _ => {
            init_anim(m, w, id, 9);
            let anims = w.object_anims;
            if cur_obj_check_anim_frame(object(&w.objects, &m.obj, id), 31) {
                o(m, w, id).raw.set_s32(O_KING_BOBOMB_UNK88, 2);
                cur_obj_play_sound_2(m, w, id, SOUND_OBJ_UNKNOWN4);
            } else if super::animation::cur_obj_check_if_near_animation_end(
                object(&w.objects, &m.obj, id),
                anims,
            ) {
                let k = o(m, w, id);
                k.raw.set_s32(O_ACTION, 1);
                k.raw.set_u32(
                    O_INTERACT_STATUS,
                    k.raw.u32(O_INTERACT_STATUS) & !INT_STATUS_GRABBED_MARIO,
                );
            }
        }
    }
}

/// king_bobomb_act_1: after throwing Mario, face him and wait for him to
/// come back.
fn king_bobomb_act_1(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let k = o(m, w, id);
    k.raw.set_f32(O_FORWARD_VEL, 0.0);
    k.raw.set_f32(O_VEL_Y, 0.0);
    init_anim(m, w, id, 11);
    let k = o(m, w, id);
    let yaw = approach_s16_symmetric(
        k.raw.s32(O_MOVE_ANGLE_YAW) as i16,
        k.raw.s32(O_ANGLE_TO_MARIO) as i16,
        0x200,
    );
    k.raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
    if k.raw.f32(O_DISTANCE_TO_MARIO) < 2500.0 {
        k.raw.set_s32(O_ACTION, 2);
    }
    if mario_is_far_below_object(m, w, id, 1200.0) {
        o(m, w, id).raw.set_s32(O_ACTION, 0);
        stop_boss_music(w);
    }
}

/// king_bobomb_act_6: hit — shake, steam, then turn back to Mario.
fn king_bobomb_act_6(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    match object(&w.objects, &m.obj, id).raw.s32(O_SUB_ACTION) {
        0 => {
            if object(&w.objects, &m.obj, id).raw.s32(O_TIMER) == 0 {
                o(m, w, id).raw.set_s32(O_KING_BOBOMB_UNK104, 0);
                cur_obj_play_sound_2(m, w, id, SOUND_OBJ_KING_BOBOMB);
                cur_obj_play_sound_2(m, w, id, SOUND_OBJ2_KING_BOBOMB_DAMAGE);
                shake(m, w, id);
                spawn_mist_particles_variable(m, w, id, 0, 0, 100.0);
                let k = o(m, w, id);
                k.raw.set_u32(O_INTERACT_TYPE, INTERACT_DAMAGE);
                cur_obj_become_tangible(k);
            }
            if init_anim_near_end(m, w, id, 2) {
                let k = o(m, w, id);
                k.raw
                    .set_s32(O_KING_BOBOMB_UNK104, k.raw.s32(O_KING_BOBOMB_UNK104) + 1);
            }
            let k = o(m, w, id);
            if k.raw.s32(O_KING_BOBOMB_UNK104) > 3 {
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
            }
        }
        1 => {
            if init_anim_near_end(m, w, id, 10) {
                let k = o(m, w, id);
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
                k.raw.set_u32(O_INTERACT_TYPE, INTERACT_GRABBABLE);
                cur_obj_become_intangible(k);
            }
        }
        _ => {
            init_anim(m, w, id, 11);
            let k = o(m, w, id);
            let target = k.raw.s32(O_ANGLE_TO_MARIO) as i16;
            if cur_obj_rotate_yaw_toward(k, target, 0x800) {
                k.raw.set_s32(O_ACTION, 2);
            }
        }
    }
}

/// king_bobomb_act_7: defeated — his last dialog, then he bursts and the
/// star spawns.
fn king_bobomb_act_7(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    init_anim(m, w, id, 2);
    if cur_obj_update_dialog_with_cutscene(
        m,
        w,
        id,
        MARIO_DIALOG_LOOK_UP,
        DIALOG_FLAG_TEXT_DEFAULT,
        CUTSCENE_DIALOG,
        DIALOG_116,
    ) != 0
    {
        create_sound_spawner(m, w, id, SOUND_OBJ_KING_WHOMP_DEATH);
        let k = o(m, w, id);
        cur_obj_hide(k);
        cur_obj_become_intangible(k);
        spawn_mist_particles_variable(m, w, id, 0, 0, 200.0);
        spawn_triangle_break_particles(m, w, id, 20, MODEL_DIRT_ANIMATION as i16, 3.0, 4);
        shake(m, w, id);
        // cur_obj_spawn_star_at_y_offset(2000, 4500, -4500, 200): the star
        // spawns 200 units above him (his position is restored after).
        w.event(Event::Unsupported(Unsupported::StarSpawn));
        o(m, w, id).raw.set_s32(O_ACTION, 8);
    }
}

/// king_bobomb_act_8: his music stops two seconds after his defeat.
fn king_bobomb_act_8(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    if object(&w.objects, &m.obj, id).raw.s32(O_TIMER) == 60 {
        stop_boss_music(w);
    }
}

/// king_bobomb_act_4: thrown — a landing on the summit costs health; off
/// the summit he settles, then heads back.
fn king_bobomb_act_4(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let k = o(m, w, id);
    if k.raw.f32(O_POS_Y) - k.raw.f32(O_HOME_Y) > -100.0 {
        // Not thrown off the hill.
        if k.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_LANDED != 0 {
            k.raw.set_s32(O_HEALTH, k.raw.s32(O_HEALTH).wrapping_sub(1));
            k.raw.set_f32(O_FORWARD_VEL, 0.0);
            k.raw.set_f32(O_VEL_Y, 0.0);
            cur_obj_play_sound_2(m, w, id, SOUND_OBJ_KING_BOBOMB);
            let k = o(m, w, id);
            let action = if k.raw.s32(O_HEALTH) != 0 { 6 } else { 7 };
            k.raw.set_s32(O_ACTION, action);
        }
    } else if k.raw.s32(O_SUB_ACTION) == 0 {
        if k.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_ON_GROUND != 0 {
            k.raw.set_f32(O_FORWARD_VEL, 0.0);
            k.raw.set_f32(O_VEL_Y, 0.0);
            k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
        } else if k.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_LANDED != 0 {
            cur_obj_play_sound_2(m, w, id, SOUND_OBJ_KING_BOBOMB);
        }
    } else {
        if init_anim_near_end(m, w, id, 10) {
            // Go back to the top of the hill.
            o(m, w, id).raw.set_s32(O_ACTION, 5);
        }
        let k = o(m, w, id);
        k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
    }
}

/// king_bobomb_act_5: jump back onto the summit, then scold Mario.
#[allow(clippy::collapsible_match)] // the original's switch, case by case
fn king_bobomb_act_5(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    match object(&w.objects, &m.obj, id).raw.s32(O_SUB_ACTION) {
        0 => {
            if object(&w.objects, &m.obj, id).raw.s32(O_TIMER) == 0 {
                cur_obj_play_sound_2(m, w, id, SOUND_OBJ_KING_BOBOMB_JUMP);
            }
            o(m, w, id).raw.set_s32(O_KING_BOBOMB_UNK_F8, 1);
            let anims = w.object_anims;
            let trig = w.trig;
            let k = o(m, w, id);
            cur_obj_init_animation_and_extend_if_at_end(k, anims, 8);
            let yaw = cur_obj_angle_to_home(k, trig);
            k.raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
            if k.raw.f32(O_POS_Y) < k.raw.f32(O_HOME_Y) {
                k.raw.set_f32(O_VEL_Y, 100.0);
            } else {
                let home = [
                    k.raw.f32(O_HOME_X),
                    k.raw.f32(O_HOME_Y),
                    k.raw.f32(O_HOME_Z),
                ];
                arc_to_goal_pos(k, trig, home, 100.0, -4.0);
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
            }
        }
        1 => {
            let anims = w.object_anims;
            let k = o(m, w, id);
            cur_obj_init_animation_and_extend_if_at_end(k, anims, 8);
            if k.raw.f32(O_VEL_Y) < 0.0 && k.raw.f32(O_POS_Y) < k.raw.f32(O_HOME_Y) {
                k.raw.set_f32(O_POS_Y, k.raw.f32(O_HOME_Y));
                k.raw.set_f32(O_VEL_Y, 0.0);
                k.raw.set_f32(O_FORWARD_VEL, 0.0);
                k.raw.set_f32(O_GRAVITY, -4.0);
                k.raw.set_s32(O_KING_BOBOMB_UNK_F8, 0);
                init_anim(m, w, id, 7);
                cur_obj_play_sound_2(m, w, id, SOUND_OBJ_KING_BOBOMB);
                shake(m, w, id);
                let k = o(m, w, id);
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
            }
        }
        2 => {
            if init_anim_near_end(m, w, id, 7) {
                let k = o(m, w, id);
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
            }
        }
        3 => {
            if mario_is_far_below_object(m, w, id, 1200.0) {
                o(m, w, id).raw.set_s32(O_ACTION, 0);
                stop_boss_music(w);
            }
            if cur_obj_can_mario_activate_textbox_2(object(&w.objects, &m.obj, id), m, 500.0, 100.0)
            {
                let k = o(m, w, id);
                k.raw.set_s32(O_SUB_ACTION, k.raw.s32(O_SUB_ACTION) + 1);
            }
        }
        4 => {
            if cur_obj_update_dialog_with_cutscene(
                m,
                w,
                id,
                MARIO_DIALOG_LOOK_UP,
                DIALOG_FLAG_TURN_TO_MARIO,
                CUTSCENE_DIALOG,
                DIALOG_128,
            ) != 0
            {
                o(m, w, id).raw.set_s32(O_ACTION, 2);
            }
        }
        _ => {}
    }
}

/// NO_SOUND (sounds.h).
const NO_SOUND: u32 = 0;

/// sKingBobombSoundStates.
const SOUND_STATES: [SoundState; 12] = {
    const NONE: SoundState = SoundState {
        play_sound: 0,
        anim_frame1: 0,
        anim_frame2: 0,
        sound_magic: NO_SOUND,
    };
    const fn pounding(anim_frame1: i8, anim_frame2: i8) -> SoundState {
        SoundState {
            play_sound: 1,
            anim_frame1,
            anim_frame2,
            sound_magic: SOUND_OBJ_POUNDING1_HIGHPRIO,
        }
    }
    [
        NONE,
        pounding(1, 20),
        NONE,
        NONE,
        pounding(15, -1),
        NONE,
        NONE,
        NONE,
        NONE,
        pounding(33, -1),
        NONE,
        pounding(1, 15),
    ]
};

/// king_bobomb_move: the standard movement (or the return jump's arc), the
/// action, the action's sounds and distance culling.
fn king_bobomb_move(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    with_object(m, w, id, |k, w| {
        cur_obj_update_floor_and_walls(k, w);
        if k.raw.s32(O_KING_BOBOMB_UNK_F8) == 0 {
            cur_obj_move_standard(k, w, -78);
        } else {
            cur_obj_move_using_fvel_and_gravity(k, w.trig);
        }
    });
    match object(&w.objects, &m.obj, id).raw.s32(O_ACTION) {
        0 => king_bobomb_act_0(m, w, id),
        1 => king_bobomb_act_1(m, w, id),
        2 => king_bobomb_act_2(m, w, id),
        3 => king_bobomb_act_3(m, w, id),
        4 => king_bobomb_act_4(m, w, id),
        5 => king_bobomb_act_5(m, w, id),
        6 => king_bobomb_act_6(m, w, id),
        7 => king_bobomb_act_7(m, w, id),
        8 => king_bobomb_act_8(m, w, id),
        action => panic!("King Bob-omb action {action} reads past sKingBobombActions"),
    }
    exec_anim_sound_state(m, w, id, &SOUND_STATES);
    let k = o(m, w, id);
    if k.raw.f32(O_DISTANCE_TO_MARIO) < 5000.0 {
        cur_obj_enable_rendering(k);
    } else {
        cur_obj_disable_rendering(k);
    }
}

/// bhv_king_bobomb_loop.
pub fn bhv_king_bobomb_loop(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let k = o(m, w, id);
    k.raw.set_u32(
        O_INTERACTION_SUBTYPE,
        k.raw.u32(O_INTERACTION_SUBTYPE) | INT_SUBTYPE_GRABS_MARIO,
    );
    match k.raw.s32(O_HELD_STATE) {
        HELD_FREE => king_bobomb_move(m, w, id),
        HELD_HELD => {
            // cur_obj_unrender_set_action_and_anim(6, 1).
            let k = o(m, w, id);
            cur_obj_become_intangible(k);
            cur_obj_disable_rendering(k);
            init_anim(m, w, id, 6);
            o(m, w, id).raw.set_s32(O_ACTION, 1);
        }
        HELD_THROWN | HELD_DROPPED => {
            cur_obj_get_thrown_or_placed(m, w, id, 20.0, 50.0, 4);
            let k = o(m, w, id);
            cur_obj_become_intangible(k);
            k.raw.set_f32(O_POS_Y, k.raw.f32(O_POS_Y) + 20.0);
        }
        _ => {}
    }
    o(m, w, id).raw.set_u32(O_INTERACT_STATUS, 0);
}
