//! Radial rotation and zoom from pinned CC0 camera.c. This is movement inside
//! a mode, not mode input/height/pan, initialization or update_camera dispatch.
use super::{lakitu::Rig, obstruction::rotate_camera_around_walls, *};

const fn degrees(value: i32) -> i16 {
    (value * 0x10000 / 360) as i16
}

/// Area center and the second-press rotation flags are mode-controller state.
/// Shared camera globals stay in Rig, rather than being copied between stages.
#[derive(Debug, Default, Clone, Copy)]
pub struct RadialMovement {
    pub area: i32,
    pub center: [f32; 2],
    pub second_rotate: u16,
}

impl RadialMovement {
    pub fn outward_offset(
        &self,
        rig: &Rig,
        mario: [f32; 3],
        forward_vel: f32,
        area_yaw: i16,
    ) -> i16 {
        let mut yaw_goal = degrees(60);
        let mut yaw = rig.mode_offset_yaw;
        match self.area {
            c::AREA_TTC => {
                let center = [self.center[0], mario[1], self.center[1]];
                if 800.0 > calc_abs_dist(center, mario) {
                    yaw_goal = 0x3800;
                }
            }
            c::AREA_SSL_PYRAMID => {
                yaw_goal = ((i32::from(area_yaw) & 0xc000) - i32::from(area_yaw)
                    + i32::from(degrees(45))) as i16;
                if yaw_goal < 0 {
                    yaw_goal = yaw_goal.wrapping_neg();
                }
                yaw_goal = (i32::from(yaw_goal) / 32 * 48) as i16;
            }
            c::AREA_LLL_OUTSIDE => yaw_goal = 0,
            _ => {}
        }
        let d_yaw = (forward_vel / 32.0 * 128.0) as i32 as i16;
        if rig.area_yaw_change < 0 {
            camera_approach_s16_symmetric_bool(&mut yaw, yaw_goal.wrapping_neg(), d_yaw);
        }
        if rig.area_yaw_change > 0 {
            camera_approach_s16_symmetric_bool(&mut yaw, yaw_goal, d_yaw);
        }
        if yaw < -degrees(60) {
            camera_approach_s16_symmetric_bool(&mut yaw, yaw_goal.wrapping_neg(), 0x200);
        }
        if yaw > degrees(60) {
            camera_approach_s16_symmetric_bool(&mut yaw, yaw_goal, 0x200);
        }
        yaw
    }

    pub fn move_camera(
        &mut self,
        rig: &mut Rig,
        mario: [f32; 3],
        forward_vel: f32,
        curr_floor_type: i16,
        prev_floor_type: i16,
        world: &CollisionWorld,
        flags: CollisionFlags,
        trig: &TrigTables,
    ) {
        let max_yaw = degrees(60);
        let min_yaw = degrees(-60);
        let mut rotate_speed = 0x1000;
        let mut dx = mario[0] - self.center[0];
        let mut dz = mario[2] - self.center[1];
        let mut yaw_offset =
            calculate_yaw(mario, rig.camera.pos, trig).wrapping_sub(trig.atan2s(dz, dx));
        if yaw_offset > max_yaw {
            yaw_offset = max_yaw;
        }
        if yaw_offset < min_yaw {
            yaw_offset = min_yaw;
        }
        if rig.movement & c::CAM_MOVE_ROTATE == 0 {
            for (surface, movement) in [
                (c::SURFACE_CAMERA_MIDDLE, c::CAM_MOVE_RETURN_TO_MIDDLE),
                (c::SURFACE_CAMERA_ROTATE_RIGHT, c::CAM_MOVE_ROTATE_RIGHT),
                (c::SURFACE_CAMERA_ROTATE_LEFT, c::CAM_MOVE_ROTATE_LEFT),
            ] {
                if curr_floor_type == surface && prev_floor_type != surface {
                    rig.movement |= movement | c::CAM_MOVE_ENTERED_ROTATE_SURFACE;
                }
            }
        }
        if rig.movement & c::CAM_MOVE_ENTERED_ROTATE_SURFACE != 0 {
            rotate_speed = 0x200;
        }
        if i16::from(rig.camera.mode) == c::CAMERA_MODE_OUTWARD_RADIAL {
            dx = -dx;
            dz = -dz;
        }
        let mut avoid_yaw = 0; // Native only reads this when the scan writes it.
        let avoid_status = rotate_camera_around_walls(
            mario,
            rig.camera.pos,
            &mut avoid_yaw,
            0x400,
            &mut rig.status,
            world,
            flags,
            false,
            trig,
        );
        if avoid_status == 3 {
            // This condition is promoted s32 arithmetic, NOT a wrapped s16.
            if i32::from(avoid_yaw) - i32::from(trig.atan2s(dz, dx)) + 0x4000 < 0 {
                avoid_yaw = avoid_yaw.wrapping_add(i16::MIN);
            }
            avoid_yaw = avoid_yaw.wrapping_sub(trig.atan2s(dz, dx));
            if avoid_yaw > degrees(105) {
                avoid_yaw = degrees(105);
            }
            if avoid_yaw < degrees(-105) {
                avoid_yaw = degrees(-105);
            }
        }
        if rig.movement & c::CAM_MOVE_RETURN_TO_MIDDLE != 0 {
            if !camera_approach_s16_symmetric_bool(&mut rig.mode_offset_yaw, 0, rotate_speed) {
                rig.movement &= !c::CAM_MOVE_RETURN_TO_MIDDLE;
            }
        } else {
            if rig.movement & c::CAM_MOVE_ROTATE_RIGHT != 0
                && avoid_status == 3
                && i32::from(avoid_yaw) + 0x10 < i32::from(rig.mode_offset_yaw)
            {
                rig.mode_offset_yaw = avoid_yaw;
                rig.movement &= !(c::CAM_MOVE_ROTATE_RIGHT | c::CAM_MOVE_ENTERED_ROTATE_SURFACE);
            }
            if rig.movement & c::CAM_MOVE_ROTATE_LEFT != 0
                && avoid_status == 3
                && i32::from(avoid_yaw) - 0x10 > i32::from(rig.mode_offset_yaw)
            {
                rig.mode_offset_yaw = avoid_yaw;
                rig.movement &= !(c::CAM_MOVE_ROTATE_LEFT | c::CAM_MOVE_ENTERED_ROTATE_SURFACE);
            }
            // Preserve source order: right first, then left, first rotations
            // before second rotations. Conflicting flags are not sanitized.
            for (movement, target) in [
                (c::CAM_MOVE_ROTATE_RIGHT, max_yaw),
                (c::CAM_MOVE_ROTATE_LEFT, min_yaw),
            ] {
                if self.second_rotate & movement == 0
                    && rig.movement & movement != 0
                    && !camera_approach_s16_symmetric_bool(
                        &mut rig.mode_offset_yaw,
                        target,
                        rotate_speed,
                    )
                {
                    rig.movement &= !(movement | c::CAM_MOVE_ENTERED_ROTATE_SURFACE);
                }
            }
            for (movement, target) in [
                (c::CAM_MOVE_ROTATE_RIGHT, degrees(105)),
                (c::CAM_MOVE_ROTATE_LEFT, degrees(-105)),
            ] {
                if self.second_rotate & movement != 0
                    && rig.movement & movement != 0
                    && !camera_approach_s16_symmetric_bool(
                        &mut rig.mode_offset_yaw,
                        target,
                        rotate_speed,
                    )
                {
                    rig.movement &= !(movement | c::CAM_MOVE_ENTERED_ROTATE_SURFACE);
                    self.second_rotate &= !movement;
                }
            }
        }
        if rig.movement & c::CAM_MOVE_ROTATE == 0 {
            if avoid_status == 3 {
                approach_s16_asymptotic_bool(&mut rig.mode_offset_yaw, avoid_yaw, 10);
            } else {
                if i16::from(rig.camera.mode) == c::CAMERA_MODE_RADIAL {
                    rotate_speed = (forward_vel / 32.0 * 128.0) as i32 as i16;
                    camera_approach_s16_symmetric_bool(
                        &mut rig.mode_offset_yaw,
                        yaw_offset,
                        rotate_speed,
                    );
                }
                if i16::from(rig.camera.mode) == c::CAMERA_MODE_OUTWARD_RADIAL {
                    rig.mode_offset_yaw =
                        self.outward_offset(rig, mario, forward_vel, trig.atan2s(dz, dx));
                }
            }
        }
        if rig.mode_offset_yaw > 0x5554 {
            rig.mode_offset_yaw = 0x5554;
        }
        if rig.mode_offset_yaw < -0x5554 {
            rig.mode_offset_yaw = -0x5554;
        }
    }

    pub fn zoom(&self, rig: &mut Rig, range_dist: f32, mut range_pitch: i16) {
        if rig.lakitu_dist < 0 {
            rig.lakitu_dist = rig.lakitu_dist.wrapping_add(30);
            if rig.lakitu_dist > 0 {
                rig.lakitu_dist = 0;
            }
        } else if range_dist < f32::from(rig.lakitu_dist) {
            rig.lakitu_dist = rig.lakitu_dist.wrapping_sub(30);
            if f32::from(rig.lakitu_dist) < range_dist {
                rig.lakitu_dist = range_dist as i32 as i16;
            }
        } else if rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
            rig.lakitu_dist = rig.lakitu_dist.wrapping_add(30);
            if f32::from(rig.lakitu_dist) > range_dist {
                rig.lakitu_dist = range_dist as i32 as i16;
            }
        } else {
            rig.lakitu_dist = rig.lakitu_dist.wrapping_sub(30);
            if rig.lakitu_dist < 0 {
                rig.lakitu_dist = 0;
            }
        }
        if self.area == c::AREA_SSL_PYRAMID
            && i16::from(rig.camera.mode) == c::CAMERA_MODE_OUTWARD_RADIAL
        {
            range_pitch = (i32::from(range_pitch) / 2) as i16;
        }
        if rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
            rig.lakitu_pitch = rig
                .lakitu_pitch
                .wrapping_add((i32::from(range_pitch) / 13) as i16);
            if rig.lakitu_pitch > range_pitch {
                rig.lakitu_pitch = range_pitch;
            }
        } else {
            rig.lakitu_pitch = rig
                .lakitu_pitch
                .wrapping_sub((i32::from(range_pitch) / 13) as i16);
            if rig.lakitu_pitch < 0 {
                rig.lakitu_pitch = 0;
            }
        }
    }
}
