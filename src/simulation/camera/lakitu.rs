//! Persistent Lakitu/transition stage of pinned CC0 camera.c. A mode controller
//! must supply `camera.pos/focus/next_yaw` before `update`; this is not the full
//! update_camera dispatcher. All smoothing and shakes run once per 30 Hz tick.
#![allow(clippy::too_many_arguments)]
use super::*;

mod record;
pub use record::{STATE_FIELD_NAMES, STATE_WORDS};

#[derive(Debug, Default, Clone, Copy)]
pub struct Camera {
    pub mode: u8,
    pub def_mode: u8,
    pub yaw: i16,
    pub next_yaw: i16,
    pub focus: [f32; 3],
    pub pos: [f32; 3],
    pub cutscene: u8,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Lakitu {
    pub cur_focus: [f32; 3],
    pub cur_pos: [f32; 3],
    pub goal_focus: [f32; 3],
    pub goal_pos: [f32; 3],
    pub mode: u8,
    pub def_mode: u8,
    pub focus_distance: f32,
    pub old_pitch: i16,
    pub old_yaw: i16,
    pub shake_magnitude: [i16; 3],
    pub shake_phase: [i16; 3],
    pub shake_velocity: [i16; 3],
    pub shake_decay: [i16; 3],
    pub roll: i16,
    pub yaw: i16,
    pub next_yaw: i16,
    pub focus: [f32; 3],
    pub pos: [f32; 3],
    pub foc_h_speed: f32,
    pub foc_v_speed: f32,
    pub pos_h_speed: f32,
    pub pos_v_speed: f32,
    pub key_dance_roll: i16,
    pub last_frame_action: u32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Transition {
    pub pos_pitch: i16,
    pub pos_yaw: i16,
    pub pos_dist: f32,
    pub foc_pitch: i16,
    pub foc_yaw: i16,
    pub foc_dist: f32,
    pub frames_left: i32,
    pub mario_pos: [f32; 3],
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FovShake {
    pub amplitude: f32,
    pub decay: i16,
    pub speed: i16,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Rig {
    pub camera: Camera,
    pub lakitu: Lakitu,
    pub transition: Transition,
    pub status: i16,
    pub movement: u16,
    pub yaw_speed: i16,
    pub old_pos: [f32; 3],
    pub old_focus: [f32; 3],
    pub player2_focus_offset: [f32; 3],
    pub handheld_angles: [i16; 3],
    pub handheld_magnitude: i16,
    pub handheld_increment: f32,
    pub fov_shake: FovShake,
    pub new_mode: i16,
    pub last_mode: i16,
    pub c_up_pitch: i16,
    pub mode_offset_yaw: i16,
    pub lakitu_dist: i16,
    pub lakitu_pitch: i16,
    pub area_yaw_change: i16,
    pub pan_distance: f32,
    pub cannon_y_offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    HandheldRandomShake,
    ShockRandomShake,
}

/// C compound assignment converts the complete float sum, then narrows to s16.
/// Truncating the increment first gives different answers across zero.
fn add_float_angle(angle: i16, delta: f32) -> i16 {
    (f32::from(angle) + delta) as i32 as i16
}
fn abs_float(x: f32) -> f32 {
    if x > 0.0 { x } else { -x }
}
fn get_dist_angle(from: [f32; 3], to: [f32; 3], trig: &TrigTables) -> (f32, i16, i16) {
    // math_util.c accumulates x*x + y*y + z*z, just like calc_abs_dist.
    let [pitch, yaw] = calculate_angles(from, to, trig);
    (calc_abs_dist(from, to), pitch, yaw)
}

impl Rig {
    pub fn transition_next_state(&mut self, frames: i16) {
        if self.status & c::CAM_FLAG_FRAME_AFTER_CAM_INIT == 0 {
            self.status |= c::CAM_FLAG_START_TRANSITION | c::CAM_FLAG_TRANSITION_OUT_OF_C_UP;
            self.transition.frames_left = i32::from(frames);
        }
    }

    pub fn transition_to_camera_mode(&mut self, new_mode: i16, frames: i16) {
        if i16::from(self.camera.mode) != new_mode {
            self.new_mode = if new_mode != -1 {
                new_mode
            } else {
                self.last_mode
            };
            self.last_mode = i16::from(self.camera.mode);
            self.camera.mode = self.new_mode as u8;
            self.movement &= !(c::CAM_MOVE_RESTRICT | c::CAM_MOVE_ROTATE);
            if self.status & c::CAM_FLAG_FRAME_AFTER_CAM_INIT == 0 {
                self.transition_next_state(frames);
                self.c_up_pitch = 0;
                self.mode_offset_yaw = 0;
                self.lakitu_dist = 0;
                self.lakitu_pitch = 0;
                self.area_yaw_change = 0;
                self.pan_distance = 0.0;
                self.cannon_y_offset = 0.0;
            }
        }
    }

    pub fn next_state(
        &mut self,
        mario: [f32; 3],
        world: &CollisionWorld,
        flags: &mut CollisionFlags,
        trig: &TrigTables,
    ) -> ([f32; 3], [f32; 3], i16) {
        let mut pos = self.camera.pos;
        let mut focus = self.camera.focus;
        let mut yaw = self.camera.next_yaw;
        let t = &mut self.transition;
        let dist_timer = t.frames_left as f32;
        let angle_timer = t.frames_left as i16;
        if self.status & c::CAM_FLAG_START_TRANSITION != 0 {
            let start_pos = std::array::from_fn(|i| self.old_pos[i] + mario[i] - t.mario_pos[i]);
            let start_focus =
                std::array::from_fn(|i| self.old_focus[i] + mario[i] - t.mario_pos[i]);
            (t.foc_dist, t.foc_pitch, t.foc_yaw) = get_dist_angle(focus, start_focus, trig);
            (t.pos_dist, t.pos_pitch, t.pos_yaw) = get_dist_angle(focus, start_pos, trig);
            self.status &= !c::CAM_FLAG_START_TRANSITION;
        }
        if t.frames_left > 0 {
            assert!(
                t.frames_left <= i32::from(i16::MAX),
                "camera transition requires an s16 frame count"
            );
            let (goal_dist, goal_pitch, goal_yaw) = get_dist_angle(focus, pos, trig);
            let dist_velocity = abs_float(goal_dist - t.pos_dist) / dist_timer;
            let pitch_velocity = (abs_float(f32::from(goal_pitch) - f32::from(t.pos_pitch))
                / f32::from(angle_timer)) as i32 as i16;
            let yaw_velocity = (abs_float(f32::from(goal_yaw) - f32::from(t.pos_yaw))
                / f32::from(angle_timer)) as i32 as i16;
            camera_approach_f32_symmetric_bool(&mut t.pos_dist, goal_dist, dist_velocity);
            camera_approach_s16_symmetric_bool(&mut t.pos_yaw, goal_yaw, yaw_velocity);
            camera_approach_s16_symmetric_bool(&mut t.pos_pitch, goal_pitch, pitch_velocity);
            let next_pos = set_dist_and_angle(focus, t.pos_dist, t.pos_pitch, t.pos_yaw, trig);
            let (_, goal_pitch, goal_yaw) = get_dist_angle(pos, focus, trig);
            let pitch_velocity = (i32::from(t.foc_pitch) / i32::from(angle_timer)) as i16;
            let yaw_velocity = (i32::from(t.foc_yaw) / i32::from(angle_timer)) as i16;
            let dist_velocity = t.foc_dist / t.frames_left as f32;
            camera_approach_s16_symmetric_bool(&mut t.foc_pitch, goal_pitch, pitch_velocity);
            camera_approach_s16_symmetric_bool(&mut t.foc_yaw, goal_yaw, yaw_velocity);
            camera_approach_f32_symmetric_bool(&mut t.foc_dist, 0.0, dist_velocity);
            focus = set_dist_and_angle(focus, t.foc_dist, t.foc_pitch, t.foc_yaw, trig);
            pos = next_pos;
            if self.camera.cutscene != 0 || self.movement & c::CAM_MOVE_C_UP_MODE == 0 {
                let (floor_height, _) = world.find_floor(pos[0], pos[1], pos[2], flags);
                if floor_height != FLOOR_LOWER_LIMIT && floor_height + 125.0 > pos[1] {
                    pos[1] = floor_height + 125.0;
                }
                let mut data = WallCollisionData::new(pos, 0.0, 100.0);
                world.find_wall_collisions(&mut data, *flags, false);
                pos = [data.x, data.y, data.z];
            }
            t.frames_left -= 1;
            yaw = calculate_yaw(focus, pos, trig);
        } else {
            t.pos_dist = 0.0;
            t.pos_pitch = 0;
            t.pos_yaw = 0;
            self.status &= !c::CAM_FLAG_TRANSITION_OUT_OF_C_UP;
        }
        t.mario_pos = mario;
        (pos, focus, yaw)
    }

    pub fn set_pitch_shake(&mut self, magnitude: i16, decay: i16, speed: i16) {
        if self.lakitu.shake_magnitude[0] < magnitude {
            self.set_shake(0, magnitude, decay, speed);
        }
    }
    pub fn set_yaw_shake(&mut self, magnitude: i16, decay: i16, speed: i16) {
        if abs_float(f32::from(magnitude)) > abs_float(f32::from(self.lakitu.shake_magnitude[1])) {
            self.set_shake(1, magnitude, decay, speed);
        }
    }
    pub fn set_roll_shake(&mut self, magnitude: i16, decay: i16, speed: i16) {
        if self.lakitu.shake_magnitude[2] < magnitude {
            self.set_shake(2, magnitude, decay, speed);
        }
    }
    fn set_shake(&mut self, axis: usize, magnitude: i16, decay: i16, speed: i16) {
        self.lakitu.shake_magnitude[axis] = magnitude;
        self.lakitu.shake_decay[axis] = decay;
        self.lakitu.shake_velocity[axis] = speed;
    }
    pub fn set_fov_shake(&mut self, amplitude: i16, decay: i16, speed: i16) {
        if f32::from(amplitude) > self.fov_shake.amplitude {
            self.fov_shake = FovShake {
                amplitude: f32::from(amplitude),
                decay,
                speed,
            };
        }
    }
    pub fn shake_from_hit(&mut self, shake: i16, action: u32) -> Result<(), Unsupported> {
        match shake {
            c::SHAKE_ATTACK => {
                self.lakitu.foc_h_speed = 0.0;
                self.lakitu.pos_h_speed = 0.0;
            }
            c::SHAKE_FALL_DAMAGE => {
                self.set_pitch_shake(0x60, 3, i16::MIN);
                self.set_roll_shake(0x60, 3, i16::MIN);
            }
            c::SHAKE_GROUND_POUND => self.set_pitch_shake(0x60, 0xc, i16::MIN),
            c::SHAKE_SMALL_DAMAGE | c::SHAKE_MED_DAMAGE | c::SHAKE_LARGE_DAMAGE => {
                let underwater = action & (c::ACT_FLAG_SWIMMING | c::ACT_FLAG_METAL_WATER) != 0;
                let scale = match shake {
                    c::SHAKE_SMALL_DAMAGE => 1,
                    c::SHAKE_MED_DAMAGE => 2,
                    _ => 3,
                };
                if underwater {
                    self.set_yaw_shake(scale * 0x200, scale * 0x10, 0x1000);
                    self.set_roll_shake((scale + 1) * 0x200, (scale + 1) * 0x10, 0x1000);
                } else {
                    self.set_yaw_shake(
                        scale * 0x80,
                        if scale == 3 { 0x20 } else { scale * 8 },
                        0x4000,
                    );
                    self.set_roll_shake(
                        if scale == 3 { 0x200 } else { scale * 0x80 },
                        if scale == 3 { 0x20 } else { scale * 8 },
                        0x4000,
                    );
                }
                self.set_fov_shake((scale + 1) * 0x80, (scale + 2) * 0x10, i16::MIN);
                self.lakitu.foc_h_speed = 0.0;
                self.lakitu.pos_h_speed = 0.0;
            }
            c::SHAKE_HIT_FROM_BELOW => {
                self.lakitu.foc_h_speed = 0.07;
                self.lakitu.pos_h_speed = 0.07;
            }
            c::SHAKE_SHOCK => return Err(Unsupported::ShockRandomShake),
            _ => {}
        }
        Ok(())
    }

    /// Original update_lakitu only. `last_frame_action` is written by the outer
    /// update_camera AFTER this stage; the caller must preserve that ordering.
    /// RNG-driven handheld requests are rejected before any state changes.
    pub fn update(
        &mut self,
        mario: [f32; 3],
        action: u32,
        world: &CollisionWorld,
        flags: &mut CollisionFlags,
        trig: &TrigTables,
    ) -> Result<(), Unsupported> {
        if self.movement & c::CAM_MOVE_PAUSE_SCREEN == 0 {
            if self.handheld_magnitude != 0 {
                return Err(Unsupported::HandheldRandomShake);
            }
            let (new_pos, new_focus, new_yaw) = self.next_state(mario, world, flags, trig);
            set_or_approach_s16_symmetric(
                &mut self.camera.yaw,
                new_yaw,
                self.yaw_speed,
                self.status,
            );
            self.status &= !c::CAM_FLAG_UNUSED_CUTSCENE_ACTIVE;
            self.old_pos = new_pos;
            self.old_focus = new_focus;
            let l = &mut self.lakitu;
            l.yaw = self.camera.yaw;
            l.next_yaw = self.camera.next_yaw;
            l.goal_pos = self.camera.pos;
            l.goal_focus = self.camera.focus;
            set_or_approach_vec3f_asymptotic(
                &mut l.cur_pos,
                new_pos,
                [l.pos_h_speed, l.pos_v_speed, l.pos_h_speed],
                self.status,
            );
            set_or_approach_vec3f_asymptotic(
                &mut l.cur_focus,
                new_focus,
                [l.foc_h_speed, l.foc_v_speed, l.foc_h_speed],
                self.status,
            );
            set_or_approach_f32_asymptotic(&mut l.foc_h_speed, 0.8, 0.05, self.status);
            set_or_approach_f32_asymptotic(&mut l.foc_v_speed, 0.3, 0.05, self.status);
            set_or_approach_f32_asymptotic(&mut l.pos_h_speed, 0.3, 0.05, self.status);
            set_or_approach_f32_asymptotic(&mut l.pos_v_speed, 0.3, 0.05, self.status);
            if self.status & c::CAM_FLAG_BLOCK_SMOOTH_MOVEMENT != 0 {
                self.status &= !c::CAM_FLAG_BLOCK_SMOOTH_MOVEMENT;
            } else {
                self.status |= c::CAM_FLAG_SMOOTH_MOVEMENT;
            }
            l.pos = l.cur_pos;
            l.focus = l.cur_focus;
            if self.camera.cutscene != 0 {
                for i in 0..3 {
                    l.focus[i] += self.player2_focus_offset[i];
                }
                self.player2_focus_offset = [0.0; 3];
            }
            (l.focus_distance, l.old_pitch, l.old_yaw) = get_dist_angle(l.pos, l.focus, trig);
            l.roll = 0;
            // Pitch is reconstructed even when ONLY yaw magnitude is nonzero.
            if l.shake_magnitude[0] | l.shake_magnitude[1] != 0 {
                apply_angle_shake(l, 0, trig);
            }
            if l.shake_magnitude[1] != 0 {
                apply_angle_shake(l, 1, trig);
            }
            if l.shake_magnitude[2] != 0 {
                increment_shake_offset(&mut l.shake_phase[2], l.shake_velocity[2]);
                l.roll = add_float_angle(
                    l.roll,
                    f32::from(l.shake_magnitude[2]) * trig.sins(i32::from(l.shake_phase[2])),
                );
                decay_shake(l, 2);
            }
            for angle in &mut self.handheld_angles {
                approach_s16_asymptotic_bool(angle, 0, 8);
            }
            if self.handheld_angles[0] | self.handheld_angles[1] != 0 {
                let (dist, pitch, yaw) = get_dist_angle(l.pos, l.focus, trig);
                l.focus = set_dist_and_angle(
                    l.pos,
                    dist,
                    pitch.wrapping_add(self.handheld_angles[0]),
                    yaw.wrapping_add(self.handheld_angles[1]),
                    trig,
                );
            }
            self.handheld_magnitude = 0;
            self.handheld_increment = 0.0;
            if action == c::ACT_DIVE && l.last_frame_action != c::ACT_DIVE {
                // SHAKE_HIT_FROM_BELOW: after this tick's interpolation/speed restore.
                l.foc_h_speed = 0.07;
                l.pos_h_speed = 0.07;
            }
            l.roll = l
                .roll
                .wrapping_add(self.handheld_angles[2])
                .wrapping_add(l.key_dance_roll);
            if i16::from(self.camera.mode) != c::CAMERA_MODE_C_UP && self.camera.cutscene == 0 {
                flags.checking_for_camera = true;
                let (floor_height, _) =
                    world.find_floor(l.pos[0], l.pos[1] + 20.0, l.pos[2], flags);
                if floor_height != FLOOR_LOWER_LIMIT {
                    if l.pos[1] < floor_height + 100.0 {
                        l.pos[1] = floor_height + 100.0;
                    } else {
                        flags.checking_for_camera = false;
                    }
                }
            }
            self.transition.mario_pos = mario;
        }
        // This reconstruction and mode copy still happen while paused.
        clamp_pitch(
            self.lakitu.pos,
            &mut self.lakitu.focus,
            0x3e00,
            -0x3e00,
            trig,
        );
        self.lakitu.mode = self.camera.mode;
        self.lakitu.def_mode = self.camera.def_mode;
        Ok(())
    }
}

pub fn increment_shake_offset(offset: &mut i16, increment: i16) {
    *offset = if increment == i16::MIN {
        ((*offset as u16 & 0x8000) as i32 + 0xc000) as i16
    } else {
        offset.wrapping_add(increment)
    };
}
fn decay_shake(l: &mut Lakitu, axis: usize) {
    if !camera_approach_s16_symmetric_bool(&mut l.shake_magnitude[axis], 0, l.shake_decay[axis]) {
        l.shake_phase[axis] = 0;
    }
}
fn apply_angle_shake(l: &mut Lakitu, axis: usize, trig: &TrigTables) {
    let (dist, mut pitch, mut yaw) = get_dist_angle(l.pos, l.focus, trig);
    let delta = f32::from(l.shake_magnitude[axis]) * trig.sins(i32::from(l.shake_phase[axis]));
    if axis == 0 {
        pitch = add_float_angle(pitch, delta);
    } else {
        yaw = add_float_angle(yaw, delta);
    }
    l.focus = set_dist_and_angle(l.pos, dist, pitch, yaw, trig);
    increment_shake_offset(&mut l.shake_phase[axis], l.shake_velocity[axis]);
    decay_shake(l, axis);
}
