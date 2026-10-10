//! Authored initial-condition and call transport for shared boss grabbing.
//! Use tick_begin first; tick_snapshot captures the independently evolving
//! Mario/object state. No per-call state is copied from Rust to native C.
use crate::Oracle;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GrabFixture {
    pub action: u32,
    pub action_arg: u32,
    pub subtype: u32,
    pub object_status: u32,
    pub mario_status: u32,
    pub invinc_timer: i32,
    pub object_yaw: i32,
    pub mario_yaw: i32,
    pub anchor_state: i32,
    pub parent_active: i32,
    /// 0: nothing held; 1: another object; 2: the grabbing actor.
    pub holding: i32,
    pub object_pos: [f32; 3],
    pub anchor_pos: [f32; 3],
    pub gfx_pos: [f32; 3],
    pub forward_vel: f32,
    pub vel_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum GrabCall {
    Interactions,
    AutomaticAction,
    AcknowledgeGrab,
    Anchor {
        forward_vel: f32,
        vel_y: f32,
        status: u32,
    },
    Escape {
        stick_mag: f32,
        pressed: u16,
    },
    Release {
        forward_vel: f32,
        vel_y: f32,
        action: i32,
    },
    ObjectAction,
    StationaryAction,
    MovingAction,
    AdvanceAnimation,
    Animation(i32),
    AnchorState(i32),
    Intent {
        input: u16,
        magnitude: f32,
        yaw: i16,
    },
}

#[repr(C)]
#[derive(Default)]
struct Call {
    operation: i32,
    argument: i32,
    a: f32,
    b: f32,
    status: u32,
}
unsafe extern "C" {
    fn oracle_grab_fixture(fixture: *const GrabFixture);
    fn oracle_grab_call(call: *const Call) -> i32;
}

impl Oracle {
    pub fn grab_fixture(&self, fixture: &GrabFixture) {
        // Fixed repr(C) record under the process-wide Oracle lock.
        unsafe { oracle_grab_fixture(fixture) }
    }

    pub fn grab_call(&self, call: GrabCall) -> i32 {
        let mut c = Call::default();
        match call {
            GrabCall::Interactions => {}
            GrabCall::AutomaticAction => c.operation = 1,
            GrabCall::AcknowledgeGrab => c.operation = 2,
            GrabCall::Anchor {
                forward_vel,
                vel_y,
                status,
            } => {
                c.operation = 3;
                c.a = forward_vel;
                c.b = vel_y;
                c.status = status;
            }
            GrabCall::Escape { stick_mag, pressed } => {
                c.operation = 4;
                c.a = stick_mag;
                c.status = u32::from(pressed);
            }
            GrabCall::Release {
                forward_vel,
                vel_y,
                action,
            } => {
                c.operation = 5;
                c.a = forward_vel;
                c.b = vel_y;
                c.argument = action;
            }
            GrabCall::ObjectAction => c.operation = 6,
            GrabCall::StationaryAction => c.operation = 7,
            GrabCall::MovingAction => c.operation = 8,
            GrabCall::AdvanceAnimation => c.operation = 9,
            GrabCall::Animation(index) => {
                c.operation = 12;
                c.argument = index;
            }
            GrabCall::AnchorState(state) => {
                c.operation = 10;
                c.argument = state;
            }
            GrabCall::Intent {
                input,
                magnitude,
                yaw,
            } => {
                c.operation = 11;
                c.argument = i32::from(input);
                c.a = magnitude;
                c.status = yaw as u32;
            }
        }
        // Live fixed record under the process-wide Oracle lock.
        unsafe { oracle_grab_call(&c) }
    }
}
