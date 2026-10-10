//! Mario's interaction hooks, translated from pinned CC0
//! src/game/interaction.c (the items vendored as oracle excerpts).
//!
//! The handler dispatch runs in the original table order; interact_coin is
//! ported, and objects with other interaction types are not spawned (their
//! behaviors are not ported), so the other handlers are unreachable and
//! panic. Code that would dereference a held, ridden or used object panics,
//! as the original would crash without one. Special floors, the death
//! barrier, lava boosts and punch/kick wall hits are fully ported.
use super::{
    Event, MarioState, ObjectId, StepWorld, Unsupported,
    constants::*,
    core::{self, drop_and_set_mario_action},
    step::{mario_set_forward_vel, resolve_and_return_wall_collisions},
};

fn needs_objects(what: &str) -> ! {
    panic!("{what} needs objects, which are not simulated yet")
}

/// mario_stop_riding_object.
pub fn mario_stop_riding_object(m: &mut MarioState) {
    if m.ridden_obj.is_some() {
        needs_objects("mario_stop_riding_object");
    }
}

/// mario_grab_used_object.
pub fn mario_grab_used_object(m: &mut MarioState) {
    if m.held_obj.is_none() {
        needs_objects("mario_grab_used_object");
    }
}

/// mario_drop_held_object.
pub fn mario_drop_held_object(m: &mut MarioState) {
    if m.held_obj.is_some() {
        needs_objects("mario_drop_held_object");
    }
}

/// mario_throw_held_object.
pub fn mario_throw_held_object(m: &mut MarioState) {
    if m.held_obj.is_some() {
        needs_objects("mario_throw_held_object");
    }
}

/// mario_stop_riding_and_holding.
pub fn mario_stop_riding_and_holding(m: &mut MarioState, _w: &mut StepWorld<'_>) {
    mario_drop_held_object(m);
    mario_stop_riding_object(m);
    if m.action == ACT_RIDING_HOOT {
        needs_objects("releasing Hoot");
    }
}

/// does_mario_have_normal_cap_on_head.
pub fn does_mario_have_normal_cap_on_head(m: &MarioState) -> bool {
    (m.flags & (MARIO_CAPS | MARIO_CAP_ON_HEAD)) == (MARIO_NORMAL_CAP | MARIO_CAP_ON_HEAD)
}

/// mario_blow_off_cap: spawns a cap object, so it is unsupported with a cap.
pub fn mario_blow_off_cap(m: &mut MarioState, _cap_speed: f32) {
    if does_mario_have_normal_cap_on_head(m) {
        needs_objects("mario_blow_off_cap");
    }
}

/// mario_check_object_grab: only objects set INPUT_INTERACT_OBJ_GRABBABLE.
pub fn mario_check_object_grab(m: &mut MarioState) -> bool {
    if m.input & INPUT_INTERACT_OBJ_GRABBABLE != 0 {
        needs_objects("mario_check_object_grab");
    }
    false
}

/// check_kick_or_punch_wall.
pub fn check_kick_or_punch_wall(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.flags & (MARIO_PUNCHING | MARIO_KICKING | MARIO_TRIPPING) != 0 {
        let yaw = i32::from(m.face_angle[1]);
        let mut detector = [
            m.pos[0] + 50.0 * w.trig.sins(yaw),
            m.pos[1],
            m.pos[2] + 50.0 * w.trig.coss(yaw),
        ];
        if resolve_and_return_wall_collisions(m, w, &mut detector, 80.0, 5.0).is_some() {
            if m.action != ACT_MOVE_PUNCHING || m.forward_vel >= 0.0 {
                if m.action == ACT_PUNCHING {
                    m.action = ACT_MOVE_PUNCHING;
                }
                mario_set_forward_vel(m, w, -48.0);
                w.play_sound(SOUND_ACTION_HIT_2);
                m.particle_flags |= PARTICLE_TRIANGLE;
            } else if m.action & ACT_FLAG_AIR != 0 {
                mario_set_forward_vel(m, w, -16.0);
                w.play_sound(SOUND_ACTION_HIT_2);
                m.particle_flags |= PARTICLE_TRIANGLE;
            }
        }
    }
}

/// sInteractionHandlers' interaction types, in table order.
const INTERACTION_HANDLERS: [(u32, &str); 31] = [
    (INTERACT_COIN, "interact_coin"),
    (INTERACT_WATER_RING, "interact_water_ring"),
    (INTERACT_STAR_OR_KEY, "interact_star_or_key"),
    (INTERACT_BBH_ENTRANCE, "interact_bbh_entrance"),
    (INTERACT_WARP, "interact_warp"),
    (INTERACT_WARP_DOOR, "interact_warp_door"),
    (INTERACT_DOOR, "interact_door"),
    (INTERACT_CANNON_BASE, "interact_cannon_base"),
    (INTERACT_IGLOO_BARRIER, "interact_igloo_barrier"),
    (INTERACT_TORNADO, "interact_tornado"),
    (INTERACT_WHIRLPOOL, "interact_whirlpool"),
    (INTERACT_STRONG_WIND, "interact_strong_wind"),
    (INTERACT_FLAME, "interact_flame"),
    (INTERACT_SNUFIT_BULLET, "interact_snufit_bullet"),
    (INTERACT_CLAM_OR_BUBBA, "interact_clam_or_bubba"),
    (INTERACT_BULLY, "interact_bully"),
    (INTERACT_SHOCK, "interact_shock"),
    (INTERACT_BOUNCE_TOP2, "interact_bounce_top"),
    (INTERACT_MR_BLIZZARD, "interact_mr_blizzard"),
    (INTERACT_HIT_FROM_BELOW, "interact_hit_from_below"),
    (INTERACT_BOUNCE_TOP, "interact_bounce_top"),
    (INTERACT_DAMAGE, "interact_damage"),
    (INTERACT_POLE, "interact_pole"),
    (INTERACT_HOOT, "interact_hoot"),
    (INTERACT_BREAKABLE, "interact_breakable"),
    (INTERACT_KOOPA, "interact_bounce_top"),
    (INTERACT_KOOPA_SHELL, "interact_koopa_shell"),
    (INTERACT_UNKNOWN_08, "interact_unknown_08"),
    (INTERACT_CAP, "interact_cap"),
    (INTERACT_GRABBABLE, "interact_grabbable"),
    (INTERACT_TEXT, "interact_text"),
];

/// mario_get_collided_object: the first collided object whose interaction
/// type equals `interact_type`.
pub fn mario_get_collided_object(
    m: &MarioState,
    w: &StepWorld<'_>,
    interact_type: u32,
) -> Option<ObjectId> {
    (0..m.obj.num_collided_objs as usize)
        .filter_map(|i| m.obj.collided_objs[i])
        .find(|&id| w.objects.slot(id).raw.u32(O_INTERACT_TYPE) == interact_type)
}

/// interact_coin.
fn interact_coin(m: &mut MarioState, w: &mut StepWorld<'_>, o: ObjectId) -> bool {
    let value = w.objects.slot(o).raw.s32(O_DAMAGE_OR_COIN_VALUE);
    m.num_coins = (i32::from(m.num_coins) + value) as i16;
    m.heal_counter = (i32::from(m.heal_counter) + 4 * value) as u8;
    w.objects
        .slot_mut(o)
        .raw
        .set_u32(O_INTERACT_STATUS, INT_STATUS_INTERACTED);
    let course = w.course_num;
    if (COURSE_MIN..=COURSE_STAGES_MAX).contains(&course)
        && i32::from(m.num_coins) - value < 100
        && m.num_coins >= 100
    {
        // bhv_spawn_star_no_level_exit(STAR_INDEX_100_COINS).
        w.event(Event::Unsupported(Unsupported::HundredCoinStar));
    }
    false
}

/// mario_process_interactions.
pub fn mario_process_interactions(m: &mut MarioState, w: &mut StepWorld<'_>) {
    w.interaction.delay_invinc_timer = 0;
    w.interaction.invulnerable =
        i16::from(m.action & ACT_FLAG_INVULNERABLE != 0 || m.invinc_timer != 0);
    if m.action & ACT_FLAG_INTANGIBLE == 0 && m.collided_obj_interact_types != 0 {
        for (interact_type, name) in INTERACTION_HANDLERS {
            if m.collided_obj_interact_types & interact_type != 0 {
                let object = mario_get_collided_object(m, w, interact_type);
                m.collided_obj_interact_types &= !interact_type;
                let object = object.unwrap_or_else(|| {
                    panic!("no collided object has interaction type {interact_type:#X} (NULL dereference)")
                });
                if w.objects.slot(object).raw.u32(O_INTERACT_STATUS) & INT_STATUS_INTERACTED == 0 {
                    let stop = match interact_type {
                        INTERACT_COIN => interact_coin(m, w, object),
                        _ => needs_objects(name),
                    };
                    if stop {
                        break;
                    }
                }
            }
        }
    }
    if m.invinc_timer > 0 && w.interaction.delay_invinc_timer == 0 {
        m.invinc_timer -= 1;
    }
    // (Kick/punch wall speed) applies even if a collision changed the action.
    check_kick_or_punch_wall(m, w);
    m.flags &= !MARIO_PUNCHING & !MARIO_KICKING & !MARIO_TRIPPING;
    if m.obj.collided_obj_interact_types & (INTERACT_WARP_DOOR | INTERACT_DOOR) == 0 {
        w.interaction.displaying_door_text = 0;
    }
    if m.obj.collided_obj_interact_types & INTERACT_WARP == 0 {
        w.interaction.just_teleported = 0;
    }
}

/// level_trigger_warp (level_update.c) at the level-runtime boundary: the
/// request is recorded and no transition starts, so it returns the original's
/// "no transition" 0. The original also sets Mario's invincibility timer and
/// starts the warp; a tick that records a warp ends faithful comparison until
/// the level runtime is ported.
pub fn level_trigger_warp(w: &mut StepWorld<'_>, warp_op: i32) -> i16 {
    w.event(Event::Warp(warp_op));
    0
}

/// check_death_barrier.
pub fn check_death_barrier(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.pos[1] < m.floor_height + 2048.0
        && level_trigger_warp(w, WARP_OP_WARP_FLOOR) == 20
        && m.flags & MARIO_UNKNOWN_18 == 0
    {
        w.play_sound(SOUND_MARIO_WAAAOOOW);
    }
}

/// check_lava_boost.
pub fn check_lava_boost(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.action & ACT_FLAG_RIDING_SHELL == 0 && m.pos[1] < m.floor_height + 10.0 {
        if m.flags & MARIO_METAL_CAP == 0 {
            m.hurt_counter = m
                .hurt_counter
                .wrapping_add(if m.flags & MARIO_CAP_ON_HEAD != 0 {
                    12
                } else {
                    18
                });
        }
        core::update_mario_sound_and_camera(m, w);
        drop_and_set_mario_action(m, w, ACT_LAVA_BOOST, 0);
    }
}

/// mario_handle_special_floors. The PSS slide timer floors need the level
/// timer, which is not simulated; they only exist in the Princess's Secret
/// Slide.
pub fn mario_handle_special_floors(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.action & ACT_GROUP_MASK == ACT_GROUP_CUTSCENE {
        return;
    }
    if let Some(floor) = m.floor {
        let floor_type = w.surface(floor).surface_type;
        match floor_type {
            SURFACE_DEATH_PLANE | SURFACE_VERTICAL_WIND => check_death_barrier(m, w),
            SURFACE_WARP => {
                level_trigger_warp(w, WARP_OP_WARP_FLOOR);
            }
            SURFACE_TIMER_START | SURFACE_TIMER_END => {
                panic!("the PSS slide timer is not simulated yet")
            }
            _ => {}
        }
        if m.action & ACT_FLAG_AIR == 0
            && m.action & ACT_FLAG_SWIMMING == 0
            && floor_type == SURFACE_BURNING
        {
            check_lava_boost(m, w);
        }
    }
}
