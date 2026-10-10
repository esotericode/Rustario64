//! Object animations, translated from pinned CC0 src/engine/graph_node.c
//! (geo_obj_init_animation), src/engine/behavior_script.c
//! (bhv_cmd_load_animations, bhv_cmd_animate) and src/game/object_helpers.c
//! (cur_obj_init_animation).
//!
//! oAnimations holds the segmented address LOAD_ANIMATIONS stores (the
//! original keeps the script's segmented word there and converts it when it
//! reads an entry); curAnim names the struct Animation an entry points to by
//! its segmented address. The frame advance is the render pass's
//! (`mario::animation::update_animation_frame`, run for objects by
//! `object::render`), as for Mario.
use super::{AnimRef, GfxState, Object};
use crate::{
    content::animation::{ANIM_FLAG_BACKWARD, Animation, ObjectAnimations},
    simulation::mario::constants::O_ANIMATIONS,
};

/// geo_obj_init_animation: `entry` is the segmented address of the table
/// entry (`&anims[animIndex]`). A new animation starts one frame before (or
/// after, backward) its start frame, without acceleration.
pub fn geo_obj_init_animation(gfx: &mut GfxState, anims: &ObjectAnimations, entry: u32) {
    let address = anims.entry(entry).unwrap_or_else(|| {
        panic!("animation entry 0x{entry:08X} is outside every imported table (or NULL)")
    });
    let anim = anims
        .get(address)
        .unwrap_or_else(|| panic!("animation 0x{address:08X} was not imported"));
    if gfx.anim.cur_anim != Some(AnimRef::Object(address)) {
        gfx.anim.cur_anim = Some(AnimRef::Object(address));
        gfx.anim.anim_frame = (i32::from(anim.start_frame)
            + if anim.flags & ANIM_FLAG_BACKWARD != 0 {
                1
            } else {
                -1
            }) as i16;
        gfx.anim.anim_accel = 0;
        gfx.anim.anim_y_trans = 0;
    }
}

/// The entry address of animation `index` in the object's oAnimations table:
/// `&anims[index]`, four bytes per pointer.
pub fn animation_entry(o: &Object, index: i32) -> u32 {
    o.raw
        .u32(O_ANIMATIONS)
        .wrapping_add((index as u32).wrapping_mul(4))
}

/// cur_obj_init_animation.
pub fn cur_obj_init_animation(o: &mut Object, anims: &ObjectAnimations, index: i32) {
    let entry = animation_entry(o, index);
    geo_obj_init_animation(&mut o.gfx, anims, entry);
}

/// Where [`authored_object_animations`] places its Bob-omb-shaped table.
pub const AUTHORED_BOBOMB_ANIMATIONS: u32 = 0x0800_1000;

/// An authored animation set for ROM-free tests: a two-entry table at
/// [`AUTHORED_BOBOMB_ANIMATIONS`] shaped like the Bob-omb's (13 animated
/// parts; a looping walk and a looping held pose with different loop
/// points). The values are invented, not game data.
pub fn authored_object_animations() -> ObjectAnimations {
    let animation = |flags: i16, start: i16, loop_start: i16, loop_end: i16, seed: i16| {
        let parts = 13;
        let attributes = (parts + 1) * 3;
        let mut index = vec![];
        let mut values = vec![];
        for attribute in 0..attributes {
            let frames = if attribute % 4 == 0 {
                1
            } else {
                loop_end as usize
            };
            index.push(frames as u16);
            index.push(values.len() as u16);
            for frame in 0..frames {
                values.push(
                    seed.wrapping_mul(attribute as i16 + 3)
                        .wrapping_add(frame as i16 * 97),
                );
            }
        }
        Animation {
            flags,
            y_trans_divisor: 0,
            start_frame: start,
            loop_start,
            loop_end,
            bone_count: parts as i16,
            index,
            values,
        }
    };
    let mut out = ObjectAnimations::default();
    let (walk, held) = (0x0800_2000, 0x0800_3000);
    out.tables
        .insert(AUTHORED_BOBOMB_ANIMATIONS, vec![walk, held]);
    out.animations.insert(walk, animation(0, 0, 0, 25, 37));
    out.animations.insert(held, animation(0, 4, 2, 15, -53));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_resolve_inside_tables_only() {
        let anims = authored_object_animations();
        let base = AUTHORED_BOBOMB_ANIMATIONS;
        assert_eq!(anims.entry(base), Some(0x0800_2000));
        assert_eq!(anims.entry(base + 4), Some(0x0800_3000));
        assert_eq!(anims.entry(base + 8), None, "the NULL ends the table");
        assert_eq!(anims.entry(base + 2), None, "misaligned");
        assert_eq!(anims.entry(base - 4), None);
    }

    #[test]
    fn init_resets_only_on_a_new_animation() {
        let anims = authored_object_animations();
        let mut o = Object::default();
        o.raw.set_u32(O_ANIMATIONS, AUTHORED_BOBOMB_ANIMATIONS);
        o.gfx.anim.anim_accel = 7;
        cur_obj_init_animation(&mut o, &anims, 1);
        assert_eq!(o.gfx.anim.cur_anim, Some(AnimRef::Object(0x0800_3000)));
        assert_eq!((o.gfx.anim.anim_frame, o.gfx.anim.anim_accel), (3, 0));
        o.gfx.anim.anim_frame = 9;
        cur_obj_init_animation(&mut o, &anims, 1);
        assert_eq!(
            o.gfx.anim.anim_frame, 9,
            "the same animation keeps its frame"
        );
        cur_obj_init_animation(&mut o, &anims, 0);
        assert_eq!(o.gfx.anim.anim_frame, -1);
    }
}
