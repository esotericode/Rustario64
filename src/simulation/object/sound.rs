//! Object sounds, translated from pinned CC0 src/game/spawn_sound.c
//! (create_sound_spawner, cur_obj_play_sound_1, cur_obj_play_sound_2) and
//! src/game/behaviors/sound_spawner.inc.c (bhv_sound_spawner_init).
//!
//! Sound has no simulated state: play_sound is a recorded event, as for
//! Mario. Its position argument (the render pass's cameraToObject) only
//! places the sound, so it is not recorded. The sound spawner is an ordinary
//! object, so it takes a pool slot and runs its script like the original's.
use super::{ObjectId, object, object_mut, script::Behavior, spawn::spawn_object};
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// cur_obj_play_sound_1: only while the object's node is active.
pub fn cur_obj_play_sound_1(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, sound: u32) {
    cur_obj_play_sound_2(m, w, id, sound);
}

/// cur_obj_play_sound_2 (the rumble pak cases do nothing in the US version).
pub fn cur_obj_play_sound_2(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, sound: u32) {
    if object(&w.objects, &m.obj, id).gfx.node_flags & GRAPH_RENDER_ACTIVE != 0 {
        w.play_sound(sound);
    }
}

/// create_sound_spawner: an object with no model that plays `sound` after
/// three frames.
pub fn create_sound_spawner(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, sound: u32) {
    let behavior = w.behaviors.address(Behavior::SoundSpawner);
    let spawner = spawn_object(m, w, id, 0, behavior);
    object_mut(&mut w.objects, &mut m.obj, spawner)
        .raw
        .set_u32(O_SOUND_EFFECT_UNK_F4, sound);
}

/// bhv_sound_spawner_init.
pub fn bhv_sound_spawner_init(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    let sound = object(&w.objects, &m.obj, id)
        .raw
        .u32(O_SOUND_EFFECT_UNK_F4);
    w.play_sound(sound);
}
