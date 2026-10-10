//! The enemy `object_step` family, translated from pinned CC0
//! src/game/obj_behaviors.c. Bob-ombs use this, rather than Mario's steps or
//! King Bob-omb's separate `cur_obj_move_standard` family.
//!
//! This component changes the object's original raw fields and collision-query
//! flags. It returns sObjFloor, a newly allocated terrain matrix and ordered
//! splash requests to the caller. Actor integration must install that matrix
//! and execute the requests through the object pool; water particle behaviors
//! and enemy animation/interaction are still unported. No actor is enabled by
//! this module alone. Existing throw matrices are not cleared by a step.
//!
//! Integer casts cover finite positions/velocities and yaw additions within
//! s32 range; original N64 overflow exceptions are outside this boundary.
use super::Object;
use crate::simulation::{
    collision::{SurfaceIndex, WallCollisionData},
    mario::{StepWorld, constants::*, f32_to_s32},
    math::{Mat4, mtxf_align_terrain_normal},
};

pub const OBJ_COL_FLAG_GROUNDED: i16 = 1 << 0;
pub const OBJ_COL_FLAG_HIT_WALL: i16 = 1 << 1;
pub const OBJ_COL_FLAG_UNDERWATER: i16 = 1 << 2;
pub const OBJ_COL_FLAG_NO_Y_VEL: i16 = 1 << 3;
pub const OBJ_COL_FLAGS_LANDED: i16 = OBJ_COL_FLAG_GROUNDED | OBJ_COL_FLAG_NO_Y_VEL;

/// Original rendering inputs: sOrientObjWithFloor and whether
/// alloc_display_list(sizeof(Mat4)) can supply a matrix. Neither changes motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStepOptions {
    pub orient_with_floor: bool,
    pub terrain_matrix_available: bool,
}

impl Default for ObjectStepOptions {
    fn default() -> Self {
        Self {
            orient_with_floor: true,
            terrain_matrix_available: true,
        }
    }
}

/// obj_splash's ordered requests. WaterWave is MODEL_IDLE_WATER_WAVE /
/// bhvObjectWaterWave; SmallBubble is MODEL_WHITE_PARTICLE_SMALL / bhvObjectBubble.
/// Both are spawned from the stepping object at its final position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectStepEffect {
    WaterWave,
    Sound(u32),
    SmallBubble,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectStepResult {
    pub collision_flags: i16,
    /// sObjFloor: queried at the original proposed position, before wall pushes.
    pub floor: Option<SurfaceIndex>,
    /// A newly produced throwMatrix, captured before the final X/Z movement.
    /// None means leave any existing matrix alone.
    pub terrain_matrix: Option<Mat4>,
    pub effects: Vec<ObjectStepEffect>,
}

/// turn_obj_away_from_surface. Reflect both components using the original
/// operations; a first wall's normal wins even when several walls push.
fn reflected_velocity([vx, vz]: [f32; 2], [nx, _, nz]: [f32; 3]) -> [f32; 2] {
    [
        (nz * nz - nx * nx) * vx / (nx * nx + nz * nz) - 2.0 * vz * (nx * nz) / (nx * nx + nz * nz),
        (nx * nx - nz * nz) * vz / (nx * nx + nz * nz) - 2.0 * vx * (nx * nz) / (nx * nx + nz * nz),
    ]
}

/// obj_find_wall.
fn find_wall(o: &mut Object, w: &StepWorld<'_>, proposed: [f32; 3], velocity: [f32; 2]) -> bool {
    let mut hitbox = WallCollisionData::new(proposed, o.hitbox_height / 2.0, o.hitbox_radius);
    if w.collision.find_wall_collisions(
        &mut hitbox,
        w.collision_flags,
        o.active_flags & ACTIVE_FLAG_MOVE_THROUGH_GRATE != 0,
    ) != 0
    {
        o.raw.set_f32(O_POS_X, hitbox.x);
        o.raw.set_f32(O_POS_Y, hitbox.y);
        o.raw.set_f32(O_POS_Z, hitbox.z);
        let normal = w.collision.surface(hitbox.walls[0].unwrap()).normal;
        let [vx, vz] = reflected_velocity(velocity, normal);
        o.raw
            .set_s32(O_MOVE_ANGLE_YAW, i32::from(w.trig.atan2s(vz, vx)));
        false
    } else {
        true
    }
}

/// turn_obj_away_from_steep_floor. Preserve the double literal and truncation
/// in the no-floor yaw change, and the strict below-floor test.
fn turn_from_floor(
    o: &mut Object,
    w: &StepWorld<'_>,
    floor: Option<SurfaceIndex>,
    height: f32,
    velocity: [f32; 2],
) -> bool {
    let Some(floor) = floor else {
        let yaw = f64::from(o.raw.s32(O_MOVE_ANGLE_YAW)) + 32767.999200000002;
        assert!(
            yaw >= f64::from(i32::MIN) && yaw < f64::from(i32::MAX) + 1.0,
            "object_step no-floor yaw exceeds the supported s32 conversion range"
        );
        o.raw.set_s32(O_MOVE_ANGLE_YAW, yaw as i32);
        return false;
    };
    let normal = w.collision.surface(floor).normal;
    if normal[1] < 0.5 && height > o.raw.f32(O_POS_Y) {
        let [vx, vz] = reflected_velocity(velocity, normal);
        o.raw
            .set_s32(O_MOVE_ANGLE_YAW, i32::from(w.trig.atan2s(vz, vx)));
        false
    } else {
        true
    }
}

/// obj_orient_graph: allocation failure and billboarding suppress only the
/// matrix. The caller owns its lifetime instead of a display-list arena pointer.
fn orient_graph(
    o: &Object,
    w: &StepWorld<'_>,
    normal: [f32; 3],
    options: ObjectStepOptions,
) -> Option<Mat4> {
    (options.orient_with_floor
        && options.terrain_matrix_available
        && o.gfx.node_flags & GRAPH_RENDER_BILLBOARD == 0)
        .then(|| {
            mtxf_align_terrain_normal(
                w.trig,
                normal,
                [
                    o.raw.f32(O_POS_X),
                    o.raw.f32(O_POS_Y) + o.raw.f32(O_GRAPH_Y_OFFSET),
                    o.raw.f32(O_POS_Z),
                ],
                o.raw.s32(O_FACE_ANGLE_YAW) as i16,
            )
        })
}

/// Shared statements of calc_new_obj_vel_and_pos_y and its underwater variant.
fn vertical_step(o: &mut Object, floor_y: f32, accel: f32) {
    let mut vy = (o.raw.f32(O_VEL_Y) - accel).clamp(-75.0, 75.0);
    let mut y = o.raw.f32(O_POS_Y) + vy;
    if y < floor_y {
        y = floor_y;
        vy = if vy < -17.5 { -(vy / 2.0) } else { 0.0 };
    }
    o.raw.set_f32(O_VEL_Y, vy);
    o.raw.set_f32(O_POS_Y, y);
}

fn close_to_floor(y: f32, floor_y: f32) -> bool {
    f32_to_s32(y) >= f32_to_s32(floor_y) && f32_to_s32(y) < f32_to_s32(floor_y).wrapping_add(37)
}

fn slope_velocity([vx, vz]: [f32; 2], [nx, ny, nz]: [f32; 3], gravity: f32) -> [f32; 2] {
    [
        vx + nx * (nx * nx + nz * nz) / (nx * nx + ny * ny + nz * nz) * gravity * 2.0,
        vz + nz * (nx * nx + nz * nz) / (nx * nx + ny * ny + nz * nz) * gravity * 2.0,
    ]
}

fn zero_tiny(x: f32) -> f32 {
    if f64::from(x) < 0.000001 && f64::from(x) > -0.000001 {
        0.0
    } else {
        x
    }
}

/// object_step / object_step_without_floor_orient. These routines deliberately
/// do not set oVelX/Z, oFloorHeight, oMoveFlags or graphics position/angles.
#[must_use = "actor callers must apply the new terrain matrix and ordered splash requests"]
pub fn object_step(o: &mut Object, w: &mut StepWorld<'_>) -> ObjectStepResult {
    object_step_with_options(o, w, ObjectStepOptions::default())
}

#[must_use = "actor callers must apply the ordered splash requests"]
pub fn object_step_without_floor_orient(o: &mut Object, w: &mut StepWorld<'_>) -> ObjectStepResult {
    object_step_with_options(
        o,
        w,
        ObjectStepOptions {
            orient_with_floor: false,
            ..Default::default()
        },
    )
}

#[must_use = "actor callers must apply the new terrain matrix and ordered splash requests"]
pub fn object_step_with_options(
    o: &mut Object,
    w: &mut StepWorld<'_>,
    options: ObjectStepOptions,
) -> ObjectStepResult {
    let [x, y, z] = o.pos();
    let velocity = [
        o.raw.f32(O_FORWARD_VEL) * w.trig.sins(o.raw.s32(O_MOVE_ANGLE_YAW)),
        o.raw.f32(O_FORWARD_VEL) * w.trig.coss(o.raw.s32(O_MOVE_ANGLE_YAW)),
    ];
    let proposed = [x + velocity[0], y, z + velocity[1]];
    let mut flags = 0;
    if !find_wall(o, w, proposed, velocity) {
        flags += OBJ_COL_FLAG_HIT_WALL;
    }
    let (floor_y, floor) = w.collision.find_floor(
        proposed[0],
        proposed[1],
        proposed[2],
        &mut w.collision_flags,
    );
    let mut water_y = FLOOR_LOWER_LIMIT_MISC as f32;
    let mut matrix = None;
    if turn_from_floor(o, w, floor, floor_y, velocity) {
        let normal = w.collision.surface(floor.unwrap()).normal;
        water_y = w.collision.find_water_level(proposed[0], proposed[2]);
        if water_y > y {
            let accel = (1.0 - o.raw.f32(O_BUOYANCY)) * (-o.raw.f32(O_GRAVITY));
            vertical_step(o, floor_y, accel);
            if o.raw.f32(O_FORWARD_VEL) > 12.5
                && water_y + 30.0 > o.raw.f32(O_POS_Y)
                && water_y - 30.0 < o.raw.f32(O_POS_Y)
            {
                o.raw.set_f32(O_VEL_Y, -o.raw.f32(O_VEL_Y));
            }
            let mut v = velocity;
            if close_to_floor(o.raw.f32(O_POS_Y), floor_y) {
                matrix = orient_graph(o, w, normal, options);
                v = slope_velocity(v, normal, accel);
            }
            v = v.map(zero_tiny);
            o.raw.set_f32(O_VEL_Y, zero_tiny(o.raw.f32(O_VEL_Y)));
            if v[0] != 0.0 || v[1] != 0.0 {
                o.raw
                    .set_s32(O_MOVE_ANGLE_YAW, i32::from(w.trig.atan2s(v[1], v[0])));
            }
            // The literal 0.8 is double in C; preserve the final f32 narrowing.
            o.raw.set_f32(
                O_FORWARD_VEL,
                (f64::from((v[0] * v[0] + v[1] * v[1]).sqrt()) * 0.8) as f32,
            );
            o.raw
                .set_f32(O_VEL_Y, (f64::from(o.raw.f32(O_VEL_Y)) * 0.8) as f32);
            flags += OBJ_COL_FLAG_UNDERWATER;
        } else {
            vertical_step(o, floor_y, o.raw.f32(O_GRAVITY));
            if close_to_floor(o.raw.f32(O_POS_Y), floor_y) {
                matrix = orient_graph(o, w, normal, options);
                let v = slope_velocity(velocity, normal, o.raw.f32(O_GRAVITY)).map(zero_tiny);
                if v[0] != 0.0 || v[1] != 0.0 {
                    o.raw
                        .set_s32(O_MOVE_ANGLE_YAW, i32::from(w.trig.atan2s(v[1], v[0])));
                }
                let friction =
                    if f64::from(normal[1]) < 0.2 && f64::from(o.raw.f32(O_FRICTION)) < 0.9999 {
                        0.0
                    } else {
                        o.raw.f32(O_FRICTION)
                    };
                o.raw
                    .set_f32(O_FORWARD_VEL, (v[0] * v[0] + v[1] * v[1]).sqrt() * friction);
            }
        }
    } else {
        flags += (flags & OBJ_COL_FLAG_HIT_WALL) ^ OBJ_COL_FLAG_HIT_WALL;
    }
    // obj_update_pos_vel_xz recomputes velocity from the possibly changed yaw
    // and speed. A wall push and this move both apply in the same original tick.
    let yaw = o.raw.s32(O_MOVE_ANGLE_YAW);
    let speed = o.raw.f32(O_FORWARD_VEL);
    o.raw
        .set_f32(O_POS_X, o.raw.f32(O_POS_X) + speed * w.trig.sins(yaw));
    o.raw
        .set_f32(O_POS_Z, o.raw.f32(O_POS_Z) + speed * w.trig.coss(yaw));
    if f32_to_s32(o.raw.f32(O_POS_Y)) == f32_to_s32(floor_y) {
        flags += OBJ_COL_FLAG_GROUNDED;
    }
    if f32_to_s32(o.raw.f32(O_VEL_Y)) == 0 {
        flags += OBJ_COL_FLAG_NO_Y_VEL;
    }

    // obj_splash takes truncated s32 arguments, then tests the live f32 Y.
    let water = f32_to_s32(water_y);
    let obj_y = f32_to_s32(o.raw.f32(O_POS_Y));
    let mut effects = Vec::new();
    if water.wrapping_add(30) as f32 > o.raw.f32(O_POS_Y)
        && o.raw.f32(O_POS_Y) > water.wrapping_sub(30) as f32
    {
        effects.push(ObjectStepEffect::WaterWave);
        if o.raw.f32(O_VEL_Y) < -20.0 {
            effects.push(ObjectStepEffect::Sound(SOUND_OBJ_DIVING_INTO_WATER));
        }
    }
    if obj_y.wrapping_add(50) < water && w.global_timer & 31 == 0 {
        effects.push(ObjectStepEffect::SmallBubble);
    }
    ObjectStepResult {
        collision_flags: flags,
        floor,
        terrain_matrix: matrix,
        effects,
    }
}
