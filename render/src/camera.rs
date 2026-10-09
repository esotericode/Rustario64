//! Viewing cameras for the renderer. `FlyCamera` is either a presentation-only
//! free camera for inspecting imported levels, or the view the original camera
//! computed (`play::reference_view`); either way it is only drawn from and
//! never feeds movement input or any authoritative state.
use crate::math::{self, Mat4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyCamera {
    pub position: [f32; 3],
    /// Radians around +Y; 0 looks toward -Z.
    pub yaw: f32,
    /// Radians; positive looks up.
    pub pitch: f32,
    pub fov_y_degrees: f32,
    pub near: f32,
    pub far: f32,
    /// Screen roll in radians, counterclockwise on screen (the original
    /// camera's rollScreen).
    pub roll: f32,
}

impl FlyCamera {
    pub fn looking_at(position: [f32; 3], target: [f32; 3]) -> Self {
        let d = math::normalize(math::sub(target, position));
        Self {
            position,
            yaw: (-d[0]).atan2(-d[2]),
            // Keep away from straight up/down, where the look-at basis degenerates.
            pitch: d[1].asin().clamp(-1.5, 1.5),
            fov_y_degrees: 45.0,
            near: 100.0,
            far: 30000.0,
            roll: 0.0,
        }
    }

    pub fn forward(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [-sy * cp, sp, -cy * cp]
    }

    pub fn right(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        [cy, 0.0, -sy]
    }

    pub fn view(&self) -> Mat4 {
        let f = self.forward();
        let target = [
            self.position[0] + f[0],
            self.position[1] + f[1],
            self.position[2] + f[2],
        ];
        let view = math::look_at(self.position, target, [0.0, 1.0, 0.0]);
        if self.roll == 0.0 {
            return view;
        }
        // Rotate eye space about the view axis, as the original's screen
        // roll matrix does ahead of the projection.
        let (s, c) = self.roll.sin_cos();
        let roll = [
            [c, s, 0.0, 0.0],
            [-s, c, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        math::mul(&roll, &view)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        math::perspective(self.fov_y_degrees, aspect, self.near, self.far)
    }

    /// Move in camera-relative axes: x = right, y = world up, z = forward.
    pub fn translate(&mut self, amount: [f32; 3]) {
        let f = self.forward();
        let r = self.right();
        for i in 0..3 {
            self.position[i] += r[i] * amount[0] + f[i] * amount[2];
        }
        self.position[1] += amount[1];
    }

    pub fn rotate(&mut self, yaw: f32, pitch: f32) {
        self.yaw += yaw;
        self.pitch = (self.pitch + pitch).clamp(-1.5, 1.5);
    }
}

/// Named inspection viewpoints. `start` is placed behind the level's Mario start
/// position using the script yaw (degrees); others are fixed overviews.
pub fn preset(name: &str, mario_start: Option<(i16, [i16; 3])>) -> Option<FlyCamera> {
    let (yaw, [x, y, z]) = mario_start.unwrap_or((0, [0, 0, 0]));
    let start = [f32::from(x), f32::from(y), f32::from(z)];
    let yaw = f32::from(yaw).to_radians();
    // SM64 yaw 0 faces +Z; facing = (sin yaw, cos yaw) in X/Z.
    let facing = [yaw.sin(), 0.0, yaw.cos()];
    Some(match name {
        "start" => FlyCamera::looking_at(
            [
                start[0] - facing[0] * 1400.0,
                start[1] + 600.0,
                start[2] - facing[2] * 1400.0,
            ],
            [
                start[0] + facing[0] * 600.0,
                start[1] + 200.0,
                start[2] + facing[2] * 600.0,
            ],
        ),
        "overview" => FlyCamera::looking_at([-9000.0, 9000.0, 11000.0], [0.0, 500.0, 0.0]),
        "summit" => FlyCamera::looking_at([5500.0, 6500.0, 2500.0], [1000.0, 3600.0, -4500.0]),
        "top" => FlyCamera::looking_at([0.0, 21000.0, 1500.0], [0.0, 0.0, 0.0]),
        _ => return None,
    })
}

pub const PRESETS: [&str; 4] = ["start", "overview", "summit", "top"];
