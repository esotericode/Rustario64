//! The standard object movement family, translated from pinned CC0
//! src/game/object_helpers.c. King Bob-omb uses this rather than object_step.
//! Floor/wall queries, drag, ground/air/water flags and partial updates keep
//! the original widths and operation order. These helpers alone enable no actor.
use super::Object;
use crate::simulation::{
    collision::{SurfaceIndex, WallCollisionData},
    mario::{StepWorld, constants::*, f32_to_s16},
    math::TrigTables,
};

/// abs_angle_diff: the half-turn maps to 0x7FFF rather than overflowing.
pub fn abs_angle_diff(x0: i16, x1: i16) -> i16 {
    let diff = x1.wrapping_sub(x0);
    if diff == i16::MIN {
        i16::MAX
    } else {
        diff.abs()
    }
}

/// cur_obj_update_floor_height_and_get_floor.
pub fn cur_obj_update_floor_height_and_get_floor(
    o: &mut Object,
    w: &mut StepWorld<'_>,
) -> Option<SurfaceIndex> {
    let [x, y, z] = o.pos();
    let (height, floor) = w.collision.find_floor(x, y, z, &mut w.collision_flags);
    o.raw.set_f32(O_FLOOR_HEIGHT, height);
    floor
}

/// apply_drag_to_value. The squared velocity rounds to f32 before the
/// original long-double literal's multiplication and final f32 assignment.
fn apply_drag_to_value(value: f32, drag: f32) -> f32 {
    if value == 0.0 {
        return value;
    }
    let decel = (f64::from(value * value) * (f64::from(drag) * 0.0001)) as f32;
    if value > 0.0 {
        let value = value - decel;
        if f64::from(value) < 0.001 { 0.0 } else { value }
    } else {
        let value = value + decel;
        if f64::from(value) > -0.001 {
            0.0
        } else {
            value
        }
    }
}

/// cur_obj_apply_drag_xz, including the original large-speed overshoot.
pub fn cur_obj_apply_drag_xz(o: &mut Object, drag: f32) {
    for field in [O_VEL_X, O_VEL_Z] {
        o.raw
            .set_f32(field, apply_drag_to_value(o.raw.f32(field), drag));
    }
}

/// cur_obj_compute_vel_xz; does not move the object.
pub fn cur_obj_compute_vel_xz(o: &mut Object, trig: &TrigTables) {
    let yaw = o.raw.s32(O_MOVE_ANGLE_YAW);
    o.raw
        .set_f32(O_VEL_X, o.raw.f32(O_FORWARD_VEL) * trig.sins(yaw));
    o.raw
        .set_f32(O_VEL_Z, o.raw.f32(O_FORWARD_VEL) * trig.coss(yaw));
}

fn clear_move_flag(o: &mut Object, flag: u32) -> bool {
    let set = o.raw.u32(O_MOVE_FLAGS) & flag != 0;
    if set {
        o.raw.set_u32(O_MOVE_FLAGS, o.raw.u32(O_MOVE_FLAGS) & !flag);
    }
    set
}

fn add_move_flag(o: &mut Object, flag: u32) {
    o.raw.set_u32(O_MOVE_FLAGS, o.raw.u32(O_MOVE_FLAGS) | flag);
}

fn cur_obj_move_xz(o: &mut Object, w: &mut StepWorld<'_>, steep_y: f32, care: bool) -> bool {
    let x = o.raw.f32(O_POS_X) + o.raw.f32(O_VEL_X);
    let z = o.raw.f32(O_POS_Z) + o.raw.f32(O_VEL_Z);
    let (height, floor) = w
        .collision
        .find_floor(x, o.raw.f32(O_POS_Y), z, &mut w.collision_flags);
    let delta = height - o.raw.f32(O_FLOOR_HEIGHT);
    clear_move_flag(o, OBJ_MOVE_HIT_EDGE);
    if o.raw.s32(O_ROOM) != -1
        && let Some(floor) = floor
    {
        let room = w.collision.surface(floor).room;
        if room != 0 && o.raw.s32(O_ROOM) != i32::from(room) && room != 18 {
            return false;
        }
    }
    if height < FLOOR_LOWER_LIMIT_MISC as f32 {
        add_move_flag(o, OBJ_MOVE_HIT_EDGE);
        return false;
    }
    if delta < 5.0 {
        if !care {
            o.raw.set_f32(O_POS_X, x);
            o.raw.set_f32(O_POS_Z, z);
            return true;
        }
        if delta < -50.0 && o.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_ON_GROUND != 0 {
            add_move_flag(o, OBJ_MOVE_HIT_EDGE);
            return false;
        }
        let floor = floor.expect("standard movement dereferences a NULL intended floor");
        if w.collision.surface(floor).normal[1] > steep_y {
            o.raw.set_f32(O_POS_X, x);
            o.raw.set_f32(O_POS_Z, z);
            return true;
        }
        add_move_flag(o, OBJ_MOVE_HIT_EDGE);
        return false;
    }
    let floor = floor.expect("standard movement dereferences a NULL intended floor");
    if w.collision.surface(floor).normal[1] > steep_y || o.raw.f32(O_POS_Y) > height {
        o.raw.set_f32(O_POS_X, x);
        o.raw.set_f32(O_POS_Z, z);
        // The source moves but returns FALSE here; callers ignore the result.
    }
    false
}

fn cur_obj_move_update_ground_air_flags(o: &mut Object, bounciness: f32) {
    clear_move_flag(o, OBJ_MOVE_BOUNCE);
    if o.raw.f32(O_POS_Y) < o.raw.f32(O_FLOOR_HEIGHT) {
        if o.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_ON_GROUND == 0 {
            if clear_move_flag(o, OBJ_MOVE_LANDED) {
                add_move_flag(o, OBJ_MOVE_ON_GROUND);
            } else {
                add_move_flag(o, OBJ_MOVE_LANDED);
            }
        }
        o.raw.set_f32(O_POS_Y, o.raw.f32(O_FLOOR_HEIGHT));
        if o.raw.f32(O_VEL_Y) < 0.0 {
            o.raw.set_f32(O_VEL_Y, o.raw.f32(O_VEL_Y) * bounciness);
        }
        if o.raw.f32(O_VEL_Y) > 5.0 {
            add_move_flag(o, OBJ_MOVE_BOUNCE);
        }
    } else {
        clear_move_flag(o, OBJ_MOVE_LANDED);
        if clear_move_flag(o, OBJ_MOVE_ON_GROUND) {
            add_move_flag(o, OBJ_MOVE_LEFT_GROUND);
        }
    }
    clear_move_flag(o, OBJ_MOVE_MASK_IN_WATER);
}

fn cur_obj_move_update_underwater_flags(o: &mut Object) {
    let vy = o.raw.f32(O_VEL_Y);
    let decel = (f64::from((vy * vy).sqrt() * (o.raw.f32(O_DRAG_STRENGTH) * 7.0)) / 100.0) as f32;
    o.raw
        .set_f32(O_VEL_Y, if vy > 0.0 { vy - decel } else { vy + decel });
    if o.raw.f32(O_POS_Y) < o.raw.f32(O_FLOOR_HEIGHT) {
        o.raw.set_f32(O_POS_Y, o.raw.f32(O_FLOOR_HEIGHT));
        add_move_flag(o, OBJ_MOVE_UNDERWATER_ON_GROUND);
    } else {
        add_move_flag(o, OBJ_MOVE_UNDERWATER_OFF_GROUND);
    }
}

fn cur_obj_move_y_and_get_water_level(
    o: &mut Object,
    w: &StepWorld<'_>,
    gravity: f32,
    buoyancy: f32,
) -> f32 {
    let vy = o.raw.f32(O_VEL_Y) + (gravity + buoyancy);
    let vy = if vy < -78.0 { -78.0 } else { vy };
    o.raw.set_f32(O_VEL_Y, vy);
    o.raw.set_f32(O_POS_Y, o.raw.f32(O_POS_Y) + vy);
    if o.active_flags & ACTIVE_FLAG_UNK10 != 0 {
        FLOOR_LOWER_LIMIT as f32
    } else {
        w.collision
            .find_water_level(o.raw.f32(O_POS_X), o.raw.f32(O_POS_Z))
    }
}

/// cur_obj_move_y: landing/ground/leaving-ground flags and the complete
/// water transition rules, including entering water through a floor.
pub fn cur_obj_move_y(
    o: &mut Object,
    w: &StepWorld<'_>,
    gravity: f32,
    bounciness: f32,
    buoyancy: f32,
) {
    clear_move_flag(o, OBJ_MOVE_LEFT_GROUND);
    if o.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_AT_WATER_SURFACE != 0 && o.raw.f32(O_VEL_Y) > 5.0 {
        clear_move_flag(o, OBJ_MOVE_MASK_IN_WATER);
        add_move_flag(o, OBJ_MOVE_LEAVING_WATER);
    }
    if o.raw.u32(O_MOVE_FLAGS) & OBJ_MOVE_MASK_IN_WATER == 0 {
        let water = cur_obj_move_y_and_get_water_level(o, w, gravity, 0.0);
        if o.raw.f32(O_POS_Y) > water {
            cur_obj_move_update_ground_air_flags(o, bounciness);
        } else {
            add_move_flag(o, OBJ_MOVE_ENTERED_WATER);
            clear_move_flag(o, OBJ_MOVE_MASK_ON_GROUND);
        }
    } else {
        clear_move_flag(o, OBJ_MOVE_ENTERED_WATER);
        let water = cur_obj_move_y_and_get_water_level(o, w, gravity, buoyancy);
        if o.raw.f32(O_POS_Y) < water {
            cur_obj_move_update_underwater_flags(o);
        } else if o.raw.f32(O_POS_Y) < o.raw.f32(O_FLOOR_HEIGHT) {
            o.raw.set_f32(O_POS_Y, o.raw.f32(O_FLOOR_HEIGHT));
            clear_move_flag(o, OBJ_MOVE_MASK_IN_WATER);
        } else {
            o.raw.set_f32(O_POS_Y, water);
            o.raw.set_f32(O_VEL_Y, 0.0);
            clear_move_flag(
                o,
                OBJ_MOVE_UNDERWATER_OFF_GROUND | OBJ_MOVE_UNDERWATER_ON_GROUND,
            );
            add_move_flag(o, OBJ_MOVE_AT_WATER_SURFACE);
        }
    }
    if o.raw.u32(O_MOVE_FLAGS)
        & (OBJ_MOVE_MASK_ON_GROUND | OBJ_MOVE_AT_WATER_SURFACE | OBJ_MOVE_UNDERWATER_OFF_GROUND)
        != 0
    {
        clear_move_flag(o, OBJ_MOVE_IN_AIR);
    } else {
        add_move_flag(o, OBJ_MOVE_IN_AIR);
    }
}

/// cur_obj_resolve_wall_collisions: coordinates truncate to signed
/// halfwords before querying; the last wall and a strict 90-degree test win.
pub fn cur_obj_resolve_wall_collisions(o: &mut Object, w: &StepWorld<'_>) -> bool {
    let radius = o.raw.f32(O_WALL_HITBOX_RADIUS);
    if f64::from(radius) > 0.1 {
        let mut data =
            WallCollisionData::new(o.pos().map(|v| f32::from(f32_to_s16(v))), 10.0, radius);
        if w.collision.find_wall_collisions(
            &mut data,
            w.collision_flags,
            o.active_flags & ACTIVE_FLAG_MOVE_THROUGH_GRATE != 0,
        ) != 0
        {
            o.raw.set_f32(O_POS_X, data.x);
            o.raw.set_f32(O_POS_Y, data.y);
            o.raw.set_f32(O_POS_Z, data.z);
            let normal = w
                .collision
                .surface(data.walls[usize::try_from(data.num_walls).unwrap() - 1].unwrap())
                .normal;
            let angle = w.trig.atan2s(normal[2], normal[0]);
            o.raw.set_s32(O_WALL_ANGLE, i32::from(angle));
            return abs_angle_diff(angle, o.raw.s32(O_MOVE_ANGLE_YAW) as i16) > 0x4000;
        }
    }
    false
}

fn cur_obj_detect_steep_floor(o: &mut Object, w: &mut StepWorld<'_>, degrees: i16) -> i32 {
    let steep_y = w
        .trig
        .coss(i32::from((i32::from(degrees) * (0x10000 / 360)) as i16));
    if o.raw.f32(O_FORWARD_VEL) != 0.0 {
        let x = o.raw.f32(O_POS_X) + o.raw.f32(O_VEL_X);
        let z = o.raw.f32(O_POS_Z) + o.raw.f32(O_VEL_Z);
        let (height, floor) =
            w.collision
                .find_floor(x, o.raw.f32(O_POS_Y), z, &mut w.collision_flags);
        let delta = height - o.raw.f32(O_FLOOR_HEIGHT);
        if height < FLOOR_LOWER_LIMIT_MISC as f32 {
            o.raw.set_s32(
                O_WALL_ANGLE,
                o.raw.s32(O_MOVE_ANGLE_YAW).wrapping_add(0x8000),
            );
            return 2;
        }
        let floor = floor.expect("steep-floor detection dereferences a NULL intended floor");
        let normal = w.collision.surface(floor).normal;
        if normal[1] < steep_y && delta > 0.0 && height > o.raw.f32(O_POS_Y) {
            o.raw
                .set_s32(O_WALL_ANGLE, i32::from(w.trig.atan2s(normal[2], normal[0])));
            return 1;
        }
    }
    0
}

fn cur_obj_update_floor(o: &mut Object, w: &mut StepWorld<'_>) {
    let floor = cur_obj_update_floor_height_and_get_floor(o, w);
    o.set_floor(floor);
    let (floor_type, room) = floor.map_or((0, 0), |index| {
        let surface = w.collision.surface(index);
        (surface.surface_type, i16::from(surface.room))
    });
    if floor_type == SURFACE_BURNING {
        add_move_flag(o, OBJ_MOVE_ABOVE_LAVA);
    } else if floor_type == SURFACE_DEATH_PLANE {
        add_move_flag(o, OBJ_MOVE_ABOVE_DEATH_BARRIER);
    }
    o.raw.set_s16(O_FLOOR_TYPE, 0, floor_type);
    o.raw.set_s16(O_FLOOR_ROOM, 1, room);
}

/// cur_obj_update_floor_and_walls, with the original 60-degree probe.
/// Far-away/in-different-room objects refresh their floor but skip wall
/// collision and clear water flags, preserving the source's partial update.
pub fn cur_obj_update_floor_and_walls(o: &mut Object, w: &mut StepWorld<'_>) {
    clear_move_flag(o, OBJ_MOVE_ABOVE_LAVA | OBJ_MOVE_ABOVE_DEATH_BARRIER);
    if o.active_flags & (ACTIVE_FLAG_FAR_AWAY | ACTIVE_FLAG_IN_DIFFERENT_ROOM) != 0 {
        cur_obj_update_floor(o, w);
        clear_move_flag(o, OBJ_MOVE_HIT_WALL | OBJ_MOVE_MASK_IN_WATER);
        if o.raw.f32(O_POS_Y) > o.raw.f32(O_FLOOR_HEIGHT) {
            add_move_flag(o, OBJ_MOVE_IN_AIR);
        }
    } else {
        clear_move_flag(o, OBJ_MOVE_HIT_WALL);
        if cur_obj_resolve_wall_collisions(o, w) {
            add_move_flag(o, OBJ_MOVE_HIT_WALL);
        }
        cur_obj_update_floor(o, w);
        if o.raw.f32(O_POS_Y) > o.raw.f32(O_FLOOR_HEIGHT) {
            add_move_flag(o, OBJ_MOVE_IN_AIR);
        }
        if cur_obj_detect_steep_floor(o, w, 60) != 0 {
            add_move_flag(o, OBJ_MOVE_HIT_WALL);
        }
    }
}

/// cur_obj_move_standard. Negative degrees enable edge/slope avoidance.
/// Backward speeds keep their sign after drag; deactivated objects do not move.
pub fn cur_obj_move_standard(o: &mut Object, w: &mut StepWorld<'_>, degrees: i16) {
    if o.active_flags & (ACTIVE_FLAG_FAR_AWAY | ACTIVE_FLAG_IN_DIFFERENT_ROOM) == 0 {
        let care = degrees < 0;
        let degrees = if care {
            degrees.wrapping_neg()
        } else {
            degrees
        };
        let steep_y = w.trig.coss(i32::from(degrees) * (0x10000 / 360));
        cur_obj_compute_vel_xz(o, w.trig);
        cur_obj_apply_drag_xz(o, o.raw.f32(O_DRAG_STRENGTH));
        cur_obj_move_xz(o, w, steep_y, care);
        cur_obj_move_y(
            o,
            w,
            o.raw.f32(O_GRAVITY),
            o.raw.f32(O_BOUNCINESS),
            o.raw.f32(O_BUOYANCY),
        );
        let negative = o.raw.f32(O_FORWARD_VEL) < 0.0;
        let vx = o.raw.f32(O_VEL_X);
        let vz = o.raw.f32(O_VEL_Z);
        let speed = (vx * vx + vz * vz).sqrt();
        o.raw
            .set_f32(O_FORWARD_VEL, if negative { -speed } else { speed });
    }
}

/// cur_obj_move_using_fvel_and_gravity: King Bob-omb's return-home arc.
/// Inclusive 12,000-unit bounds are checked before motion; no terminal speed.
pub fn cur_obj_move_using_fvel_and_gravity(o: &mut Object, trig: &TrigTables) {
    cur_obj_compute_vel_xz(o, trig);
    if o.pos().iter().any(|v| *v < -12000.0 || 12000.0 < *v) {
        return;
    }
    o.raw
        .set_f32(O_POS_X, o.raw.f32(O_POS_X) + o.raw.f32(O_VEL_X));
    o.raw
        .set_f32(O_POS_Z, o.raw.f32(O_POS_Z) + o.raw.f32(O_VEL_Z));
    o.raw
        .set_f32(O_VEL_Y, o.raw.f32(O_VEL_Y) + o.raw.f32(O_GRAVITY));
    o.raw
        .set_f32(O_POS_Y, o.raw.f32(O_POS_Y) + o.raw.f32(O_VEL_Y));
}
