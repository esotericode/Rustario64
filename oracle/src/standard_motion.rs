//! Authored transport for original standard object motion. Collision and trig
//! data are independent native globals guarded by the existing Oracle lock.
use crate::Oracle;
use rustario64::simulation::{collision::CollisionFlags, object::Object};

#[derive(Debug, Clone, Copy)]
pub enum StandardMotionCall {
    UpdateFloorAndWalls,
    MoveStandard(i16),
    MoveY {
        gravity: f32,
        bounciness: f32,
        buoyancy: f32,
    },
    MoveUsingFvelAndGravity,
    ResolveWalls,
    ApplyDrag(f32),
    FloorHeight,
    AngleDiff(i16, i16),
    Release {
        forward_vel: f32,
        vel_y: f32,
        mario_pos: [f32; 3],
    },
}

#[repr(C)]
struct Input {
    raw: [u32; 0x50],
    active_flags: i32,
    include_intangible: i32,
    for_camera: i32,
    operation: i32,
    angle: i32,
    other_angle: i32,
    gravity: f32,
    bounciness: f32,
    buoyancy: f32,
    drag: f32,
    mario_pos: [f32; 3],
}

#[repr(C)]
struct Output {
    raw: [u32; 0x50],
    result: i32,
    include_after: i32,
}

unsafe extern "C" {
    fn oracle_standard_motion(input: *const Input, output: *mut Output);
}

impl Oracle {
    /// Returns zero for void functions, a boolean for ResolveWalls, the
    /// surface index (-1 for NULL) for FloorHeight, and the absolute angle
    /// for AngleDiff. Surface pointer words use index + 1, zero for NULL.
    pub fn standard_motion(
        &self,
        object: &mut Object,
        flags: &mut CollisionFlags,
        call: StandardMotionCall,
    ) -> i32 {
        let mut input = Input {
            raw: object.raw.0,
            active_flags: i32::from(object.active_flags),
            include_intangible: i32::from(flags.find_floor_include_surface_intangible),
            for_camera: i32::from(flags.checking_for_camera),
            operation: 0,
            angle: 0,
            other_angle: 0,
            gravity: 0.0,
            bounciness: 0.0,
            buoyancy: 0.0,
            drag: 0.0,
            mario_pos: [0.0; 3],
        };
        match call {
            StandardMotionCall::UpdateFloorAndWalls => {}
            StandardMotionCall::MoveStandard(angle) => {
                input.operation = 1;
                input.angle = i32::from(angle);
            }
            StandardMotionCall::MoveY {
                gravity,
                bounciness,
                buoyancy,
            } => {
                input.operation = 2;
                input.gravity = gravity;
                input.bounciness = bounciness;
                input.buoyancy = buoyancy;
            }
            StandardMotionCall::MoveUsingFvelAndGravity => input.operation = 3,
            StandardMotionCall::ResolveWalls => input.operation = 4,
            StandardMotionCall::ApplyDrag(drag) => {
                input.operation = 5;
                input.drag = drag;
            }
            StandardMotionCall::FloorHeight => input.operation = 6,
            StandardMotionCall::AngleDiff(a, b) => {
                input.operation = 7;
                input.angle = i32::from(a);
                input.other_angle = i32::from(b);
            }
            StandardMotionCall::Release {
                forward_vel,
                vel_y,
                mario_pos,
            } => {
                input.operation = 8;
                input.gravity = forward_vel;
                input.bounciness = vel_y;
                input.mario_pos = mario_pos;
            }
        }
        let mut output = Output {
            raw: [0; 0x50],
            result: 0,
            include_after: 0,
        };
        // Fixed repr(C) records stay alive through the locked native call.
        unsafe { oracle_standard_motion(&input, &mut output) };
        object.raw.0 = output.raw;
        flags.find_floor_include_surface_intangible = output.include_after != 0;
        output.result
    }
}
