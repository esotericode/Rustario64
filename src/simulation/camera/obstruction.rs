//! Obstruction tests and the eight-probe wall rotation from pinned CC0 camera.c.
//! Query ordering, last-wall selection and source integer promotions matter.
use super::*;
use crate::simulation::collision::Surface;

pub fn calc_avoid_yaw(yaw_from_mario: i16, wall_yaw: i16) -> i16 {
    let difference = wall_yaw.wrapping_sub(yaw_from_mario).wrapping_add(0x4000);
    if difference < 0 {
        wall_yaw
    } else {
        wall_yaw.wrapping_add(i16::MIN)
    }
}

/// Source maxima are s16, although the differences are computed as promoted
/// integers and assigned through f32. Bounds use strict comparisons.
pub fn is_surf_within_bounding_box(surface: &Surface, bounds: [f32; 3]) -> bool {
    let vertices = [surface.vertex1, surface.vertex2, surface.vertex3];
    let mut maxima = [0i16; 3];
    for i in 0..3 {
        let j = (i + 1) % 3;
        for axis in 0..3 {
            let difference = (i32::from(vertices[i][axis]) - i32::from(vertices[j][axis])).abs();
            let difference = difference as f32;
            if difference > f32::from(maxima[axis]) {
                maxima[axis] = difference as i32 as i16;
            }
        }
    }
    (bounds[1] != -1.0 && f32::from(maxima[1]) < bounds[1])
        || (bounds[0] != -1.0
            && bounds[2] != -1.0
            && f32::from(maxima[0]) < bounds[0]
            && f32::from(maxima[2]) < bounds[2])
}

/// The reference recomputes the unnormalized normal from vertices, performing
/// integer products/subtractions before converting to f32. A normalized plane
/// query has different rounding at the boundary.
pub fn is_behind_surface(pos: [f32; 3], surface: &Surface) -> bool {
    let a = surface.vertex1.map(i32::from);
    let b = surface.vertex2.map(i32::from);
    let d = surface.vertex3.map(i32::from);
    let cross = |i: usize, j: usize| {
        (b[i] - a[i])
            .wrapping_mul(d[j] - b[j])
            .wrapping_sub((d[i] - b[i]).wrapping_mul(b[j] - a[j])) as f32
    };
    let nx = cross(1, 2);
    let ny = cross(2, 0);
    let nz = cross(0, 1);
    let dx = a[0] as f32 - pos[0];
    let dy = a[1] as f32 - pos[1];
    let dz = a[2] as f32 - pos[2];
    dx * nx + dy * ny + dz * nz < 0.0
}

/// A missing or excluded surface returns true, including the source's special
/// treatment of SURFACE_WALL_MISC in the obstruction scan.
pub fn is_range_behind_surface(
    from: [f32; 3],
    to: [f32; 3],
    surface: Option<&Surface>,
    range: i16,
    excluded_type: i16,
    trig: &TrigTables,
) -> bool {
    let Some(surface) = surface else {
        return true;
    };
    if excluded_type != -1 && surface.surface_type == excluded_type {
        return true;
    }
    if range == 0 {
        return is_behind_surface(to, surface);
    }
    let dist = calc_abs_dist(from, to);
    let [pitch, yaw] = calculate_angles(from, to, trig);
    let left = set_dist_and_angle(from, dist, pitch, yaw.wrapping_add(range), trig);
    let right = set_dist_and_angle(from, dist, pitch, yaw.wrapping_sub(range), trig);
    is_behind_surface(left, surface) && is_behind_surface(right, surface)
}

/// Returns 0 for clear, 1 for a wall near the eye, or 3 for an obstruction.
/// `avoid_yaw` is left unchanged if no wall qualifies. Only the near-wall bit
/// in `status_flags` changes. The original scan does not modify its endpoints.
pub fn rotate_camera_around_walls(
    mario: [f32; 3],
    pos: [f32; 3],
    avoid_yaw: &mut i16,
    yaw_range: i16,
    status_flags: &mut i16,
    world: &CollisionWorld,
    flags: CollisionFlags,
    pass_vanish_walls: bool,
    trig: &TrigTables,
) -> i32 {
    let yaw_from_mario = calculate_yaw(mario, pos, trig);
    *status_flags &= !c::CAM_FLAG_CAM_NEAR_WALL;
    let mut status = 0;
    let mut check_dist = 0.0;
    let mut coarse_radius = 150.0;
    let mut fine_radius = 100.0;
    for step in 0..8 {
        let check_pos = std::array::from_fn(|i| mario[i] + ((pos[i] - mario[i]) * check_dist));
        let mut data = WallCollisionData::new(check_pos, 100.0, coarse_radius);
        camera_approach_f32_symmetric_bool(&mut coarse_radius, 250.0, 30.0);
        // The collision query itself caps the coarse radius at 200, as in C.
        if world.find_wall_collisions(&mut data, flags, pass_vanish_walls) != 0 {
            if step >= 5 {
                *status_flags |= c::CAM_FLAG_CAM_NEAR_WALL;
                if status <= 0 {
                    status = 1;
                    let wall = &world.surfaces()
                        [usize::from(data.walls[data.num_walls as usize - 1].unwrap())];
                    let wall_yaw = trig
                        .atan2s(wall.normal[2], wall.normal[0])
                        .wrapping_add(0x4000);
                    *avoid_yaw = calc_avoid_yaw(yaw_from_mario, wall_yaw).wrapping_add(i16::MIN);
                }
            }
            // Re-query from the ORIGINAL probe, not the coarse query's push.
            data = WallCollisionData::new(check_pos, 100.0, fine_radius);
            // Fine radius grows only following a coarse hit.
            camera_approach_f32_symmetric_bool(&mut fine_radius, 200.0, 20.0);
            if world.find_wall_collisions(&mut data, flags, pass_vanish_walls) != 0 {
                let wall = &world.surfaces()
                    [usize::from(data.walls[data.num_walls as usize - 1].unwrap())];
                let normal_yaw = trig.atan2s(wall.normal[2], wall.normal[0]);
                let wall_yaw = normal_yaw.wrapping_add(0x4000);
                if !is_range_behind_surface(
                    mario,
                    pos,
                    Some(wall),
                    yaw_range,
                    c::SURFACE_WALL_MISC,
                    trig,
                ) && is_behind_surface(mario, wall)
                    && !is_surf_within_bounding_box(wall, [-1.0, 150.0, -1.0])
                {
                    *avoid_yaw = calc_avoid_yaw(yaw_from_mario, wall_yaw).wrapping_add(i16::MIN);
                    camera_approach_s16_symmetric_bool(avoid_yaw, normal_yaw, yaw_range);
                    status = 3;
                    break;
                }
            }
        }
        check_dist += 0.125;
    }
    status
}
