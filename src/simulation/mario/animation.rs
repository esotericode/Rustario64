//! Mario's animation state, translated from the pinned CC0 decomp: the
//! animation helpers in src/game/mario.c and geo_update_animation_frame in
//! src/engine/graph_node.c.
//!
//! The original advances the frame while rendering (geo_set_animation_globals
//! in src/game/rendering_graph_node.c), once per area update, after objects
//! update. Actions read the frame, so [`update_animation_frame`] runs in the
//! tick's render stage (`tick::render_mario_object`); presentation never calls
//! it. Integer arithmetic follows C promotion: s16 fields widen to int, sums
//! wrap as with the oracle's -fwrapv, stores truncate to the field width.
use super::{AnimInfo, AnimRef, MarioObject, MarioState, StepWorld, constants::*, f32_to_s16};
use crate::content::animation::{
    ANIM_FLAG_2, ANIM_FLAG_6, ANIM_FLAG_BACKWARD, ANIM_FLAG_HOR_TRANS, ANIM_FLAG_NOLOOP,
    ANIM_FLAG_VERT_TRANS, Animation,
};

/// The animation `curAnim` points to.
pub fn cur_anim<'w>(anim: &AnimInfo, w: &'w StepWorld<'_>) -> &'w Animation {
    match anim.cur_anim {
        Some(AnimRef::MarioDmaBuffer) => {
            let entry = w
                .anim_dma_loaded
                .expect("curAnim points at a DMA buffer that was never loaded");
            w.anims
                .get(entry)
                .expect("DMA buffer entry outside the animation table")
        }
        Some(AnimRef::Object(address)) => w
            .object_anims
            .get(address)
            .unwrap_or_else(|| panic!("curAnim 0x{address:08X} was not imported")),
        None => panic!("curAnim is NULL (the original dereferences it)"),
    }
}

/// load_patchable_table for Mario's animation list: load `index` into the DMA
/// buffer unless it is already there. True when it loaded.
fn load_patchable_table(w: &mut StepWorld<'_>, index: i32) -> bool {
    let count = w.anims.animations.len() as u32;
    if (index as u32) < count && w.anim_dma_loaded != Some(index as u16) {
        w.anim_dma_loaded = Some(index as u16);
        return true;
    }
    false
}

/// is_anim_at_end.
pub fn is_anim_at_end(m: &MarioState, w: &StepWorld<'_>) -> bool {
    let anim = &m.obj.gfx.anim;
    i32::from(anim.anim_frame) + 1 == i32::from(cur_anim(anim, w).loop_end)
}

/// is_anim_past_end.
pub fn is_anim_past_end(m: &MarioState, w: &StepWorld<'_>) -> bool {
    let anim = &m.obj.gfx.anim;
    i32::from(anim.anim_frame) >= i32::from(cur_anim(anim, w).loop_end) - 2
}

/// set_mario_animation: returns the (possibly reset) animation frame.
pub fn set_mario_animation(m: &mut MarioState, w: &mut StepWorld<'_>, target_anim_id: i32) -> i16 {
    // A fresh load only patches the buffer's array pointers in the original.
    let _ = load_patchable_table(w, target_anim_id);
    let unk_b0 = m.unk_b0;
    let anim = &mut m.obj.gfx.anim;
    if i32::from(anim.anim_id) != target_anim_id {
        anim.anim_id = target_anim_id as i16;
        anim.cur_anim = Some(AnimRef::MarioDmaBuffer);
        anim.anim_accel = 0;
        anim.anim_y_trans = unk_b0;
        let target = cur_anim(anim, w);
        let start = i32::from(target.start_frame);
        anim.anim_frame = if target.flags & ANIM_FLAG_2 != 0 {
            target.start_frame
        } else if target.flags & ANIM_FLAG_BACKWARD != 0 {
            (start + 1) as i16
        } else {
            (start - 1) as i16
        };
    }
    m.obj.gfx.anim.anim_frame
}

/// set_mario_anim_with_accel: `accel` is a 16.16 frame step.
pub fn set_mario_anim_with_accel(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    target_anim_id: i32,
    accel: i32,
) -> i16 {
    let _ = load_patchable_table(w, target_anim_id);
    let unk_b0 = m.unk_b0;
    let anim = &mut m.obj.gfx.anim;
    if i32::from(anim.anim_id) != target_anim_id {
        anim.anim_id = target_anim_id as i16;
        anim.cur_anim = Some(AnimRef::MarioDmaBuffer);
        anim.anim_y_trans = unk_b0;
        let target = cur_anim(anim, w);
        let start = i32::from(target.start_frame) << 0x10;
        anim.anim_frame_accel_assist = if target.flags & ANIM_FLAG_2 != 0 {
            start
        } else if target.flags & ANIM_FLAG_BACKWARD != 0 {
            start.wrapping_add(accel)
        } else {
            start.wrapping_sub(accel)
        };
        anim.anim_frame = (anim.anim_frame_accel_assist >> 0x10) as i16;
    }
    anim.anim_accel = accel;
    anim.anim_frame
}

/// set_anim_to_frame: the next advance lands on `anim_frame`.
pub fn set_anim_to_frame(m: &mut MarioState, w: &StepWorld<'_>, anim_frame: i16) {
    let anim = &mut m.obj.gfx.anim;
    let backward = cur_anim(anim, w).flags & ANIM_FLAG_BACKWARD != 0;
    let frame = i32::from(anim_frame);
    if anim.anim_accel != 0 {
        anim.anim_frame_accel_assist = if backward {
            (frame << 0x10).wrapping_add(anim.anim_accel)
        } else {
            (frame << 0x10).wrapping_sub(anim.anim_accel)
        };
    } else {
        anim.anim_frame = if backward { frame + 1 } else { frame - 1 } as i16;
    }
}

/// is_anim_past_frame.
pub fn is_anim_past_frame(m: &MarioState, w: &StepWorld<'_>, anim_frame: i16) -> bool {
    let anim = &m.obj.gfx.anim;
    let accelerated = i32::from(anim_frame) << 0x10;
    let backward = cur_anim(anim, w).flags & ANIM_FLAG_BACKWARD != 0;
    if anim.anim_accel != 0 {
        let assist = anim.anim_frame_accel_assist;
        if backward {
            assist > accelerated && accelerated >= assist.wrapping_sub(anim.anim_accel)
        } else {
            assist < accelerated && accelerated <= assist.wrapping_add(anim.anim_accel)
        }
    } else if backward {
        i32::from(anim.anim_frame) == i32::from(anim_frame) + 1
    } else {
        i32::from(anim.anim_frame) + 1 == i32::from(anim_frame)
    }
}

/// geo_update_animation_frame: the frame the next advance produces and the
/// new 16.16 accumulator. Returns the current values when the frame already
/// advanced this area update or the animation holds (ANIM_FLAG_2).
pub fn geo_update_animation_frame(
    anim: &AnimInfo,
    data: &Animation,
    area_update_counter: u16,
) -> (i16, i32) {
    if anim.anim_timer == area_update_counter || data.flags & ANIM_FLAG_2 != 0 {
        return (anim.anim_frame, anim.anim_frame_accel_assist);
    }
    let set_high = |result: i32, high: i32| (result & 0xFFFF) | (high << 16);
    let high = |result: i32| (result >> 16) as i16;
    let mut result;
    if data.flags & ANIM_FLAG_BACKWARD != 0 {
        result = if anim.anim_accel != 0 {
            anim.anim_frame_accel_assist.wrapping_sub(anim.anim_accel)
        } else {
            (i32::from(anim.anim_frame) - 1) << 16
        };
        if high(result) < data.loop_start {
            result = if data.flags & ANIM_FLAG_NOLOOP != 0 {
                set_high(result, i32::from(data.loop_start))
            } else {
                set_high(result, i32::from(data.loop_end) - 1)
            };
        }
    } else {
        result = if anim.anim_accel != 0 {
            anim.anim_frame_accel_assist.wrapping_add(anim.anim_accel)
        } else {
            (i32::from(anim.anim_frame) + 1) << 16
        };
        if high(result) >= data.loop_end {
            result = if data.flags & ANIM_FLAG_NOLOOP != 0 {
                set_high(result, i32::from(data.loop_end) - 1)
            } else {
                set_high(result, i32::from(data.loop_start))
            };
        }
    }
    (high(result), result)
}

/// The authoritative part of geo_set_animation_globals, which the render pass
/// runs for a drawn object with an animation: advance the frame when the node
/// is animated, and stamp the timer with gAreaUpdateCounter.
pub fn update_animation_frame(obj: &mut MarioObject, w: &StepWorld<'_>) {
    if obj.gfx.anim.cur_anim.is_none() {
        return;
    }
    if obj.gfx.node_flags & GRAPH_RENDER_HAS_ANIMATION != 0 {
        let data = cur_anim(&obj.gfx.anim, w);
        let (frame, assist) =
            geo_update_animation_frame(&obj.gfx.anim, data, w.area_update_counter);
        obj.gfx.anim.anim_frame = frame;
        obj.gfx.anim.anim_frame_accel_assist = assist;
    }
    obj.gfx.anim.anim_timer = w.area_update_counter;
}

/// find_mario_anim_flags_and_translation: the root translation of the frame
/// the next advance will show, rotated by `yaw`, and the animation's flags.
pub fn find_mario_anim_flags_and_translation(
    obj: &MarioObject,
    w: &StepWorld<'_>,
    yaw: i32,
) -> (i16, [i16; 3]) {
    let data = cur_anim(&obj.gfx.anim, w);
    let (frame, _) = geo_update_animation_frame(&obj.gfx.anim, data, w.area_update_counter);
    let frame = i32::from(frame);
    let s = w.trig.sins(yaw);
    let c = w.trig.coss(yaw);
    let dx = f32::from(data.value(0, frame)) / 4.0;
    let y = f32_to_s16(f32::from(data.value(1, frame)) / 4.0);
    let dz = f32::from(data.value(2, frame)) / 4.0;
    let translation = [
        f32_to_s16((dx * c) + (dz * s)),
        y,
        f32_to_s16((-dx * s) + (dz * c)),
    ];
    (data.flags, translation)
}

/// update_mario_pos_for_anim.
pub fn update_mario_pos_for_anim(m: &mut MarioState, w: &StepWorld<'_>) {
    let (flags, translation) =
        find_mario_anim_flags_and_translation(&m.obj, w, i32::from(m.face_angle[1]));
    if flags & (ANIM_FLAG_HOR_TRANS | ANIM_FLAG_6) != 0 {
        m.pos[0] += f32::from(translation[0]);
        m.pos[2] += f32::from(translation[2]);
    }
    if flags & (ANIM_FLAG_VERT_TRANS | ANIM_FLAG_6) != 0 {
        m.pos[1] += f32::from(translation[1]);
    }
}

/// return_mario_anim_y_translation.
pub fn return_mario_anim_y_translation(m: &MarioState, w: &StepWorld<'_>) -> i16 {
    find_mario_anim_flags_and_translation(&m.obj, w, 0).1[1]
}
