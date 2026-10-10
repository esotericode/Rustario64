//! Authored transport for the verbatim object_step component. Water effects
//! are requests only; this is not an actor/frame or particle-behavior comparison.
use crate::Oracle;
use rustario64::simulation::{
    collision::CollisionFlags,
    mario::constants::*,
    object::{
        Object,
        motion::{ObjectStepEffect, ObjectStepOptions, ObjectStepResult},
    },
};

#[repr(C)]
struct Input {
    raw: [u32; 0x50],
    hitbox_radius: f32,
    hitbox_height: f32,
    gfx_flags: i32,
    active_flags: i32,
    global_timer: u32,
    include_intangible: i32,
    for_camera: i32,
    orient_with_floor: i32,
    matrix_available: i32,
}

#[repr(C)]
struct Output {
    raw: [u32; 0x50],
    matrix: [f32; 16],
    has_matrix: i32,
    floor: i32,
    collision_flags: i32,
    include_after: i32,
    effect_count: i32,
    effects: [i32; 9],
}

unsafe extern "C" {
    fn oracle_object_step(input: *const Input, output: *mut Output);
}

impl Oracle {
    /// Uses the collision/trig data already installed under this Oracle's lock.
    pub fn object_step(
        &self,
        object: &mut Object,
        flags: &mut CollisionFlags,
        timer: u32,
        options: ObjectStepOptions,
    ) -> ObjectStepResult {
        let input = Input {
            raw: object.raw.0,
            hitbox_radius: object.hitbox_radius,
            hitbox_height: object.hitbox_height,
            gfx_flags: i32::from(object.gfx.node_flags),
            active_flags: i32::from(object.active_flags),
            global_timer: timer,
            include_intangible: i32::from(flags.find_floor_include_surface_intangible),
            for_camera: i32::from(flags.checking_for_camera),
            orient_with_floor: i32::from(options.orient_with_floor),
            matrix_available: i32::from(options.terrain_matrix_available),
        };
        let mut output = Output {
            raw: [0; 0x50],
            matrix: [0.0; 16],
            has_matrix: 0,
            floor: -1,
            collision_flags: 0,
            include_after: 0,
            effect_count: 0,
            effects: [0; 9],
        };
        // Fixed repr(C) records live through this call; Oracle serializes C globals.
        unsafe { oracle_object_step(&input, &mut output) };
        object.raw.0 = output.raw;
        flags.find_floor_include_surface_intangible = output.include_after != 0;
        let count = usize::try_from(output.effect_count).unwrap();
        assert!(count <= 3);
        let effects = output.effects[..count * 3]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|e| match e {
                [1, model, 1] if *model == MODEL_IDLE_WATER_WAVE => ObjectStepEffect::WaterWave,
                [1, model, 2] if *model == MODEL_WHITE_PARTICLE_SMALL => {
                    ObjectStepEffect::SmallBubble
                }
                [2, sound, 0] => ObjectStepEffect::Sound(*sound as u32),
                _ => panic!("unknown object_step boundary {e:?}"),
            })
            .collect();
        ObjectStepResult {
            collision_flags: output.collision_flags as i16,
            floor: (output.floor != -1).then(|| u16::try_from(output.floor).unwrap()),
            terrain_matrix: (output.has_matrix != 0).then(|| {
                std::array::from_fn(|r| std::array::from_fn(|c| output.matrix[4 * r + c]))
            }),
            effects,
        }
    }
}
