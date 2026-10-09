//! Reference controller processing, translated from pinned CC0 SM64
//! `game_init.c` (`adjust_analog_stick` and the connected-controller branch of
//! `read_controller_inputs`). Consume a sample once per simulation tick.
use super::TickInput;

pub const A_BUTTON: u16 = 0x8000;
pub const B_BUTTON: u16 = 0x4000;
pub const Z_TRIG: u16 = 0x2000;

/// Preserve button_down across ticks, including before the first recorded tick.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Controller {
    pub raw_stick: [i8; 2],
    pub stick_x: f32,
    pub stick_y: f32,
    pub stick_mag: f32,
    pub button_down: u16,
    pub button_pressed: u16,
}

impl Controller {
    /// TickInput represents a connected controller. A neutral sample releases
    /// all buttons; device disconnection is an application concern.
    pub fn sample(&mut self, input: TickInput) {
        self.raw_stick = input.stick;
        self.button_pressed = input.buttons & (input.buttons ^ self.button_down);
        self.button_down = input.buttons;
        self.adjust_analog_stick();
    }

    fn adjust_analog_stick(&mut self) {
        fn axis(raw: i8) -> f32 {
            if raw <= -8 {
                f32::from(raw) + 6.0
            } else if raw >= 8 {
                f32::from(raw) - 6.0
            } else {
                0.0
            }
        }
        self.stick_x = axis(self.raw_stick[0]);
        self.stick_y = axis(self.raw_stick[1]);
        self.stick_mag = (self.stick_x * self.stick_x + self.stick_y * self.stick_y).sqrt();
        if self.stick_mag > 64.0 {
            self.stick_x *= 64.0 / self.stick_mag;
            self.stick_y *= 64.0 / self.stick_mag;
            self.stick_mag = 64.0;
        }
    }
}
