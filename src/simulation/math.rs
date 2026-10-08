//! Original math utilities used by gameplay, translated from pinned CC0
//! n64decomp/sm64 src/engine/math_util.c and math_util.h (see PROVENANCE.md).
//!
//! The trigonometric tables are game data: they are loaded from the user's ROM
//! (`import::engine::trig_tables`) and never stored in this repository.

/// gSineTable is cut short and reads continue into gCosineTable, which starts
/// 0x400 entries later; one contiguous 0x1400-entry table reproduces that.
pub const SINE_ENTRIES: usize = 0x1400;
pub const ARCTAN_ENTRIES: usize = 0x401;

#[derive(Debug, Clone, PartialEq)]
pub struct TrigTables {
    sine: Vec<f32>,
    arctan: Vec<u16>,
}

impl TrigTables {
    /// Build from table contents; None unless the lengths match the original.
    pub fn new(sine: Vec<f32>, arctan: Vec<u16>) -> Option<Self> {
        (sine.len() == SINE_ENTRIES && arctan.len() == ARCTAN_ENTRIES)
            .then_some(Self { sine, arctan })
    }

    pub fn sine_table(&self) -> &[f32] {
        &self.sine
    }

    pub fn arctan_table(&self) -> &[u16] {
        &self.arctan
    }

    /// `sins(x)`: gSineTable[(u16)(x) >> 4]. Takes any integer expression, as the
    /// macro does, so callers can pass s32 sums without changing wraparound.
    pub fn sins(&self, x: i32) -> f32 {
        self.sine[usize::from((x as u16) >> 4)]
    }

    /// `coss(x)`: gCosineTable[(u16)(x) >> 4].
    pub fn coss(&self, x: i32) -> f32 {
        self.sine[0x400 + usize::from((x as u16) >> 4)]
    }

    /// atan2_lookup: arctangent of y/x for results in [0, 0x2000].
    ///
    /// Callers guarantee |y| <= |x|. A NaN ratio is outside supported coverage:
    /// the N64 conversion would fault; here it reads entry 0 like x == 0.
    fn atan2_lookup(&self, y: f32, x: f32) -> u16 {
        if x == 0.0 {
            self.arctan[0]
        } else {
            let index = (y / x * 1024.0 + 0.5) as i32;
            self.arctan[usize::try_from(index).unwrap_or(0).min(ARCTAN_ENTRIES - 1)]
        }
    }

    /// atan2s: the angle from (0, 0) to (x, y). Commonly called as (z, x) for yaw.
    pub fn atan2s(&self, y: f32, x: f32) -> i16 {
        let (mut y, mut x) = (y, x);
        let ret: u16 = if x >= 0.0 {
            if y >= 0.0 {
                if y >= x {
                    self.atan2_lookup(x, y)
                } else {
                    0x4000u16.wrapping_sub(self.atan2_lookup(y, x))
                }
            } else {
                y = -y;
                if y < x {
                    0x4000u16.wrapping_add(self.atan2_lookup(y, x))
                } else {
                    0x8000u16.wrapping_sub(self.atan2_lookup(x, y))
                }
            }
        } else {
            x = -x;
            if y < 0.0 {
                y = -y;
                if y >= x {
                    0x8000u16.wrapping_add(self.atan2_lookup(x, y))
                } else {
                    0xC000u16.wrapping_sub(self.atan2_lookup(y, x))
                }
            } else if y < x {
                0xC000u16.wrapping_add(self.atan2_lookup(y, x))
            } else {
                0u16.wrapping_sub(self.atan2_lookup(x, y))
            }
        };
        ret as i16
    }

    /// atan2f: `(f32) atan2s(y, x) * M_PI / 0x8000`, evaluated in double.
    pub fn atan2f(&self, y: f32, x: f32) -> f32 {
        (f64::from(f32::from(self.atan2s(y, x))) * std::f64::consts::PI / 32768.0) as f32
    }
}

/// approach_s32, including the original's possible overflow past the target
/// near the s32 limits (wrapping, as MIPS addition does).
pub fn approach_s32(current: i32, target: i32, inc: i32, dec: i32) -> i32 {
    let mut current = current;
    if current < target {
        current = current.wrapping_add(inc);
        if current > target {
            current = target;
        }
    } else {
        current = current.wrapping_sub(dec);
        if current < target {
            current = target;
        }
    }
    current
}

pub fn approach_f32(current: f32, target: f32, inc: f32, dec: f32) -> f32 {
    let mut current = current;
    if current < target {
        current += inc;
        if current > target {
            current = target;
        }
    } else {
        current -= dec;
        if current < target {
            current = target;
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Authored tables: index-coded values, not the game's table contents.
    fn authored() -> TrigTables {
        let sine = (0..SINE_ENTRIES).map(|i| i as f32).collect();
        let arctan = (0..ARCTAN_ENTRIES).map(|i| i as u16 * 8).collect();
        TrigTables::new(sine, arctan).unwrap()
    }

    #[test]
    fn sins_and_coss_index_with_u16_wraparound_and_overlap() {
        let t = authored();
        assert_eq!(t.sins(0x10), 1.0);
        assert_eq!(t.sins(-16), 4095.0);
        assert_eq!(t.sins(0x10000 + 0x20), 2.0);
        assert_eq!(t.coss(0), 1024.0);
        assert_eq!(t.coss(-1), 1024.0 + 4095.0);
        assert!(TrigTables::new(vec![0.0; 10], vec![0; ARCTAN_ENTRIES]).is_none());
    }

    #[test]
    fn atan2s_octants_use_the_original_offsets() {
        let t = authored();
        assert_eq!(t.atan2s(1.0, 1.0), 1024 * 8);
        assert_eq!(t.atan2s(1.0, 0.0), 0);
        assert_eq!(t.atan2s(0.0, 1.0), 0x4000);
        assert_eq!(t.atan2s(-1.0, 0.0), -0x8000);
        assert_eq!(t.atan2s(0.0, -1.0) as u16, 0xC000);
        // The original overflows past a target near the s32 limit.
        assert_eq!(approach_s32(i32::MAX - 1, i32::MAX, 5, 0), i32::MIN + 3);
        assert_eq!(approach_s32(0, 10, 3, 1), 3);
        assert_eq!(approach_f32(5.0, 0.0, 1.0, 2.0), 3.0);
    }
}
