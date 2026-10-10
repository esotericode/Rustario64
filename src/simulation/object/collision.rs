//! Object-to-object collision detection, translated from pinned CC0
//! src/game/object_collision.c: hitbox and hurtbox overlap, the per-list
//! clears, and detect_object_collisions' list pairs in original order.
use super::{Node, ObjectId, ObjectList, object, object_mut, pair_mut};
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// detect_object_hitbox_overlap: record a cylinder overlap on both objects
/// (at most four collisions each).
fn detect_object_hitbox_overlap(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    a: ObjectId,
    b: ObjectId,
) -> bool {
    let (oa, ob) = pair_mut(&mut w.objects, &mut m.obj, a, b);
    let sp3c = oa.raw.f32(O_POS_Y) - oa.hitbox_down_offset;
    let sp38 = ob.raw.f32(O_POS_Y) - ob.hitbox_down_offset;
    let dx = oa.raw.f32(O_POS_X) - ob.raw.f32(O_POS_X);
    let dz = oa.raw.f32(O_POS_Z) - ob.raw.f32(O_POS_Z);
    let collision_radius = oa.hitbox_radius + ob.hitbox_radius;
    let distance = (dx * dx + dz * dz).sqrt();
    if collision_radius > distance {
        let sp20 = oa.hitbox_height + sp3c;
        let sp1c = ob.hitbox_height + sp38;
        if sp3c > sp1c || sp20 < sp38 || oa.num_collided_objs >= 4 || ob.num_collided_objs >= 4 {
            return false;
        }
        oa.collided_objs[oa.num_collided_objs as usize] = Some(b);
        ob.collided_objs[ob.num_collided_objs as usize] = Some(a);
        oa.collided_obj_interact_types |= ob.raw.u32(O_INTERACT_TYPE);
        ob.collided_obj_interact_types |= oa.raw.u32(O_INTERACT_TYPE);
        oa.num_collided_objs += 1;
        ob.num_collided_objs += 1;
        return true;
    }
    // The original has no return value here; AVOID_UB's is 0.
    false
}

/// detect_object_hurtbox_overlap.
fn detect_object_hurtbox_overlap(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    a: ObjectId,
    b: ObjectId,
) -> bool {
    let a_is_mario = w.objects.mario == Some(a);
    let (oa, ob) = pair_mut(&mut w.objects, &mut m.obj, a, b);
    let sp3c = oa.raw.f32(O_POS_Y) - oa.hitbox_down_offset;
    let sp38 = ob.raw.f32(O_POS_Y) - ob.hitbox_down_offset;
    let sp34 = oa.raw.f32(O_POS_X) - ob.raw.f32(O_POS_X);
    let sp2c = oa.raw.f32(O_POS_Z) - ob.raw.f32(O_POS_Z);
    let sp28 = oa.hurtbox_radius + ob.hurtbox_radius;
    let sp24 = (sp34 * sp34 + sp2c * sp2c).sqrt();
    let subtype = |o: &mut super::Object, set: bool| {
        let value = o.raw.u32(O_INTERACTION_SUBTYPE);
        o.raw.set_u32(
            O_INTERACTION_SUBTYPE,
            if set {
                value | INT_SUBTYPE_DELAY_INVINCIBILITY
            } else {
                value & !INT_SUBTYPE_DELAY_INVINCIBILITY
            },
        );
    };
    if a_is_mario {
        subtype(ob, true);
    }
    if sp28 > sp24 {
        let sp20 = oa.hitbox_height + sp3c;
        let sp1c = ob.hurtbox_height + sp38;
        if sp3c > sp1c || sp20 < sp38 {
            return false;
        }
        if a_is_mario {
            subtype(ob, false);
        }
        return true;
    }
    false
}

/// The objects of a list from `first` to the end.
fn from(w: &StepWorld<'_>, first: Node, list: ObjectList) -> Vec<ObjectId> {
    let mut out = vec![];
    let mut node = first;
    while node != Node::Head(list) {
        let Node::Slot(id) = node else {
            panic!("a list contains another list's head")
        };
        out.push(id);
        node = w.objects.next(node);
    }
    out
}

/// clear_object_collision for one list.
pub fn clear_object_collision(m: &mut MarioState, w: &mut StepWorld<'_>, list: ObjectList) {
    for id in w.objects.list(list) {
        let o = object_mut(&mut w.objects, &mut m.obj, id);
        o.num_collided_objs = 0;
        o.collided_obj_interact_types = 0;
        let timer = o.raw.s32(O_INTANGIBLE_TIMER);
        if timer > 0 {
            o.raw.set_s32(O_INTANGIBLE_TIMER, timer - 1);
        }
    }
}

/// check_collision_in_list: `a` against `b` and the rest of `list`. The
/// remaining objects are read as the loop reaches them, as the original's
/// `b = b->header.next` does (nothing unlinks objects during detection).
fn check_collision_in_list(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    a: ObjectId,
    first: Node,
    list: ObjectList,
) {
    if object(&w.objects, &m.obj, a).raw.s32(O_INTANGIBLE_TIMER) != 0 {
        return;
    }
    for b in from(w, first, list) {
        if object(&w.objects, &m.obj, b).raw.s32(O_INTANGIBLE_TIMER) == 0
            && detect_object_hitbox_overlap(m, w, a, b)
            && object(&w.objects, &m.obj, b).hurtbox_radius != 0.0
        {
            detect_object_hurtbox_overlap(m, w, a, b);
        }
    }
}

const PLAYER_TARGETS: [ObjectList; 6] = [
    ObjectList::Polelike,
    ObjectList::Level,
    ObjectList::GeneralActor,
    ObjectList::Pushable,
    ObjectList::Surface,
    ObjectList::Destructive,
];

/// detect_object_collisions.
pub fn detect_object_collisions(m: &mut MarioState, w: &mut StepWorld<'_>) {
    for list in [
        ObjectList::Polelike,
        ObjectList::Player,
        ObjectList::Pushable,
        ObjectList::GeneralActor,
        ObjectList::Level,
        ObjectList::Surface,
        ObjectList::Destructive,
    ] {
        clear_object_collision(m, w, list);
    }
    // check_player_object_collision.
    for a in w.objects.list(ObjectList::Player) {
        let next = w.objects.next(Node::Slot(a));
        check_collision_in_list(m, w, a, next, ObjectList::Player);
        for list in PLAYER_TARGETS {
            let first = w.objects.next(Node::Head(list));
            check_collision_in_list(m, w, a, first, list);
        }
    }
    // check_destructive_object_collision.
    for a in w.objects.list(ObjectList::Destructive) {
        let o = object(&w.objects, &m.obj, a);
        if o.raw.f32(O_DISTANCE_TO_MARIO) < 2000.0 && o.active_flags & ACTIVE_FLAG_UNK9 == 0 {
            let next = w.objects.next(Node::Slot(a));
            check_collision_in_list(m, w, a, next, ObjectList::Destructive);
            for list in [
                ObjectList::GeneralActor,
                ObjectList::Pushable,
                ObjectList::Surface,
            ] {
                let first = w.objects.next(Node::Head(list));
                check_collision_in_list(m, w, a, first, list);
            }
        }
    }
    // check_pushable_object_collision.
    for a in w.objects.list(ObjectList::Pushable) {
        let next = w.objects.next(Node::Slot(a));
        check_collision_in_list(m, w, a, next, ObjectList::Pushable);
    }
}
