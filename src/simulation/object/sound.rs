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

/// struct SoundState: whether the state plays its sound, the two animation
/// frames that play it (-1 for none) and the sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundState {
    pub play_sound: i16,
    pub anim_frame1: i8,
    pub anim_frame2: i8,
    pub sound_magic: u32,
}

/// exec_anim_sound_state: the object's sound state (oSoundStateID) plays
/// its sound on its animation frames.
pub fn exec_anim_sound_state(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    id: ObjectId,
    states: &[SoundState],
) {
    let o = object(&w.objects, &m.obj, id);
    let index = o.raw.s32(O_SOUND_STATE_I_D);
    let state = *usize::try_from(index)
        .ok()
        .and_then(|i| states.get(i))
        .unwrap_or_else(|| panic!("sound state {index} is outside the object's table"));
    match state.play_sound {
        0 => {}
        1 => {
            for frame in [state.anim_frame1, state.anim_frame2] {
                if frame >= 0
                    && i32::from(object(&w.objects, &m.obj, id).gfx.anim.anim_frame)
                        == i32::from(frame)
                {
                    cur_obj_play_sound_2(m, w, id, state.sound_magic);
                }
            }
        }
        // The original's switch has no case for other values.
        _ => {}
    }
}
