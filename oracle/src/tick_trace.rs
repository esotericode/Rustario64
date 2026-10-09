//! Development-only full-tick comparison. One scenario runs in the native
//! decomp (c/tick.c, state persisting across frames) and in the Rust port
//! (`simulation::mario::tick`); after every frame both sides report every
//! compared word under the same name, and schema-1 traces of those words are
//! compared exactly. Nothing is copied from one side to the other after the
//! level entry, so a divergence carries forward as it would in play.
use crate::{Oracle, TickSetup};
use rustario64::{
    content::animation::MarioAnimations,
    import::version,
    presentation::{self, GraphicsOptions, Snapshot},
    simulation::{
        FixedClock, TickInput,
        collision::CollisionWorld,
        mario::{
            AnimRef, MarioState, ObjectId, SaveInputs, StepWorld, SurfaceRef,
            core::SpawnPoint,
            tick::{LevelEntry, enter_level, tick},
        },
        math::TrigTables,
    },
    trace::{Frame, Metadata, TRACE_SCHEMA, TickState, Trace},
};
use std::{collections::BTreeMap, time::Duration};

impl TickSetup {
    /// gMarioSpawnInfo after a level script's MARIO and MARIO_POS commands
    /// (Mario's behavior parameter is 1), entering `level_num` from a fresh
    /// boot with an empty save file and the area's camera in `camera_mode`.
    pub fn from_level_script(
        level_num: i16,
        area: u8,
        yaw: i16,
        pos: [i16; 3],
        terrain_type: u16,
        camera_mode: u8,
    ) -> Self {
        Self::from_entry(&LevelEntry::from_level_script(
            level_num,
            area,
            yaw,
            pos,
            terrain_type,
            camera_mode,
        ))
    }

    /// The native setup for a Rust level entry (see `entry`).
    pub fn from_entry(entry: &LevelEntry) -> Self {
        let spawn = entry.spawn;
        assert!(
            entry.save.cap_pos.is_none(),
            "a saved cap position needs the cap object"
        );
        Self {
            start_pos: spawn.start_pos.map(i32::from),
            start_angle: spawn.start_angle.map(i32::from),
            area_index: i32::from(spawn.area_index),
            active_area_index: i32::from(spawn.active_area_index),
            behavior_arg: spawn.behavior_arg,
            level_num: i32::from(entry.level_num),
            terrain_type: u32::from(entry.terrain_type),
            camera_mode: i32::from(entry.camera_mode),
            camera_def_mode: i32::from(entry.camera_def_mode),
            save_flags: entry.save.flags,
            total_stars: entry.save.total_star_count,
            root_area_index: i32::from(spawn.area_index),
            camera_linked: 0,
            camera_pos: [0.0; 3],
            camera_focus: [0.0; 3],
            act_num: 0,
            rng_seed: 0,
        }
    }

    pub fn spawn(&self) -> SpawnPoint {
        SpawnPoint {
            start_pos: self.start_pos.map(|v| v as i16),
            start_angle: self.start_angle.map(|v| v as i16),
            area_index: self.area_index as i8,
            active_area_index: self.active_area_index as i8,
            behavior_arg: self.behavior_arg,
        }
    }

    /// The same entry for the Rust side. The rendered area is the spawn's
    /// area, as load_mario_area makes it.
    pub fn entry(&self) -> LevelEntry {
        assert_eq!(
            self.root_area_index, self.area_index,
            "the level entry renders the spawn's area"
        );
        LevelEntry {
            spawn: self.spawn(),
            level_num: self.level_num as i16,
            terrain_type: self.terrain_type as u16,
            camera_mode: self.camera_mode as u8,
            camera_def_mode: self.camera_def_mode as u8,
            save: SaveInputs {
                flags: self.save_flags,
                cap_pos: None,
                total_star_count: self.total_stars,
            },
        }
    }
}

/// The Rust side of oracle_tick_begin.
pub fn rust_begin<'a>(
    collision: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    setup: &TickSetup,
) -> (MarioState, StepWorld<'a>) {
    enter_level(collision, trig, anims, &setup.entry())
}

/// Named words, inserted once each.
pub struct Words(pub BTreeMap<String, u32>);

impl Words {
    pub(crate) fn put(&mut self, name: impl Into<String>, value: u32) {
        let name = name.into();
        assert!(
            self.0.insert(name.clone(), value).is_none(),
            "duplicate word {name}"
        );
    }
    pub(crate) fn i(&mut self, name: impl Into<String>, value: i32) {
        self.put(name, value as u32);
    }
    pub(crate) fn f(&mut self, name: impl Into<String>, value: f32) {
        self.put(name, value.to_bits());
    }
    pub(crate) fn s16v(&mut self, name: &str, values: &[i16]) {
        for (i, v) in values.iter().enumerate() {
            self.i(format!("{name}[{i}]"), i32::from(*v));
        }
    }
    pub(crate) fn f32v(&mut self, name: &str, values: &[f32]) {
        for (i, v) in values.iter().enumerate() {
            self.f(format!("{name}[{i}]"), *v);
        }
    }
}

fn surface_id(surface: Option<SurfaceRef>) -> i32 {
    match surface {
        None => -1,
        Some(SurfaceRef::WaterPseudoFloor) => -2,
        Some(SurfaceRef::Collision(i)) => i32::from(i),
    }
}

/// Objects are compared as NULL; the harness has no others.
fn object_id(object: Option<ObjectId>) -> i32 {
    assert!(object.is_none(), "a referenced object exists");
    -1
}

/// Every compared word, named as c/tick.c's oracle_tick_snapshot names it.
/// Events are the ones recorded since `w.events` was last cleared.
pub fn capture(m: &MarioState, w: &StepWorld<'_>) -> BTreeMap<String, u32> {
    let mut o = Words(BTreeMap::new());
    o.put("m.unk00", u32::from(m.unk00));
    o.put("m.input", u32::from(m.input));
    o.put("m.flags", m.flags);
    o.put("m.particleFlags", m.particle_flags);
    o.put("m.action", m.action);
    o.put("m.prevAction", m.prev_action);
    o.put("m.terrainSoundAddend", m.terrain_sound_addend);
    o.put("m.actionState", u32::from(m.action_state));
    o.put("m.actionTimer", u32::from(m.action_timer));
    o.put("m.actionArg", m.action_arg);
    o.f("m.intendedMag", m.intended_mag);
    o.i("m.intendedYaw", i32::from(m.intended_yaw));
    o.i("m.invincTimer", i32::from(m.invinc_timer));
    o.put("m.framesSinceA", u32::from(m.frames_since_a));
    o.put("m.framesSinceB", u32::from(m.frames_since_b));
    o.put("m.wallKickTimer", u32::from(m.wall_kick_timer));
    o.put("m.doubleJumpTimer", u32::from(m.double_jump_timer));
    o.s16v("m.faceAngle", &m.face_angle);
    o.s16v("m.angleVel", &m.angle_vel);
    o.i("m.slideYaw", i32::from(m.slide_yaw));
    o.i("m.twirlYaw", i32::from(m.twirl_yaw));
    o.f32v("m.pos", &m.pos);
    o.f32v("m.vel", &m.vel);
    o.f("m.forwardVel", m.forward_vel);
    o.f("m.slideVelX", m.slide_vel_x);
    o.f("m.slideVelZ", m.slide_vel_z);
    o.i("m.wall", surface_id(m.wall));
    o.i("m.ceil", surface_id(m.ceil));
    o.i("m.floor", surface_id(m.floor));
    o.f("m.ceilHeight", m.ceil_height);
    o.f("m.floorHeight", m.floor_height);
    o.i("m.floorAngle", i32::from(m.floor_angle));
    o.i("m.waterLevel", i32::from(m.water_level));
    o.i("m.interactObj", object_id(m.interact_obj));
    o.i("m.heldObj", object_id(m.held_obj));
    o.i("m.usedObj", object_id(m.used_obj));
    o.i("m.riddenObj", object_id(m.ridden_obj));
    o.put("m.collidedObjInteractTypes", m.collided_obj_interact_types);
    o.i("m.numCoins", i32::from(m.num_coins));
    o.i("m.numStars", i32::from(m.num_stars));
    o.i("m.numKeys", i32::from(m.num_keys));
    o.i("m.numLives", i32::from(m.num_lives));
    o.i("m.health", i32::from(m.health));
    o.i("m.unkB0", i32::from(m.unk_b0));
    o.put("m.hurtCounter", u32::from(m.hurt_counter));
    o.put("m.healCounter", u32::from(m.heal_counter));
    o.put("m.squishTimer", u32::from(m.squish_timer));
    o.put("m.fadeWarpOpacity", u32::from(m.fade_warp_opacity));
    o.put("m.capTimer", u32::from(m.cap_timer));
    o.i(
        "m.prevNumStarsForDialog",
        i32::from(m.prev_num_stars_for_dialog),
    );
    o.f("m.peakHeight", m.peak_height);
    o.f("m.quicksandDepth", m.quicksand_depth);
    o.f("m.gettingBlownGravity", m.getting_blown_gravity);

    let obj = &m.obj;
    let gfx = &obj.gfx;
    o.i("obj.gfx.flags", i32::from(gfx.node_flags));
    o.i("obj.gfx.areaIndex", i32::from(gfx.area_index));
    o.i("obj.gfx.activeAreaIndex", i32::from(gfx.active_area_index));
    o.s16v("obj.gfx.angle", &gfx.angle);
    o.f32v("obj.gfx.pos", &gfx.pos);
    o.f32v("obj.gfx.scale", &gfx.scale);
    o.i("obj.gfx.anim.animID", i32::from(gfx.anim.anim_id));
    o.i("obj.gfx.anim.animYTrans", i32::from(gfx.anim.anim_y_trans));
    o.i(
        "obj.gfx.anim.curAnim",
        match gfx.anim.cur_anim {
            None => 0,
            Some(AnimRef::MarioDmaBuffer) => 1,
        },
    );
    o.i("obj.gfx.anim.animFrame", i32::from(gfx.anim.anim_frame));
    o.put("obj.gfx.anim.animTimer", u32::from(gfx.anim.anim_timer));
    o.i(
        "obj.gfx.anim.animFrameAccelAssist",
        gfx.anim.anim_frame_accel_assist,
    );
    o.i("obj.gfx.anim.animAccel", gfx.anim.anim_accel);
    o.i(
        "obj.gfx.throwMatrix",
        gfx.throw_matrix.map_or(-1, |i| i as i32),
    );
    o.put(
        "obj.collidedObjInteractTypes",
        obj.collided_obj_interact_types,
    );
    o.i("obj.activeFlags", i32::from(obj.active_flags));
    o.i("obj.numCollidedObjs", i32::from(obj.num_collided_objs));
    for (i, word) in obj.raw.0.iter().enumerate() {
        o.put(format!("obj.raw[0x{i:02X}]"), *word);
    }
    o.f("obj.hitboxRadius", obj.hitbox_radius);
    o.f("obj.hitboxHeight", obj.hitbox_height);
    o.f("obj.hurtboxRadius", obj.hurtbox_radius);
    o.f("obj.hurtboxHeight", obj.hurtbox_height);
    o.f("obj.hitboxDownOffset", obj.hitbox_down_offset);
    o.i("obj.platform", object_id(obj.platform));
    o.put("obj.bhvLoopEntered", u32::from(obj.bhv_loop_entered));

    let b = &m.body;
    o.put("body.action", b.action);
    o.i("body.capState", i32::from(b.cap_state));
    o.i("body.eyeState", i32::from(b.eye_state));
    o.i("body.handState", i32::from(b.hand_state));
    o.i("body.wingFlutter", i32::from(b.wing_flutter));
    o.i("body.modelState", i32::from(b.model_state));
    o.i("body.grabPos", i32::from(b.grab_pos));
    o.put("body.punchState", u32::from(b.punch_state));
    o.s16v("body.torsoAngle", &b.torso_angle);
    o.s16v("body.headAngle", &b.head_angle);
    o.f32v("body.heldObjLastPosition", &b.held_obj_last_position);
    let c = &m.camera_status;
    o.put("cam.action", c.action);
    o.f32v("cam.pos", &c.pos);
    o.s16v("cam.faceAngle", &c.face_angle);
    o.s16v("cam.headRotation", &c.head_rotation);
    o.i("cam.unused", i32::from(c.unused));
    o.i("cam.cameraEvent", i32::from(c.camera_event));
    o.i("cam.usedObj", object_id(c.used_obj));

    o.put("world.globalTimer", w.global_timer);
    o.put("world.areaUpdateCounter", u32::from(w.area_update_counter));
    o.i(
        "world.cameraMovementFlags",
        i32::from(w.camera_movement_flags),
    );
    o.put("world.camera.mode", u32::from(w.camera.mode));
    o.put("world.camera.defMode", u32::from(w.camera.def_mode));
    o.i("world.camera.yaw", i32::from(w.camera.yaw));
    o.i("world.levelNum", i32::from(w.level_num));
    o.put("world.terrainType", u32::from(w.area_terrain_type));
    o.put("world.specialTripleJump", u32::from(w.special_triple_jump));
    o.f(
        "world.waterPseudoFloorOriginOffset",
        w.water_pseudo_floor_origin_offset,
    );
    o.put(
        "world.findFloorIncludeSurfaceIntangible",
        u32::from(w.collision_flags.find_floor_include_surface_intangible),
    );
    o.put(
        "world.checkingSurfaceCollisionsForCamera",
        u32::from(w.collision_flags.checking_for_camera),
    );
    o.i("world.marioPlatform", object_id(w.mario_platform));
    o.put(
        "world.delayInvincTimer",
        u32::from(w.interaction.delay_invinc_timer),
    );
    o.i("world.invulnerable", i32::from(w.interaction.invulnerable));
    o.put(
        "world.displayingDoorText",
        u32::from(w.interaction.displaying_door_text),
    );
    o.put(
        "world.justTeleported",
        u32::from(w.interaction.just_teleported),
    );
    o.put(
        "world.pssSlideStarted",
        u32::from(w.interaction.pss_slide_started),
    );
    for (i, matrix) in w.floor_align_matrix.iter().enumerate() {
        for j in 0..16 {
            o.f(
                format!("world.floorAlignMatrix[{i}][{j}]"),
                matrix[j / 4][j % 4],
            );
        }
    }
    o.i(
        "world.animDmaLoaded",
        w.anim_dma_loaded.map_or(-1, i32::from),
    );
    let ctl = &w.controller;
    o.i("ctl.rawStickX", i32::from(ctl.raw_stick[0]));
    o.i("ctl.rawStickY", i32::from(ctl.raw_stick[1]));
    o.f("ctl.stickX", ctl.stick_x);
    o.f("ctl.stickY", ctl.stick_y);
    o.f("ctl.stickMag", ctl.stick_mag);
    o.put("ctl.buttonDown", u32::from(ctl.button_down));
    o.put("ctl.buttonPressed", u32::from(ctl.button_pressed));
    o.put("events.count", w.events.len() as u32);
    for (i, event) in w.events.iter().enumerate() {
        let (kind, a, b) = event.raw();
        o.i(format!("events[{i}].kind"), kind);
        o.i(format!("events[{i}].a"), a);
        o.i(format!("events[{i}].b"), b);
    }
    o.0
}

/// The trace state for one frame's words. The schema's fixed fields repeat
/// the words they name; `rng` is unused (the ported code draws no random
/// numbers; audio variants read gAudioRandom, a recorded input).
fn tick_state(fields: BTreeMap<String, u32>) -> TickState {
    let word = |name: &str| {
        *fields
            .get(name)
            .unwrap_or_else(|| panic!("missing word {name}"))
    };
    let surfaces = ["m.wall", "m.ceil", "m.floor"];
    TickState {
        action: word("m.action"),
        action_state: word("m.actionState") as u16,
        action_timer: word("m.actionTimer") as u16,
        position_bits: [word("m.pos[0]"), word("m.pos[1]"), word("m.pos[2]")],
        velocity_bits: [word("m.vel[0]"), word("m.vel[1]"), word("m.vel[2]")],
        face_angles: [
            word("m.faceAngle[0]") as i16,
            word("m.faceAngle[1]") as i16,
            word("m.faceAngle[2]") as i16,
        ],
        camera_yaw: word("world.camera.yaw") as i16,
        rng: 0,
        contacts: surfaces
            .iter()
            .filter_map(|s| u32::try_from(word(s) as i32).ok())
            .collect(),
        interactions: vec![],
        objects: vec![],
        fields,
    }
}

/// One full-tick scenario: terrain, tables, animations and a level entry.
pub struct TickScenario<'a> {
    pub collision: &'a CollisionWorld,
    pub trig: &'a TrigTables,
    pub anims: &'a MarioAnimations,
    pub setup: TickSetup,
    pub rom_sha1: String,
    pub world_digest: String,
    pub scenario: String,
    pub course: u8,
}

impl TickScenario<'_> {
    fn trace(&self, producer: &str, initial: BTreeMap<String, u32>, frames: usize) -> Trace {
        Trace {
            schema: TRACE_SCHEMA,
            producer: producer.into(),
            metadata: Metadata {
                rom_sha1: self.rom_sha1.clone(),
                reference_revision: version::REFERENCE_REVISION.into(),
                reference_configuration: "full-tick-v1; native C IEEE/fwrapv/no-FMA/AVOID_UB; \
                    Mario's object only; level entry without warp from a fresh boot; \
                    camera yaw and mode recorded inputs; sound, camera, warp and particle \
                    calls recorded as events; debug pages off"
                    .into(),
                scenario: self.scenario.clone(),
                tick_rate: 30,
                course: self.course,
                area: self.setup.area_index as u8,
                act: 1,
                initial_world_digest: self.world_digest.clone(),
                gameplay_options: BTreeMap::new(),
                initial_state: tick_state(initial),
            },
            frames: Vec::with_capacity(frames),
        }
    }

    /// The native decomp's trace. The oracle must have this scenario's
    /// terrain loaded and animations set.
    pub fn native(&self, oracle: &Oracle, inputs: &[TickInput]) -> Trace {
        oracle.tick_begin(&self.setup);
        let initial = oracle.tick_snapshot();
        let mut trace = self.trace("native-decomp full tick", initial, inputs.len());
        for (i, input) in inputs.iter().enumerate() {
            oracle.tick(*input);
            trace.frames.push(Frame {
                tick: i as u64 + 1,
                input: *input,
                state: tick_state(oracle.tick_snapshot()),
            });
        }
        trace
    }

    /// The Rust side's words after the level entry.
    pub fn rust_initial(&self) -> BTreeMap<String, u32> {
        let (m, w) = rust_begin(self.collision, self.trig, self.anims, &self.setup);
        capture(&m, &w)
    }

    /// Rust ticks until the first panic (a path the port does not support,
    /// such as one needing objects). Returns each completed tick's words and
    /// the panic message, if any; callers compare the native run over the
    /// completed ticks only, since the original would leave coverage there too.
    pub fn rust_until_unsupported(
        &self,
        inputs: &[TickInput],
    ) -> (Vec<BTreeMap<String, u32>>, Option<String>) {
        let (mut m, mut w) = rust_begin(self.collision, self.trig, self.anims, &self.setup);
        let mut out = Vec::with_capacity(inputs.len());
        for input in inputs {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                w.events.clear();
                tick(&mut m, &mut w, *input);
                capture(&m, &w)
            }));
            match result {
                Ok(words) => out.push(words),
                Err(payload) => {
                    let message = payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                        .unwrap_or_else(|| "non-string panic".into());
                    return (out, Some(message));
                }
            }
        }
        (out, None)
    }

    /// The Rust trace, with the simulation driven by the fixed clock at
    /// `render_hz` and presentation interpolated from completed ticks.
    pub fn rust(
        &self,
        inputs: &[TickInput],
        render_hz: u32,
        graphics: GraphicsOptions,
    ) -> Result<Trace, &'static str> {
        if inputs.is_empty() || !(1..=1000).contains(&render_hz) {
            return Err("nonempty inputs and render rate 1..=1000 required");
        }
        let (mut m, mut w) = rust_begin(self.collision, self.trig, self.anims, &self.setup);
        let mut trace = self.trace("Rust full tick", capture(&m, &w), inputs.len());
        let mut clock = FixedClock::default();
        let mut elapsed = 0;
        let mut frame = 0u64;
        let mut previous = None;
        let mut current = Snapshot {
            entity: 1,
            epoch: 0,
            position: m.obj.gfx.pos,
            yaw: m.obj.gfx.angle[1],
            animation: m.obj.gfx.anim.anim_id as u16,
            discontinuity: false,
        };
        while trace.frames.len() < inputs.len() {
            frame += 1;
            let end = frame * 1_000_000_000 / u64::from(render_hz);
            clock.add_elapsed(Duration::from_nanos(end - elapsed))?;
            elapsed = end;
            let budget = (inputs.len() - trace.frames.len()).min(8) as u32;
            for _ in 0..clock.drain(budget) {
                let index = trace.frames.len();
                let input = inputs[index];
                w.events.clear();
                tick(&mut m, &mut w, input);
                previous = Some(current);
                current.position = m.obj.gfx.pos;
                current.yaw = m.obj.gfx.angle[1];
                current.animation = m.obj.gfx.anim.anim_id as u16;
                trace.frames.push(Frame {
                    tick: index as u64 + 1,
                    input,
                    state: tick_state(capture(&m, &w)),
                });
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
