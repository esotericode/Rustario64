//! Mario's state and the ported Mario code, translated from the pinned CC0
//! n64decomp/sm64. Each module mirrors one original file (see PROVENANCE.md):
//! `core` (mario.c), `inputs` (mario.c's input stage), `step` (mario_step.c),
//! `animation` (graph_node.c's frame update), `interaction` (interaction.c),
//! the action files, and `tick` (the per-frame object update around Mario).
//!
//! Globals become explicit fields of [`StepWorld`]; original struct members
//! keep their widths and names. Paths the original takes through objects,
//! cutscenes or water are boundaries: they are recorded as [`Event`]s, and
//! paths that would dereference an object Mario does not have panic, as the
//! original would crash.
// Translations keep the original's comparisons and literals: clamps stay two
// ifs, ranges stay explicit comparisons, and f32 literals keep their digits.
#![allow(
    clippy::manual_clamp,
    clippy::manual_range_contains,
    clippy::excessive_precision
)]
pub mod airborne;
pub mod animation;
pub mod automatic;
pub mod constants;
pub mod core;
pub mod inputs;
pub mod interaction;
pub mod moving;
pub mod object;
pub mod stationary;
pub mod step;
pub mod tick;

use crate::{
    content::animation::MarioAnimations,
    simulation::{
        collision::{CollisionFlags, CollisionWorld, Surface, SurfaceIndex},
        controller::Controller,
        math::TrigTables,
        object::{
            ObjectPool,
            render::{NO_MODELS, ObjectModels},
            script::{BehaviorScripts, NO_SCRIPTS},
            spawn::AreaObjects,
        },
        rng::Rng,
    },
};

/// A surface Mario references: original collision, or the step code's water
/// pseudo-floor (gWaterSurfacePseudoFloor) used while riding a shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRef {
    Collision(SurfaceIndex),
    WaterPseudoFloor,
}

/// Mario's object is an ordinary `struct Object`; it lives in MarioState
/// (m->marioObj) rather than in the object pool's slot data.
pub use crate::simulation::object::{
    AnimInfo, AnimRef, GfxState, Object as MarioObject, ObjectFields, ObjectId,
};

/// struct MarioBodyState (gBodyStates[0]): model presentation state that the
/// action code sets every tick.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MarioBodyState {
    pub action: u32,
    pub cap_state: i8,
    pub eye_state: i8,
    pub hand_state: i8,
    pub wing_flutter: i8,
    pub model_state: i16,
    pub grab_pos: i8,
    pub punch_state: u8,
    pub torso_angle: [i16; 3],
    pub head_angle: [i16; 3],
    pub held_obj_last_position: [f32; 3],
}

/// struct PlayerCameraState (gPlayerCameraState[0]): what Mario reports to the
/// camera each tick. `head_rotation` is written by the camera.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PlayerCameraState {
    pub action: u32,
    pub pos: [f32; 3],
    pub face_angle: [i16; 3],
    pub head_rotation: [i16; 3],
    pub unused: i16,
    pub camera_event: i16,
    pub used_obj: Option<ObjectId>,
}

/// struct MarioState with original member widths and names.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MarioState {
    /// unk00: the player index (0 for Mario), used to pick a floor-align matrix.
    pub unk00: u16,
    pub input: u16,
    pub flags: u32,
    pub particle_flags: u32,
    pub action: u32,
    pub prev_action: u32,
    pub terrain_sound_addend: u32,
    pub action_state: u16,
    pub action_timer: u16,
    pub action_arg: u32,
    pub intended_mag: f32,
    pub intended_yaw: i16,
    pub invinc_timer: i16,
    pub frames_since_a: u8,
    pub frames_since_b: u8,
    pub wall_kick_timer: u8,
    pub double_jump_timer: u8,
    pub face_angle: [i16; 3],
    pub angle_vel: [i16; 3],
    pub slide_yaw: i16,
    pub twirl_yaw: i16,
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
    pub interact_obj: Option<ObjectId>,
    pub held_obj: Option<ObjectId>,
    pub used_obj: Option<ObjectId>,
    pub ridden_obj: Option<ObjectId>,
    pub collided_obj_interact_types: u32,
    pub num_coins: i16,
    pub num_stars: i16,
    pub num_keys: i8,
    pub num_lives: i8,
    pub health: i16,
    pub unk_b0: i16,
    pub hurt_counter: u8,
    pub heal_counter: u8,
    pub squish_timer: u8,
    pub fade_warp_opacity: u8,
    pub cap_timer: u16,
    pub prev_num_stars_for_dialog: i16,
    pub peak_height: f32,
    pub quicksand_depth: f32,
    pub getting_blown_gravity: f32,
    /// m->marioObj.
    pub obj: MarioObject,
    /// m->marioBodyState.
    pub body: MarioBodyState,
    /// m->statusForCamera.
    pub camera_status: PlayerCameraState,
}

/// The fields of the area's struct Camera that Mario code reads. `yaw` is the
/// reference camera's movement yaw. Without a linked camera they are explicit
/// inputs (the caller supplies the yaw and records it in replays). With one
/// (`simulation::game`), they are copied from it before Mario's update, and
/// `last_mode` and `level_area` let a request change `mode` immediately, as
/// the original set_camera_mode does for the rest of Mario's update.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CameraState {
    pub mode: u8,
    pub def_mode: u8,
    pub yaw: i16,
    /// Whether a camera receives Mario's requests (see `StepWorld::set_camera_mode`).
    pub linked: bool,
    /// sModeInfo.lastMode and gCurrLevelArea, while linked.
    pub last_mode: i16,
    pub level_area: i32,
}

/// Save-file inputs Mario code reads (save_file_get_flags and friends).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SaveInputs {
    pub flags: u32,
    pub cap_pos: Option<[i16; 3]>,
    pub total_star_count: i32,
}

/// interaction.c's file-scope state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InteractionGlobals {
    pub delay_invinc_timer: u8,
    pub invulnerable: i16,
    pub displaying_door_text: u8,
    pub just_teleported: u8,
    pub pss_slide_started: u8,
}

/// Why execution reached a boundary the port does not implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// The cutscene action group (the action is given).
    CutsceneGroup(u32),
    /// The submerged action group.
    SubmergedGroup(u32),
    /// The castle's endless-stairs music hook.
    InfiniteStairs,
    /// interact_coin reached 100 coins in a main course, which spawns the
    /// 100-coin star (bhv_spawn_star_no_level_exit).
    HundredCoinStar,
}

/// Calls into systems outside the simulated state, in call order. Mirrors the
/// oracle's event log; none of them changes simulated state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Sound(u32),
    StopSound(u32),
    MovingSpeed {
        bank: u8,
        speed: u8,
    },
    RaiseBackgroundNoise(i32),
    LowerBackgroundNoise(i32),
    StopCapMusic,
    FadeoutCapMusic,
    CameraMode {
        mode: i16,
        frames: i16,
    },
    CameraShake(i16),
    /// A level warp request (warp operation). No level runtime handles it yet.
    Warp(i32),
    LevelInitText(u32),
    WindParticles {
        pitch: i16,
        yaw: i16,
    },
    Unsupported(Unsupported),
}

impl Event {
    /// (kind, a, b) as recorded by the oracle's event log.
    pub fn raw(self) -> (i32, i32, i32) {
        match self {
            Event::Sound(bits) => (1, bits as i32, 0),
            Event::StopSound(bits) => (2, bits as i32, 0),
            Event::MovingSpeed { bank, speed } => (3, i32::from(bank), i32::from(speed)),
            Event::RaiseBackgroundNoise(a) => (4, a, 0),
            Event::LowerBackgroundNoise(a) => (5, a, 0),
            Event::StopCapMusic => (6, 0, 0),
            Event::FadeoutCapMusic => (7, 0, 0),
            Event::CameraMode { mode, frames } => (8, i32::from(mode), i32::from(frames)),
            Event::CameraShake(shake) => (9, i32::from(shake), 0),
            Event::Warp(op) => (10, op, 0),
            Event::LevelInitText(arg) => (11, arg as i32, 0),
            Event::WindParticles { pitch, yaw } => (12, i32::from(pitch), i32::from(yaw)),
            Event::Unsupported(reason) => match reason {
                Unsupported::CutsceneGroup(action) => (13, 1, action as i32),
                Unsupported::SubmergedGroup(action) => (13, 2, action as i32),
                Unsupported::InfiniteStairs => (13, 3, 0),
                Unsupported::HundredCoinStar => (13, 4, 0),
            },
        }
    }
}

/// A row-major 4x4 matrix (Mat4).
pub use crate::simulation::math::Mat4;

/// Everything Mario code reads or writes outside MarioState: the original
/// globals, made explicit.
pub struct StepWorld<'a> {
    pub collision: &'a CollisionWorld,
    pub collision_flags: CollisionFlags,
    pub trig: &'a TrigTables,
    pub anims: &'a MarioAnimations,
    /// gGlobalTimer.
    pub global_timer: u32,
    /// gAreaUpdateCounter.
    pub area_update_counter: u16,
    /// m->area->terrainType.
    pub area_terrain_type: u16,
    /// gCurrLevelNum.
    pub level_num: i16,
    /// The rendered area's index (gCurGraphNodeRoot->areaIndex). Mario's
    /// animation advances only while his object belongs to it.
    pub area_index: i8,
    /// gWaterSurfacePseudoFloor.originOffset; the step code rewrites it.
    pub water_pseudo_floor_origin_offset: f32,
    /// gControllers[0], sampled once per tick before Mario updates.
    pub controller: Controller,
    /// m->area->camera.
    pub camera: CameraState,
    /// gCameraMovementFlags.
    pub camera_movement_flags: i16,
    /// gSpecialTripleJump.
    pub special_triple_jump: u8,
    /// gAudioRandom, which only selects sound variants.
    pub audio_random: u32,
    /// gDebugLevelSelect and gShowDebugText.
    pub debug_level_select: i8,
    pub show_debug_text: i8,
    /// gHudDisplay.
    pub hud: crate::simulation::hud::HudDisplay,
    pub save: SaveInputs,
    pub interaction: InteractionGlobals,
    /// gMarioPlatform.
    pub mario_platform: Option<ObjectId>,
    /// mario_actions_moving.c's sFloorAlignMatrix (presentation matrices).
    pub floor_align_matrix: [Mat4; 2],
    /// gMarioAnimsBuf.currentAddr: the table entry held in the DMA buffer.
    pub anim_dma_loaded: Option<u16>,
    pub events: Vec<Event>,
    /// The behavior segment and the natives it names.
    pub behaviors: &'a BehaviorScripts,
    /// gLoadedGraphNodes, with the render traversal of spawned models.
    pub models: &'a ObjectModels,
    /// The object pool and lists (Mario's object data stays in MarioState).
    pub objects: ObjectPool,
    /// The loaded area's placements and their respawn records.
    pub area: AreaObjects,
    /// gRandomSeed16, shared by objects and the camera.
    pub rng: Rng,
    /// gTimeStopState.
    pub time_stop_state: u32,
    /// gCurrCourseNum.
    pub course_num: i16,
}

impl<'a> StepWorld<'a> {
    /// A world with original defaults: timers and flags zero, no events.
    pub fn new(
        collision: &'a CollisionWorld,
        trig: &'a TrigTables,
        anims: &'a MarioAnimations,
    ) -> Self {
        Self {
            collision,
            collision_flags: CollisionFlags::default(),
            trig,
            anims,
            global_timer: 0,
            area_update_counter: 0,
            area_terrain_type: 0,
            level_num: 0,
            area_index: 1,
            water_pseudo_floor_origin_offset: 0.0,
            controller: Controller::default(),
            camera: CameraState::default(),
            camera_movement_flags: 0,
            special_triple_jump: 0,
            audio_random: 0,
            debug_level_select: 0,
            show_debug_text: 0,
            hud: Default::default(),
            save: SaveInputs::default(),
            interaction: InteractionGlobals::default(),
            mario_platform: None,
            floor_align_matrix: [[[0.0; 4]; 4]; 2],
            anim_dma_loaded: None,
            events: Vec::new(),
            behaviors: &NO_SCRIPTS,
            models: &NO_MODELS,
            objects: ObjectPool::new(),
            area: AreaObjects::default(),
            rng: Rng::default(),
            time_stop_state: 0,
            course_num: 0,
        }
    }

    /// gLoadedGraphNodes[model]: the model ID if the level loaded it, NULL
    /// for MODEL_NONE and models it did not load. IDs index a 256-entry array.
    pub fn loaded_model(&self, model: i32) -> Option<u16> {
        let index = u16::try_from(model)
            .ok()
            .filter(|m| *m < 256)
            .unwrap_or_else(|| panic!("model ID {model} indexes outside gLoadedGraphNodes"));
        (index != 0 && self.models.is_loaded(index)).then_some(index)
    }

    /// find_floor_height (surface_collision.c): find_floor's height.
    pub fn find_floor_height(&mut self, x: f32, y: f32, z: f32) -> f32 {
        self.collision
            .find_floor(x, y, z, &mut self.collision_flags)
            .0
    }

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

    pub fn event(&mut self, event: Event) {
        self.events.push(event);
    }

    /// set_camera_mode(m->area->camera, mode, frames): recorded in call order.
    /// While a camera is linked, the camera's mode changes now, as the
    /// original's does for the rest of Mario's update; the linked camera
    /// performs the rest of the request after Mario's update
    /// (`simulation::game`).
    pub fn set_camera_mode(&mut self, mode: i16, frames: i16) {
        self.events.push(Event::CameraMode { mode, frames });
        let camera = &mut self.camera;
        if !camera.linked
            || (mode == constants::CAMERA_MODE_WATER_SURFACE
                && camera.level_area == constants::AREA_TTM_OUTSIDE)
        {
            return;
        }
        let mode = if mode == constants::CAMERA_MODE_NONE {
            constants::CAMERA_MODE_CLOSE
        } else {
            mode
        };
        let new_mode = if mode != -1 { mode } else { camera.last_mode };
        camera.last_mode = i16::from(camera.mode);
        camera.mode = new_mode as u8;
    }

    /// play_sound: a boundary event; sound has no simulated state.
    pub fn play_sound(&mut self, bits: u32) {
        self.events.push(Event::Sound(bits));
    }
}

/// `(s32)` conversion of an f32, as the N64's truncating conversion. Values
/// the N64 cannot represent raise an unhandled FPU exception there (the
/// documented "speed crash"); the port panics instead of inventing a value.
pub fn f32_to_s32(value: f32) -> i32 {
    assert!(
        value > -2147483904.0 && value < 2147483648.0,
        "float-to-int conversion of {value} would crash the original"
    );
    value as i32
}

/// `(s16)` conversion of an f32: truncation to s32, then the low 16 bits.
pub fn f32_to_s16(value: f32) -> i16 {
    f32_to_s32(value) as i16
}
