//! Engine-owned original animation data.
//!
//! Field meanings follow `struct Animation` in the pinned CC0 decomp
//! (include/types.h). Gameplay reads the frame fields: Mario's actions test
//! animation ends and frames, and some actions move Mario by the root
//! translation. Rendering reads the same data for the skeleton. Values are
//! loaded from the user's ROM and never stored in this repository.
use serde::{Deserialize, Serialize};

/// ANIM_FLAG_* from the pinned include/types.h.
pub const ANIM_FLAG_NOLOOP: i16 = 1 << 0;
pub const ANIM_FLAG_BACKWARD: i16 = 1 << 1;
pub const ANIM_FLAG_2: i16 = 1 << 2;
pub const ANIM_FLAG_HOR_TRANS: i16 = 1 << 3;
pub const ANIM_FLAG_VERT_TRANS: i16 = 1 << 4;
pub const ANIM_FLAG_5: i16 = 1 << 5;
pub const ANIM_FLAG_6: i16 = 1 << 6;
pub const ANIM_FLAG_7: i16 = 1 << 7;

/// One original animation. `index` holds (frame count, value offset) pairs:
/// three for the root translation, then three rotations per animated part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animation {
    pub flags: i16,
    pub y_trans_divisor: i16,
    pub start_frame: i16,
    pub loop_start: i16,
    pub loop_end: i16,
    /// `unusedBoneCount`: animated parts excluding the root translation.
    pub bone_count: i16,
    pub index: Vec<u16>,
    pub values: Vec<i16>,
}

impl Animation {
    /// retrieve_animation_index for attribute `attribute` (0-based). The
    /// importer guarantees every attribute stays inside `values` for frames in
    /// 0..; a negative frame reads before the attribute, which the original
    /// permits but no supported path reaches, so it is outside coverage.
    pub fn value(&self, attribute: usize, frame: i32) -> i16 {
        let frames = i32::from(self.index[attribute * 2]);
        let offset = i32::from(self.index[attribute * 2 + 1]);
        let at = if frame < frames {
            offset + frame
        } else {
            offset + frames - 1
        };
        let at = usize::try_from(at).expect("animation read before its value array");
        self.values[at]
    }
}

/// Mario's animation table, indexed by the original MARIO_ANIM_* IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarioAnimations {
    pub animations: Vec<Animation>,
}

impl MarioAnimations {
    pub fn get(&self, id: u16) -> Option<&Animation> {
        self.animations.get(usize::from(id))
    }
}
