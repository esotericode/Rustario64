//! Reference-camera components translated from pinned CC0 SM64 camera.c.
//! See PROVENANCE.md. These are simulation operations, never frame-rate
//! smoothing. The complete mode dispatcher and Lakitu update are not ported;
//! the viewer still uses its explicitly labeled follow camera.
#![allow(clippy::manual_clamp, clippy::too_many_arguments)]

use super::{
    collision::{
        CELL_HEIGHT_LIMIT, CollisionFlags, CollisionWorld, FLOOR_LOWER_LIMIT, SurfaceIndex,
        WallCollisionData,
    },
    mario::constants as c,
    math::TrigTables,
};

pub const U_CBUTTONS: u16 = 0x0008;
pub const D_CBUTTONS: u16 = 0x0004;
pub const L_CBUTTONS: u16 = 0x0002;
pub const R_CBUTTONS: u16 = 0x0001;
pub const CBUTTON_MASK: u16 = U_CBUTTONS | D_CBUTTONS | L_CBUTTONS | R_CBUTTONS;

/// Original C-button precedence: new right/down presses win over left/up.
/// Unrelated bits already in `current` survive, as they do in camera.c.
pub fn find_c_buttons_pressed(mut current: u16, pressed: u16, down: u16) -> u16 {
    let pressed = pressed & CBUTTON_MASK;
    let down = down & CBUTTON_MASK;
    for (button, opposite) in [
        (L_CBUTTONS, R_CBUTTONS),
        (R_CBUTTONS, L_CBUTTONS),
        (U_CBUTTONS, D_CBUTTONS),
        (D_CBUTTONS, U_CBUTTONS),
    ] {
        if pressed & button != 0 {
            current |= button;
            current &= !opposite;
        }
        if down & button == 0 {
            current &= !button;
        }
    }
    current
}

/// Returns true while the value differs from its target (the source comment
/// describes the opposite). Only the pointer form clamps multipliers above 1.
pub fn approach_f32_asymptotic_bool(current: &mut f32, target: f32, mut multiplier: f32) -> bool {
    if multiplier > 1.0 {
        multiplier = 1.0;
    }
    *current = *current + (target - *current) * multiplier;
    *current != target
}

pub fn approach_f32_asymptotic(current: f32, target: f32, multiplier: f32) -> f32 {
    current + (target - current) * multiplier
}

pub fn set_or_approach_f32_asymptotic(
    current: &mut f32,
    target: f32,
    multiplier: f32,
    status: i16,
) -> bool {
    if status & c::CAM_FLAG_SMOOTH_MOVEMENT != 0 {
        approach_f32_asymptotic_bool(current, target, multiplier);
    } else {
        *current = target;
    }
    *current != target
}

pub fn approach_s16_asymptotic_bool(current: &mut i16, target: i16, divisor: i16) -> bool {
    let mut temp = *current;
    if divisor == 0 {
        *current = target;
    } else {
        temp = temp.wrapping_sub(target);
        // C promotes both operands before dividing: MIN / -1 is valid here.
        temp = (i32::from(temp) - i32::from(temp) / i32::from(divisor)) as i16;
        temp = temp.wrapping_add(target);
        *current = temp;
    }
    *current != target
}

pub fn approach_s16_asymptotic(mut current: i16, target: i16, divisor: i16) -> i16 {
    approach_s16_asymptotic_bool(&mut current, target, divisor);
    current
}

pub fn approach_vec3f_asymptotic(current: &mut [f32; 3], target: [f32; 3], mul: [f32; 3]) {
    for i in 0..3 {
        approach_f32_asymptotic_bool(&mut current[i], target[i], mul[i]);
    }
}

pub fn set_or_approach_vec3f_asymptotic(
    current: &mut [f32; 3],
    target: [f32; 3],
    mul: [f32; 3],
    status: i16,
) {
    for i in 0..3 {
        set_or_approach_f32_asymptotic(&mut current[i], target[i], mul[i], status);
    }
}

pub fn approach_vec3s_asymptotic(current: &mut [i16; 3], target: [i16; 3], divisor: [i16; 3]) {
    for i in 0..3 {
        approach_s16_asymptotic_bool(&mut current[i], target[i], divisor[i]);
    }
}

pub fn camera_approach_s16_symmetric_bool(current: &mut i16, target: i16, mut inc: i16) -> bool {
    let mut dist = target.wrapping_sub(*current);
    if inc < 0 {
        inc = inc.wrapping_neg();
    }
    if dist > 0 {
        dist = dist.wrapping_sub(inc);
        *current = if dist >= 0 {
            target.wrapping_sub(dist)
        } else {
            target
        };
    } else {
        dist = dist.wrapping_add(inc);
        *current = if dist <= 0 {
            target.wrapping_sub(dist)
        } else {
            target
        };
    }
    *current != target
}

pub fn camera_approach_s16_symmetric(mut current: i16, target: i16, inc: i16) -> i16 {
    camera_approach_s16_symmetric_bool(&mut current, target, inc);
    current
}

pub fn set_or_approach_s16_symmetric(
    current: &mut i16,
    target: i16,
    inc: i16,
    status: i16,
) -> bool {
    if status & c::CAM_FLAG_SMOOTH_MOVEMENT != 0 {
        camera_approach_s16_symmetric_bool(current, target, inc);
    } else {
        *current = target;
    }
    *current != target
}

pub fn camera_approach_f32_symmetric_bool(current: &mut f32, target: f32, mut inc: f32) -> bool {
    let mut dist = target - *current;
    if inc < 0.0 {
        inc = -inc;
    }
    if dist > 0.0 {
        dist -= inc;
        *current = if dist > 0.0 { target - dist } else { target };
    } else {
        dist += inc;
        *current = if dist < 0.0 { target - dist } else { target };
    }
    *current != target
}

pub fn camera_approach_f32_symmetric(mut current: f32, target: f32, inc: f32) -> f32 {
    camera_approach_f32_symmetric_bool(&mut current, target, inc);
    current
}

pub fn calculate_pitch(from: [f32; 3], to: [f32; 3], trig: &TrigTables) -> i16 {
    let [x, y, z] = std::array::from_fn(|i| to[i] - from[i]);
    trig.atan2s((x * x + z * z).sqrt(), y)
}

/// Direction from focus to eye, which the reference camera uses for movement.
pub fn calculate_yaw(from: [f32; 3], to: [f32; 3], trig: &TrigTables) -> i16 {
    trig.atan2s(to[2] - from[2], to[0] - from[0])
}

pub fn calculate_angles(from: [f32; 3], to: [f32; 3], trig: &TrigTables) -> [i16; 2] {
    [
        calculate_pitch(from, to, trig),
        calculate_yaw(from, to, trig),
    ]
}

pub fn calc_abs_dist(from: [f32; 3], to: [f32; 3]) -> f32 {
    let [x, y, z] = std::array::from_fn(|i| to[i] - from[i]);
    (x * x + y * y + z * z).sqrt()
}

pub fn calc_hor_dist(from: [f32; 3], to: [f32; 3]) -> f32 {
    let x = to[0] - from[0];
    let z = to[2] - from[2];
    (x * x + z * z).sqrt()
}

pub fn rotate_in_xz(src: [f32; 3], yaw: i16, trig: &TrigTables) -> [f32; 3] {
    [
        src[2] * trig.sins(i32::from(yaw)) + src[0] * trig.coss(i32::from(yaw)),
        src[1],
        src[2] * trig.coss(i32::from(yaw)) - src[0] * trig.sins(i32::from(yaw)),
    ]
}

/// The reference also flips the Z axis in this operation.
pub fn rotate_in_yz(src: [f32; 3], pitch: i16, trig: &TrigTables) -> [f32; 3] {
    [
        src[0],
        src[2] * trig.sins(i32::from(pitch)) + src[1] * trig.coss(i32::from(pitch)),
        -(src[2] * trig.coss(i32::from(pitch)) - src[1] * trig.sins(i32::from(pitch))),
    ]
}

pub fn scale_along_line(from: [f32; 3], to: [f32; 3], scale: f32) -> [f32; 3] {
    std::array::from_fn(|i| (to[i] - from[i]) * scale + from[i])
}

fn set_dist_and_angle(
    from: [f32; 3],
    dist: f32,
    pitch: i16,
    yaw: i16,
    trig: &TrigTables,
) -> [f32; 3] {
    let (pitch, yaw) = (i32::from(pitch), i32::from(yaw));
    [
        from[0] + dist * trig.coss(pitch) * trig.sins(yaw),
        from[1] + dist * trig.sins(pitch),
        from[2] + dist * trig.coss(pitch) * trig.coss(yaw),
    ]
}

pub fn clamp_pitch(
    from: [f32; 3],
    to: &mut [f32; 3],
    max: i16,
    min: i16,
    trig: &TrigTables,
) -> i32 {
    let dist = calc_abs_dist(from, *to);
    let [mut pitch, yaw] = calculate_angles(from, *to, trig);
    let mut out_of_range = 0;
    if pitch > max {
        pitch = max;
        out_of_range += 1;
    }
    if pitch < min {
        pitch = min;
        out_of_range += 1;
    }
    *to = set_dist_and_angle(from, dist, pitch, yaw, trig);
    out_of_range
}

/// Strict trigger bounds; points exactly on a face are outside.
pub fn is_pos_in_bounds(
    pos: [f32; 3],
    center: [f32; 3],
    bounds: [f32; 3],
    yaw: i16,
    trig: &TrigTables,
) -> bool {
    let rel = rotate_in_xz(std::array::from_fn(|i| center[i] - pos[i]), yaw, trig);
    (0..3).all(|i| -bounds[i] < rel[i] && rel[i] < bounds[i])
}

pub fn clamp_positions_and_find_yaw(
    pos: &mut [f32; 3],
    origin: [f32; 3],
    x_max: f32,
    x_min: f32,
    z_max: f32,
    z_min: f32,
    trig: &TrigTables,
) -> i16 {
    if pos[0] >= x_max {
        pos[0] = x_max;
    }
    if pos[0] <= x_min {
        pos[0] = x_min;
    }
    if pos[2] >= z_max {
        pos[2] = z_max;
    }
    if pos[2] <= z_min {
        pos[2] = z_min;
    }
    calculate_yaw(origin, *pos, trig)
}

pub fn find_in_bounds_yaw_wdw_bob_thi(
    area: i32,
    pos: &mut [f32; 3],
    origin: [f32; 3],
    yaw: i16,
    trig: &TrigTables,
) -> i16 {
    let bounds = match area {
        c::AREA_WDW_MAIN => [4508.0, -3739.0, 4508.0, -3739.0],
        c::AREA_BOB => [8000.0, -8000.0, 7050.0, -8000.0],
        c::AREA_THI_HUGE => [8192.0, -8192.0, 8192.0, -8192.0],
        c::AREA_THI_TINY => [2458.0, -2458.0, 2458.0, -2458.0],
        _ => return yaw,
    };
    clamp_positions_and_find_yaw(
        pos, origin, bounds[0], bounds[1], bounds[2], bounds[3], trig,
    )
}

/// Camera.c's correction deliberately reuses the LAST returned wall in each
/// iteration, and starts with the original position rather than the query's
/// pushed coordinates. Preserve that quirk.
pub fn collide_with_walls(
    pos: &mut [f32; 3],
    offset_y: f32,
    radius: f32,
    world: &CollisionWorld,
    flags: CollisionFlags,
    pass_vanish_walls: bool,
) -> i32 {
    let mut data = WallCollisionData::new(*pos, offset_y, radius);
    let count = world.find_wall_collisions(&mut data, flags, pass_vanish_walls);
    if count != 0 {
        for _ in 0..data.num_walls {
            let wall =
                &world.surfaces()[usize::from(data.walls[data.num_walls as usize - 1].unwrap())];
            let [nx, ny, nz] = wall.normal;
            let offset = nx * pos[0] + ny * pos[1] + nz * pos[2] + wall.origin_offset;
            let abs = if offset >= 0.0 { offset } else { -offset };
            if abs < radius {
                pos[0] += nx * (radius - offset);
                pos[2] += nz * (radius - offset);
            }
        }
    }
    count
}

/// Camera.c uses f32_find_wall_collision here, NOT collide_with_walls above.
/// Its unused lastGood argument is omitted; flags remain explicit and mutable.
pub fn resolve_geometry_collisions(
    pos: &mut [f32; 3],
    world: &CollisionWorld,
    flags: &mut CollisionFlags,
    pass_vanish_walls: bool,
) {
    let mut data = WallCollisionData::new(*pos, 0.0, 100.0);
    world.find_wall_collisions(&mut data, *flags, pass_vanish_walls);
    *pos = [data.x, data.y, data.z];
    let (mut floor, _) = world.find_floor(pos[0], pos[1] + 50.0, pos[2], flags);
    let (mut ceil, _) = world.find_ceil(pos[0], pos[1] - 50.0, pos[2], *flags);
    if floor != FLOOR_LOWER_LIMIT && ceil == CELL_HEIGHT_LIMIT {
        floor += 125.0;
        if pos[1] < floor {
            pos[1] = floor;
        }
    }
    if floor == FLOOR_LOWER_LIMIT && ceil != CELL_HEIGHT_LIMIT {
        ceil -= 125.0;
        if pos[1] > ceil {
            pos[1] = ceil;
        }
    }
    if floor != FLOOR_LOWER_LIMIT && ceil != CELL_HEIGHT_LIMIT {
        floor += 125.0;
        ceil -= 125.0;
        if pos[1] <= floor && pos[1] < ceil {
            pos[1] = floor;
        }
        if pos[1] > floor && pos[1] >= ceil {
            pos[1] = ceil;
        }
        if pos[1] <= floor && pos[1] >= ceil {
            pos[1] = (floor + ceil) * 0.5;
        }
    }
}

/// Current PlayerGeometry fields; the full camera update owns previous fields.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PlayerGeometry {
    pub floor: Option<SurfaceIndex>,
    pub floor_height: f32,
    pub floor_type: i16,
    pub ceil: Option<SurfaceIndex>,
    pub ceil_height: f32,
    pub ceil_type: i16,
    pub water_height: f32,
}

/// Types use camera-filtered queries, but heights and contacts use gameplay
/// queries. The original restores the incoming camera flag after all queries.
pub fn find_mario_floor_and_ceil(
    pos: [f32; 3],
    world: &CollisionWorld,
    flags: &mut CollisionFlags,
) -> PlayerGeometry {
    let saved = flags.checking_for_camera;
    flags.checking_for_camera = true;
    let (height, camera_floor) = world.find_floor(pos[0], pos[1] + 10.0, pos[2], flags);
    let floor_type = if height != FLOOR_LOWER_LIMIT {
        let i = camera_floor.expect(
            "reference camera dereferences a missing floor with a retained intangible height",
        );
        world.surfaces()[usize::from(i)].surface_type
    } else {
        0
    };
    let (height, camera_ceil) = world.find_ceil(pos[0], pos[1] - 10.0, pos[2], *flags);
    let ceil_type = if height != CELL_HEIGHT_LIMIT {
        let i = camera_ceil.expect("reference camera dereferences a missing ceiling");
        world.surfaces()[usize::from(i)].surface_type
    } else {
        0
    };
    flags.checking_for_camera = false;
    let (floor_height, floor) = world.find_floor(pos[0], pos[1] + 10.0, pos[2], flags);
    let (ceil_height, ceil) = world.find_ceil(pos[0], pos[1] - 10.0, pos[2], *flags);
    let water_height = world.find_water_level(pos[0], pos[2]);
    flags.checking_for_camera = saved;
    PlayerGeometry {
        floor,
        floor_height,
        floor_type,
        ceil,
        ceil_height,
        ceil_type,
        water_height,
    }
}

pub fn look_down_slopes(
    pos: [f32; 3],
    yaw: i16,
    world: &CollisionWorld,
    flags: &mut CollisionFlags,
    trig: &TrigTables,
) -> i16 {
    let mut pitch: i16 = 0x05b0;
    let x = pos[0] + trig.sins(i32::from(yaw)) * 40.0;
    let z = pos[2] + trig.coss(i32::from(yaw)) * 40.0;
    let (height, floor) = world.find_floor(x, pos[1], z, flags);
    let dy = height - pos[1];
    if let Some(i) = floor {
        let floor = &world.surfaces()[usize::from(i)];
        if floor.surface_type != c::SURFACE_WALL_MISC
            && dy > 0.0
            && !(floor.normal[2] == 0.0 && dy < 100.0)
        {
            pitch = pitch.wrapping_add(trig.atan2s(40.0, dy));
        }
    }
    pitch
}

pub fn calc_y_to_curr_floor(
    pos: [f32; 3],
    action: u32,
    geometry: PlayerGeometry,
    pos_mul: f32,
    pos_bound: f32,
    foc_mul: f32,
    foc_bound: f32,
    world: &CollisionWorld,
) -> [f32; 2] {
    assert!(
        action & c::ACT_FLAG_ON_POLE == 0,
        "camera floor offsets on a pole require the object runtime"
    );
    let mut floor = geometry.floor_height;
    if action & c::ACT_FLAG_METAL_WATER == 0 {
        // The original queries water again rather than reading water_height.
        let water = world.find_water_level(pos[0], pos[2]);
        if floor < water {
            floor = water;
        }
    }
    let mut pos_off = (floor - pos[1]) * pos_mul;
    if pos_off > pos_bound {
        pos_off = pos_bound;
    }
    if pos_off < -pos_bound {
        pos_off = -pos_bound;
    }
    let mut foc_off = (floor - pos[1]) * foc_mul;
    if foc_off > foc_bound {
        foc_off = foc_bound;
    }
    if foc_off < -foc_bound {
        foc_off = -foc_bound;
    }
    [pos_off, foc_off]
}

pub fn focus_on_mario(
    mario: [f32; 3],
    pos_off: f32,
    foc_off: f32,
    dist: f32,
    pitch: i16,
    yaw: i16,
    lakitu_pitch: i16,
    trig: &TrigTables,
) -> ([f32; 3], [f32; 3]) {
    let origin = [mario[0], mario[1] + pos_off, mario[2]];
    let pos = set_dist_and_angle(origin, dist, pitch.wrapping_add(lakitu_pitch), yaw, trig);
    let focus = [mario[0], mario[1] + foc_off, mario[2]];
    (focus, pos)
}

/// Globals read by update_radial_camera, made explicit. This is goal
/// construction only, not mode_radial_camera or update_lakitu.
#[derive(Debug, Default, Clone, Copy)]
pub struct RadialState {
    pub area: i32,
    pub center_x: f32,
    pub center_z: f32,
    pub mode_offset_yaw: i16,
    pub lakitu_dist: i16,
    pub lakitu_pitch: i16,
    pub area_yaw: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialGoal {
    pub focus: [f32; 3],
    pub pos: [f32; 3],
    pub yaw: i16,
}

pub fn update_radial_camera(
    state: &mut RadialState,
    mario: [f32; 3],
    action: u32,
    geometry: PlayerGeometry,
    world: &CollisionWorld,
    flags: &mut CollisionFlags,
    trig: &TrigTables,
) -> RadialGoal {
    let dx = mario[0] - state.center_x;
    let dz = mario[2] - state.center_z;
    let mut yaw = trig.atan2s(dz, dx).wrapping_add(state.mode_offset_yaw);
    let pitch = look_down_slopes(mario, yaw, world, flags, trig);
    state.area_yaw = yaw.wrapping_sub(state.mode_offset_yaw);
    let [pos_y, foc_y] =
        calc_y_to_curr_floor(mario, action, geometry, 1.0, 200.0, 0.9, 200.0, world);
    let (focus, mut pos) = focus_on_mario(
        mario,
        pos_y + 125.0,
        foc_y + 125.0,
        f32::from(state.lakitu_dist) + 1000.0,
        pitch,
        yaw,
        state.lakitu_pitch,
        trig,
    );
    yaw = find_in_bounds_yaw_wdw_bob_thi(state.area, &mut pos, focus, yaw, trig);
    RadialGoal { focus, pos, yaw }
}
