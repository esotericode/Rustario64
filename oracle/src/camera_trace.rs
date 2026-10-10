//! Development-only comparison of complete frames with the original camera:
//! the native decomp runs c/tick.c with the camera linked (c/camera_unit.c's
//! update_camera path), the Rust port runs `simulation::game`, and after every
//! frame both report Mario's words (`tick_trace::capture`) plus every camera
//! word under the names camera_unit.c's oracle_camera_snapshot uses. Nothing
//! is copied between the sides after the level entry.
use crate::{
    Oracle, TickSetup,
    tick_trace::{Words, capture, tick_state},
};
use rustario64::{
    content::animation::MarioAnimations,
    import::version,
    simulation::{
        FixedClock, TickInput,
        camera::system::CameraSystem,
        collision::CollisionWorld,
        game::{Game, GameEntry},
        mario::tick::LevelObjects,
        math::TrigTables,
    },
    trace::{Frame, Metadata, TRACE_SCHEMA, Trace},
};
use std::{collections::BTreeMap, time::Duration};

impl TickSetup {
    /// The native setup for a linked level entry.
    pub fn from_game_entry(entry: &GameEntry) -> Self {
        Self {
            camera_mode: i32::from(entry.camera.mode),
            camera_def_mode: i32::from(entry.camera.mode),
            camera_linked: 1,
            camera_pos: entry.camera.pos,
            camera_focus: entry.camera.focus,
            act_num: i32::from(entry.act_num),
            rng_seed: u32::from(entry.rng_seed),
            ..Self::from_entry(&entry.mario)
        }
    }
}

fn surface(index: Option<u16>) -> i32 {
    index.map_or(-1, i32::from)
}

/// Every camera word, named as oracle_camera_snapshot names it.
pub fn capture_camera(camera: &CameraSystem, o: &mut Words) {
    let r = &camera.rig;
    let c = &r.camera;
    let l = &r.lakitu;
    o.i("camera.c.mode", i32::from(c.mode));
    o.i("camera.c.defMode", i32::from(c.def_mode));
    o.i("camera.c.yaw", i32::from(c.yaw));
    o.f32v("camera.c.focus", &c.focus);
    o.f32v("camera.c.pos", &c.pos);
    o.f("camera.c.areaCenX", camera.radial.center[0]);
    o.f("camera.c.areaCenY", camera.area_center_y);
    o.f("camera.c.areaCenZ", camera.radial.center[1]);
    o.i("camera.c.cutscene", i32::from(c.cutscene));
    o.i("camera.c.nextYaw", i32::from(c.next_yaw));
    o.i("camera.c.doorStatus", i32::from(camera.door_status));
    // gCamera points at the area's camera once reset_camera has run.
    o.i("camera.gCameraIsC", 1);
    o.f32v("camera.lakitu.curFocus", &l.cur_focus);
    o.f32v("camera.lakitu.curPos", &l.cur_pos);
    o.f32v("camera.lakitu.goalFocus", &l.goal_focus);
    o.f32v("camera.lakitu.goalPos", &l.goal_pos);
    o.i("camera.lakitu.mode", i32::from(l.mode));
    o.i("camera.lakitu.defMode", i32::from(l.def_mode));
    o.f("camera.lakitu.focusDistance", l.focus_distance);
    o.i("camera.lakitu.oldPitch", i32::from(l.old_pitch));
    o.i("camera.lakitu.oldYaw", i32::from(l.old_yaw));
    // Never written by the original.
    o.i("camera.lakitu.oldRoll", 0);
    o.s16v("camera.lakitu.shakeMagnitude", &l.shake_magnitude);
    o.i("camera.lakitu.shakePitchPhase", i32::from(l.shake_phase[0]));
    o.i(
        "camera.lakitu.shakePitchVel",
        i32::from(l.shake_velocity[0]),
    );
    o.i("camera.lakitu.shakePitchDecay", i32::from(l.shake_decay[0]));
    // reset_camera zeroes the unused vectors; nothing else writes them.
    o.f32v("camera.lakitu.unusedVec1", &[0.0; 3]);
    o.s16v("camera.lakitu.unusedVec2", &[0; 3]);
    o.i("camera.lakitu.roll", i32::from(l.roll));
    o.i("camera.lakitu.yaw", i32::from(l.yaw));
    o.i("camera.lakitu.nextYaw", i32::from(l.next_yaw));
    o.f32v("camera.lakitu.focus", &l.focus);
    o.f32v("camera.lakitu.pos", &l.pos);
    o.i("camera.lakitu.shakeRollPhase", i32::from(l.shake_phase[2]));
    o.i("camera.lakitu.shakeRollVel", i32::from(l.shake_velocity[2]));
    o.i("camera.lakitu.shakeRollDecay", i32::from(l.shake_decay[2]));
    o.i("camera.lakitu.shakeYawPhase", i32::from(l.shake_phase[1]));
    o.i("camera.lakitu.shakeYawVel", i32::from(l.shake_velocity[1]));
    o.i("camera.lakitu.shakeYawDecay", i32::from(l.shake_decay[1]));
    o.f("camera.lakitu.focHSpeed", l.foc_h_speed);
    o.f("camera.lakitu.focVSpeed", l.foc_v_speed);
    o.f("camera.lakitu.posHSpeed", l.pos_h_speed);
    o.f("camera.lakitu.posVSpeed", l.pos_v_speed);
    o.i("camera.lakitu.keyDanceRoll", i32::from(l.key_dance_roll));
    o.put("camera.lakitu.lastFrameAction", l.last_frame_action);
    // init_camera zeroes it; nothing else writes it.
    o.i("camera.lakitu.unused", 0);
    let t = &r.transition;
    o.i("camera.transition.posPitch", i32::from(t.pos_pitch));
    o.i("camera.transition.posYaw", i32::from(t.pos_yaw));
    o.f("camera.transition.posDist", t.pos_dist);
    o.i("camera.transition.focPitch", i32::from(t.foc_pitch));
    o.i("camera.transition.focYaw", i32::from(t.foc_yaw));
    o.f("camera.transition.focDist", t.foc_dist);
    o.i("camera.transition.framesLeft", t.frames_left);
    o.f32v("camera.transition.marioPos", &t.mario_pos);
    o.i("camera.modeInfo.newMode", i32::from(r.new_mode));
    o.i("camera.modeInfo.lastMode", i32::from(r.last_mode));
    o.i("camera.modeInfo.max", i32::from(camera.mode_info.max));
    o.i("camera.modeInfo.frame", i32::from(camera.mode_info.frame));
    for (name, p) in [
        ("camera.modeInfo.start", &camera.mode_info.start),
        ("camera.modeInfo.end", &camera.mode_info.end),
    ] {
        o.f32v(&format!("{name}.focus"), &p.focus);
        o.f32v(&format!("{name}.pos"), &p.pos);
        o.f(format!("{name}.dist"), p.dist);
        o.i(format!("{name}.pitch"), i32::from(p.pitch));
        o.i(format!("{name}.yaw"), i32::from(p.yaw));
    }
    let g = &camera.geometry;
    o.i("camera.geometry.currFloor", surface(g.curr.floor));
    o.f("camera.geometry.currFloorHeight", g.curr.floor_height);
    o.i(
        "camera.geometry.currFloorType",
        i32::from(g.curr.floor_type),
    );
    o.i("camera.geometry.currCeil", surface(g.curr.ceil));
    o.i("camera.geometry.currCeilType", i32::from(g.curr.ceil_type));
    o.f("camera.geometry.currCeilHeight", g.curr.ceil_height);
    o.i("camera.geometry.prevFloor", surface(g.prev_floor));
    o.f("camera.geometry.prevFloorHeight", g.prev_floor_height);
    o.i(
        "camera.geometry.prevFloorType",
        i32::from(g.prev_floor_type),
    );
    o.i("camera.geometry.prevCeil", surface(g.prev_ceil));
    o.f("camera.geometry.prevCeilHeight", g.prev_ceil_height);
    o.i("camera.geometry.prevCeilType", i32::from(g.prev_ceil_type));
    o.f("camera.geometry.waterHeight", g.curr.water_height);
    o.i("camera.fov.fovFunc", i32::from(camera.fov.func));
    o.f("camera.fov.fov", camera.fov.fov);
    o.f("camera.fov.fovOffset", camera.fov.offset);
    o.put("camera.fov.unusedIsSleeping", camera.fov.unused_is_sleeping);
    o.f("camera.fov.shakeAmplitude", r.fov_shake.amplitude);
    o.i("camera.fov.shakePhase", i32::from(camera.fov.shake_phase));
    o.i("camera.fov.shakeSpeed", i32::from(r.fov_shake.speed));
    o.i("camera.fov.decay", i32::from(r.fov_shake.decay));
    o.i("camera.statusFlags", i32::from(r.status));
    o.i("camera.movementFlags", i32::from(r.movement as i16));
    o.i(
        "camera.s2ndRotateFlags",
        i32::from(camera.radial.second_rotate as i16),
    );
    o.i("camera.selectionFlags", i32::from(camera.selection_flags));
    o.i("camera.soundFlags", i32::from(camera.sound_flags));
    o.i(
        "camera.cButtonsPressed",
        i32::from(camera.c_buttons_pressed),
    );
    o.i("camera.yawSpeed", i32::from(r.yaw_speed));
    o.i("camera.areaYaw", i32::from(camera.area_yaw));
    o.i("camera.areaYawChange", i32::from(r.area_yaw_change));
    o.i("camera.lakituDist", i32::from(r.lakitu_dist));
    o.i("camera.lakituPitch", i32::from(r.lakitu_pitch));
    o.i("camera.modeOffsetYaw", i32::from(r.mode_offset_yaw));
    o.i("camera.cUpCameraPitch", i32::from(r.c_up_pitch));
    o.f("camera.panDistance", r.pan_distance);
    o.f("camera.cannonYOffset", r.cannon_y_offset);
    o.f("camera.zoomAmount", camera.zoom_amount);
    o.f("camera.zeroZoomDist", camera.zero_zoom_dist);
    o.f("camera.zoomDist", camera.zoom_dist);
    o.i("camera.cSideButtonYaw", i32::from(camera.c_side_button_yaw));
    o.i("camera.avoidYawVel", i32::from(camera.avoid_yaw_vel));
    o.i(
        "camera.unusedFreeRoamWallYaw",
        i32::from(camera.unused_free_roam_wall_yaw),
    );
    o.i(
        "camera.yawAfterDoorCutscene",
        i32::from(camera.yaw_after_door_cutscene),
    );
    o.i(
        "camera.behindMarioSoundTimer",
        i32::from(camera.behind_mario_sound_timer),
    );
    o.i(
        "camera.spiralStairsYawOffset",
        i32::from(camera.spiral_stairs_yaw_offset),
    );
    o.i(
        "camera.eightDirBaseYaw",
        i32::from(camera.eight_dir_base_yaw),
    );
    o.i(
        "camera.eightDirYawOffset",
        i32::from(camera.eight_dir_yaw_offset),
    );
    o.i(
        "camera.framesSinceCutsceneEnded",
        i32::from(camera.frames_since_cutscene_ended),
    );
    o.i("camera.recentCutscene", i32::from(camera.recent_cutscene));
    o.i("camera.objectCutscene", i32::from(camera.object_cutscene));
    o.i("camera.framesPaused", i32::from(camera.frames_paused));
    o.i("camera.currLevelArea", camera.radial.area);
    o.put("camera.prevLevel", camera.prev_level);
    o.i(
        "camera.creditsPlayer2Pitch",
        i32::from(camera.credits_player2_pitch),
    );
    o.i(
        "camera.creditsPlayer2Yaw",
        i32::from(camera.credits_player2_yaw),
    );
    o.i(
        "camera.cutsceneSplineSegment",
        i32::from(camera.cutscene_spline_segment),
    );
    o.f(
        "camera.cutsceneSplineSegmentProgress",
        camera.cutscene_spline_segment_progress,
    );
    o.i("camera.cutsceneShot", i32::from(camera.cutscene_shot));
    o.i("camera.cutsceneTimer", i32::from(camera.cutscene_timer));
    o.put("camera.cutsceneObjSpawn", camera.cutscene_obj_spawn);
    o.i("camera.objCutsceneDone", camera.obj_cutscene_done);
    o.i("camera.cutsceneFocusIsNull", 1);
    o.i("camera.secondCameraFocusIsNull", 1);
    o.put("camera.unused8032CFC8", camera.unused_8032cfc8);
    o.put("camera.unused8032CFCC", camera.unused_8032cfcc);
    o.i("camera.unused8033B316", i32::from(camera.unused_8033b316));
    o.i("camera.unused8033B31A", i32::from(camera.unused_8033b31a));
    o.put("camera.unused8033B30C", camera.unused_8033b30c);
    o.put("camera.unused8033B310", camera.unused_8033b310);
    o.i("camera.unused8033B6E8", i32::from(camera.unused_8033b6e8));
    o.f32v("camera.oldPosition", &r.old_pos);
    o.f32v("camera.oldFocus", &r.old_focus);
    o.f32v("camera.player2FocusOffset", &r.player2_focus_offset);
    o.f32v(
        "camera.castleEntranceOffset",
        &camera.castle_entrance_offset,
    );
    o.f32v(
        "camera.fixedModeBasePosition",
        &camera.fixed_mode_base_position,
    );
    o.f32v("camera.storeCUp.pos", &camera.store_c_up.pos);
    o.f32v("camera.storeCUp.focus", &camera.store_c_up.focus);
    o.f("camera.storeCUp.panDist", camera.store_c_up.pan_dist);
    o.f(
        "camera.storeCUp.cannonYOffset",
        camera.store_c_up.cannon_y_offset,
    );
    for i in 0..4 {
        o.i(
            format!("camera.handheld.spline[{i}].index"),
            i32::from(r.handheld_spline_index[i]),
        );
        o.s16v(
            &format!("camera.handheld.spline[{i}].point"),
            &r.handheld_spline[i],
        );
    }
    o.i("camera.handheld.mag", i32::from(r.handheld_magnitude));
    o.f("camera.handheld.timer", r.handheld_timer);
    o.f("camera.handheld.inc", r.handheld_increment);
    o.i("camera.handheld.pitch", i32::from(r.handheld_angles[0]));
    o.i("camera.handheld.yaw", i32::from(r.handheld_angles[1]));
    o.i("camera.handheld.roll", i32::from(r.handheld_angles[2]));
    for i in 0..32 {
        o.i(
            format!("camera.creditsSplinePos[{i}].index"),
            i32::from(camera.credits_spline_pos_index[i]),
        );
        o.i(
            format!("camera.creditsSplineFocus[{i}].index"),
            i32::from(camera.credits_spline_focus_index[i]),
        );
    }
    o.i("camera.hudStatus", i32::from(camera.hud_status));
    o.f32v("camera.graph.pos", &camera.graph.pos);
    o.f32v("camera.graph.focus", &camera.graph.focus);
    o.i(
        "camera.graph.rollScreen",
        i32::from(camera.graph.roll_screen),
    );
    o.f("camera.graph.fov", camera.graph.fov);
}

/// Mario's words and the camera's after a frame (or the entry).
pub fn capture_game(game: &Game<'_>) -> BTreeMap<String, u32> {
    let mut o = Words(capture(&game.mario, &game.world));
    capture_camera(&game.camera, &mut o);
    o.i("camera.rngSeed", i32::from(game.world.rng.seed));
    o.0
}

/// One linked scenario: terrain, tables, animations and an entry.
pub struct GameScenario<'a> {
    pub collision: &'a CollisionWorld,
    pub trig: &'a TrigTables,
    pub anims: &'a MarioAnimations,
    /// The level's objects (scripts, models and the area's placements).
    pub objects: LevelObjects<'a>,
    pub entry: GameEntry,
}

/// Why a Rust run ended before its inputs did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustStop {
    /// A path the port panics on (an object, NULL dereference).
    Panic(String),
    /// A camera path this port does not implement (a mode, a cutscene).
    Camera(String),
}

impl GameScenario<'_> {
    pub fn setup(&self) -> TickSetup {
        TickSetup::from_game_entry(&self.entry)
    }

    /// The native decomp's words after the entry and after each frame. The
    /// oracle must have this scenario's terrain loaded and animations set.
    pub fn native(&self, oracle: &Oracle, inputs: &[TickInput]) -> Vec<BTreeMap<String, u32>> {
        oracle.tick_set_objects(&crate::object_trace::NativeObjects::new(&self.objects));
        oracle.tick_begin(&self.setup());
        let mut out = vec![oracle.tick_snapshot()];
        for input in inputs {
            oracle.tick(*input);
            out.push(oracle.tick_snapshot());
        }
        out
    }

    /// The Rust words after the entry and after each frame, until the first
    /// frame that reaches an unsupported path. That frame's words are kept
    /// for a camera stop (the frame completes) but not for a panic.
    pub fn rust(&self, inputs: &[TickInput]) -> (Vec<BTreeMap<String, u32>>, Option<RustStop>) {
        let mut game = Game::enter(
            self.collision,
            self.trig,
            self.anims,
            &self.objects,
            &self.entry,
        );
        let mut out = vec![capture_game(&game)];
        for input in inputs {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (_, result) = game.frame(input.buttons, input.stick);
                (capture_game(&game), result)
            }));
            match result {
                Ok((words, result)) => {
                    out.push(words);
                    if let Err(what) = result.camera {
                        return (
                            out,
                            Some(RustStop::Camera(rustario64::simulation::game::describe(
                                what,
                            ))),
                        );
                    }
                }
                Err(payload) => {
                    let message = payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                        .unwrap_or_else(|| "non-string panic".into());
                    return (out, Some(RustStop::Panic(message)));
                }
            }
        }
        (out, None)
    }

    /// The Rust frames driven by the fixed clock at `render_hz`, with the
    /// camera's graph position read every displayed frame: presentation
    /// reads completed frames only, so the words must equal `rust`'s.
    pub fn rust_at(&self, inputs: &[TickInput], render_hz: u32) -> Vec<BTreeMap<String, u32>> {
        assert!((1..=1000).contains(&render_hz));
        let mut game = Game::enter(
            self.collision,
            self.trig,
            self.anims,
            &self.objects,
            &self.entry,
        );
        let mut out = vec![capture_game(&game)];
        let mut clock = FixedClock::default();
        let (mut elapsed, mut frame) = (0u64, 0u64);
        let mut previous = game.camera.graph;
        while out.len() <= inputs.len() {
            frame += 1;
            let end = frame * 1_000_000_000 / u64::from(render_hz);
            clock
                .add_elapsed(Duration::from_nanos(end - elapsed))
                .unwrap();
            elapsed = end;
            let budget = (inputs.len() + 1 - out.len()).min(8) as u32;
            for _ in 0..clock.drain(budget) {
                let input = inputs[out.len() - 1];
                previous = game.camera.graph;
                let (_, result) = game.frame(input.buttons, input.stick);
                assert_eq!(result.camera, Ok(()), "multi-rate runs use supported paths");
                out.push(capture_game(&game));
            }
            // A presentation read between completed frames.
            let alpha = clock.alpha();
            let current = game.camera.graph;
            std::hint::black_box(std::array::from_fn::<f32, 3, _>(|i| {
                previous.pos[i] + (current.pos[i] - previous.pos[i]) * alpha
            }));
        }
        out
    }
}

/// The first differing word between two frames, by name.
pub fn first_difference(
    expected: &BTreeMap<String, u32>,
    actual: &BTreeMap<String, u32>,
) -> Option<String> {
    for (name, value) in expected {
        match actual.get(name) {
            None => return Some(format!("{name}: missing from the Rust words")),
            Some(other) if other != value => {
                return Some(format!(
                    "{name}: native {value:#010x} ({}), Rust {other:#010x} ({})",
                    f32::from_bits(*value),
                    f32::from_bits(*other)
                ));
            }
            _ => {}
        }
    }
    actual
        .keys()
        .find(|name| !expected.contains_key(*name))
        .map(|name| format!("{name}: missing from the native words"))
}

/// A schema-1 trace of linked frames: `words[0]` is the entry, `words[i]`
/// follows `inputs[i - 1]`.
pub fn game_trace(
    producer: &str,
    rom_sha1: &str,
    world_digest: &str,
    scenario: &str,
    entry: &GameEntry,
    inputs: &[TickInput],
    words: &[BTreeMap<String, u32>],
) -> Trace {
    assert_eq!(words.len(), inputs.len() + 1);
    Trace {
        schema: TRACE_SCHEMA,
        producer: producer.into(),
        metadata: Metadata {
            rom_sha1: rom_sha1.into(),
            reference_revision: version::REFERENCE_REVISION.into(),
            reference_configuration: "full-frame-camera-v1; native C IEEE/fwrapv/no-FMA/AVOID_UB; \
                Mario, supported objects, authoritative render-pass writes and the area camera \
                (update_camera, no triggers); level entry \
                without warp from a fresh boot; camera yaw produced by the camera; sound, warp \
                and particle calls recorded as events; debug pages off"
                .into(),
            scenario: scenario.into(),
            tick_rate: 30,
            course: 1,
            area: entry.mario.spawn.area_index as u8,
            act: entry.act_num as u8,
            initial_world_digest: world_digest.into(),
            gameplay_options: BTreeMap::new(),
            initial_state: tick_state(words[0].clone()),
        },
        frames: inputs
            .iter()
            .zip(&words[1..])
            .enumerate()
            .map(|(i, (input, words))| Frame {
                tick: i as u64 + 1,
                input: *input,
                state: tick_state(words.clone()),
            })
            .collect(),
    }
}
