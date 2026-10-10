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

/// A row-major 4x4 matrix (Mat4): `m[row][column]`, row 3 the translation.
pub type Mat4 = [[f32; 4]; 4];

/// mtxf_identity.
pub fn mtxf_identity() -> Mat4 {
    let mut mtx = [[0.0; 4]; 4];
    for (i, row) in mtx.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    mtx
}

/// mtxf_lookat: the camera transform for a camera at `from` looking at `to`
/// with a bank of `roll`. The inverse lengths are computed in double
/// precision and narrowed, as the original's `-1.0 / sqrtf(...)`.
pub fn mtxf_lookat(trig: &TrigTables, from: [f32; 3], to: [f32; 3], roll: i16) -> Mat4 {
    let mut dx = to[0] - from[0];
    let mut dz = to[2] - from[2];
    let mut inv_length = (-1.0 / f64::from((dx * dx + dz * dz).sqrt())) as f32;
    dx *= inv_length;
    dz *= inv_length;

    let mut y_col_y = trig.coss(i32::from(roll));
    let mut x_col_y = trig.sins(i32::from(roll)) * dz;
    let mut z_col_y = -trig.sins(i32::from(roll)) * dx;

    let mut x_col_z = to[0] - from[0];
    let mut y_col_z = to[1] - from[1];
    let mut z_col_z = to[2] - from[2];
    inv_length = (-1.0
        / f64::from((x_col_z * x_col_z + y_col_z * y_col_z + z_col_z * z_col_z).sqrt()))
        as f32;
    x_col_z *= inv_length;
    y_col_z *= inv_length;
    z_col_z *= inv_length;

    let mut x_col_x = y_col_y * z_col_z - z_col_y * y_col_z;
    let mut y_col_x = z_col_y * x_col_z - x_col_y * z_col_z;
    let mut z_col_x = x_col_y * y_col_z - y_col_y * x_col_z;
    inv_length = (1.0
        / f64::from((x_col_x * x_col_x + y_col_x * y_col_x + z_col_x * z_col_x).sqrt()))
        as f32;
    x_col_x *= inv_length;
    y_col_x *= inv_length;
    z_col_x *= inv_length;

    x_col_y = y_col_z * z_col_x - z_col_z * y_col_x;
    y_col_y = z_col_z * x_col_x - x_col_z * z_col_x;
    z_col_y = x_col_z * y_col_x - y_col_z * x_col_x;
    inv_length = (1.0
        / f64::from((x_col_y * x_col_y + y_col_y * y_col_y + z_col_y * z_col_y).sqrt()))
        as f32;
    x_col_y *= inv_length;
    y_col_y *= inv_length;
    z_col_y *= inv_length;

    [
        [x_col_x, x_col_y, x_col_z, 0.0],
        [y_col_x, y_col_y, y_col_z, 0.0],
        [z_col_x, z_col_y, z_col_z, 0.0],
        [
            -(from[0] * x_col_x + from[1] * y_col_x + from[2] * z_col_x),
            -(from[0] * x_col_y + from[1] * y_col_y + from[2] * z_col_y),
            -(from[0] * x_col_z + from[1] * y_col_z + from[2] * z_col_z),
            1.0,
        ],
    ]
}

/// mtxf_rotate_zxy_and_translate.
pub fn mtxf_rotate_zxy_and_translate(
    trig: &TrigTables,
    translate: [f32; 3],
    rotate: [i16; 3],
) -> Mat4 {
    let sx = trig.sins(i32::from(rotate[0]));
    let cx = trig.coss(i32::from(rotate[0]));
    let sy = trig.sins(i32::from(rotate[1]));
    let cy = trig.coss(i32::from(rotate[1]));
    let sz = trig.sins(i32::from(rotate[2]));
    let cz = trig.coss(i32::from(rotate[2]));
    [
        [
            cy * cz + sx * sy * sz,
            cx * sz,
            -sy * cz + sx * cy * sz,
            0.0,
        ],
        [
            -cy * sz + sx * sy * cz,
            cx * cz,
            sy * sz + sx * cy * cz,
            0.0,
        ],
        [cx * sy, -sx, cx * cy, 0.0],
        [translate[0], translate[1], translate[2], 1.0],
    ]
}

/// mtxf_billboard: a matrix facing the camera (`mtx` is the camera
/// transform) at `position`, turned by `angle`.
pub fn mtxf_billboard(trig: &TrigTables, mtx: &Mat4, position: [f32; 3], angle: i16) -> Mat4 {
    let c = trig.coss(i32::from(angle));
    let s = trig.sins(i32::from(angle));
    let row3 = |j: usize| {
        mtx[0][j] * position[0] + mtx[1][j] * position[1] + mtx[2][j] * position[2] + mtx[3][j]
    };
    [
        [c, s, 0.0, 0.0],
        [-s, c, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [row3(0), row3(1), row3(2), 1.0],
    ]
}

/// mtxf_mul: `a` applied after `b`, for affine matrices (the bottom column
/// is taken as [0, 0, 0, 1]).
pub fn mtxf_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut temp = [[0.0; 4]; 4];
    for i in 0..4 {
        let (e0, e1, e2) = (a[i][0], a[i][1], a[i][2]);
        for j in 0..3 {
            let sum = e0 * b[0][j] + e1 * b[1][j] + e2 * b[2][j];
            temp[i][j] = if i == 3 { sum + b[3][j] } else { sum };
        }
    }
    temp[3][3] = 1.0;
    temp
}

/// mtxf_scale_vec3f: rows 0-2 scaled by `s`, row 3 copied.
pub fn mtxf_scale_vec3f(mtx: &Mat4, s: [f32; 3]) -> Mat4 {
    let mut dest = *mtx;
    for i in 0..4 {
        dest[0][i] = mtx[0][i] * s[0];
        dest[1][i] = mtx[1][i] * s[1];
        dest[2][i] = mtx[2][i] * s[2];
    }
    dest
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
