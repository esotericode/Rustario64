//! The per-frame object processor, translated from pinned CC0
//! src/game/object_list_processor.c: update_objects and the list updates in
//! sObjectListUpdateOrder, the time-stop variant, unloading deactivated
//! objects with their respawn bits, and update_mario_platform's call site.
//!
//! Not modelled: the debug counters (gObjectCounter, gNumFindFloorMisses and
//! friends, read only by debug pages) and the profiler clock. Object surfaces
//! (clear_dynamic_surfaces, load_object_collision_model) and platform
//! displacement need surface objects, which no ported behavior has.
use super::{Node, ObjectId, ObjectList, UPDATE_ORDER, collision, object, object_mut, script};
use crate::simulation::mario::{MarioState, StepWorld, constants::*, tick};

/// update_objects_starting_at: each object from `first` to the end of the
/// list, reading the next node after the update (objects the update appends
/// to this list are updated this frame).
fn update_objects_starting_at(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    list: ObjectList,
    first: Node,
) {
    let mut node = first;
    while node != Node::Head(list) {
        let Node::Slot(id) = node else {
            panic!("a list contains another list's head")
        };
        w.objects.current = Some(id);
        object_mut(&mut w.objects, &mut m.obj, id).gfx.node_flags |= GRAPH_RENDER_HAS_ANIMATION;
        script::cur_obj_update(m, w, id);
        node = w.objects.next(node);
    }
}

/// update_objects_during_time_stop.
fn update_objects_during_time_stop(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    list: ObjectList,
    first: Node,
) {
    let mut node = first;
    while node != Node::Head(list) {
        let Node::Slot(id) = node else {
            panic!("a list contains another list's head")
        };
        w.objects.current = Some(id);
        let mut unfrozen = false;
        if w.time_stop_state & TIME_STOP_ALL_OBJECTS == 0 {
            let o = object(&w.objects, &m.obj, id);
            if w.objects.mario == Some(id) && w.time_stop_state & TIME_STOP_MARIO_AND_DOORS == 0 {
                unfrozen = true;
            }
            if o.raw.u32(O_INTERACT_TYPE) & (INTERACT_DOOR | INTERACT_WARP_DOOR) != 0
                && w.time_stop_state & TIME_STOP_MARIO_AND_DOORS == 0
            {
                unfrozen = true;
            }
            if o.active_flags & (ACTIVE_FLAG_UNIMPORTANT | ACTIVE_FLAG_INITIATED_TIME_STOP) != 0 {
                unfrozen = true;
            }
        }
        if unfrozen {
            object_mut(&mut w.objects, &mut m.obj, id).gfx.node_flags |= GRAPH_RENDER_HAS_ANIMATION;
            script::cur_obj_update(m, w, id);
        } else {
            object_mut(&mut w.objects, &mut m.obj, id).gfx.node_flags &=
                !GRAPH_RENDER_HAS_ANIMATION;
        }
        node = w.objects.next(node);
    }
}

/// update_objects_in_list.
fn update_objects_in_list(m: &mut MarioState, w: &mut StepWorld<'_>, list: ObjectList) {
    let first = w.objects.next(Node::Head(list));
    if w.time_stop_state & TIME_STOP_ACTIVE == 0 {
        update_objects_starting_at(m, w, list, first);
    } else {
        update_objects_during_time_stop(m, w, list, first);
    }
}

/// unload_deactivated_objects_in_list: every inactive object is unloaded;
/// unless it respawns persistently, its spawn record stops it respawning.
fn unload_deactivated_objects_in_list(m: &mut MarioState, w: &mut StepWorld<'_>, list: ObjectList) {
    let mut node = w.objects.next(Node::Head(list));
    while node != Node::Head(list) {
        let Node::Slot(id) = node else {
            panic!("a list contains another list's head")
        };
        w.objects.current = Some(id);
        node = w.objects.next(node);
        let o = object(&w.objects, &m.obj, id);
        if o.active_flags & ACTIVE_FLAG_ACTIVE != ACTIVE_FLAG_ACTIVE {
            assert!(
                w.objects.mario != Some(id),
                "Mario's object was deactivated"
            );
            if o.raw.u32(O_FLAGS) & OBJ_FLAG_PERSISTENT_RESPAWN == 0 {
                let (info_type, info) = (o.respawn_info_type, o.respawn_info);
                super::spawn::set_object_respawn_info_bits(
                    w,
                    info_type,
                    info,
                    RESPAWN_INFO_DONT_RESPAWN as u8,
                );
            }
            w.objects.unload_object(id);
        }
    }
}

/// update_objects: everything the area update does to objects in a frame.
pub fn update_objects(m: &mut MarioState, w: &mut StepWorld<'_>) {
    w.time_stop_state &= !TIME_STOP_MARIO_OPENED_DOOR;
    w.collision_flags.checking_for_camera = false;
    // clear_dynamic_surfaces: no object loads collision.
    // update_terrain_objects.
    update_objects_in_list(m, w, ObjectList::Spawner);
    update_objects_in_list(m, w, ObjectList::Surface);
    // apply_mario_platform_displacement.
    assert!(
        w.mario_platform.is_none(),
        "platform displacement needs surface objects, which are not ported"
    );
    collision::detect_object_collisions(m, w);
    // update_non_terrain_objects.
    for list in &UPDATE_ORDER[2..] {
        update_objects_in_list(m, w, *list);
    }
    // unload_deactivated_objects.
    for list in UPDATE_ORDER {
        unload_deactivated_objects_in_list(m, w, list);
    }
    w.time_stop_state &= !TIME_STOP_UNKNOWN_0;
    tick::update_mario_platform(m, w);
    if w.time_stop_state & TIME_STOP_ENABLED != 0 {
        w.time_stop_state |= TIME_STOP_ACTIVE;
    } else {
        w.time_stop_state &= !TIME_STOP_ACTIVE;
    }
}

/// Every allocated object, list by list in update order, for snapshots and
/// presentation (Mario's handle included).
pub fn all_objects(w: &StepWorld<'_>) -> Vec<(ObjectList, ObjectId)> {
    let mut out = vec![];
    for list in UPDATE_ORDER {
        out.extend(w.objects.list(list).into_iter().map(|id| (list, id)));
    }
    out
}
