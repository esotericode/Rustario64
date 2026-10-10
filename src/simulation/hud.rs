//! The HUD's values, translated from pinned CC0 src/game/level_update.c
//! (`struct HudDisplay`, `update_hud_values`). init_mario_from_save_file
//! (mario.c) and init_level set the entry values. Drawing the HUD (hud.c) is
//! presentation; the values are simulation: the counter's sounds are part
//! of the frame, and update_hud_values clamps Mario's lives and coins.
use super::mario::{MarioState, StepWorld, constants::*};

/// gHudDisplay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HudDisplay {
    pub lives: i16,
    pub coins: i16,
    pub stars: i16,
    pub wedges: i16,
    pub keys: i16,
    pub flags: i16,
    pub timer: u16,
}

/// update_hud_values, with no credits sequence running (gCurrCreditsEntry is
/// NULL). The coin counter counts up toward Mario's coins by one every
/// other frame, with a coin sound each step.
pub fn update_hud_values(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let num_health_wedges = if m.health > 0 { m.health >> 8 } else { 0 };
    if w.course_num >= COURSE_MIN {
        w.hud.flags |= HUD_DISPLAY_FLAG_COIN_COUNT as i16;
    } else {
        w.hud.flags &= !(HUD_DISPLAY_FLAG_COIN_COUNT as i16);
    }
    if w.hud.coins < m.num_coins && w.global_timer & 1 != 0 {
        let coin_sound = if m.action & (ACT_FLAG_SWIMMING | ACT_FLAG_METAL_WATER) != 0 {
            SOUND_GENERAL_COIN_WATER
        } else {
            SOUND_GENERAL_COIN
        };
        w.hud.coins = w.hud.coins.wrapping_add(1);
        w.play_sound(coin_sound);
    }
    if m.num_lives > 100 {
        m.num_lives = 100;
    }
    // BUGFIX_MAX_LIVES is set for US.
    if m.num_coins > 999 {
        m.num_coins = 999;
    }
    if w.hud.coins > 999 {
        w.hud.coins = 999;
    }
    w.hud.stars = m.num_stars;
    w.hud.lives = i16::from(m.num_lives);
    w.hud.keys = i16::from(m.num_keys);
    if num_health_wedges > w.hud.wedges {
        w.play_sound(SOUND_MENU_POWER_METER);
    }
    w.hud.wedges = num_health_wedges;
    if m.hurt_counter > 0 {
        w.hud.flags |= HUD_DISPLAY_FLAG_EMPHASIZE_POWER as i16;
    } else {
        w.hud.flags &= !(HUD_DISPLAY_FLAG_EMPHASIZE_POWER as i16);
    }
}
