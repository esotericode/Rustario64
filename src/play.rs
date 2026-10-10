//! Playing from held controls, as the development viewer does. A session runs
//! the same level entry and frame that the oracle compares with the decomp
//! (`simulation::game`): held controls become each frame's buttons and stick,
//! Mario's update reads the yaw the original camera produced last frame, and
//! the camera then updates from Mario's report. Every frame's input is kept
//! (with the camera yaw Mario read), so a session can be replayed against the
//! reference (`trace::InputLog`). Presentation reads completed frames only:
//! Mario's pose and the camera's position, focus, roll and field of view are
//! interpolated between the last two.
use crate::{
    content::animation::MarioAnimations,
    presentation::{self, GraphicsOptions, Pose, Snapshot, mario::MarioPose},
    simulation::{
        TICKS_PER_SECOND, TickInput,
        camera::{
            D_CBUTTONS, L_CBUTTONS, R_CBUTTONS, U_CBUTTONS,
            system::{self, GraphCamera},
        },
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, R_TRIG, Z_TRIG},
        game::{self, Game, GameEntry},
        mario::{
            Event, MarioState, StepWorld, Unsupported, constants,
            tick::{LevelObjects, RenderedFrame},
        },
        math::TrigTables,
    },
    trace::{CameraInput, INPUT_LOG_SCHEMA, InputLog},
};
use std::{fmt, panic};

/// The input-log name of Bob-omb Battlefield's level-script start, the entry
/// `GameEntry::script_start` builds from BOB's import.
pub const BOB_SCRIPT_START: &str = "bob-script-start";

/// Held controls: a digital stick with a walk modifier or raw analog bytes,
/// the A, B, Z and R buttons, and the four C buttons.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pad {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    /// Partial deflection, for walking.
    pub walk: bool,
    /// Desktop analog conversion, before the original controller dead zone.
    /// Any held digital direction takes priority over this sample.
    pub analog_stick: Option<[i8; 2]>,
    pub a: bool,
    pub b: bool,
    pub z: bool,
    pub r: bool,
    pub c_up: bool,
    pub c_down: bool,
    pub c_left: bool,
    pub c_right: bool,
}

impl Pad {
    /// Raw stick bytes in the held direction. Full deflection (80, or 57 per
    /// axis on diagonals) is past adjust_analog_stick's dead-zone offset and
    /// 64-unit clamp, so every direction gives the full magnitude; walking
    /// uses 40 (28 on diagonals), about half the magnitude.
    pub fn stick(&self) -> [i8; 2] {
        if !(self.up || self.down || self.left || self.right) {
            return self.analog_stick.unwrap_or([0, 0]);
        }
        let x = i8::from(self.right) - i8::from(self.left);
        let y = i8::from(self.up) - i8::from(self.down);
        let reach = match (x != 0 && y != 0, self.walk) {
            (false, false) => 80,
            (true, false) => 57,
            (false, true) => 40,
            (true, true) => 28,
        };
        [x * reach, y * reach]
    }

    pub fn buttons(&self) -> u16 {
        [
            (self.a, A_BUTTON),
            (self.b, B_BUTTON),
            (self.z, Z_TRIG),
            (self.r, R_TRIG),
            (self.c_up, U_CBUTTONS),
            (self.c_down, D_CBUTTONS),
            (self.c_left, L_CBUTTONS),
            (self.c_right, R_CBUTTONS),
        ]
        .into_iter()
        .filter(|(held, _)| *held)
        .fold(0, |buttons, (_, bit)| buttons | bit)
    }

    /// Held buttons, plus the ones in `taps` (pressed since the last frame,
    /// perhaps released already): a tap shorter than a frame still reaches
    /// the next frame, as a press longer than a frame would.
    pub fn with_taps(&self, taps: &Pad) -> Pad {
        Pad {
            a: self.a || taps.a,
            b: self.b || taps.b,
            z: self.z || taps.z,
            r: self.r || taps.r,
            c_up: self.c_up || taps.c_up,
            c_down: self.c_down || taps.c_down,
            c_left: self.c_left || taps.c_left,
            c_right: self.c_right || taps.c_right,
            ..*self
        }
    }

    /// Combine independent devices. Digital movement wins over analog movement;
    /// buttons are ORed, so releasing one device cannot release the other.
    pub fn combined(&self, other: &Pad) -> Pad {
        Pad {
            up: self.up || other.up,
            down: self.down || other.down,
            left: self.left || other.left,
            right: self.right || other.right,
            walk: self.walk || other.walk,
            analog_stick: self.analog_stick.or(other.analog_stick),
            ..self.with_taps(other)
        }
    }
}

/// Raw axis conversion: finite host axes, positive Y up, circular unit clamp,
/// then round to raw N64 bytes at radius 80. The desktop adapter applies its
/// center dead zone first; the reference controller processes these bytes.
/// Replays store these bytes, not host events or device identity.
pub fn analog_stick(x: f32, y: f32) -> [i8; 2] {
    let finite = |v: f32| {
        if v.is_finite() {
            v.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    let (x, y) = (finite(x), finite(y));
    let radius = (x * x + y * y).sqrt().max(1.0);
    [
        (x / radius * 80.0).round() as i8,
        (y / radius * 80.0).round() as i8,
    ]
}

/// Why a session stopped. The port panics on paths it does not implement
/// rather than invent behavior, and the level runtime that performs warps is
/// not ported, so play stops there until the level is entered again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// A path the port does not implement; the panic message names it.
    Panic(String),
    /// An action group the port does not implement.
    Unsupported(Unsupported),
    /// A camera mode, cutscene or level the camera port does not implement.
    Camera(system::Unsupported),
    /// A warp request (the warp operation), such as a death plane's.
    Warp(i32),
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stop::Panic(message) => write!(f, "unsupported path: {message}"),
            Stop::Unsupported(Unsupported::CutsceneGroup(action)) => {
                write!(f, "cutscene action {} is not ported", action_name(*action))
            }
            Stop::Unsupported(Unsupported::SubmergedGroup(action)) => {
                write!(f, "water action {} is not ported", action_name(*action))
            }
            Stop::Unsupported(Unsupported::InfiniteStairs) => {
                write!(f, "the endless stairs are not ported")
            }
            Stop::Unsupported(Unsupported::HundredCoinStar) => {
                write!(f, "the 100-coin star is not ported")
            }
            Stop::Camera(what) => write!(f, "{}", game::describe(*what)),
            Stop::Warp(op) => write!(f, "warp operation {op:#x} requested; warps are not ported"),
        }
    }
}

/// An action's original name, or "?".
pub fn action_name(action: u32) -> &'static str {
    constants::ALL
        .iter()
        .find(|(name, value)| {
            name.starts_with("ACT_")
                && !name.starts_with("ACT_FLAG_")
                && !name.starts_with("ACT_GROUP_")
                && !name.ends_with("_MASK")
                && *value == i64::from(action)
        })
        .map_or("?", |(name, _)| name)
}

/// A camera mode's original name, or "?".
pub fn camera_mode_name(mode: u8) -> &'static str {
    constants::ALL
        .iter()
        .find(|(name, value)| name.starts_with("CAMERA_MODE_") && *value == i64::from(mode))
        .map_or("?", |(name, _)| name)
}

fn snapshot(m: &MarioState, epoch: u64) -> Snapshot {
    Snapshot {
        entity: 1,
        epoch,
        // The original draws Mario's object at its graphics position and yaw.
        position: m.obj.gfx.pos,
        yaw: m.obj.gfx.angle[1],
        // Clip IDs are not world discontinuities. The placeholder's position
        // follows the same tick interval as Mario's model and the camera.
        animation: 0,
        discontinuity: false,
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "non-string panic".into())
}

/// Where presentation draws the scene from: the area camera graph node and
/// its perspective node between two completed frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraView {
    pub pos: [f32; 3],
    pub focus: [f32; 3],
    /// rollScreen, in original angle units.
    pub roll: i16,
    /// The vertical field of view in degrees.
    pub fov: f32,
}

/// A camera move larger than this within one frame (a cut, such as the
/// radial camera's jump to the far side) is drawn without interpolation.
pub const CAMERA_CUT_DISTANCE: f32 = 1500.0;

fn camera_view(graph: &GraphCamera) -> CameraView {
    CameraView {
        pos: graph.pos,
        focus: graph.focus,
        roll: graph.roll_screen,
        fov: graph.fov,
    }
}

/// The camera between `previous` and `current`: positions and the field of
/// view linearly, the roll along the shorter way around. Cuts snap.
pub fn interpolate_camera(
    previous: Option<&GraphCamera>,
    current: &GraphCamera,
    alpha: f32,
    options: GraphicsOptions,
) -> CameraView {
    let alpha = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let Some(previous) = previous.filter(|_| options.interpolation) else {
        return camera_view(current);
    };
    let distance = |a: [f32; 3], b: [f32; 3]| {
        let d: [f32; 3] = std::array::from_fn(|i| a[i] - b[i]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    };
    if distance(previous.pos, current.pos) > CAMERA_CUT_DISTANCE
        || distance(previous.focus, current.focus) > CAMERA_CUT_DISTANCE
    {
        return camera_view(current);
    }
    let lerp = |a: f32, b: f32| a + (b - a) * alpha;
    let roll_delta = current.roll_screen.wrapping_sub(previous.roll_screen);
    CameraView {
        pos: std::array::from_fn(|i| lerp(previous.pos[i], current.pos[i])),
        focus: std::array::from_fn(|i| lerp(previous.focus[i], current.focus[i])),
        roll: previous
            .roll_screen
            .wrapping_add((f32::from(roll_delta) * alpha) as i16),
        fov: lerp(previous.fov, current.fov),
    }
}

/// Mario and his area's original camera from one level entry: the compared
/// frame, every frame's input, and the last two completed frames for
/// presentation.
pub struct Session<'a> {
    collision: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    objects: LevelObjects<'a>,
    entry: GameEntry,
    game: Game<'a>,
    inputs: Vec<TickInput>,
    stopped: Option<Stop>,
    rendered: RenderedFrame,
    epoch: u64,
    previous: Option<Snapshot>,
    current: Snapshot,
    camera_previous: Option<GraphCamera>,
}

impl<'a> Session<'a> {
    pub fn new(
        collision: &'a CollisionWorld,
        trig: &'a TrigTables,
        anims: &'a MarioAnimations,
        objects: LevelObjects<'a>,
        entry: GameEntry,
    ) -> Self {
        let game = Game::enter(collision, trig, anims, &objects, &entry);
        let current = snapshot(&game.mario, 0);
        Self {
            collision,
            trig,
            anims,
            objects,
            entry,
            game,
            inputs: vec![],
            stopped: None,
            // The level's first frame renders after Mario's first update, so
            // the entry state is never drawn.
            rendered: RenderedFrame::default(),
            epoch: 0,
            previous: None,
            current,
            camera_previous: None,
        }
    }

    /// Enter the level again. The input log restarts and presentation snaps.
    pub fn reset(&mut self) {
        self.game = Game::enter(
            self.collision,
            self.trig,
            self.anims,
            &self.objects,
            &self.entry,
        );
        self.inputs.clear();
        self.stopped = None;
        self.rendered = RenderedFrame::default();
        self.epoch += 1;
        self.previous = None;
        self.current = snapshot(&self.game.mario, self.epoch);
        self.camera_previous = None;
    }

    /// Hold the latest presentation after a pause without changing gameplay,
    /// RNG, input history, or the camera's authoritative state.
    pub fn snap_presentation(&mut self) {
        self.previous = None;
        self.camera_previous = None;
    }

    /// One 30 Hz frame with the held controls, unless stopped; returns
    /// whether it ran. A frame that stops the session is still logged, so a
    /// replay reaches the same stop.
    pub fn step(&mut self, pad: &Pad) -> bool {
        if self.stopped.is_some() {
            return false;
        }
        let camera_before = self.game.camera.graph;
        let game = &mut self.game;
        let (buttons, stick) = (pad.buttons(), pad.stick());
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| game.frame(buttons, stick)));
        let (input, frame) = match result {
            Ok(result) => result,
            Err(payload) => {
                // The state is partly updated; it is shown but never ticked
                // again. The input still reached the frame.
                self.inputs.push(TickInput {
                    buttons,
                    stick,
                    camera_yaw: self.game.world.camera.yaw,
                });
                self.stopped = Some(Stop::Panic(panic_message(payload)));
                return true;
            }
        };
        self.inputs.push(input);
        self.rendered = frame.rendered;
        self.stopped = self
            .game
            .world
            .events
            .iter()
            .find_map(|event| match *event {
                Event::Unsupported(what) => Some(Stop::Unsupported(what)),
                Event::Warp(op) => Some(Stop::Warp(op)),
                _ => None,
            })
            .or_else(|| frame.camera.err().map(Stop::Camera));
        self.previous = Some(self.current);
        self.current = snapshot(&self.game.mario, self.epoch);
        self.camera_previous = Some(camera_before);
        true
    }

    /// What drawing Mario's model reads from the latest completed frame.
    pub fn mario_pose(&self) -> MarioPose {
        MarioPose::capture(&self.game.mario, &self.game.world, self.rendered)
    }

    /// Mario's presentation pose between the last two completed frames.
    pub fn pose(&self, alpha: f32, options: GraphicsOptions) -> Pose {
        presentation::interpolate(self.previous.as_ref(), &self.current, alpha, options)
    }

    /// The camera between the last two completed frames.
    pub fn camera_view(&self, alpha: f32, options: GraphicsOptions) -> CameraView {
        interpolate_camera(
            self.camera_previous.as_ref(),
            &self.game.camera.graph,
            alpha,
            options,
        )
    }

    pub fn entry(&self) -> &GameEntry {
        &self.entry
    }

    pub fn game(&self) -> &Game<'a> {
        &self.game
    }

    pub fn mario(&self) -> &MarioState {
        &self.game.mario
    }

    pub fn world(&self) -> &StepWorld<'a> {
        &self.game.world
    }

    /// Every frame's input since the level entry.
    pub fn inputs(&self) -> &[TickInput] {
        &self.inputs
    }

    pub fn stopped(&self) -> Option<&Stop> {
        self.stopped.as_ref()
    }

    /// This session's inputs for replay against the reference. `entry` names
    /// the level entry (for example `bob-script-start`).
    pub fn input_log(&self, producer: &str, rom_sha1: &str, entry: &str) -> InputLog {
        InputLog {
            schema: INPUT_LOG_SCHEMA,
            producer: producer.into(),
            rom_sha1: rom_sha1.into(),
            entry: entry.into(),
            tick_rate: TICKS_PER_SECOND,
            camera: CameraInput::Reference,
            inputs: self.inputs.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::object::{render::authored_models, script::authored_scripts};
    use crate::{
        content::{CollisionMesh, Triangle, animation::Animation},
        simulation::{
            camera::system::GeoCamera,
            mario::{
                constants::{ACT_IDLE, ACT_WALKING, LEVEL_BOB},
                tick::LevelEntry,
            },
            math::{ARCTAN_ENTRIES, SINE_ENTRIES},
        },
    };
    use std::f64::consts::{PI, TAU};

    /// Computed approximations of the tables' shape; not the game's values.
    fn tables() -> TrigTables {
        let sine = (0..SINE_ENTRIES)
            .map(|i| (i as f64 * TAU / 4096.0).sin() as f32)
            .collect();
        let arctan = (0..ARCTAN_ENTRIES)
            .map(|i| ((i as f64 / 1024.0).atan() * 32768.0 / PI).round() as u16)
            .collect();
        TrigTables::new(sine, arctan).unwrap()
    }

    /// One still, two-frame looping animation per MARIO_ANIM_* ID; authored,
    /// not the game's animations.
    fn still_animations() -> MarioAnimations {
        let bone_count = 20;
        let attributes = 3 + 3 * bone_count as usize;
        let still = Animation {
            flags: 0,
            y_trans_divisor: 189,
            start_frame: 0,
            loop_start: 0,
            loop_end: 2,
            bone_count,
            index: (0..attributes).flat_map(|_| [1, 0]).collect(),
            values: vec![0],
        };
        MarioAnimations {
            animations: vec![still; 0xD1],
        }
    }

    #[test]
    fn pad_maps_held_keys_to_stick_bytes_and_buttons() {
        let pad = Pad {
            up: true,
            right: true,
            a: true,
            z: true,
            ..Pad::default()
        };
        assert_eq!(pad.stick(), [57, 57]);
        assert_eq!(pad.buttons(), A_BUTTON | Z_TRIG);
        let pad = Pad {
            down: true,
            r: true,
            c_up: true,
            c_down: true,
            c_left: true,
            c_right: true,
            ..Pad::default()
        };
        assert_eq!(pad.stick(), [0, -80]);
        assert_eq!(
            pad.buttons(),
            R_TRIG | U_CBUTTONS | D_CBUTTONS | L_CBUTTONS | R_CBUTTONS
        );
        let pad = Pad {
            left: true,
            walk: true,
            ..Pad::default()
        };
        assert_eq!(pad.stick(), [-40, 0]);
        // Opposite keys cancel.
        let pad = Pad {
            left: true,
            right: true,
            ..Pad::default()
        };
        assert_eq!(pad.stick(), [0, 0]);
        // Taps reach the next frame even when released already.
        let held = Pad {
            up: true,
            ..Pad::default()
        };
        let taps = Pad {
            a: true,
            c_left: true,
            ..Pad::default()
        };
        assert_eq!(held.with_taps(&taps).buttons(), A_BUTTON | L_CBUTTONS);
        assert!(held.with_taps(&taps).up);
    }

    #[test]
    fn desktop_analog_profile_and_device_merge_preserve_raw_inputs() {
        assert_eq!(analog_stick(1.0, 0.0), [80, 0]);
        assert_eq!(analog_stick(-1.0, -1.0), [-57, -57]);
        assert_eq!(analog_stick(0.1, 0.5), [8, 40]);
        assert_eq!(analog_stick(f32::NAN, f32::INFINITY), [0, 0]);
        let analog = Pad {
            analog_stick: Some([12, 36]),
            a: true,
            z: true,
            ..Pad::default()
        };
        let keyboard = Pad {
            left: true,
            b: true,
            ..Pad::default()
        };
        assert_eq!(Pad::default().combined(&analog).stick(), [12, 36]);
        let combined = keyboard.combined(&analog);
        assert_eq!(combined.stick(), [-80, 0]);
        assert_eq!(combined.buttons(), A_BUTTON | B_BUTTON | Z_TRIG);
        assert!(analog.combined(&Pad::default()).a);
        assert!(
            Pad {
                a: true,
                ..Pad::default()
            }
            .combined(&Pad::default())
            .a
        );
    }

    #[test]
    fn camera_view_interpolates_and_snaps_on_cuts() {
        let a = GraphCamera {
            pos: [0.0, 100.0, 0.0],
            focus: [0.0, 0.0, -100.0],
            roll_screen: i16::MAX - 10,
            fov: 45.0,
        };
        let b = GraphCamera {
            pos: [100.0, 100.0, 0.0],
            focus: [0.0, 0.0, -200.0],
            roll_screen: i16::MIN + 10,
            fov: 30.0,
        };
        let options = GraphicsOptions::default();
        let half = interpolate_camera(Some(&a), &b, 0.5, options);
        assert_eq!(half.pos, [50.0, 100.0, 0.0]);
        assert_eq!(half.focus, [0.0, 0.0, -150.0]);
        assert_eq!(half.fov, 37.5);
        // The roll goes the short way across the wrap.
        assert!(half.roll == i16::MAX || half.roll == i16::MIN);
        // Without interpolation, or across a cut, the latest frame shows.
        let off = GraphicsOptions {
            interpolation: false,
            ..options
        };
        assert_eq!(interpolate_camera(Some(&a), &b, 0.5, off).pos, b.pos);
        let far = GraphCamera {
            pos: [5000.0, 100.0, 0.0],
            ..b
        };
        assert_eq!(
            interpolate_camera(Some(&a), &far, 0.5, options).pos,
            far.pos
        );
        assert_eq!(interpolate_camera(None, &b, 0.0, options).pos, b.pos);
        assert_eq!(
            interpolate_camera(Some(&a), &b, f32::NAN, options).pos,
            b.pos
        );
    }

    fn floor() -> CollisionWorld {
        let r = 4000;
        let mesh = CollisionMesh {
            vertices: vec![[-r, 0, -r], [-r, 0, r], [r, 0, -r], [r, 0, r]],
            triangles: [[0, 1, 2], [1, 3, 2]]
                .into_iter()
                .map(|indices| Triangle {
                    indices,
                    surface: 0,
                    force: None,
                })
                .collect(),
            specials: vec![],
            environment: vec![],
        };
        CollisionWorld::load_area_terrain(&mesh).unwrap()
    }

    #[test]
    fn session_runs_logs_and_resets() {
        let world = floor();
        let trig = tables();
        let anims = still_animations();
        let (scripts, models) = (authored_scripts(), authored_models());
        let objects = LevelObjects::mario_only(&scripts, &models);
        let mut session = Session::new(&world, &trig, &anims, objects, entry(90));
        assert_eq!(session.mario().action, ACT_IDLE);
        // create_camera's yaw is 0 until the first update initializes the camera.
        assert_eq!(session.game().camera.yaw(), 0);
        let run = Pad {
            up: true,
            ..Pad::default()
        };
        for _ in 0..30 {
            assert!(session.step(&run));
        }
        assert_eq!(session.stopped(), None);
        assert_eq!(session.mario().action, ACT_WALKING);
        // Mario faced +x and the camera initialized behind him, so holding
        // the stick up runs away from it.
        assert!(session.mario().pos[0] > 100.0, "{:?}", session.mario().pos);
        assert_eq!(session.inputs().len(), 30);
        assert_eq!(session.inputs()[0].camera_yaw, 0);
        assert!(session.inputs()[1..].iter().all(|i| i.camera_yaw != 0));
        // The camera view sits behind and above Mario, looking at him.
        let view = session.camera_view(1.0, GraphicsOptions::default());
        assert!(view.pos[0] < session.mario().pos[0] && view.pos[1] > 100.0);
        assert!((view.focus[0] - session.mario().pos[0]).abs() < 400.0);
        assert!(view.fov > 40.0 && view.fov <= 45.0);
        // Presentation interpolates between the last two ticks.
        let before = session.pose(0.0, GraphicsOptions::default()).position[0];
        let after = session.pose(1.0, GraphicsOptions::default()).position[0];
        assert!(before < after && after == session.mario().obj.gfx.pos[0]);
        let log = session.input_log("test", "rom", "flat");
        assert_eq!(log.inputs.len(), 30);
        assert_eq!(InputLog::from_json(&log.to_json_pretty()), Ok(log));
        session.reset();
        assert!(session.inputs().is_empty());
        assert_eq!(session.mario().pos, [0.0; 3]);
        // A reset snaps presentation to the new entry.
        assert_eq!(
            session.pose(0.0, GraphicsOptions::default()).position,
            [0.0; 3]
        );
    }

    #[test]
    fn a_death_plane_warp_stops_the_session() {
        let r = 4000;
        let mesh = CollisionMesh {
            vertices: vec![
                [-r, -3000, -r],
                [-r, -3000, r],
                [r, -3000, -r],
                [r, -3000, r],
            ],
            triangles: [[0, 1, 2], [1, 3, 2]]
                .into_iter()
                .map(|indices| Triangle {
                    indices,
                    surface: constants::SURFACE_DEATH_PLANE,
                    force: None,
                })
                .collect(),
            specials: vec![],
            environment: vec![],
        };
        let world = CollisionWorld::load_area_terrain(&mesh).unwrap();
        let trig = tables();
        let anims = still_animations();
        let (scripts, models) = (authored_scripts(), authored_models());
        let objects = LevelObjects::mario_only(&scripts, &models);
        let mut session = Session::new(&world, &trig, &anims, objects, entry(0));
        let mut ticks = 0;
        while session.step(&Pad::default()) {
            ticks += 1;
            assert!(ticks < 300, "Mario never reached the death plane");
        }
        assert!(matches!(session.stopped(), Some(Stop::Warp(_))));
        assert_eq!(session.inputs().len(), ticks);
        assert!(session.stopped().unwrap().to_string().contains("warp"));
        // Stopped sessions do not tick until reset.
        assert!(!session.step(&Pad::default()));
        session.reset();
        assert!(session.step(&Pad::default()));
    }

    #[test]
    fn action_names_come_from_the_original_constants() {
        assert_eq!(action_name(ACT_IDLE), "ACT_IDLE");
        assert_eq!(action_name(ACT_WALKING), "ACT_WALKING");
        assert_eq!(action_name(0x1FF), "?");
        assert_eq!(camera_mode_name(1), "CAMERA_MODE_RADIAL");
        assert_eq!(camera_mode_name(200), "?");
    }

    /// BOB's level and area rules with an authored camera node.
    fn entry(yaw: i16) -> GameEntry {
        GameEntry {
            mario: LevelEntry::from_level_script(LEVEL_BOB, 1, yaw, [0, 0, 0], 0, 1),
            camera: GeoCamera {
                mode: 1,
                pos: [0.0, 2000.0, 6000.0],
                focus: [3000.0, 0.0, -3000.0],
            },
            act_num: 1,
            rng_seed: 0,
        }
    }
}
