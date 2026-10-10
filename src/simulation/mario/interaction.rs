//! Mario's interaction hooks, translated from pinned CC0
//! src/game/interaction.c (the items vendored as oracle excerpts).
//!
//! The handler dispatch runs in the original table order; interact_coin,
//! interact_damage and interact_grabbable (with the attack, knockback and
//! push-out helpers they use) are ported, and objects with other interaction
//! types are not spawned (their behaviors are not ported), so the other
//! handlers are unreachable and panic. Code that would dereference a held, ridden or used object panics,
//! as the original would crash without one. Special floors, the death
//! barrier, lava boosts and punch/kick wall hits are fully ported.
use super::{
    Event, MarioState, ObjectId, StepWorld, Unsupported,
    constants::*,
    core::{self, drop_and_set_mario_action},
    step::{mario_set_forward_vel, resolve_and_return_wall_collisions},
};
use crate::simulation::object::{object_mut, script::Behavior};

fn needs_objects(what: &str) -> ! {
    panic!("{what} needs objects, which are not simulated yet")
}

/// obj_set_held_state, run from Mario's code (`o`, the object that becomes
/// the parent, is gCurrentObject: his object). Holdable objects take the
/// held state their behavior reads; others run the carry script.
fn obj_set_held_state(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    held_behavior: Behavior,
) {
    let parent = w.objects.current;
    let script = w.behaviors.address(held_behavior);
    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.parent = parent;
    if o.raw.u32(O_FLAGS) & OBJ_FLAG_HOLDABLE != 0 {
        if held_behavior == Behavior::CarrySomething3 {
            o.raw.set_s32(O_HELD_STATE, HELD_HELD);
        }
        if held_behavior == Behavior::CarrySomething5 {
            o.raw.set_s32(O_HELD_STATE, HELD_THROWN);
        }
        if held_behavior == Behavior::CarrySomething4 {
            o.raw.set_s32(O_HELD_STATE, HELD_DROPPED);
        }
    } else {
        o.cur_bhv_command = script;
        o.bhv_stack_index = 0;
    }
}

/// mario_stop_riding_object.
pub fn mario_stop_riding_object(m: &mut MarioState) {
    if m.ridden_obj.is_some() {
        needs_objects("mario_stop_riding_object");
    }
}

/// mario_grab_used_object.
pub fn mario_grab_used_object(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if m.held_obj.is_none() {
        let used = m
            .used_obj
            .expect("mario_grab_used_object with no usedObj (the original dereferences NULL)");
        m.held_obj = Some(used);
        obj_set_held_state(m, w, used, Behavior::CarrySomething3);
    }
}

/// mario_drop_held_object: at the HOLP's x and z, Mario's height. Held
/// objects are never bhvKoopaShellUnderwater (not ported), so no shell
/// music stops.
pub fn mario_drop_held_object(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if let Some(held) = m.held_obj {
        obj_set_held_state(m, w, held, Behavior::CarrySomething4);
        let holp = m.body.held_obj_last_position;
        let (y, yaw) = (m.pos[1], m.face_angle[1]);
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, held).raw;
        raw.set_f32(O_POS_X, holp[0]);
        raw.set_f32(O_POS_Y, y);
        raw.set_f32(O_POS_Z, holp[2]);
        raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
        m.held_obj = None;
    }
}

/// mario_throw_held_object: from the HOLP, 32 units ahead of Mario.
pub fn mario_throw_held_object(m: &mut MarioState, w: &mut StepWorld<'_>) {
    if let Some(held) = m.held_obj {
        obj_set_held_state(m, w, held, Behavior::CarrySomething5);
        let holp = m.body.held_obj_last_position;
        let yaw = m.face_angle[1];
        let (s, c) = (w.trig.sins(i32::from(yaw)), w.trig.coss(i32::from(yaw)));
        let raw = &mut object_mut(&mut w.objects, &mut m.obj, held).raw;
        raw.set_f32(O_POS_X, holp[0] + 32.0 * s);
        raw.set_f32(O_POS_Y, holp[1]);
        raw.set_f32(O_POS_Z, holp[2] + 32.0 * c);
        raw.set_s32(O_MOVE_ANGLE_YAW, i32::from(yaw));
        m.held_obj = None;
    }
}

/// mario_stop_riding_and_holding.
pub fn mario_stop_riding_and_holding(m: &mut MarioState, w: &mut StepWorld<'_>) {
    mario_drop_held_object(m, w);
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

/// mario_check_object_grab: a grabbable object in front of Mario becomes
/// his used object, and on the ground he starts picking it up. No ported
/// object is Bowser, whose branch is therefore unreachable.
pub fn mario_check_object_grab(m: &mut MarioState, w: &mut StepWorld<'_>) -> bool {
    let mut result = false;
    if m.input & INPUT_INTERACT_OBJ_GRABBABLE != 0 {
        let object = m
            .interact_obj
            .expect("INPUT_INTERACT_OBJ_GRABBABLE without interactObj");
        let facing_d_yaw = mario_obj_angle_to_object(m, w, object).wrapping_sub(m.face_angle[1]);
        if (-0x2AAA..=0x2AAA).contains(&facing_d_yaw) {
            m.used_obj = Some(object);
            if m.action & ACT_FLAG_AIR == 0 {
                let action = if m.action & ACT_FLAG_DIVING != 0 {
                    ACT_DIVE_PICKING_UP
                } else {
                    ACT_PICKING_UP
                };
                core::set_mario_action(m, w, action, 0);
            }
            result = true;
        }
    }
    result
}

// interaction.c's file-local interaction kinds.
const INT_GROUND_POUND_OR_TWIRL: u32 = 1 << 0;
const INT_PUNCH: u32 = 1 << 1;
const INT_KICK: u32 = 1 << 2;
const INT_TRIP: u32 = 1 << 3;
const INT_SLIDE_KICK: u32 = 1 << 4;
const INT_FAST_ATTACK_OR_SHELL: u32 = 1 << 5;
const INT_HIT_FROM_ABOVE: u32 = 1 << 6;
const INT_HIT_FROM_BELOW: u32 = 1 << 7;

/// sForwardKnockbackActions, by terrain (ground, air, water) and strength.
const FORWARD_KNOCKBACK_ACTIONS: [[u32; 3]; 3] = [
    [
        ACT_SOFT_FORWARD_GROUND_KB,
        ACT_FORWARD_GROUND_KB,
        ACT_HARD_FORWARD_GROUND_KB,
    ],
    [
        ACT_FORWARD_AIR_KB,
        ACT_FORWARD_AIR_KB,
        ACT_HARD_FORWARD_AIR_KB,
    ],
    [
        ACT_FORWARD_WATER_KB,
        ACT_FORWARD_WATER_KB,
        ACT_FORWARD_WATER_KB,
    ],
];

/// sBackwardKnockbackActions.
const BACKWARD_KNOCKBACK_ACTIONS: [[u32; 3]; 3] = [
    [
        ACT_SOFT_BACKWARD_GROUND_KB,
        ACT_BACKWARD_GROUND_KB,
        ACT_HARD_BACKWARD_GROUND_KB,
    ],
    [
        ACT_BACKWARD_AIR_KB,
        ACT_BACKWARD_AIR_KB,
        ACT_HARD_BACKWARD_AIR_KB,
    ],
    [
        ACT_BACKWARD_WATER_KB,
        ACT_BACKWARD_WATER_KB,
        ACT_BACKWARD_WATER_KB,
    ],
];

/// mario_obj_angle_to_object.
pub fn mario_obj_angle_to_object(m: &MarioState, w: &StepWorld<'_>, o: ObjectId) -> i16 {
    let o = w.objects.slot(o);
    let dx = o.raw.f32(O_POS_X) - m.pos[0];
    let dz = o.raw.f32(O_POS_Z) - m.pos[2];
    w.trig.atan2s(dz, dx)
}

/// determine_interaction: how Mario's current action meets the object. The
/// riding-shell and fast-attack branches stay separate, as written.
#[allow(clippy::if_same_then_else)]
fn determine_interaction(m: &MarioState, w: &StepWorld<'_>, o: ObjectId) -> u32 {
    let mut interaction = 0;
    let action = m.action;
    if action & ACT_FLAG_ATTACKING != 0 {
        if action == ACT_PUNCHING || action == ACT_MOVE_PUNCHING || action == ACT_JUMP_KICK {
            let d_yaw_to_object = mario_obj_angle_to_object(m, w, o).wrapping_sub(m.face_angle[1]);
            if m.flags & MARIO_PUNCHING != 0 && (-0x2AAA..=0x2AAA).contains(&d_yaw_to_object) {
                interaction = INT_PUNCH;
            }
            if m.flags & MARIO_KICKING != 0 && (-0x2AAA..=0x2AAA).contains(&d_yaw_to_object) {
                interaction = INT_KICK;
            }
            if m.flags & MARIO_TRIPPING != 0 && (-0x4000..=0x4000).contains(&d_yaw_to_object) {
                interaction = INT_TRIP;
            }
        } else if action == ACT_GROUND_POUND || action == ACT_TWIRLING {
            if m.vel[1] < 0.0 {
                interaction = INT_GROUND_POUND_OR_TWIRL;
            }
        } else if action == ACT_GROUND_POUND_LAND || action == ACT_TWIRL_LAND {
            if m.vel[1] < 0.0 && m.action_state == 0 {
                interaction = INT_GROUND_POUND_OR_TWIRL;
            }
        } else if action == ACT_SLIDE_KICK || action == ACT_SLIDE_KICK_SLIDE {
            interaction = INT_SLIDE_KICK;
        } else if action & ACT_FLAG_RIDING_SHELL != 0 {
            interaction = INT_FAST_ATTACK_OR_SHELL;
        } else if m.forward_vel <= -26.0 || 26.0 <= m.forward_vel {
            interaction = INT_FAST_ATTACK_OR_SHELL;
        }
    }
    if interaction == 0 && action & ACT_FLAG_AIR != 0 {
        let object_y = w.objects.slot(o).raw.f32(O_POS_Y);
        if m.vel[1] < 0.0 {
            if m.pos[1] > object_y {
                interaction = INT_HIT_FROM_ABOVE;
            }
        } else if m.pos[1] < object_y {
            interaction = INT_HIT_FROM_BELOW;
        }
    }
    interaction
}

/// attack_object: the object learns how it was attacked.
fn attack_object(w: &mut StepWorld<'_>, o: ObjectId, interaction: u32) -> u32 {
    let attack_type = match interaction {
        INT_GROUND_POUND_OR_TWIRL => ATTACK_GROUND_POUND_OR_TWIRL,
        INT_PUNCH => ATTACK_PUNCH,
        INT_KICK | INT_TRIP => ATTACK_KICK_OR_TRIP,
        INT_SLIDE_KICK | INT_FAST_ATTACK_OR_SHELL => ATTACK_FAST_ATTACK,
        INT_HIT_FROM_ABOVE => ATTACK_FROM_ABOVE,
        INT_HIT_FROM_BELOW => ATTACK_FROM_BELOW,
        _ => 0,
    };
    w.objects.slot_mut(o).raw.set_u32(
        O_INTERACT_STATUS,
        attack_type + (INT_STATUS_INTERACTED | INT_STATUS_WAS_ATTACKED),
    );
    attack_type
}

/// bounce_back_from_attack.
fn bounce_back_from_attack(m: &mut MarioState, w: &mut StepWorld<'_>, interaction: u32) {
    if interaction & (INT_PUNCH | INT_KICK | INT_TRIP) != 0 {
        if m.action == ACT_PUNCHING {
            m.action = ACT_MOVE_PUNCHING;
        }
        if m.action & ACT_FLAG_AIR != 0 {
            mario_set_forward_vel(m, w, -16.0);
        } else {
            mario_set_forward_vel(m, w, -48.0);
        }
        w.event(Event::CameraShake(SHAKE_ATTACK));
        m.particle_flags |= PARTICLE_TRIANGLE;
    }
    if interaction & (INT_PUNCH | INT_KICK | INT_TRIP | INT_FAST_ATTACK_OR_SHELL) != 0 {
        w.play_sound(SOUND_ACTION_HIT_2);
    }
}

/// push_mario_out_of_object: out to the combined hitbox radius plus
/// `padding`, through walls, and only onto a floor (Mario's floor reference
/// is not updated, as in the original).
fn push_mario_out_of_object(m: &mut MarioState, w: &mut StepWorld<'_>, o: ObjectId, padding: f32) {
    let object = w.objects.slot(o);
    let min_distance = object.hitbox_radius + m.obj.hitbox_radius + padding;
    let offset_x = m.pos[0] - object.raw.f32(O_POS_X);
    let offset_z = m.pos[2] - object.raw.f32(O_POS_Z);
    let distance = (offset_x * offset_x + offset_z * offset_z).sqrt();
    if distance < min_distance {
        let push_angle = if distance == 0.0 {
            m.face_angle[1]
        } else {
            w.trig.atan2s(offset_z, offset_x)
        };
        let new_x = object.raw.f32(O_POS_X) + min_distance * w.trig.sins(i32::from(push_angle));
        let new_z = object.raw.f32(O_POS_Z) + min_distance * w.trig.coss(i32::from(push_angle));
        let mut position = [new_x, m.pos[1], new_z];
        w.collision.f32_find_wall_collision(
            &mut position,
            60.0,
            50.0,
            w.collision_flags,
            m.flags & MARIO_VANISH_CAP != 0,
        );
        let [new_x, y, new_z] = position;
        m.pos[1] = y;
        let (_, floor) = w
            .collision
            .find_floor(new_x, m.pos[1], new_z, &mut w.collision_flags);
        if floor.is_some() {
            m.pos[0] = new_x;
            m.pos[2] = new_z;
        }
    }
}

/// able_to_grab_object.
fn able_to_grab_object(m: &MarioState, w: &StepWorld<'_>, o: ObjectId) -> bool {
    let action = m.action;
    if action == ACT_DIVE_SLIDE || action == ACT_DIVE {
        if w.objects.slot(o).raw.u32(O_INTERACTION_SUBTYPE) & INT_SUBTYPE_GRABS_MARIO == 0 {
            return true;
        }
    } else if (action == ACT_PUNCHING || action == ACT_MOVE_PUNCHING) && m.action_arg < 2 {
        return true;
    }
    false
}

/// object_facing_mario: both ends of the signed angle range are inclusive.
fn object_facing_mario(m: &MarioState, w: &StepWorld<'_>, o: ObjectId, range: i16) -> bool {
    let o = w.objects.slot(o);
    let angle = w
        .trig
        .atan2s(m.pos[2] - o.raw.f32(O_POS_Z), m.pos[0] - o.raw.f32(O_POS_X));
    let delta = angle.wrapping_sub(o.raw.s32(O_MOVE_ANGLE_YAW) as i16);
    -i32::from(range) <= i32::from(delta) && delta <= range
}

/// check_object_grab_mario. Preserve the source's OR with !sInvulnerable:
/// air/attacking actions can still be grabbed without invulnerability.
fn check_object_grab_mario(m: &mut MarioState, w: &mut StepWorld<'_>, o: ObjectId) -> bool {
    if (m.action & (ACT_FLAG_AIR | ACT_FLAG_INVULNERABLE | ACT_FLAG_ATTACKING) == 0
        || w.interaction.invulnerable == 0)
        && w.objects.slot(o).raw.u32(O_INTERACTION_SUBTYPE) & INT_SUBTYPE_GRABS_MARIO != 0
        && object_facing_mario(m, w, o, 0x2AAA)
    {
        mario_stop_riding_and_holding(m, w);
        let o_fields = &mut w.objects.slot_mut(o).raw;
        o_fields.set_u32(
            O_INTERACT_STATUS,
            INT_STATUS_INTERACTED | INT_STATUS_GRABBED_MARIO,
        );
        m.face_angle[1] = o_fields.s32(O_MOVE_ANGLE_YAW) as i16;
        m.interact_obj = Some(o);
        m.used_obj = Some(o);
        core::update_mario_sound_and_camera(m, w);
        w.play_sound(SOUND_MARIO_OOOF);
        return core::set_mario_action(m, w, ACT_GRABBED, 0) != 0;
    }
    push_mario_out_of_object(m, w, o, -5.0);
    false
}

/// interact_grabbable: a kick or trip launches a kickable object; a grab
/// attempt marks it for mario_check_object_grab; otherwise Mario is pushed
/// out (every ported object is not Bowser).
fn interact_grabbable(m: &mut MarioState, w: &mut StepWorld<'_>, o: ObjectId) -> bool {
    let subtype = w.objects.slot(o).raw.u32(O_INTERACTION_SUBTYPE);
    if subtype & INT_SUBTYPE_KICKABLE != 0 {
        let interaction = determine_interaction(m, w, o);
        if interaction & (INT_KICK | INT_TRIP) != 0 {
            attack_object(w, o, interaction);
            bounce_back_from_attack(m, w, interaction);
            return false;
        }
    }
    if subtype & INT_SUBTYPE_GRABS_MARIO != 0 && check_object_grab_mario(m, w, o) {
        return true;
    }
    if able_to_grab_object(m, w, o) && subtype & INT_SUBTYPE_NOT_GRABBABLE == 0 {
        m.interact_obj = Some(o);
        m.input |= INPUT_INTERACT_OBJ_GRABBABLE;
        return true;
    }
    push_mario_out_of_object(m, w, o, -5.0);
    false
}

/// take_damage_from_interact_object: the hurt counter and hit shake for
/// the interacting object's damage.
fn take_damage_from_interact_object(m: &mut MarioState, w: &mut StepWorld<'_>) -> i32 {
    let object = m.interact_obj.expect("interactObj");
    let mut damage = w.objects.slot(object).raw.s32(O_DAMAGE_OR_COIN_VALUE);
    let shake = if damage >= 4 {
        SHAKE_LARGE_DAMAGE
    } else if damage >= 2 {
        SHAKE_MED_DAMAGE
    } else {
        SHAKE_SMALL_DAMAGE
    };
    if m.flags & MARIO_CAP_ON_HEAD == 0 {
        damage += (damage + 1) / 2;
    }
    if m.flags & MARIO_METAL_CAP != 0 {
        damage = 0;
    }
    m.hurt_counter = (i32::from(m.hurt_counter) + 4 * damage) as u8;
    w.event(Event::CameraShake(shake));
    damage
}

/// determine_knockback_action: faces the object, gets pushed away from it.
fn determine_knockback_action(m: &mut MarioState, w: &mut StepWorld<'_>) -> u32 {
    let object = m.interact_obj.expect("interactObj");
    let angle_to_object = mario_obj_angle_to_object(m, w, object);
    let facing_d_yaw = angle_to_object.wrapping_sub(m.face_angle[1]);
    let remaining_health = (i32::from(m.health) - 0x40 * i32::from(m.hurt_counter)) as i16;
    let terrain_index = if m.action & (ACT_FLAG_SWIMMING | ACT_FLAG_METAL_WATER) != 0 {
        2
    } else if m.action & (ACT_FLAG_AIR | ACT_FLAG_ON_POLE | ACT_FLAG_HANGING) != 0 {
        1
    } else {
        0
    };
    let damage = w.objects.slot(object).raw.s32(O_DAMAGE_OR_COIN_VALUE);
    let strength_index = if remaining_health < 0x100 || damage >= 4 {
        2
    } else if damage >= 2 {
        1
    } else {
        0
    };
    m.face_angle[1] = angle_to_object;
    if terrain_index == 2 {
        if m.forward_vel < 28.0 {
            mario_set_forward_vel(m, w, 28.0);
        }
        if m.pos[1] >= w.objects.slot(object).raw.f32(O_POS_Y) {
            if m.vel[1] < 20.0 {
                m.vel[1] = 20.0;
            }
        } else if m.vel[1] > 0.0 {
            m.vel[1] = 0.0;
        }
    } else if m.forward_vel < 16.0 {
        mario_set_forward_vel(m, w, 16.0);
    }
    if (-0x4000..=0x4000).contains(&facing_d_yaw) {
        m.forward_vel *= -1.0;
        BACKWARD_KNOCKBACK_ACTIONS[terrain_index][strength_index]
    } else {
        m.face_angle[1] = m.face_angle[1].wrapping_add(i16::MIN);
        FORWARD_KNOCKBACK_ACTIONS[terrain_index][strength_index]
    }
}

/// take_damage_and_knock_back.
fn take_damage_and_knock_back(m: &mut MarioState, w: &mut StepWorld<'_>, o: ObjectId) -> bool {
    let subtype = w.objects.slot(o).raw.u32(O_INTERACTION_SUBTYPE);
    if w.interaction.invulnerable == 0
        && m.flags & MARIO_VANISH_CAP == 0
        && subtype & INT_SUBTYPE_DELAY_INVINCIBILITY == 0
    {
        w.objects.slot_mut(o).raw.set_u32(
            O_INTERACT_STATUS,
            INT_STATUS_INTERACTED | INT_STATUS_ATTACKED_MARIO,
        );
        m.interact_obj = Some(o);
        let damage = take_damage_from_interact_object(m, w);
        if subtype & INT_SUBTYPE_BIG_KNOCKBACK != 0 {
            m.forward_vel = 40.0;
        }
        if w.objects.slot(o).raw.s32(O_DAMAGE_OR_COIN_VALUE) > 0 {
            w.play_sound(SOUND_MARIO_ATTACKED);
        }
        core::update_mario_sound_and_camera(m, w);
        let action = determine_knockback_action(m, w);
        return drop_and_set_mario_action(m, w, action, damage as u32) != 0;
    }
    false
}

/// interact_damage.
fn interact_damage(m: &mut MarioState, w: &mut StepWorld<'_>, o: ObjectId) -> bool {
    if take_damage_and_knock_back(m, w, o) {
        return true;
    }
    if w.objects.slot(o).raw.u32(O_INTERACTION_SUBTYPE) & INT_SUBTYPE_DELAY_INVINCIBILITY == 0 {
        w.interaction.delay_invinc_timer = 1;
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
                        INTERACT_DAMAGE => interact_damage(m, w, object),
                        INTERACT_GRABBABLE => interact_grabbable(m, w, object),
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
