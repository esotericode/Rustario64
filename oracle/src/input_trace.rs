//! Development-only schema-1 adapters for the input stage. Action timers/RNG
//! and object lists are explicitly excluded placeholders until those systems exist.
use crate::{OracleInput, OracleMario};
use rustario64::{
    simulation::{
        TickInput,
        controller::Controller,
        mario::{
            MarioState, StepWorld, SurfaceRef,
            inputs::{InputContext, InputOutcome},
        },
    },
    trace::TickState,
};
use std::collections::BTreeMap;
impl OracleMario {
    pub fn capture(m: &MarioState, w: &StepWorld<'_>) -> Self {
        let surface = |s: Option<SurfaceRef>| match s {
            None => -1,
            Some(SurfaceRef::Collision(i)) => i32::from(i),
            Some(SurfaceRef::WaterPseudoFloor) => -2,
        };
        Self {
            input: m.input,
            flags: m.flags,
            action: m.action,
            terrain_sound_addend: m.terrain_sound_addend,
            particle_flags: m.particle_flags,
            collided_obj_interact_types: m.collided_obj_interact_types,
            intended_mag: m.intended_mag,
            intended_yaw: m.intended_yaw,
            frames_since_a: m.frames_since_a,
            frames_since_b: m.frames_since_b,
            squish_timer: m.squish_timer,
            wall_kick_timer: m.wall_kick_timer,
            double_jump_timer: m.double_jump_timer,
            face_angle: m.face_angle,
            angle_vel: m.angle_vel,
            pos: m.pos,
            vel: m.vel,
            forward_vel: m.forward_vel,
            slide_vel_x: m.slide_vel_x,
            slide_vel_z: m.slide_vel_z,
            wall: surface(m.wall),
            ceil: surface(m.ceil),
            floor: surface(m.floor),
            ceil_height: m.ceil_height,
            floor_height: m.floor_height,
            floor_angle: m.floor_angle,
            water_level: m.water_level,
            peak_height: m.peak_height,
            quicksand_depth: m.quicksand_depth,
            getting_blown_gravity: m.getting_blown_gravity,
            wing_flutter: i8::from(m.wing_flutter),
            gfx_pos: m.gfx_pos,
            gfx_angle: m.gfx_angle,
            global_timer: w.global_timer,
            area_terrain_type: w.area_terrain_type,
            level_num: w.level_num,
            water_pseudo_origin_offset: w.water_pseudo_floor_origin_offset,
            include_intangible: i16::from(w.collision_flags.find_floor_include_surface_intangible),
        }
    }
}
impl OracleInput {
    /// Capture previous controller + next sample before C, or completed controller
    /// after Rust. Both serialize the same post-stage observation contract.
    pub fn capture(
        c: &Controller,
        context: &InputContext,
        sample: TickInput,
        outcome: InputOutcome,
    ) -> Self {
        Self {
            raw_stick: sample.stick.map(i16::from),
            stick_x: c.stick_x,
            stick_y: c.stick_y,
            stick_mag: c.stick_mag,
            button_down: c.button_down,
            button_pressed: c.button_pressed,
            sample_buttons: sample.buttons,
            camera_yaw: context.camera_yaw,
            camera_movement_flags: context.camera_movement_flags,
            object_interact_status: context.object_interact_status,
            object_collided_interact_types: context.object_collided_interact_types,
            death_warp_requests: i32::from(outcome == InputOutcome::DeathWarpRequested),
        }
    }
}
/// Every exposed Mario/controller/world field, with exact float bits and stable
/// surface indices (-1 NULL / -2 pseudo-floor). No implicit gameplay coverage.
pub fn snapshot(m: &OracleMario, input: &OracleInput) -> TickState {
    let mut fields = BTreeMap::new();
    fields.insert("input".into(), m.input as u32);
    fields.insert("flags".into(), m.flags);
    fields.insert("terrain_sound_addend".into(), m.terrain_sound_addend);
    fields.insert("particle_flags".into(), m.particle_flags);
    fields.insert(
        "collided_obj_interact_types".into(),
        m.collided_obj_interact_types,
    );
    fields.insert("intended_mag".into(), m.intended_mag.to_bits());
    fields.insert("intended_yaw".into(), m.intended_yaw as u32);
    fields.insert("frames_since_a".into(), m.frames_since_a as u32);
    fields.insert("frames_since_b".into(), m.frames_since_b as u32);
    fields.insert("squish_timer".into(), m.squish_timer as u32);
    fields.insert("wall_kick_timer".into(), m.wall_kick_timer as u32);
    fields.insert("double_jump_timer".into(), m.double_jump_timer as u32);
    for axis in 0..3 {
        fields.insert(format!("angle_vel.{axis}"), m.angle_vel[axis] as u32);
    }
    fields.insert("forward_vel".into(), m.forward_vel.to_bits());
    fields.insert("slide_vel_x".into(), m.slide_vel_x.to_bits());
    fields.insert("slide_vel_z".into(), m.slide_vel_z.to_bits());
    fields.insert("wall".into(), m.wall as u32);
    fields.insert("ceil".into(), m.ceil as u32);
    fields.insert("floor".into(), m.floor as u32);
    fields.insert("ceil_height".into(), m.ceil_height.to_bits());
    fields.insert("floor_height".into(), m.floor_height.to_bits());
    fields.insert("floor_angle".into(), m.floor_angle as u32);
    fields.insert("water_level".into(), m.water_level as u32);
    fields.insert("peak_height".into(), m.peak_height.to_bits());
    fields.insert("quicksand_depth".into(), m.quicksand_depth.to_bits());
    fields.insert(
        "getting_blown_gravity".into(),
        m.getting_blown_gravity.to_bits(),
    );
    fields.insert("wing_flutter".into(), m.wing_flutter as u32);
    for axis in 0..3 {
        fields.insert(format!("gfx_pos.{axis}"), m.gfx_pos[axis].to_bits());
    }
    for axis in 0..3 {
        fields.insert(format!("gfx_angle.{axis}"), m.gfx_angle[axis] as u32);
    }
    fields.insert("global_timer".into(), m.global_timer);
    fields.insert("area_terrain_type".into(), m.area_terrain_type as u32);
    fields.insert("level_num".into(), m.level_num as u32);
    fields.insert(
        "water_pseudo_origin_offset".into(),
        m.water_pseudo_origin_offset.to_bits(),
    );
    fields.insert("include_intangible".into(), m.include_intangible as u32);
    fields.insert("raw_stick.0".into(), input.raw_stick[0] as u32);
    fields.insert("raw_stick.1".into(), input.raw_stick[1] as u32);
    fields.insert("stick_x".into(), input.stick_x.to_bits());
    fields.insert("stick_y".into(), input.stick_y.to_bits());
    fields.insert("stick_mag".into(), input.stick_mag.to_bits());
    fields.insert("button_down".into(), input.button_down as u32);
    fields.insert("button_pressed".into(), input.button_pressed as u32);
    fields.insert("sample_buttons".into(), input.sample_buttons as u32);
    fields.insert(
        "camera_movement_flags".into(),
        input.camera_movement_flags as u32,
    );
    fields.insert(
        "object_interact_status".into(),
        input.object_interact_status,
    );
    fields.insert(
        "object_collided_interact_types".into(),
        input.object_collided_interact_types,
    );
    fields.insert(
        "death_warp_requests".into(),
        input.death_warp_requests as u32,
    );
    TickState {
        action: m.action,
        action_state: 0,
        action_timer: 0,
        position_bits: m.pos.map(f32::to_bits),
        velocity_bits: m.vel.map(f32::to_bits),
        face_angles: m.face_angle,
        camera_yaw: input.camera_yaw,
        rng: 0,
        contacts: [m.wall, m.ceil, m.floor]
            .into_iter()
            .filter_map(|i| u32::try_from(i).ok())
            .collect(),
        interactions: vec![],
        objects: vec![],
        fields,
    }
}

use crate::{InputCall, Oracle};
use rustario64::{
    import::{sha1_hex, version},
    presentation::{self, GraphicsOptions, Snapshot},
    simulation::{
        FixedClock,
        collision::{CollisionFlags, CollisionWorld},
        math::TrigTables,
    },
    trace::{Frame, Metadata, TRACE_SCHEMA, Trace},
};
use std::time::Duration;

pub fn world_digest(stream: &[i16], trig: &TrigTables) -> String {
    let mut bytes = b"rustario64-input-world-v1\0".to_vec();
    bytes.extend_from_slice(&(stream.len() as u64).to_be_bytes());
    bytes.extend(stream.iter().flat_map(|v| v.to_be_bytes()));
    bytes.extend(
        trig.sine_table()
            .iter()
            .flat_map(|v| v.to_bits().to_be_bytes()),
    );
    bytes.extend(trig.arctan_table().iter().flat_map(|v| v.to_be_bytes()));
    sha1_hex(&bytes)
}
/// Explicit component-fixture initialization, not init_mario or a spawn warp.
pub struct Replay<'a> {
    pub collision: &'a CollisionWorld,
    pub trig: &'a TrigTables,
    pub initial: MarioState,
    pub controller: Controller,
    pub context: InputContext,
    pub area_terrain_type: u16,
    pub level_num: i16,
    pub rom_sha1: String,
    pub world_digest: String,
    pub scenario: String,
}
impl Replay<'_> {
    /// Independent state for each producer. Graphics read completed snapshots;
    /// controller edges are consumed only inside the fixed tick loop.
    pub fn run(
        &self,
        samples: &[TickInput],
        render_hz: u32,
        graphics: GraphicsOptions,
        native: Option<&Oracle>,
    ) -> Result<Trace, &'static str> {
        if samples.is_empty() || !(1..=1000).contains(&render_hz) {
            return Err("nonempty samples and render rate 1..=1000 required");
        }
        let mut m = self.initial.clone();
        let mut w = StepWorld {
            collision: self.collision,
            collision_flags: CollisionFlags::default(),
            trig: self.trig,
            global_timer: 0,
            area_terrain_type: self.area_terrain_type,
            level_num: self.level_num,
            water_pseudo_floor_origin_offset: 0.0,
        };
        let mut controller = self.controller;
        let mut context = self.context;
        let mut reference = OracleMario::capture(&m, &w);
        let mut reference_input = OracleInput::capture(
            &controller,
            &context,
            TickInput {
                buttons: controller.button_down,
                stick: controller.raw_stick,
                camera_yaw: context.camera_yaw,
            },
            InputOutcome::Continue,
        );
        let initial_state = snapshot(&reference, &reference_input);
        let mut trace = Trace { schema: TRACE_SCHEMA,
            producer: if native.is_some() { "native-decomp input stage" } else { "Rust input stage" }.into(),
            metadata: Metadata {
                rom_sha1: self.rom_sha1.clone(), reference_revision: version::REFERENCE_REVISION.into(),
                reference_configuration: "input-stage-v1; native C IEEE/fwrapv/no-FMA; static terrain; debug disabled; death-warp requests only; fixture initialization; no actions, camera update, objects, RNG, animation, or warp execution".into(),
                scenario: self.scenario.clone(), tick_rate: 30, course: 1, area: 1, act: 1,
                initial_world_digest: self.world_digest.clone(), gameplay_options: BTreeMap::new(), initial_state,
            }, frames: Vec::with_capacity(samples.len()) };
        let mut clock = FixedClock::default();
        let mut elapsed = 0;
        let mut frame = 0u64;
        let mut previous = None;
        let mut current = Snapshot {
            entity: 1,
            epoch: 0,
            position: m.pos,
            yaw: m.face_angle[1],
            animation: 0,
            discontinuity: false,
        };
        while trace.frames.len() < samples.len() {
            frame += 1;
            let end = frame * 1_000_000_000 / u64::from(render_hz);
            clock.add_elapsed(Duration::from_nanos(end - elapsed))?;
            elapsed = end;
            let budget = (samples.len() - trace.frames.len()).min(8) as u32;
            for _ in 0..clock.drain(budget) {
                let tick = trace.frames.len() as u64 + 1;
                let input = samples[tick as usize - 1];
                let state = if let Some(oracle) = native {
                    reference.global_timer = tick as u32;
                    reference_input.raw_stick = input.stick.map(i16::from);
                    reference_input.sample_buttons = input.buttons;
                    reference_input.camera_yaw = input.camera_yaw;
                    oracle.input_tick(&mut reference, &mut reference_input, InputCall::Full);
                    snapshot(&reference, &reference_input)
                } else {
                    w.global_timer = tick as u32;
                    context.camera_yaw = input.camera_yaw;
                    controller.sample(input);
                    let outcome = rustario64::simulation::mario::inputs::update_mario_inputs(
                        &mut m,
                        &mut w,
                        &controller,
                        &mut context,
                    );
                    snapshot(
                        &OracleMario::capture(&m, &w),
                        &OracleInput::capture(&controller, &context, input, outcome),
                    )
                };
                previous = Some(current);
                current.position = state.position_bits.map(f32::from_bits);
                current.yaw = state.face_angles[1];
                current.discontinuity = current.position != previous.unwrap().position;
                trace.frames.push(Frame { tick, input, state });
            }
            std::hint::black_box(presentation::interpolate(
                previous.as_ref(),
                &current,
                clock.alpha(),
                graphics,
            ));
        }
        Ok(trace)
    }
}
