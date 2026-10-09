//! The original pseudo-random generator, translated from pinned CC0
//! n64decomp/sm64 `src/engine/behavior_script.c` (`random_u16`,
//! `random_float`). One seed (gRandomSeed16) is shared by every caller:
//! objects and the camera's random shakes draw from the same sequence, so the
//! order of calls within a tick is part of the simulation.

/// gRandomSeed16. A fresh boot starts at zero; every draw advances it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rng {
    pub seed: u16,
}

impl Rng {
    pub fn new(seed: u16) -> Self {
        Self { seed }
    }

    /// random_u16: the next value, which is also the new seed.
    pub fn random_u16(&mut self) -> u16 {
        if self.seed == 22026 {
            self.seed = 0;
        }
        let mut temp1 = (self.seed & 0x00FF) << 8;
        temp1 ^= self.seed;
        self.seed = ((temp1 & 0x00FF) << 8) + ((temp1 & 0xFF00) >> 8);
        temp1 = ((temp1 & 0x00FF) << 1) ^ self.seed;
        let temp2 = (temp1 >> 1) ^ 0xFF80;
        if temp1 & 1 == 0 {
            if temp2 == 43605 {
                self.seed = 0;
            } else {
                self.seed = temp2 ^ 0x1FF4;
            }
        } else {
            self.seed = temp2 ^ 0x8180;
        }
        self.seed
    }

    /// random_float: the draw as an f32, divided in double precision as the
    /// original's `rnd / (double) 0x10000`, in [0, 1).
    pub fn random_float(&mut self) -> f32 {
        let rnd = f32::from(self.random_u16());
        (f64::from(rnd) / 65536.0) as f32
    }
}
