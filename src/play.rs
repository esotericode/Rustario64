//! Playing from held controls, as the development viewer does. A session runs
//! the same level entry and tick that the oracle compares with the decomp:
//! held controls become each tick's `TickInput`, and a follow camera (not the
//! original camera, which is not ported) supplies the camera yaw the tick
//! reads, changing once per tick after Mario's update as the original
//! camera's yaw does. Every tick's input is kept, so a session can be replayed
//! against the reference (`trace::InputLog`). Presentation reads completed
//! snapshots only.
use crate::{
    content::animation::MarioAnimations,
    presentation::{self, GraphicsOptions, Pose, Snapshot},
    simulation::{
        TICKS_PER_SECOND, TickInput,
        collision::CollisionWorld,
        controller::{A_BUTTON, B_BUTTON, Z_TRIG},
        mario::{
            Event, MarioState, StepWorld, Unsupported, constants,
            tick::{LevelEntry, enter_level, tick},
        },
        math::TrigTables,
    },
    trace::{INPUT_LOG_SCHEMA, InputLog},
};
use std::{fmt, panic};

/// The input-log name of Bob-omb Battlefield's level-script start, the entry
/// `LevelEntry::script_start` builds from BOB's import.
pub const BOB_SCRIPT_START: &str = "bob-script-start";

/// Held controls: a digital stick with a walk modifier, the A, B and Z
/// buttons, and the follow camera's turn controls.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pad {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    /// Partial deflection, for walking.
    pub walk: bool,
    pub a: bool,
    pub b: bool,
    pub z: bool,
    pub camera_left: bool,
    pub camera_right: bool,
}

impl Pad {
    /// Raw stick bytes in the held direction. Full deflection (80, or 57 per
    /// axis on diagonals) is past adjust_analog_stick's dead-zone offset and
    /// 64-unit clamp, so every direction gives the full magnitude; walking
    /// uses 40 (28 on diagonals), about half the magnitude.
    pub fn stick(&self) -> [i8; 2] {
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
        [(self.a, A_BUTTON), (self.b, B_BUTTON), (self.z, Z_TRIG)]
            .into_iter()
            .filter(|(held, _)| *held)
            .fold(0, |buttons, (_, bit)| buttons | bit)
    }

    pub fn tick_input(&self, camera_yaw: i16) -> TickInput {
        TickInput {
            buttons: self.buttons(),
            stick: self.stick(),
            camera_yaw,
        }
    }
}

/// A follow camera's yaw (not the original camera's). `yaw` is the direction
/// from Mario to the camera in original angle units, which is what the
/// original camera's yaw means to Mario's input code: holding the stick up
/// moves Mario away from the camera. It turns only on request, by a fixed
/// step per tick, so it is part of the deterministic tick input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCamera {
    pub yaw: i16,
    previous_yaw: i16,
}

impl FollowCamera {
    pub const TURN_PER_TICK: i16 = 0x300;

    /// Behind Mario, who faces `mario_yaw`.
    pub fn behind(mario_yaw: i16) -> Self {
        let yaw = mario_yaw.wrapping_add(i16::MIN);
        Self {
            yaw,
            previous_yaw: yaw,
        }
    }

    /// The camera's update for one tick (after Mario's, as in the original).
    pub fn advance(&mut self, pad: &Pad) {
        self.previous_yaw = self.yaw;
        if pad.camera_left {
            self.yaw = self.yaw.wrapping_add(Self::TURN_PER_TICK);
        }
        if pad.camera_right {
            self.yaw = self.yaw.wrapping_sub(Self::TURN_PER_TICK);
        }
    }

    /// The yaw between the last two ticks, for presentation only.
    pub fn presentation_yaw(&self, alpha: f32) -> i16 {
        let alpha = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let delta = self.yaw.wrapping_sub(self.previous_yaw);
        self.previous_yaw
            .wrapping_add((f32::from(delta) * alpha) as i16)
    }
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

fn snapshot(m: &MarioState, epoch: u64) -> Snapshot {
    Snapshot {
        entity: 1,
        epoch,
        // The original draws Mario's object at its graphics position and yaw.
        position: m.obj.gfx.pos,
        yaw: m.obj.gfx.angle[1],
        // Without a skeleton there is no pose to keep from blending across an
        // animation switch, so the position always interpolates.
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

/// Mario's simulation from one level entry: the compared tick, the follow
/// camera that supplies its camera yaw, every tick's input, and the last two
/// completed snapshots for presentation.
pub struct Session<'a> {
    collision: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    entry: LevelEntry,
    mario: MarioState,
    world: StepWorld<'a>,
    camera: FollowCamera,
    inputs: Vec<TickInput>,
    stopped: Option<Stop>,
    epoch: u64,
    previous: Option<Snapshot>,
    current: Snapshot,
}

impl<'a> Session<'a> {
    pub fn new(
        collision: &'a CollisionWorld,
        trig: &'a TrigTables,
        anims: &'a MarioAnimations,
        entry: LevelEntry,
    ) -> Self {
        let (mario, world) = enter_level(collision, trig, anims, &entry);
        let camera = FollowCamera::behind(mario.face_angle[1]);
        let current = snapshot(&mario, 0);
        Self {
            collision,
            trig,
            anims,
            entry,
            mario,
            world,
            camera,
            inputs: vec![],
            stopped: None,
            epoch: 0,
            previous: None,
            current,
        }
    }

    /// Enter the level again. The input log restarts and presentation snaps.
    pub fn reset(&mut self) {
        let (mario, world) = enter_level(self.collision, self.trig, self.anims, &self.entry);
        self.mario = mario;
        self.world = world;
        self.camera = FollowCamera::behind(self.mario.face_angle[1]);
        self.inputs.clear();
        self.stopped = None;
        self.epoch += 1;
        self.previous = None;
        self.current = snapshot(&self.mario, self.epoch);
    }

    /// One 30 Hz tick with the held controls, unless stopped; returns whether
    /// it ran. The tick reads the yaw the camera produced last tick; the
    /// camera then updates. A tick that stops the session is still logged,
    /// so a replay reaches the same stop.
    pub fn step(&mut self, pad: &Pad) -> bool {
        if self.stopped.is_some() {
            return false;
        }
        let input = pad.tick_input(self.camera.yaw);
        self.inputs.push(input);
        let (mario, world) = (&mut self.mario, &mut self.world);
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            world.events.clear();
            tick(mario, world, input);
        }));
        if let Err(payload) = result {
            // The state is partly updated; it is shown but never ticked again.
            self.stopped = Some(Stop::Panic(panic_message(payload)));
            return true;
        }
        self.stopped = self.world.events.iter().find_map(|event| match *event {
            Event::Unsupported(what) => Some(Stop::Unsupported(what)),
            Event::Warp(op) => Some(Stop::Warp(op)),
            _ => None,
        });
        self.camera.advance(pad);
        self.previous = Some(self.current);
        self.current = snapshot(&self.mario, self.epoch);
        true
    }

    /// Mario's presentation pose between the last two completed ticks.
    pub fn pose(&self, alpha: f32, options: GraphicsOptions) -> Pose {
        presentation::interpolate(self.previous.as_ref(), &self.current, alpha, options)
    }

    pub fn entry(&self) -> &LevelEntry {
        &self.entry
    }

    pub fn mario(&self) -> &MarioState {
        &self.mario
    }

    pub fn world(&self) -> &StepWorld<'a> {
        &self.world
    }

    pub fn camera(&self) -> &FollowCamera {
        &self.camera
    }

    /// Every tick's input since the level entry.
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
            inputs: self.inputs.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::{CollisionMesh, Triangle, animation::Animation},
        simulation::{
            mario::constants::{ACT_IDLE, ACT_WALKING, LEVEL_BOB},
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
            ..Pad::default()
        };
        assert_eq!(pad.stick(), [0, -80]);
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
        assert_eq!(Pad::default().tick_input(7).camera_yaw, 7);
    }

    #[test]
    fn follow_camera_starts_behind_and_turns_once_per_tick() {
        let mut camera = FollowCamera::behind(0x4000);
        assert_eq!(camera.yaw, -0x4000);
        camera.advance(&Pad {
            camera_left: true,
            ..Pad::default()
        });
        assert_eq!(camera.yaw, -0x4000 + FollowCamera::TURN_PER_TICK);
        assert_eq!(
            camera.presentation_yaw(0.5),
            -0x4000 + FollowCamera::TURN_PER_TICK / 2
        );
        assert_eq!(camera.presentation_yaw(f32::NAN), camera.yaw);
        camera.advance(&Pad::default());
        assert_eq!(camera.presentation_yaw(0.0), camera.yaw);
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
        let entry = LevelEntry::from_level_script(LEVEL_BOB, 1, 90, [0, 0, 0], 0, 1);
        let mut session = Session::new(&world, &trig, &anims, entry);
        assert_eq!(session.mario().action, ACT_IDLE);
        // Mario faces +x; the camera starts behind him, toward -x.
        assert_eq!(session.camera().yaw, -0x4000);
        let run = Pad {
            up: true,
            ..Pad::default()
        };
        for _ in 0..30 {
            assert!(session.step(&run));
        }
        assert_eq!(session.stopped(), None);
        assert_eq!(session.mario().action, ACT_WALKING);
        assert!(session.mario().pos[0] > 100.0 && session.mario().pos[2].abs() < 1.0);
        assert_eq!(session.inputs().len(), 30);
        assert!(session.inputs().iter().all(|i| i.camera_yaw == -0x4000));
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
        let entry = LevelEntry::from_level_script(LEVEL_BOB, 1, 0, [0, 0, 0], 0, 1);
        let mut session = Session::new(&world, &trig, &anims, entry);
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
    }
}
