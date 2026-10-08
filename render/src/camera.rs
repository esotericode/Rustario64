//! Development viewing cameras. These are presentation-only free cameras for
//! inspecting imported levels; they are not the original Lakitu/Mario camera and
//! never feed movement input or any authoritative state.
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
        math::look_at(self.position, target, [0.0, 1.0, 0.0])
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
