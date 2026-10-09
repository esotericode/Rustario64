//! Mario state and ported Mario code. Only the parts verified against the
//! pinned decomp are here; actions, camera, interactions and animation
//! are not ported yet.
pub mod constants;
pub mod inputs;
pub mod step;

use crate::simulation::{
    collision::{CollisionFlags, CollisionWorld, Surface, SurfaceIndex},
    math::TrigTables,
};

/// A surface Mario references: original collision, or the step code's water
/// pseudo-floor (gWaterSurfacePseudoFloor) used while riding a shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRef {
    Collision(SurfaceIndex),
    WaterPseudoFloor,
}

/// The MarioState fields read or written by the ported code, named after the
/// original struct members. Graphics fields mirror marioObj->header.gfx.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MarioState {
    pub input: u16,
    pub flags: u32,
    pub action: u32,
    pub terrain_sound_addend: u32,
    pub particle_flags: u32,
    pub collided_obj_interact_types: u32,
    pub intended_mag: f32,
    pub intended_yaw: i16,
    pub frames_since_a: u8,
    pub frames_since_b: u8,
    pub squish_timer: u8,
    pub wall_kick_timer: u8,
    pub double_jump_timer: u8,
    pub face_angle: [i16; 3],
    pub angle_vel: [i16; 3],
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub forward_vel: f32,
    pub slide_vel_x: f32,
    pub slide_vel_z: f32,
    pub wall: Option<SurfaceRef>,
    pub ceil: Option<SurfaceRef>,
    pub floor: Option<SurfaceRef>,
    pub ceil_height: f32,
    pub floor_height: f32,
    pub floor_angle: i16,
    pub water_level: i16,
    pub peak_height: f32,
    pub quicksand_depth: f32,
    pub getting_blown_gravity: f32,
    /// marioBodyState->wingFlutter.
    pub wing_flutter: bool,
    pub gfx_pos: [f32; 3],
    pub gfx_angle: [i16; 3],
}

/// Everything the step code reads outside MarioState, made explicit.
pub struct StepWorld<'a> {
    pub collision: &'a CollisionWorld,
    pub collision_flags: CollisionFlags,
    pub trig: &'a TrigTables,
    /// gGlobalTimer.
    pub global_timer: u32,
    /// m->area->terrainType.
    pub area_terrain_type: u16,
    /// gCurrLevelNum.
    pub level_num: i16,
    /// gWaterSurfacePseudoFloor.originOffset; the step code rewrites it.
    pub water_pseudo_floor_origin_offset: f32,
}

impl StepWorld<'_> {
    /// The referenced surface's data (a copy; the pseudo-floor is synthesized).
    pub fn surface(&self, reference: SurfaceRef) -> Surface {
        match reference {
            SurfaceRef::Collision(index) => *self.collision.surface(index),
            SurfaceRef::WaterPseudoFloor => Surface {
                surface_type: constants::SURFACE_VERY_SLIPPERY,
                force: 0,
                flags: 0,
                room: 0,
                lower_y: 0,
                upper_y: 0,
                vertex1: [0; 3],
                vertex2: [0; 3],
                vertex3: [0; 3],
                normal: [0.0, 1.0, 0.0],
                origin_offset: self.water_pseudo_floor_origin_offset,
                object: None,
            },
        }
    }
}
