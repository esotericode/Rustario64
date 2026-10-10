//! One frame of the original game loop in Mario's area with the original
//! camera: the controller read, the object update, Mario's requests to the
//! camera, update_hud_values, update_camera, and the render pass's authoritative stages (the
//! camera callbacks and Mario's animation frame), in the order of pinned CC0
//! game_init.c, level_update.c and rendering_graph_node.c. The camera's yaw,
//! which Mario's controls read, comes from the previous frame's update.
//!
//! Mario reports his state to the camera (gPlayerCameraState) only at the end
//! of his update, and the camera functions he calls (set_camera_mode,
//! set_camera_shake_from_hit) change camera state that the rest of his update
//! does not read, except the camera mode. So the requests run after his
//! update, in call order, against the report from before it, while
//! `StepWorld::set_camera_mode` changes the mode Mario reads at the call; the
//! frame checks that both agree. See docs/DECISIONS.md.
use crate::{
    content::{ImportedLevel, animation::MarioAnimations, visual},
    import::version::AREA_CAMERA_CALLBACKS,
    simulation::{
        TickInput,
        camera::system::{self, CameraSystem, Frame, GeoCamera, Level, MarioView, Unsupported},
        collision::CollisionWorld,
        hud::update_hud_values,
        mario::{
            Event, MarioState, PlayerCameraState, StepWorld,
            constants::{ACTIVE_FLAG_MOVE_THROUGH_GRATE, MARIO_VANISH_CAP},
            tick::{
                LevelEntry, LevelObjects, RenderedFrame, enter_level_with, render_mario_object,
                update_objects,
            },
        },
        math::TrigTables,
        object::{object, render::render_objects},
        rng::Rng,
    },
};

/// A level entry with the area's camera node: Mario's entry, the GEO_CAMERA
/// node that creates the camera, the act, and the random seed at entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameEntry {
    pub mario: LevelEntry,
    pub camera: GeoCamera,
    pub act_num: i16,
    pub rng_seed: u16,
}

impl GameEntry {
    /// An imported level's script start in its start area, whose camera node
    /// must call the original geo_camera_main and geo_camera_fov (the
    /// version adapter's addresses). Fresh boot: act 1, seed 0.
    pub fn script_start(level: &ImportedLevel, camera: &visual::GeoCamera) -> Result<Self, String> {
        let mario = LevelEntry::script_start(level, camera.mode).map_err(String::from)?;
        let name = |address: u32| {
            AREA_CAMERA_CALLBACKS
                .iter()
                .find(|(_, a)| *a == address)
                .map(|(name, _)| *name)
        };
        if name(camera.callback) != Some("geo_camera_main") {
            return Err(format!(
                "camera node callback 0x{:08X} is not geo_camera_main",
                camera.callback
            ));
        }
        match camera.perspective_callback.map(name) {
            Some(Some("geo_camera_fov")) => {}
            other => {
                return Err(format!(
                    "perspective node callback {other:?} is not geo_camera_fov"
                ));
            }
        }
        Ok(Self {
            mario,
            camera: GeoCamera {
                mode: mario.camera_mode,
                pos: camera.position.map(f32::from),
                focus: camera.focus.map(f32::from),
            },
            act_num: 1,
            rng_seed: 0,
        })
    }
}

/// What one frame produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameResult {
    pub rendered: RenderedFrame,
    /// A camera path this port does not implement, reached this frame.
    pub camera: Result<(), Unsupported>,
}

/// Mario, the area's objects and its camera from one level entry. The
/// shared random seed (gRandomSeed16) is `world.rng`.
pub struct Game<'a> {
    pub mario: MarioState,
    pub world: StepWorld<'a>,
    pub camera: CameraSystem,
}

impl<'a> Game<'a> {
    /// Enter the level: the area loads (create_camera), the save file is
    /// initialized (select_mario_cam_mode), Mario is spawned and initialized,
    /// init_level's reset_camera runs, then Mario idles.
    pub fn enter(
        collision: &'a CollisionWorld,
        trig: &'a TrigTables,
        anims: &'a MarioAnimations,
        objects: &LevelObjects<'a>,
        entry: &GameEntry,
    ) -> Self {
        let mut camera = CameraSystem::create(
            Level {
                level_num: entry.mario.level_num,
                area_index: entry.mario.spawn.area_index,
                act_num: entry.act_num,
            },
            entry.camera,
        );
        let (mario, world) = enter_level_with(
            collision,
            trig,
            anims,
            objects,
            Rng::new(entry.rng_seed),
            &entry.mario,
            |m, w| {
                camera.reset(&mut m.camera_status);
                share(&camera, w);
            },
        );
        Self {
            mario,
            world,
            camera,
        }
    }

    /// One frame with the controller's buttons and raw stick. The tick input
    /// it returns records the camera yaw Mario read.
    pub fn frame(&mut self, buttons: u16, stick: [i8; 2]) -> (TickInput, FrameResult) {
        let (m, w, camera) = (&mut self.mario, &mut self.world, &mut self.camera);
        w.events.clear();
        share(camera, w);
        let input = TickInput {
            buttons,
            stick,
            camera_yaw: camera.yaw(),
        };
        // read_controller_inputs; Mario reads the camera's last yaw.
        w.controller.sample(input);
        w.camera.yaw = input.camera_yaw;
        // area_update_objects.
        w.area_update_counter = w.area_update_counter.wrapping_add(1);
        let reported = m.camera_status;
        update_objects(m, w);
        camera.rig.movement = w.camera_movement_flags as u16;
        // Mario's requests, in call order, each followed by what it produced.
        let mario_events = std::mem::take(&mut w.events);
        for event in mario_events {
            w.events.push(event);
            if !matches!(event, Event::CameraMode { .. } | Event::CameraShake(_)) {
                continue;
            }
            let mut status = PlayerCameraState {
                action: reported.action,
                pos: reported.pos,
                face_angle: reported.face_angle,
                ..m.camera_status
            };
            let mario = view(m, w);
            let mut frame = Frame {
                world: w.collision,
                trig: w.trig,
                flags: &mut w.collision_flags,
                controller: &w.controller,
                mario_cam: &mut status,
                mario,
                rng: &mut w.rng,
                events: &mut w.events,
            };
            match event {
                Event::CameraMode { mode, frames } => {
                    camera.set_camera_mode(mode, frames, &mut frame)
                }
                Event::CameraShake(shake) => camera.shake_from_hit(shake, &mut frame),
                _ => unreachable!(),
            }
            m.camera_status = PlayerCameraState {
                action: m.camera_status.action,
                pos: m.camera_status.pos,
                face_angle: m.camera_status.face_angle,
                ..status
            };
        }
        assert!(
            camera.rig.camera.mode == w.camera.mode && camera.rig.last_mode == w.camera.last_mode,
            "Mario's view of the camera mode diverged from the camera's"
        );
        w.camera_movement_flags = camera.rig.movement as i16;
        update_hud_values(m, w);
        // update_camera.
        let mario = view(m, w);
        let result = camera.update(&mut Frame {
            world: w.collision,
            trig: w.trig,
            flags: &mut w.collision_flags,
            controller: &w.controller,
            mario_cam: &mut m.camera_status,
            mario,
            rng: &mut w.rng,
            events: &mut w.events,
        });
        share(camera, w);
        // render_game: the camera nodes enclose the object nodes, Mario's first.
        camera.render(m.action, m.camera_status.pos, w.trig);
        let rendered = render_mario_object(&mut m.obj, w);
        render_objects(w, &camera.graph);
        // display_and_vsync.
        w.global_timer = w.global_timer.wrapping_add(1);
        (
            input,
            FrameResult {
                rendered,
                camera: result,
            },
        )
    }
}

/// What the camera reads from gMarioStates[0], and whether its wall queries
/// pass vanish-cap walls: gCurrentObject is the last object the processor
/// visited (unload_deactivated_objects ends on the last listed object).
fn view(m: &MarioState, w: &StepWorld<'_>) -> MarioView {
    let current = w.objects.current;
    let pass_vanish_walls = current.is_some_and(|id| {
        object(&w.objects, &m.obj, id).active_flags & ACTIVE_FLAG_MOVE_THROUGH_GRATE != 0
            || (current == w.objects.mario && m.flags & MARIO_VANISH_CAP != 0)
    });
    MarioView {
        action: m.action,
        forward_vel: m.forward_vel,
        angle_vel_yaw: m.angle_vel[1],
        pass_vanish_walls,
    }
}

/// The camera globals Mario's code shares with the camera.
fn share(camera: &CameraSystem, w: &mut StepWorld<'_>) {
    let c = &camera.rig.camera;
    w.camera.mode = c.mode;
    w.camera.def_mode = c.def_mode;
    w.camera.yaw = c.yaw;
    w.camera.linked = true;
    w.camera.last_mode = camera.rig.last_mode;
    w.camera.level_area = camera.radial.area;
    w.camera_movement_flags = camera.rig.movement as i16;
}

/// The unsupported camera path, for messages.
pub fn describe(what: Unsupported) -> String {
    match what {
        system::Unsupported::Mode(mode) => format!("camera mode {mode} is not ported"),
        system::Unsupported::Cutscene(cutscene) => {
            format!("camera cutscene {cutscene:#x} is not ported")
        }
        system::Unsupported::Triggers(level) => {
            format!("level {level}'s camera triggers are not ported")
        }
        system::Unsupported::LevelInit(level) => {
            format!("level {level}'s camera entry cutscene is not ported")
        }
    }
}
