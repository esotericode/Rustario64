//! Fixed cadence, input edges, and original collision. No approximate Mario equations.
pub mod camera;
pub mod collision;
pub mod controller;
pub mod mario;
pub mod math;

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const TICKS_PER_SECOND: u32 = 30;
const PHASE_PER_TICK: u128 = 1_000_000_000;

/// Rational integer scheduler: one tick per 1/30 s, without rounding a tick to ns.
/// A drain budget bounds work per display frame. Backlog is retained, never dropped.
#[derive(Debug, Default)]
pub struct FixedClock {
    phase: u128,
}

impl FixedClock {
    pub fn add_elapsed(&mut self, elapsed: Duration) -> Result<(), &'static str> {
        let increment = elapsed
            .as_nanos()
            .checked_mul(u128::from(TICKS_PER_SECOND))
            .ok_or("elapsed time overflow")?;
        self.phase = self
            .phase
            .checked_add(increment)
            .ok_or("clock backlog overflow")?;
        Ok(())
    }

    pub fn drain(&mut self, budget: u32) -> u32 {
        let ticks = (self.phase / PHASE_PER_TICK).min(u128::from(budget)) as u32;
        self.phase -= u128::from(ticks) * PHASE_PER_TICK;
        ticks
    }

    pub fn pending_ticks(&self) -> u128 {
        self.phase / PHASE_PER_TICK
    }

    /// A full backlog means the displayed pose holds at the latest completed tick.
    pub fn alpha(&self) -> f32 {
        if self.pending_ticks() > 0 {
            1.0
        } else {
            self.phase as f32 / PHASE_PER_TICK as f32
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickInput {
    pub buttons: u16,
    /// Signed N64 stick bytes, normalized by `controller::Controller` at ticks.
    pub stick: [i8; 2],
    pub camera_yaw: i16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputEdges {
    previous: u16,
}
impl InputEdges {
    /// Consume exactly once per gameplay tick. Presentation never calls this.
    pub fn consume(&mut self, input: TickInput) -> (u16, u16) {
        let pressed = input.buttons & !self.previous;
        let released = self.previous & !input.buttons;
        self.previous = input.buttons;
        (pressed, released)
    }
}
