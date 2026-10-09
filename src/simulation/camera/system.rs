//! The complete camera update of pinned CC0 camera.c: `update_camera` and what
//! it reaches in areas without camera triggers, with the level entry
//! (`create_camera`, `select_mario_cam_mode`, `reset_camera`, `init_camera`),
//! Mario's requests (`set_camera_mode`, `set_camera_shake_from_hit`) and the
//! render pass's camera callbacks (`geo_camera_fov`, `geo_camera_main`).
//!
//! Every camera.c global the update touches is a field here or in `Rig`. The
//! modes ported are radial, close and free roam (Lakitu), Mario's close camera
//! (R), the boss-fight camera and C-Up; reaching another mode, a cutscene or a
//! level whose camera trigger table is not ported is reported as
//! [`Unsupported`] after the tick completes, and paths that would need an
//! object (poles, held bosses) panic as the original would dereference NULL.
//! All of this runs once per 30 Hz tick; presentation reads `graph`.
#![allow(clippy::too_many_arguments)]
use super::{
    lakitu::{Rig, get_dist_angle},
    obstruction::{is_range_behind_surface, rotate_camera_around_walls},
    radial::RadialMovement,
    *,
};
use crate::simulation::{
    collision::SurfaceIndex,
    controller::{A_BUTTON, B_BUTTON, Controller, R_TRIG},
    mario::{Event, PlayerCameraState, f32_to_s16},
    rng::Rng,
};

const fn degrees(value: i32) -> i16 {
    (value * 0x10000 / 360) as i16
}

/// Levels with a camera trigger table (sCameraTriggers); none is ported.
const LEVELS_WITH_TRIGGERS: [i16; 9] = [
    c::LEVEL_BBH,
    c::LEVEL_CCM,
    c::LEVEL_CASTLE,
    c::LEVEL_HMC,
    c::LEVEL_SSL,
    c::LEVEL_SL,
    c::LEVEL_THI,
    c::LEVEL_RR,
    c::LEVEL_COTMC,
];

/// sZoomOutAreaMasks: whether pausing zooms the camera out, two levels per
/// byte (ZOOMOUT_AREA_MASK packs the second level's areas in the high bits).
const ZOOM_OUT_AREA_MASKS: [u8; 20] = {
    const fn mask(first: [u8; 4], second: [u8; 4]) -> u8 {
        second[3] << 7
            | second[2] << 6
            | second[1] << 5
            | second[0] << 4
            | first[3] << 3
            | first[2] << 2
            | first[1] << 1
            | first[0]
    }
    [
        mask([0, 0, 0, 0], [0, 0, 0, 0]),
        mask([0, 0, 0, 0], [0, 0, 0, 0]),
        mask([0, 0, 0, 0], [1, 0, 0, 0]),
        mask([0, 0, 0, 0], [0, 0, 0, 0]),
        mask([1, 0, 0, 0], [1, 0, 0, 0]),
        mask([1, 0, 0, 0], [1, 0, 0, 0]),
        mask([0, 0, 0, 0], [1, 1, 0, 0]),
        mask([0, 0, 0, 0], [1, 0, 0, 0]),
        mask([1, 0, 0, 0], [1, 0, 0, 0]),
        mask([0, 0, 0, 0], [1, 0, 0, 0]),
        mask([0, 0, 0, 0], [1, 0, 0, 0]),
        mask([1, 0, 0, 0], [0, 0, 0, 0]),
        mask([1, 0, 0, 0], [0, 0, 0, 0]),
        mask([0, 0, 0, 0], [0, 0, 0, 0]),
        mask([0, 0, 0, 0], [1, 0, 0, 0]),
        mask([1, 0, 0, 0], [1, 0, 0, 0]),
        mask([0, 0, 0, 0], [1, 0, 0, 0]),
        mask([1, 0, 0, 0], [0, 0, 0, 0]),
        mask([1, 0, 0, 0], [0, 0, 0, 0]),
        mask([0, 0, 0, 0], [0, 0, 0, 0]),
    ]
};

/// Why the camera reached a path the port does not implement. The tick that
/// reached it completes as far as the original's own state allows; play stops
/// afterward rather than inventing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// A camera mode whose controller is not ported (the mode).
    Mode(u8),
    /// A cutscene (the cutscene ID).
    Cutscene(u8),
    /// A level whose camera trigger table is not ported (the level).
    Triggers(i16),
    /// A level whose camera initialization starts a cutscene (the level).
    LevelInit(i16),
}

/// The area's GEO_CAMERA node: the camera's initial mode, position and focus
/// (the focus is also the area center the radial camera turns around).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GeoCamera {
    pub mode: u8,
    pub pos: [f32; 3],
    pub focus: [f32; 3],
}

/// The level and area the camera belongs to (gCurrLevelNum,
/// gCurrentArea->index, gCurrActNum).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub level_num: i16,
    pub area_index: i8,
    pub act_num: i16,
}

/// What the camera reads from gMarioStates[0] beyond PlayerCameraState.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MarioView {
    pub action: u32,
    pub forward_vel: f32,
    pub angle_vel_yaw: i16,
    /// Whether Mario passes vanish-cap walls (he is the current object when
    /// the camera updates, as the last object updated).
    pub pass_vanish_walls: bool,
}

/// sMarioGeometry: Mario's floor and ceiling this tick and last tick.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MarioGeometry {
    pub curr: PlayerGeometry,
    pub prev_floor: Option<SurfaceIndex>,
    pub prev_floor_height: f32,
    pub prev_floor_type: i16,
    pub prev_ceil: Option<SurfaceIndex>,
    pub prev_ceil_height: f32,
    pub prev_ceil_type: i16,
}

impl MarioGeometry {
    fn store_previous(&mut self) {
        self.prev_floor_height = self.curr.floor_height;
        self.prev_ceil_height = self.curr.ceil_height;
        self.prev_floor = self.curr.floor;
        self.prev_ceil = self.curr.ceil;
        self.prev_floor_type = self.curr.floor_type;
        self.prev_ceil_type = self.curr.ceil_type;
    }
}

/// struct LinearTransitionPoint.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TransitionPoint {
    pub focus: [f32; 3],
    pub pos: [f32; 3],
    pub dist: f32,
    pub pitch: i16,
    pub yaw: i16,
}

/// sModeInfo's frame counter and endpoints (its new and last modes are
/// `Rig::new_mode` and `Rig::last_mode`).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ModeInfo {
    pub max: i16,
    pub frame: i16,
    pub start: TransitionPoint,
    pub end: TransitionPoint,
}

/// sFOVState apart from its shake (`Rig::fov_shake`).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FovState {
    pub func: u8,
    pub fov: f32,
    pub offset: f32,
    pub unused_is_sleeping: u32,
    pub shake_phase: i16,
}

/// struct CameraStoredInfo (sCameraStoreCUp).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct StoredInfo {
    pub pos: [f32; 3],
    pub focus: [f32; 3],
    pub pan_dist: f32,
    pub cannon_y_offset: f32,
}

/// The area's camera graph node and perspective node after the render pass:
/// what presentation draws from (Lakitu's position, focus and roll, and the
/// field of view in degrees).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GraphCamera {
    pub pos: [f32; 3],
    pub focus: [f32; 3],
    pub roll_screen: i16,
    pub fov: f32,
}

/// What one camera update reads and writes outside camera.c's own globals.
pub struct Frame<'a, 'w> {
    pub world: &'w CollisionWorld,
    pub trig: &'w TrigTables,
    /// gCheckingSurfaceCollisionsForCamera and the intangible-floor flag.
    pub flags: &'a mut CollisionFlags,
    /// gPlayer1Controller, sampled this tick.
    pub controller: &'a Controller,
    /// sMarioCamState (gPlayerCameraState[0]); the camera writes the head
    /// rotation, the camera event and the used object.
    pub mario_cam: &'a mut PlayerCameraState,
    pub mario: MarioView,
    /// gRandomSeed16.
    pub rng: &'a mut Rng,
    /// Sounds, in call order.
    pub events: &'a mut Vec<Event>,
}

/// One area's camera and every camera.c global it uses.
#[derive(Debug, Clone)]
pub struct CameraSystem {
    pub level: Level,
    /// gCamera's state and the Lakitu/transition globals.
    pub rig: Rig,
    /// gCurrLevelArea, areaCenX/Z and s2ndRotateFlags.
    pub radial: RadialMovement,
    pub area_center_y: f32,
    pub door_status: u8,
    pub geometry: MarioGeometry,
    /// sAreaYaw.
    pub area_yaw: i16,
    pub mode_info: ModeInfo,
    pub fov: FovState,
    pub selection_flags: i16,
    pub sound_flags: i16,
    pub c_buttons_pressed: u16,
    pub zoom_amount: f32,
    pub zero_zoom_dist: f32,
    /// gCameraZoomDist.
    pub zoom_dist: f32,
    pub c_side_button_yaw: i16,
    pub avoid_yaw_vel: i16,
    pub unused_free_roam_wall_yaw: i16,
    pub yaw_after_door_cutscene: i16,
    pub behind_mario_sound_timer: i16,
    pub spiral_stairs_yaw_offset: i16,
    pub eight_dir_base_yaw: i16,
    pub eight_dir_yaw_offset: i16,
    pub frames_since_cutscene_ended: u8,
    pub recent_cutscene: u8,
    pub object_cutscene: u8,
    pub frames_paused: u8,
    /// gPrevLevel.
    pub prev_level: u32,
    pub credits_player2_pitch: i16,
    pub credits_player2_yaw: i16,
    pub cutscene_spline_segment: i16,
    pub cutscene_spline_segment_progress: f32,
    pub cutscene_shot: i16,
    pub cutscene_timer: i16,
    pub cutscene_obj_spawn: u32,
    pub obj_cutscene_done: i32,
    pub unused_8032cfc8: u32,
    pub unused_8032cfcc: u32,
    pub unused_8033b316: i16,
    pub unused_8033b31a: i16,
    pub unused_8033b30c: u32,
    pub unused_8033b310: u32,
    pub unused_8033b6e8: i16,
    pub castle_entrance_offset: [f32; 3],
    pub fixed_mode_base_position: [f32; 3],
    pub store_c_up: StoredInfo,
    /// sCurCreditsSplinePos/Focus indices (init_camera resets them).
    pub credits_spline_pos_index: [i8; 32],
    pub credits_spline_focus_index: [i8; 32],
    /// set_hud_camera_status's argument (the HUD's camera icon).
    pub hud_status: i16,
    pub graph: GraphCamera,
    /// A path reached this tick that the port does not implement.
    unsupported: Option<Unsupported>,
}

impl CameraSystem {
    /// A fresh boot's camera globals, select_mario_cam_mode
    /// (lvl_init_from_save_file), then create_camera for the area's
    /// GEO_CAMERA node when the area loads. create_camera leaves nextYaw as
    /// its pool memory holds it; the update writes it before any read.
    pub fn create(level: Level, geo: GeoCamera) -> Self {
        let mut rig = Rig {
            yaw_speed: 0x400,
            ..Rig::default()
        };
        rig.camera.mode = geo.mode;
        rig.camera.def_mode = geo.mode;
        rig.camera.cutscene = 0;
        rig.camera.yaw = 0;
        rig.camera.pos = geo.pos;
        rig.camera.focus = geo.focus;
        let mut system = Self {
            level,
            rig,
            radial: RadialMovement {
                area: 0,
                center: [geo.focus[0], geo.focus[2]],
                second_rotate: 0,
            },
            area_center_y: geo.focus[1],
            door_status: c::DOOR_DEFAULT,
            geometry: MarioGeometry::default(),
            area_yaw: 0,
            mode_info: ModeInfo::default(),
            fov: FovState::default(),
            selection_flags: 0,
            sound_flags: 0,
            c_buttons_pressed: 0,
            zoom_amount: 0.0,
            zero_zoom_dist: 0.0,
            zoom_dist: 800.0,
            c_side_button_yaw: 0,
            avoid_yaw_vel: 0,
            unused_free_roam_wall_yaw: 0,
            yaw_after_door_cutscene: 0,
            behind_mario_sound_timer: 0,
            spiral_stairs_yaw_offset: 0,
            eight_dir_base_yaw: 0,
            eight_dir_yaw_offset: 0,
            frames_since_cutscene_ended: 0,
            recent_cutscene: 0,
            object_cutscene: 0,
            frames_paused: 0,
            prev_level: 0,
            credits_player2_pitch: 0,
            credits_player2_yaw: 0,
            cutscene_spline_segment: 0,
            cutscene_spline_segment_progress: 0.0,
            cutscene_shot: 0,
            cutscene_timer: 0,
            cutscene_obj_spawn: 0,
            obj_cutscene_done: 0,
            unused_8032cfc8: 0,
            unused_8032cfcc: 0,
            unused_8033b316: 0,
            unused_8033b31a: 0,
            unused_8033b30c: 0,
            unused_8033b310: 0,
            unused_8033b6e8: 0,
            castle_entrance_offset: [0.0; 3],
            fixed_mode_base_position: [646.0, 143.0, -1513.0],
            store_c_up: StoredInfo::default(),
            credits_spline_pos_index: [0; 32],
            credits_spline_focus_index: [0; 32],
            hud_status: 0,
            graph: GraphCamera {
                pos: geo.pos,
                focus: geo.focus,
                roll_screen: 0,
                fov: 0.0,
            },
            unsupported: None,
        };
        system.select_mario_cam_mode();
        system
    }

    /// select_mario_cam_mode.
    pub fn select_mario_cam_mode(&mut self) {
        self.selection_flags = c::CAM_MODE_MARIO_SELECTED;
    }

    /// The camera's yaw: the direction from its focus to its position, which
    /// Mario's controls are relative to.
    pub fn yaw(&self) -> i16 {
        self.rig.camera.yaw
    }

    fn sound(events: &mut Vec<Event>, bits: u32) {
        events.push(Event::Sound(bits));
    }

    fn unsupported(&mut self, what: Unsupported) {
        self.unsupported.get_or_insert(what);
    }

    /// reset_camera, as init_level calls it after init_mario.
    pub fn reset(&mut self, mario_cam: &mut PlayerCameraState) {
        let r = &mut self.rig;
        r.movement = 0;
        self.radial.second_rotate = 0;
        r.status = 0;
        self.cutscene_timer = 0;
        self.cutscene_shot = 0;
        self.cutscene_obj_spawn = 0;
        self.obj_cutscene_done = 0;
        self.unused_8032cfc8 = 0;
        self.unused_8032cfcc = 0;
        self.c_buttons_pressed = 0;
        r.transition.mario_pos = mario_cam.pos;
        r.transition.frames_left = 0;
        self.unused_8032cfcc = u32::MAX;
        self.unused_8032cfc8 = u32::MAX;
        r.movement = 0;
        r.movement |= c::CAM_MOVE_INIT_CAMERA;
        self.unused_8033b316 = 0;
        r.status = 0;
        self.unused_8033b31a = 0;
        self.sound_flags = 0;
        r.c_up_pitch = 0;
        r.mode_offset_yaw = 0;
        self.spiral_stairs_yaw_offset = 0;
        r.lakitu_dist = 0;
        r.lakitu_pitch = 0;
        self.area_yaw = 0;
        r.area_yaw_change = 0;
        r.pan_distance = 0.0;
        r.cannon_y_offset = 0.0;
        self.zoom_amount = 0.0;
        self.zero_zoom_dist = 0.0;
        self.behind_mario_sound_timer = 0;
        self.c_side_button_yaw = 0;
        self.eight_dir_base_yaw = 0;
        self.eight_dir_yaw_offset = 0;
        self.door_status = c::DOOR_DEFAULT;
        mario_cam.head_rotation[0] = 0;
        mario_cam.head_rotation[1] = 0;
        mario_cam.camera_event = 0;
        mario_cam.used_obj = None;
        r.lakitu.shake_magnitude = [0; 3];
        r.lakitu.last_frame_action = 0;
        self.fov.func = c::CAM_FOV_DEFAULT;
        self.fov.fov = 45.0;
        self.fov.offset = 0.0;
        self.fov.unused_is_sleeping = 0;
        r.fov_shake.amplitude = 0.0;
        self.fov.shake_phase = 0;
        self.object_cutscene = 0;
        self.recent_cutscene = 0;
        self.unused_8033b30c = 0;
        self.unused_8033b310 = 0;
    }

    /// init_camera, run by the first update after a reset.
    fn init_camera(&mut self, f: &mut Frame<'_, '_>) {
        self.credits_player2_pitch = 0;
        self.credits_player2_yaw = 0;
        self.prev_level = (self.radial.area / 16) as u32;
        self.radial.area = i32::from(self.level.level_num) * 16 + i32::from(self.level.area_index);
        self.selection_flags &= c::CAM_MODE_MARIO_SELECTED;
        self.frames_paused = 0;
        let r = &mut self.rig;
        r.lakitu.mode = r.camera.mode;
        r.lakitu.def_mode = r.camera.def_mode;
        r.lakitu.pos_h_speed = 0.3;
        r.lakitu.pos_v_speed = 0.3;
        r.lakitu.foc_h_speed = 0.8;
        // The original sets focHSpeed twice; focVSpeed keeps its value.
        r.lakitu.foc_h_speed = 0.3;
        r.lakitu.roll = 0;
        r.lakitu.key_dance_roll = 0;
        r.status &= !c::CAM_FLAG_SMOOTH_MOVEMENT;
        self.castle_entrance_offset = [0.0; 3];
        r.player2_focus_offset = [0.0; 3];
        self.geometry.curr = find_mario_floor_and_ceil(f.mario_cam.pos, f.world, f.flags);
        self.geometry.store_previous();
        self.credits_spline_pos_index = [-1; 32];
        self.credits_spline_focus_index = [-1; 32];
        self.cutscene_spline_segment = 0;
        self.cutscene_spline_segment_progress = 0.0;
        self.unused_8033b6e8 = 0;
        let r = &mut self.rig;
        r.handheld_increment = 0.0;
        r.handheld_timer = 0.0;
        r.handheld_magnitude = 0;
        r.handheld_spline_index = [-1; 4];
        r.handheld_angles = [0; 3];
        r.camera.cutscene = 0;
        let mut mario_offset = [0.0, 125.0, 400.0];
        let level = self.level.level_num;
        match level {
            c::LEVEL_BOWSER_1 | c::LEVEL_BOWSER_2 | c::LEVEL_BOWSER_3 => {
                self.unsupported(Unsupported::LevelInit(level));
            }
            c::LEVEL_CASTLE_GROUNDS => {
                if is_within_100_units_of_mario(f.mario_cam.pos, [-1328.0, 260.0, 4664.0]) != 1 {
                    mario_offset[0] = -400.0;
                    mario_offset[2] = -800.0;
                }
                if is_within_100_units_of_mario(f.mario_cam.pos, [-6901.0, 2376.0, -6509.0]) == 1
                    || is_within_100_units_of_mario(f.mario_cam.pos, [5408.0, 4500.0, 3637.0]) == 1
                {
                    self.unsupported(Unsupported::LevelInit(level));
                }
                self.rig.lakitu.mode = c::CAMERA_MODE_FREE_ROAM as u8;
            }
            c::LEVEL_SA => mario_offset[2] = 200.0,
            c::LEVEL_CASTLE_COURTYARD => mario_offset[2] = -300.0,
            c::LEVEL_LLL => self.rig.movement |= c::CAM_MOVE_ZOOMED_OUT,
            c::LEVEL_CASTLE => mario_offset[2] = 150.0,
            c::LEVEL_RR => self.fixed_mode_base_position = [-2985.0, 478.0, -5568.0],
            _ => {}
        }
        if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_8_DIRECTIONS {
            self.rig.movement |= c::CAM_MOVE_ZOOMED_OUT;
        }
        match self.radial.area {
            c::AREA_SSL_EYEROK => mario_offset = [0.0, 500.0, -100.0],
            c::AREA_CCM_SLIDE | c::AREA_THI_WIGGLER | c::AREA_SL_IGLOO => {
                mario_offset[2] = -300.0;
            }
            c::AREA_SL_OUTSIDE => {
                if is_within_100_units_of_mario(f.mario_cam.pos, [257.0, 2150.0, 1399.0]) == 1 {
                    mario_offset[2] = -300.0;
                }
            }
            c::AREA_CCM_OUTSIDE => self.rig.movement |= c::CAM_MOVE_ZOOMED_OUT,
            c::AREA_TTM_OUTSIDE => self.rig.lakitu.mode = c::CAMERA_MODE_RADIAL as u8,
            _ => {}
        }
        let r = &mut self.rig;
        r.camera.pos = offset_rotated(
            f.mario_cam.pos,
            mario_offset,
            f.mario_cam.face_angle,
            f.trig,
        );
        if i16::from(r.camera.mode) != c::CAMERA_MODE_BEHIND_MARIO {
            let m = f.mario_cam.pos;
            let (height, _) = f.world.find_floor(m[0], m[1] + 100.0, m[2], f.flags);
            r.camera.pos[1] = height + 125.0;
        }
        r.camera.focus = f.mario_cam.pos;
        r.lakitu.cur_pos = r.camera.pos;
        r.lakitu.cur_focus = r.camera.focus;
        r.lakitu.goal_pos = r.camera.pos;
        r.lakitu.goal_focus = r.camera.focus;
        r.lakitu.pos = r.camera.pos;
        r.lakitu.focus = r.camera.focus;
        if i16::from(r.camera.mode) == c::CAMERA_MODE_FIXED {
            // set_fixed_cam_axis_sa_lobby, which the fixed mode needs.
            self.unsupported(Unsupported::Mode(c::CAMERA_MODE_FIXED as u8));
        }
        self.store_lakitu_cam_info_for_c_up(f.mario_cam);
        let r = &mut self.rig;
        r.lakitu.yaw = calculate_yaw(r.camera.focus, r.camera.pos, f.trig);
        r.lakitu.next_yaw = r.lakitu.yaw;
        r.camera.yaw = r.lakitu.yaw;
        r.camera.next_yaw = r.lakitu.yaw;
    }

    /// update_camera, run after the objects each tick. A reached path the
    /// port does not implement is returned after the tick's update completes.
    pub fn update(&mut self, f: &mut Frame<'_, '_>) -> Result<(), Unsupported> {
        self.unsupported = None;
        self.update_camera_hud_status(f.controller);
        if self.rig.camera.cutscene == 0 {
            if self.cam_select_alt_mode(0) == c::CAM_SELECTION_MARIO
                && f.controller.button_pressed & R_TRIG != 0
            {
                if self.set_cam_angle(0) == c::CAM_ANGLE_LAKITU {
                    self.set_cam_angle(c::CAM_ANGLE_MARIO);
                } else {
                    self.set_cam_angle(c::CAM_ANGLE_LAKITU);
                }
            }
            self.play_sound_if_cam_switched_to_lakitu_or_mario(f.events);
        }
        self.rig.status &= !c::CAM_FLAG_FRAME_AFTER_CAM_INIT;
        if self.rig.movement & c::CAM_MOVE_INIT_CAMERA != 0 {
            self.init_camera(f);
            self.rig.movement &= !c::CAM_MOVE_INIT_CAMERA;
            self.rig.status |= c::CAM_FLAG_FRAME_AFTER_CAM_INIT;
        }
        self.geometry.store_previous();
        self.geometry.curr = find_mario_floor_and_ceil(f.mario_cam.pos, f.world, f.flags);
        f.flags.checking_for_camera = true;
        let r = &mut self.rig;
        r.camera.pos = r.lakitu.goal_pos;
        r.camera.focus = r.lakitu.goal_focus;
        r.camera.yaw = r.lakitu.yaw;
        r.camera.next_yaw = r.lakitu.next_yaw;
        r.camera.mode = r.lakitu.mode;
        r.camera.def_mode = r.lakitu.def_mode;
        self.camera_course_processing(f);
        self.c_buttons_pressed = find_c_buttons_pressed(
            self.c_buttons_pressed,
            f.controller.button_pressed,
            f.controller.button_down,
        );
        if self.rig.camera.cutscene != 0 {
            self.rig.yaw_speed = 0;
            // play_cutscene: no cutscene is ported.
            self.unsupported(Unsupported::Cutscene(self.rig.camera.cutscene));
            self.frames_since_cutscene_ended = 0;
        } else if self.recent_cutscene != 0 && self.frames_since_cutscene_ended < 8 {
            self.frames_since_cutscene_ended += 1;
            if self.frames_since_cutscene_ended >= 8 {
                self.recent_cutscene = 0;
                self.frames_since_cutscene_ended = 0;
            }
        }
        if self.rig.camera.cutscene == 0 {
            self.rig.yaw_speed = 0x400;
            let mode = i16::from(self.rig.camera.mode);
            if self.selection_flags & c::CAM_MODE_MARIO_ACTIVE != 0 {
                match mode {
                    c::CAMERA_MODE_C_UP => self.mode_c_up_camera(f),
                    c::CAMERA_MODE_BEHIND_MARIO
                    | c::CAMERA_MODE_WATER_SURFACE
                    | c::CAMERA_MODE_INSIDE_CANNON => {
                        self.unsupported(Unsupported::Mode(mode as u8));
                    }
                    _ => self.mode_mario_camera(f),
                }
            } else {
                match mode {
                    c::CAMERA_MODE_C_UP => self.mode_c_up_camera(f),
                    c::CAMERA_MODE_RADIAL => self.mode_radial_camera(f),
                    c::CAMERA_MODE_CLOSE | c::CAMERA_MODE_FREE_ROAM => {
                        self.mode_lakitu_camera(f);
                    }
                    c::CAMERA_MODE_BOSS_FIGHT => self.mode_boss_fight_camera(f),
                    c::CAMERA_MODE_BEHIND_MARIO
                    | c::CAMERA_MODE_WATER_SURFACE
                    | c::CAMERA_MODE_INSIDE_CANNON
                    | c::CAMERA_MODE_8_DIRECTIONS
                    | c::CAMERA_MODE_OUTWARD_RADIAL
                    | c::CAMERA_MODE_PARALLEL_TRACKING
                    | c::CAMERA_MODE_SLIDE_HOOT
                    | c::CAMERA_MODE_FIXED
                    | c::CAMERA_MODE_SPIRAL_STAIRS => {
                        self.unsupported(Unsupported::Mode(mode as u8));
                    }
                    // Other values match no case and run no controller.
                    _ => {}
                }
            }
        }
        let cutscene = self.get_cutscene_from_mario_status(f.mario_cam);
        self.start_cutscene(cutscene);
        if self.rig.camera.cutscene != 0 {
            self.unsupported(Unsupported::Cutscene(self.rig.camera.cutscene));
        }
        f.flags.checking_for_camera = false;
        if self.level.level_num != c::LEVEL_CASTLE {
            let r_held = f.controller.button_down & R_TRIG != 0;
            let r_pressed = f.controller.button_pressed & R_TRIG != 0;
            let fixed = self.cam_select_alt_mode(0) == c::CAM_SELECTION_FIXED;
            if (self.rig.camera.cutscene == 0 && r_held && fixed)
                || self.rig.movement & c::CAM_MOVE_FIX_IN_PLACE != 0
                || f.mario_cam.action == c::ACT_GETTING_BLOWN
            {
                if self.rig.camera.cutscene == 0 && r_pressed && fixed {
                    self.sound_flags |= c::CAM_SOUND_FIXED_ACTIVE;
                    Self::sound(f.events, c::SOUND_MENU_CLICK_CHANGE_VIEW);
                }
                let r = &mut self.rig;
                r.lakitu.pos_h_speed = 0.0;
                r.lakitu.pos_v_speed = 0.0;
                r.camera.next_yaw = calculate_yaw(r.lakitu.focus, r.lakitu.pos, f.trig);
                r.camera.yaw = r.camera.next_yaw;
                r.movement &= !c::CAM_MOVE_FIX_IN_PLACE;
            } else if self.sound_flags & c::CAM_SOUND_FIXED_ACTIVE != 0 {
                Self::sound(f.events, c::SOUND_MENU_CLICK_CHANGE_VIEW);
                self.sound_flags &= !c::CAM_SOUND_FIXED_ACTIVE;
            }
        } else if f.controller.button_pressed & R_TRIG != 0
            && self.cam_select_alt_mode(0) == c::CAM_SELECTION_FIXED
        {
            Self::sound(f.events, c::SOUND_MENU_CAMERA_BUZZ);
        }
        self.rig.update(
            f.mario_cam.pos,
            f.mario_cam.action,
            f.world,
            f.flags,
            f.trig,
            f.rng,
            f.mario.pass_vanish_walls,
        );
        self.rig.lakitu.last_frame_action = f.mario_cam.action;
        match self.unsupported.take() {
            Some(what) => Err(what),
            None => Ok(()),
        }
    }

    /// update_camera_hud_status.
    fn update_camera_hud_status(&mut self, controller: &Controller) -> i16 {
        let mut status = c::CAM_STATUS_NONE;
        if self.rig.camera.cutscene != 0
            || (controller.button_down & R_TRIG != 0
                && self.cam_select_alt_mode(0) == c::CAM_SELECTION_FIXED)
        {
            status |= c::CAM_STATUS_FIXED;
        } else if self.set_cam_angle(0) == c::CAM_ANGLE_MARIO {
            status |= c::CAM_STATUS_MARIO;
        } else {
            status |= c::CAM_STATUS_LAKITU;
        }
        if self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
            status |= c::CAM_STATUS_C_DOWN;
        }
        if self.rig.movement & c::CAM_MOVE_C_UP_MODE != 0 {
            status |= c::CAM_STATUS_C_UP;
        }
        self.hud_status = status;
        status
    }

    /// cam_select_alt_mode: 0 queries; Mario (1) or fixed (2) selects, as
    /// the pause menu does.
    pub fn cam_select_alt_mode(&mut self, selection: i32) -> i32 {
        let mut mode = c::CAM_SELECTION_FIXED;
        if selection == c::CAM_SELECTION_MARIO {
            if self.selection_flags & c::CAM_MODE_MARIO_SELECTED == 0 {
                self.selection_flags |= c::CAM_MODE_MARIO_SELECTED;
            }
            self.sound_flags |= c::CAM_SOUND_UNUSED_SELECT_MARIO;
        }
        if selection == c::CAM_SELECTION_FIXED
            && self.selection_flags & c::CAM_MODE_MARIO_SELECTED != 0
        {
            self.set_cam_angle(c::CAM_ANGLE_LAKITU);
            self.selection_flags &= !c::CAM_MODE_MARIO_SELECTED;
            self.sound_flags |= c::CAM_SOUND_UNUSED_SELECT_FIXED;
        }
        if self.selection_flags & c::CAM_MODE_MARIO_SELECTED != 0 {
            mode = c::CAM_SELECTION_MARIO;
        }
        mode
    }

    /// set_cam_angle: 0 queries; Mario (1) or Lakitu (2) switches.
    pub fn set_cam_angle(&mut self, mode: i32) -> i32 {
        let mut current = c::CAM_ANGLE_LAKITU;
        if mode == c::CAM_ANGLE_MARIO && self.selection_flags & c::CAM_MODE_MARIO_ACTIVE == 0 {
            self.selection_flags |= c::CAM_MODE_MARIO_ACTIVE;
            if self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
                self.selection_flags |= c::CAM_MODE_LAKITU_WAS_ZOOMED_OUT;
                self.rig.movement &= !c::CAM_MOVE_ZOOMED_OUT;
            }
            self.sound_flags |= c::CAM_SOUND_MARIO_ACTIVE;
        }
        if mode == c::CAM_ANGLE_LAKITU && self.selection_flags & c::CAM_MODE_MARIO_ACTIVE != 0 {
            self.selection_flags &= !c::CAM_MODE_MARIO_ACTIVE;
            if self.selection_flags & c::CAM_MODE_LAKITU_WAS_ZOOMED_OUT != 0 {
                self.selection_flags &= !c::CAM_MODE_LAKITU_WAS_ZOOMED_OUT;
                self.rig.movement |= c::CAM_MOVE_ZOOMED_OUT;
            } else {
                self.rig.movement &= !c::CAM_MOVE_ZOOMED_OUT;
            }
            self.sound_flags |= c::CAM_SOUND_NORMAL_ACTIVE;
        }
        if self.selection_flags & c::CAM_MODE_MARIO_ACTIVE != 0 {
            current = c::CAM_ANGLE_MARIO;
        }
        current
    }

    fn play_sound_if_cam_switched_to_lakitu_or_mario(&mut self, events: &mut Vec<Event>) {
        if self.sound_flags & c::CAM_SOUND_MARIO_ACTIVE != 0 {
            Self::sound(events, c::SOUND_MENU_CLICK_CHANGE_VIEW);
        }
        if self.sound_flags & c::CAM_SOUND_NORMAL_ACTIVE != 0 {
            Self::sound(events, c::SOUND_MENU_CLICK_CHANGE_VIEW);
        }
        self.sound_flags &= !(c::CAM_SOUND_MARIO_ACTIVE | c::CAM_SOUND_NORMAL_ACTIVE);
    }

    // ---- Area processing ----

    /// camera_course_processing for levels without a camera trigger table.
    fn camera_course_processing(&mut self, f: &mut Frame<'_, '_>) {
        let level = self.level.level_num;
        let old_mode = self.rig.camera.mode;
        if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_C_UP {
            self.rig.camera.mode = self.rig.last_mode as u8;
        }
        self.check_blocking_area_processing(f.mario_cam.action);
        if LEVELS_WITH_TRIGGERS.contains(&level) {
            self.unsupported(Unsupported::Triggers(level));
        }
        if self.rig.status & c::CAM_FLAG_BLOCK_AREA_PROCESSING == 0 {
            let floor_type = self.geometry.curr.floor_type;
            match self.radial.area {
                c::AREA_WF => {
                    if f.mario_cam.action == c::ACT_RIDING_HOOT {
                        self.rig
                            .transition_to_camera_mode(c::CAMERA_MODE_SLIDE_HOOT, 60);
                    } else {
                        match floor_type {
                            c::SURFACE_CAMERA_8_DIR => {
                                self.rig
                                    .transition_to_camera_mode(c::CAMERA_MODE_8_DIRECTIONS, 90);
                                self.eight_dir_base_yaw = degrees(90);
                            }
                            c::SURFACE_BOSS_FIGHT_CAMERA if self.level.act_num == 1 => {
                                self.set_camera_mode_boss_fight();
                            }
                            _ => self.set_camera_mode_radial(f, 60),
                        }
                    }
                }
                c::AREA_BBH => {
                    let base = self.fixed_mode_base_position;
                    if base == [210.0, 420.0, 3109.0] && f.mario_cam.pos[1] < 1800.0 {
                        self.rig.transition_to_camera_mode(c::CAMERA_MODE_CLOSE, 30);
                    }
                }
                c::AREA_SSL_PYRAMID => {
                    self.set_mode_if_not_set_by_surface(c::CAMERA_MODE_OUTWARD_RADIAL as u8);
                }
                c::AREA_SSL_OUTSIDE => {
                    self.set_mode_if_not_set_by_surface(c::CAMERA_MODE_RADIAL as u8);
                }
                c::AREA_THI_HUGE => {}
                c::AREA_THI_TINY => self.surface_type_modes_thi(),
                c::AREA_TTC => {
                    self.set_mode_if_not_set_by_surface(c::CAMERA_MODE_OUTWARD_RADIAL as u8);
                }
                c::AREA_BOB => {
                    if self.set_mode_if_not_set_by_surface(c::CAMERA_MODE_NONE as u8) == 0 {
                        if self.geometry.curr.floor_type == c::SURFACE_BOSS_FIGHT_CAMERA {
                            self.set_camera_mode_boss_fight();
                        } else if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_CLOSE {
                            self.rig
                                .transition_to_camera_mode(c::CAMERA_MODE_RADIAL, 60);
                        } else {
                            self.set_camera_mode_radial(f, 60);
                        }
                    }
                }
                c::AREA_WDW_MAIN => {
                    if floor_type == c::SURFACE_INSTANT_WARP_1B {
                        self.rig.camera.def_mode = c::CAMERA_MODE_RADIAL as u8;
                    }
                }
                c::AREA_WDW_TOWN => {
                    if floor_type == c::SURFACE_INSTANT_WARP_1C {
                        self.rig.camera.def_mode = c::CAMERA_MODE_CLOSE as u8;
                    }
                }
                c::AREA_DDD_WHIRLPOOL => {
                    // Overwritten from the camera at the next update.
                    self.rig.lakitu.def_mode = c::CAMERA_MODE_OUTWARD_RADIAL as u8;
                }
                c::AREA_DDD_SUB => {
                    let mode = i16::from(self.rig.camera.mode);
                    if mode != c::CAMERA_MODE_BEHIND_MARIO && mode != c::CAMERA_MODE_WATER_SURFACE {
                        if f.mario_cam.action & c::ACT_FLAG_ON_POLE != 0
                            || self.geometry.curr.floor_height > 800.0
                        {
                            self.rig
                                .transition_to_camera_mode(c::CAMERA_MODE_8_DIRECTIONS, 60);
                        } else if f.mario_cam.pos[1] < 800.0 {
                            self.rig
                                .transition_to_camera_mode(c::CAMERA_MODE_FREE_ROAM, 60);
                        }
                    }
                    self.rig.lakitu.def_mode = c::CAMERA_MODE_FREE_ROAM as u8;
                }
                _ => {}
            }
        }
        self.rig.status &= !c::CAM_FLAG_BLOCK_AREA_PROCESSING;
        if i16::from(old_mode) == c::CAMERA_MODE_C_UP {
            self.rig.last_mode = i16::from(self.rig.camera.mode);
            self.rig.camera.mode = old_mode;
        }
    }

    fn check_blocking_area_processing(&mut self, action: u32) {
        let mode = i16::from(self.rig.camera.mode);
        if action & c::ACT_FLAG_METAL_WATER != 0
            || mode == c::CAMERA_MODE_BEHIND_MARIO
            || mode == c::CAMERA_MODE_WATER_SURFACE
        {
            self.rig.status |= c::CAM_FLAG_BLOCK_AREA_PROCESSING;
        }
        let level = self.level.level_num;
        if level == c::LEVEL_DDD || level == c::LEVEL_WDW || level == c::LEVEL_COTMC {
            self.rig.status &= !c::CAM_FLAG_BLOCK_AREA_PROCESSING;
        }
        if (mode == c::CAMERA_MODE_BEHIND_MARIO
            && action & (c::ACT_FLAG_SWIMMING | c::ACT_FLAG_METAL_WATER) == 0)
            || mode == c::CAMERA_MODE_INSIDE_CANNON
        {
            self.rig.status |= c::CAM_FLAG_BLOCK_AREA_PROCESSING;
        }
    }

    fn surface_type_modes(&mut self) -> u32 {
        let mut changed = 0;
        match self.geometry.curr.floor_type {
            c::SURFACE_CLOSE_CAMERA | c::SURFACE_NO_CAM_COL_SLIPPERY => {
                self.rig.transition_to_camera_mode(c::CAMERA_MODE_CLOSE, 90);
                changed += 1;
            }
            c::SURFACE_CAMERA_FREE_ROAM => {
                self.rig
                    .transition_to_camera_mode(c::CAMERA_MODE_FREE_ROAM, 90);
                changed += 1;
            }
            _ => {}
        }
        changed
    }

    fn set_mode_if_not_set_by_surface(&mut self, mode: u8) -> u32 {
        let changed = self.surface_type_modes();
        if changed == 0 && mode != 0 {
            self.rig.transition_to_camera_mode(i16::from(mode), 90);
        }
        changed
    }

    fn surface_type_modes_thi(&mut self) {
        let not_close = i16::from(self.rig.camera.mode) != c::CAMERA_MODE_CLOSE;
        match self.geometry.curr.floor_type {
            c::SURFACE_CLOSE_CAMERA
            | c::SURFACE_CAMERA_FREE_ROAM
            | c::SURFACE_NO_CAM_COL_SLIPPERY => {
                if not_close {
                    self.rig
                        .transition_to_camera_mode(c::CAMERA_MODE_FREE_ROAM, 90);
                }
            }
            c::SURFACE_CAMERA_8_DIR => {
                self.rig
                    .transition_to_camera_mode(c::CAMERA_MODE_8_DIRECTIONS, 90);
            }
            _ => self
                .rig
                .transition_to_camera_mode(c::CAMERA_MODE_RADIAL, 90),
        }
    }

    /// set_camera_mode_boss_fight.
    fn set_camera_mode_boss_fight(&mut self) {
        if i16::from(self.rig.camera.mode) != c::CAMERA_MODE_BOSS_FIGHT {
            self.rig
                .transition_to_camera_mode(c::CAMERA_MODE_BOSS_FIGHT, 15);
            self.rig.mode_offset_yaw = self.rig.camera.next_yaw.wrapping_sub(degrees(45));
        }
    }

    /// set_camera_mode_radial: transition unless the camera faces away from
    /// the area center by more than 90 degrees, then jump.
    fn set_camera_mode_radial(&mut self, f: &Frame<'_, '_>, transition_time: i16) {
        let m = f.mario_cam.pos;
        let focus = [self.radial.center[0], m[1], self.radial.center[1]];
        if i16::from(self.rig.camera.mode) != c::CAMERA_MODE_RADIAL {
            let yaw = calculate_yaw(focus, m, f.trig)
                .wrapping_sub(calculate_yaw(
                    self.rig.camera.focus,
                    self.rig.camera.pos,
                    f.trig,
                ))
                .wrapping_add(degrees(90));
            if yaw > 0 {
                self.rig
                    .transition_to_camera_mode(c::CAMERA_MODE_RADIAL, transition_time);
            } else {
                self.rig.camera.mode = c::CAMERA_MODE_RADIAL as u8;
                self.rig.status &= !c::CAM_FLAG_SMOOTH_MOVEMENT;
            }
            self.rig.mode_offset_yaw = 0;
        }
    }

    // ---- Mario's requests ----

    /// set_camera_mode, as Mario's code calls it. `mario_cam` must be
    /// sMarioCamState as the call saw it: Mario reports his state to the
    /// camera only at the end of his update, so a request from his tick
    /// reads the previous tick's report. -1 returns to the last mode.
    pub fn set_camera_mode(&mut self, mode: i16, frames: i16, f: &mut Frame<'_, '_>) {
        if mode == c::CAMERA_MODE_WATER_SURFACE && self.radial.area == c::AREA_TTM_OUTSIDE {
            return;
        }
        let mut mode = mode;
        let r = &mut self.rig;
        r.movement &= !(c::CAM_MOVE_RESTRICT | c::CAM_MOVE_ROTATE);
        r.movement |= c::CAM_MOVING_INTO_MODE;
        if mode == c::CAMERA_MODE_NONE {
            mode = c::CAMERA_MODE_CLOSE;
        }
        r.c_up_pitch = 0;
        r.mode_offset_yaw = 0;
        r.lakitu_dist = 0;
        r.lakitu_pitch = 0;
        r.area_yaw_change = 0;
        r.new_mode = if mode != -1 { mode } else { r.last_mode };
        r.last_mode = i16::from(r.camera.mode);
        self.mode_info.max = frames;
        self.mode_info.frame = 1;
        r.camera.mode = r.new_mode as u8;
        r.lakitu.mode = r.camera.mode;
        let m = f.mario_cam.pos;
        let sub = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| v[i] - m[i]) };
        let mut end_focus = sub(r.camera.focus);
        let mut end_pos = sub(r.camera.pos);
        let new_mode = r.new_mode;
        self.area_yaw = self.mode_transition(new_mode, &mut end_focus, &mut end_pos, f);
        let end_focus = sub(end_focus);
        let end_pos = sub(end_pos);
        let r = &self.rig;
        let start_focus = sub(r.lakitu.cur_focus);
        let start_pos = sub(r.lakitu.cur_pos);
        let (dist, pitch, yaw) = get_dist_angle(start_focus, start_pos, f.trig);
        self.mode_info.start = TransitionPoint {
            focus: start_focus,
            pos: start_pos,
            dist,
            pitch,
            yaw,
        };
        let (dist, pitch, yaw) = get_dist_angle(end_focus, end_pos, f.trig);
        self.mode_info.end = TransitionPoint {
            focus: end_focus,
            pos: end_pos,
            dist,
            pitch,
            yaw,
        };
    }

    /// sModeTransitions[mode](c, focus, pos) for the modes this port has.
    fn mode_transition(
        &mut self,
        mode: i16,
        focus: &mut [f32; 3],
        pos: &mut [f32; 3],
        f: &mut Frame<'_, '_>,
    ) -> i16 {
        match mode {
            c::CAMERA_MODE_RADIAL => {
                let goal = self.update_radial_camera(f);
                *focus = goal.focus;
                *pos = goal.pos;
                goal.yaw
            }
            c::CAMERA_MODE_CLOSE | 7 | c::CAMERA_MODE_FREE_ROAM => {
                self.update_mario_camera(f, focus, pos)
            }
            c::CAMERA_MODE_C_UP => self.update_c_up(f, focus, pos),
            c::CAMERA_MODE_WATER_SURFACE => 0,
            c::CAMERA_MODE_BOSS_FIGHT => self.update_boss_fight_camera(f, focus, pos),
            c::CAMERA_MODE_NONE => {
                panic!("set_camera_mode calls the NULL transition of camera mode 0")
            }
            c::CAMERA_MODE_OUTWARD_RADIAL
            | c::CAMERA_MODE_BEHIND_MARIO
            | 5
            | c::CAMERA_MODE_SLIDE_HOOT
            | c::CAMERA_MODE_INSIDE_CANNON
            | c::CAMERA_MODE_PARALLEL_TRACKING
            | c::CAMERA_MODE_FIXED
            | c::CAMERA_MODE_8_DIRECTIONS
            | 15
            | c::CAMERA_MODE_SPIRAL_STAIRS => {
                self.unsupported(Unsupported::Mode(mode as u8));
                // The transition is not run; the endpoints keep the current camera.
                0
            }
            _ => panic!("set_camera_mode reads past the transition table (mode {mode})"),
        }
    }

    /// set_camera_shake_from_hit, as Mario's code calls it.
    pub fn shake_from_hit(&mut self, shake: i16, f: &mut Frame<'_, '_>) {
        self.rig.shake_from_hit(shake, f.mario_cam.action, f.rng);
    }

    // ---- Shared mode helpers ----

    fn store_lakitu_cam_info_for_c_up(&mut self, mario_cam: &PlayerCameraState) {
        let m = mario_cam.pos;
        let c = &self.rig.camera;
        self.store_c_up.pos = std::array::from_fn(|i| c.pos[i] - m[i]);
        self.store_c_up.focus = [0.0, c.focus[1] - m[1], 0.0];
    }

    /// set_mode_c_up: Mario enters first person on his next update.
    fn set_mode_c_up(&mut self, mario_cam: &PlayerCameraState) -> i32 {
        if self.rig.movement & c::CAM_MOVE_C_UP_MODE == 0 {
            self.rig.movement |= c::CAM_MOVE_C_UP_MODE;
            self.store_lakitu_cam_info_for_c_up(mario_cam);
            self.sound_flags &= !c::CAM_SOUND_C_UP_PLAYED;
            return 1;
        }
        0
    }

    fn approach_camera_height(&mut self, goal: f32, inc: f32) {
        let y = &mut self.rig.camera.pos[1];
        if self.rig.status & c::CAM_FLAG_SMOOTH_MOVEMENT != 0 {
            if *y < goal {
                *y += inc;
                if *y > goal {
                    *y = goal;
                }
            } else {
                *y -= inc;
                if *y < goal {
                    *y = goal;
                }
            }
        } else {
            *y = goal;
        }
    }

    /// set_camera_height: the radial camera's height between floors and
    /// ceilings, with its own rule while Mario hangs.
    fn set_camera_height(&mut self, mut goal_height: f32, f: &mut Frame<'_, '_>) {
        let base_off = 125.0;
        let pos = self.rig.camera.pos;
        let (mut cam_ceil_height, _) =
            f.world
                .find_ceil(pos[0], self.rig.lakitu.goal_pos[1] - 50.0, pos[2], *f.flags);
        let g = self.geometry.curr;
        let action = f.mario_cam.action;
        if action & c::ACT_FLAG_HANGING != 0 {
            let mario_ceil_height = g.ceil_height;
            let mut mario_floor_height = g.floor_height;
            if mario_floor_height < mario_ceil_height - 400.0 {
                mario_floor_height = mario_ceil_height - 400.0;
            }
            goal_height = mario_floor_height + (mario_ceil_height - mario_floor_height) * 0.4;
            if f.mario_cam.pos[1] - 400.0 > goal_height {
                goal_height = f.mario_cam.pos[1] - 400.0;
            }
            self.approach_camera_height(goal_height, 5.0);
        } else {
            let (floor, _) = f.world.find_floor(pos[0], pos[1] + 100.0, pos[2], f.flags);
            let mut cam_floor_height = floor + base_off;
            let mario_floor_height = base_off + g.floor_height;
            if cam_floor_height < mario_floor_height {
                cam_floor_height = mario_floor_height;
            }
            if goal_height < cam_floor_height {
                goal_height = cam_floor_height;
                self.rig.camera.pos[1] = goal_height;
            }
            if action == c::ACT_BUTT_STUCK_IN_GROUND
                || action == c::ACT_HEAD_STUCK_IN_GROUND
                || action == c::ACT_FEET_STUCK_IN_GROUND
            {
                let d = self.rig.camera.pos[1] - goal_height;
                if (if d > 0.0 { d } else { -d }) > 1000.0 {
                    self.rig.camera.pos[1] = goal_height;
                }
            }
            self.approach_camera_height(goal_height, 20.0);
            if cam_ceil_height != CELL_HEIGHT_LIMIT {
                cam_ceil_height -= base_off;
                let y = self.rig.camera.pos[1];
                if (y > cam_ceil_height && g.floor_height + base_off < cam_ceil_height)
                    || (g.ceil_height != CELL_HEIGHT_LIMIT
                        && g.ceil_height > cam_ceil_height
                        && y > cam_ceil_height)
                {
                    self.rig.camera.pos[1] = cam_ceil_height;
                }
            }
        }
    }

    fn pan_ahead_of_player(&mut self, f: &Frame<'_, '_>) {
        self.rig.pan_ahead_of_player(
            f.mario_cam.pos,
            f.mario_cam.action,
            f.mario_cam.face_angle[1],
            f.trig,
        );
    }

    // ---- Radial mode ----

    fn update_radial_camera(&mut self, f: &mut Frame<'_, '_>) -> RadialGoal {
        let mut state = RadialState {
            area: self.radial.area,
            center_x: self.radial.center[0],
            center_z: self.radial.center[1],
            mode_offset_yaw: self.rig.mode_offset_yaw,
            lakitu_dist: self.rig.lakitu_dist,
            lakitu_pitch: self.rig.lakitu_pitch,
            area_yaw: self.area_yaw,
        };
        let goal = update_radial_camera(
            &mut state,
            f.mario_cam.pos,
            f.mario_cam.action,
            self.geometry.curr,
            f.world,
            f.flags,
            f.trig,
        );
        self.area_yaw = state.area_yaw;
        goal
    }

    /// update_yaw_and_dist_from_c_up: the radial camera resumes where C-Up
    /// left it.
    fn update_yaw_and_dist_from_c_up(&mut self) {
        let dist = 1000.0;
        self.rig.mode_offset_yaw = self.mode_info.start.yaw.wrapping_sub(self.area_yaw);
        self.rig.lakitu_dist = f32_to_s16(self.mode_info.start.dist - dist);
        self.rig.movement &= !c::CAM_MOVING_INTO_MODE;
    }

    /// radial_camera_input: C-Left/Right rotate, C-Up zooms in or enters
    /// C-Up, C-Down zooms out.
    fn radial_camera_input(&mut self, f: &mut Frame<'_, '_>) {
        let pressed = f.controller.button_pressed;
        let offset = self.rig.mode_offset_yaw;
        let radial = i16::from(self.rig.camera.mode) == c::CAMERA_MODE_RADIAL;
        if self.rig.movement & c::CAM_MOVE_ENTERED_ROTATE_SURFACE != 0
            || self.rig.movement & c::CAM_MOVE_ROTATE == 0
        {
            if pressed & (L_CBUTTONS | R_CBUTTONS) != 0 {
                // The original does not clear the surface's rotation flags.
                self.rig.movement &= !c::CAM_MOVE_ENTERED_ROTATE_SURFACE;
            }
            if pressed & R_CBUTTONS != 0 {
                if offset > -0x800 {
                    if self.rig.movement & c::CAM_MOVE_ROTATE_RIGHT == 0 {
                        self.rig.movement |= c::CAM_MOVE_ROTATE_RIGHT;
                    }
                    let limit = if radial {
                        if offset > 0x22AA {
                            self.radial.second_rotate |= c::CAM_MOVE_ROTATE_RIGHT;
                        }
                        degrees(105)
                    } else {
                        degrees(60)
                    };
                    Self::sound(
                        f.events,
                        if offset == limit {
                            c::SOUND_MENU_CAMERA_BUZZ
                        } else {
                            c::SOUND_MENU_CAMERA_TURN
                        },
                    );
                } else {
                    self.rig.movement |= c::CAM_MOVE_RETURN_TO_MIDDLE;
                    Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_IN);
                }
            }
            if pressed & L_CBUTTONS != 0 {
                if offset < 0x800 {
                    if self.rig.movement & c::CAM_MOVE_ROTATE_LEFT == 0 {
                        self.rig.movement |= c::CAM_MOVE_ROTATE_LEFT;
                    }
                    let limit = if radial {
                        if offset < -0x22AA {
                            self.radial.second_rotate |= c::CAM_MOVE_ROTATE_LEFT;
                        }
                        degrees(-105)
                    } else {
                        degrees(-60)
                    };
                    Self::sound(
                        f.events,
                        if offset == limit {
                            c::SOUND_MENU_CAMERA_BUZZ
                        } else {
                            c::SOUND_MENU_CAMERA_TURN
                        },
                    );
                } else {
                    self.rig.movement |= c::CAM_MOVE_RETURN_TO_MIDDLE;
                    Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_IN);
                }
            }
        }
        if pressed & U_CBUTTONS != 0 {
            if self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
                self.rig.movement &= !c::CAM_MOVE_ZOOMED_OUT;
                Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_IN);
            } else {
                self.set_mode_c_up(f.mario_cam);
            }
        }
        if pressed & D_CBUTTONS != 0 {
            if self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
                self.rig.movement |= c::CAM_MOVE_ALREADY_ZOOMED_OUT;
                // play_camera_buzz_if_cdown: C-Down is pressed here.
                Self::sound(f.events, c::SOUND_MENU_CAMERA_BUZZ);
            } else {
                self.rig.movement |= c::CAM_MOVE_ZOOMED_OUT;
                Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_OUT);
            }
        }
    }

    /// mode_radial_camera: rotate around the area center.
    fn mode_radial_camera(&mut self, f: &mut Frame<'_, '_>) {
        let old_area_yaw = self.area_yaw;
        if self.rig.movement & c::CAM_MOVING_INTO_MODE != 0 {
            self.update_yaw_and_dist_from_c_up();
        }
        self.radial_camera_input(f);
        let floor_types = (self.geometry.curr.floor_type, self.geometry.prev_floor_type);
        self.radial.move_camera(
            &mut self.rig,
            f.mario_cam.pos,
            f.mario.forward_vel,
            floor_types.0,
            floor_types.1,
            f.world,
            *f.flags,
            f.trig,
        );
        if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_RADIAL {
            self.radial.zoom(&mut self.rig, 400.0, 0x900);
        }
        let goal = self.update_radial_camera(f);
        self.rig.camera.focus = goal.focus;
        self.rig.camera.next_yaw = goal.yaw;
        self.rig.camera.pos[0] = goal.pos[0];
        self.rig.camera.pos[2] = goal.pos[2];
        self.rig.area_yaw_change = self.area_yaw.wrapping_sub(old_area_yaw);
        let mut height = goal.pos[1];
        if f.mario_cam.action == c::ACT_RIDING_HOOT {
            height += 500.0;
        }
        self.set_camera_height(height, f);
        self.pan_ahead_of_player(f);
    }

    // ---- Lakitu (close, free roam) and Mario modes ----

    /// update_mario_camera: the CLOSE and FREE_ROAM transition target.
    fn update_mario_camera(
        &mut self,
        f: &Frame<'_, '_>,
        focus: &mut [f32; 3],
        pos: &mut [f32; 3],
    ) -> i16 {
        let face = f.mario_cam.face_angle[1];
        let yaw = face
            .wrapping_add(self.rig.mode_offset_yaw)
            .wrapping_add(degrees(180));
        (*focus, *pos) = focus_on_mario(
            f.mario_cam.pos,
            125.0,
            125.0,
            self.zoom_dist,
            0x05B0,
            yaw,
            self.rig.lakitu_pitch,
            f.trig,
        );
        face
    }

    /// handle_c_button_movement: the C buttons in the Lakitu, Mario and boss
    /// cameras.
    fn handle_c_button_movement(&mut self, f: &mut Frame<'_, '_>) {
        let pressed = f.controller.button_pressed;
        let fixed = i16::from(self.rig.camera.mode) == c::CAMERA_MODE_FIXED;
        if pressed & U_CBUTTONS != 0 {
            if !fixed && self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
                self.rig.movement &= !c::CAM_MOVE_ZOOMED_OUT;
                Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_IN);
            } else {
                self.set_mode_c_up(f.mario_cam);
                self.zoom_amount = if self.zero_zoom_dist > self.zoom_dist {
                    -self.zoom_dist
                } else {
                    self.zoom_dist
                };
            }
        }
        if !fixed {
            if pressed & D_CBUTTONS != 0 {
                if self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
                    self.rig.movement |= c::CAM_MOVE_ALREADY_ZOOMED_OUT;
                    self.zoom_amount = self.zoom_dist + 400.0;
                    Self::sound(f.events, c::SOUND_MENU_CAMERA_BUZZ);
                } else {
                    self.rig.movement |= c::CAM_MOVE_ZOOMED_OUT;
                    self.zoom_amount = self.zoom_dist + 400.0;
                    Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_OUT);
                }
            }
            let c_side_yaw: i16 = 0x1000;
            if pressed & R_CBUTTONS != 0 {
                if self.rig.movement & c::CAM_MOVE_ROTATE_LEFT != 0 {
                    self.rig.movement &= !c::CAM_MOVE_ROTATE_LEFT;
                } else {
                    self.rig.movement |= c::CAM_MOVE_ROTATE_RIGHT;
                    if self.c_side_button_yaw == 0 {
                        Self::sound(f.events, c::SOUND_MENU_CAMERA_TURN);
                    }
                    self.c_side_button_yaw = -c_side_yaw;
                }
            }
            if pressed & L_CBUTTONS != 0 {
                if self.rig.movement & c::CAM_MOVE_ROTATE_RIGHT != 0 {
                    self.rig.movement &= !c::CAM_MOVE_ROTATE_RIGHT;
                } else {
                    self.rig.movement |= c::CAM_MOVE_ROTATE_LEFT;
                    if self.c_side_button_yaw == 0 {
                        Self::sound(f.events, c::SOUND_MENU_CAMERA_TURN);
                    }
                    self.c_side_button_yaw = c_side_yaw;
                }
            }
        }
    }

    /// set_handheld_shake: lasts one tick (update_lakitu clears it).
    fn set_handheld_shake(&mut self, mode: u8) {
        let (magnitude, increment) = match mode {
            c::HAND_CAM_SHAKE_CUTSCENE => (0x600, 0.04),
            c::HAND_CAM_SHAKE_LOW => (0x300, 0.06),
            c::HAND_CAM_SHAKE_HIGH => (0x1000, 0.1),
            c::HAND_CAM_SHAKE_UNUSED | c::HAND_CAM_SHAKE_HANG_OWL => (0x600, 0.07),
            c::HAND_CAM_SHAKE_STAR_DANCE => (0x400, 0.07),
            _ => (0, 0.0),
        };
        self.rig.handheld_magnitude = magnitude;
        self.rig.handheld_increment = increment;
    }

    /// update_default_camera: behind Mario, turning toward his facing while
    /// he stands still, avoiding walls, and resting on the floor under him.
    fn update_default_camera(&mut self, f: &mut Frame<'_, '_>) -> i16 {
        let trig = f.trig;
        let world = f.world;
        let m = f.mario_cam.pos;
        let action = f.mario_cam.action;
        let yaw_goal = f.mario_cam.face_angle[1].wrapping_add(degrees(180));
        let mut yaw_vel: i16 = 0;
        let mut avoid_status;
        let mut close_to_mario = 0;
        let goal_pos = self.rig.lakitu.goal_pos;
        let (mut ceil_height, ceil) =
            world.find_ceil(goal_pos[0], goal_pos[1], goal_pos[2], *f.flags);
        self.handle_c_button_movement(f);
        let (mut dist, mut pitch, mut yaw) = get_dist_angle(m, self.rig.camera.pos, trig);
        let mut zoom_dist = if self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0 {
            // The Mario camera zooms out further (1400 vs 1200).
            if self.set_cam_angle(0) == c::CAM_ANGLE_MARIO {
                self.zoom_dist + 1050.0
            } else {
                self.zoom_dist + 400.0
            }
        } else {
            self.zoom_dist
        };
        if action & c::ACT_FLAG_HANGING != 0 || action == c::ACT_RIDING_HOOT {
            zoom_dist *= 0.8;
            self.set_handheld_shake(c::HAND_CAM_SHAKE_HANG_OWL);
        }
        if self.zoom_amount == 0.0 {
            if dist > zoom_dist {
                dist -= 50.0;
                if dist < zoom_dist {
                    dist = zoom_dist;
                }
            }
        } else {
            self.zoom_amount -= 30.0;
            if self.zoom_amount < 0.0 {
                self.zoom_amount = 0.0;
            }
            if dist > zoom_dist {
                dist -= 30.0;
                if dist < zoom_dist {
                    dist = zoom_dist;
                }
            }
            if dist < zoom_dist {
                dist += 30.0;
                if dist > zoom_dist {
                    dist = zoom_dist;
                }
            }
        }
        let mut next_yaw_vel: i16;
        if self.c_side_button_yaw == 0 {
            next_yaw_vel = if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_FREE_ROAM {
                0xC0
            } else {
                0x100
            };
            if f.controller.stick_x != 0.0 || f.controller.stick_y != 0.0 {
                next_yaw_vel = 0x20;
            }
        } else {
            if self.c_side_button_yaw < 0 {
                yaw = yaw.wrapping_add(0x200);
            }
            if self.c_side_button_yaw > 0 {
                yaw = yaw.wrapping_sub(0x200);
            }
            camera_approach_s16_symmetric_bool(&mut self.c_side_button_yaw, 0, 0x100);
            next_yaw_vel = 0;
        }
        self.rig.yaw_speed = 0x400;
        let xz_dist = calc_hor_dist(m, self.rig.camera.pos);
        if self.rig.status & c::CAM_FLAG_BEHIND_MARIO_POST_DOOR != 0 {
            if xz_dist >= 250.0 {
                self.rig.status &= !c::CAM_FLAG_BEHIND_MARIO_POST_DOOR;
            }
            let half = (i32::from(f.mario_cam.face_angle[1]) - i32::from(yaw)) / 2;
            if half.abs() < 0x1800 {
                self.rig.status &= !c::CAM_FLAG_BEHIND_MARIO_POST_DOOR;
                yaw = self.yaw_after_door_cutscene.wrapping_add(degrees(180));
                dist = 800.0;
                self.rig.status |= c::CAM_FLAG_BLOCK_SMOOTH_MOVEMENT;
            }
        } else if xz_dist < 250.0 {
            // Turn rapidly if very close to Mario.
            let pos = &mut self.rig.camera.pos;
            pos[0] += (250.0 - xz_dist) * trig.sins(i32::from(yaw));
            pos[2] += (250.0 - xz_dist) * trig.coss(i32::from(yaw));
            if self.c_side_button_yaw == 0 {
                next_yaw_vel = 0x1000;
                self.rig.yaw_speed = 0;
                (dist, pitch, yaw) = get_dist_angle(m, self.rig.camera.pos, trig);
            }
            close_to_mario |= 1;
        }
        if -16.0 < f.controller.stick_y {
            self.rig.camera.yaw = yaw;
        }
        let [pos_height, foc_height] =
            calc_y_to_curr_floor(m, action, self.geometry.curr, 1.0, 200.0, 0.9, 200.0, world);
        let mut avoid_yaw = 0;
        avoid_status = rotate_camera_around_walls(
            m,
            self.rig.camera.pos,
            &mut avoid_yaw,
            0x600,
            &mut self.rig.status,
            world,
            *f.flags,
            f.mario.pass_vanish_walls,
            trig,
        );
        if avoid_status == 3 {
            self.unused_free_roam_wall_yaw = avoid_yaw;
            self.avoid_yaw_vel = yaw;
            self.rig.status |= c::CAM_FLAG_COLLIDED_WITH_WALL;
            // The original also rebuilds cPos from its own angles around this
            // approach; the position is recomputed below before any read.
            approach_s16_asymptotic_bool(&mut yaw, avoid_yaw, 10);
            self.avoid_yaw_vel = ((i32::from(self.avoid_yaw_vel) - i32::from(yaw)) / 0x100) as i16;
        } else {
            if f.mario.forward_vel == 0.0 {
                if self.rig.status & c::CAM_FLAG_COLLIDED_WITH_WALL != 0 {
                    let yaw_dir = if (i32::from(yaw_goal) - i32::from(yaw)) / 0x100 >= 0 {
                        -1
                    } else {
                        1
                    };
                    if (self.avoid_yaw_vel > 0 && yaw_dir > 0)
                        || (self.avoid_yaw_vel < 0 && yaw_dir < 0)
                    {
                        yaw_vel = next_yaw_vel;
                    }
                } else {
                    yaw_vel = next_yaw_vel;
                }
            } else {
                if next_yaw_vel == 0x1000 {
                    yaw_vel = next_yaw_vel;
                }
                self.rig.status &= !c::CAM_FLAG_COLLIDED_WITH_WALL;
            }
            if avoid_status != 0 {
                yaw_vel = yaw_vel.wrapping_add(yaw_vel);
            }
            if close_to_mario & 1 != 0 && avoid_status != 0 {
                yaw_vel = 0;
            }
            // get_dialog_id() == DIALOG_NONE: no dialog system runs.
            if yaw_vel != 0 {
                camera_approach_s16_symmetric_bool(&mut yaw, yaw_goal, yaw_vel);
            }
        }
        if avoid_status == 0 && self.rig.status & c::CAM_FLAG_COLLIDED_WITH_WALL == 0 {
            approach_f32_asymptotic_bool(&mut dist, zoom_dist - 100.0, 0.05);
        }
        let mut c_pos = set_dist_and_angle(m, dist, pitch, yaw, trig);
        c_pos[1] += pos_height + 125.0;
        if collide_with_walls(
            &mut c_pos,
            10.0,
            80.0,
            world,
            *f.flags,
            f.mario.pass_vanish_walls,
        ) != 0
        {
            self.rig.status |= c::CAM_FLAG_COLLIDED_WITH_WALL;
        }
        self.rig.camera.focus = [m[0], m[1] + 125.0 + foc_height, m[2]];
        let mut mario_floor_height = 125.0 + self.geometry.curr.floor_height;
        let mut mario_floor = self.geometry.curr.floor;
        let (floor, _) = world.find_floor(c_pos[0], c_pos[1] + 50.0, c_pos[2], f.flags);
        let mut cam_floor_height = floor + 125.0;
        let mut scale = 0.1f32;
        while scale < 1.0 {
            let temp = scale_along_line(c_pos, m, scale);
            let (height, temp_floor) = world.find_floor(temp[0], temp[1], temp[2], f.flags);
            let height = height + 125.0;
            if temp_floor.is_some() && height > mario_floor_height {
                mario_floor_height = height;
                mario_floor = temp_floor;
            }
            scale += 0.2;
        }
        if self.selection_flags & c::CAM_MODE_MARIO_ACTIVE != 0 {
            mario_floor_height -= 35.0;
            cam_floor_height -= 35.0;
            self.rig.camera.focus[1] -= 25.0;
        }
        let mut water_height = world.find_water_level(c_pos[0], c_pos[2]);
        if water_height != FLOOR_LOWER_LIMIT {
            water_height += 125.0;
            let dist_from_water = water_height - mario_floor_height;
            if self.rig.movement & c::CAM_MOVE_METAL_BELOW_WATER == 0 {
                if dist_from_water > 800.0 && action & c::ACT_FLAG_METAL_WATER != 0 {
                    self.rig.movement |= c::CAM_MOVE_METAL_BELOW_WATER;
                }
            } else if dist_from_water < 400.0 || action & c::ACT_FLAG_METAL_WATER == 0 {
                self.rig.movement &= !c::CAM_MOVE_METAL_BELOW_WATER;
            }
            if self.rig.movement & c::CAM_MOVE_METAL_BELOW_WATER == 0
                && cam_floor_height < water_height
            {
                cam_floor_height = water_height;
            }
        } else {
            self.rig.movement &= !c::CAM_MOVE_METAL_BELOW_WATER;
        }
        c_pos[1] = cam_floor_height;
        let mut temp = c_pos;
        temp[1] -= 125.0;
        if let Some(floor) = mario_floor
            && cam_floor_height <= mario_floor_height
        {
            let surface = world.surface(floor);
            avoid_status = i32::from(is_range_behind_surface(
                self.rig.camera.focus,
                temp,
                Some(surface),
                0,
                -1,
                trig,
            ));
            if avoid_status != 1 && ceil_height > mario_floor_height {
                cam_floor_height = mario_floor_height;
            }
        }
        let mut pos_height = 0.0;
        let zoomed_out = self.rig.movement & c::CAM_MOVE_ZOOMED_OUT != 0;
        if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_FREE_ROAM {
            if zoomed_out {
                pos_height = 375.0;
                if self.radial.area == c::AREA_SSL_PYRAMID {
                    pos_height /= 2.0;
                }
            } else {
                pos_height = 100.0;
            }
        }
        if zoomed_out && self.selection_flags & c::CAM_MODE_MARIO_ACTIVE != 0 {
            pos_height = 610.0;
            if self.radial.area == c::AREA_SSL_PYRAMID || self.level.level_num == c::LEVEL_CASTLE {
                pos_height /= 2.0;
            }
        }
        let mut gas_height = world.find_poison_gas_level(c_pos[0], c_pos[2]);
        if gas_height != FLOOR_LOWER_LIMIT {
            gas_height += 130.0;
            if gas_height > self.rig.camera.pos[1] {
                self.rig.camera.pos[1] = gas_height;
            }
        }
        if action & c::ACT_FLAG_HANGING != 0 || action == c::ACT_RIDING_HOOT {
            cam_floor_height = m[1] + 400.0;
            if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_FREE_ROAM {
                cam_floor_height -= 100.0;
            }
            ceil_height = CELL_HEIGHT_LIMIT;
            self.rig.camera.focus = m;
        }
        assert!(
            action & c::ACT_FLAG_ON_POLE == 0,
            "the default camera on a pole reads the pole object, which is not simulated"
        );
        if cam_floor_height != FLOOR_LOWER_LIMIT {
            cam_floor_height += pos_height;
            self.approach_camera_height(cam_floor_height, 20.0);
        }
        self.rig.camera.pos[0] = c_pos[0];
        self.rig.camera.pos[2] = c_pos[2];
        let c_pos = [goal_pos[0], self.rig.camera.pos[1], goal_pos[2]];
        let (mut d, temp_pitch, temp_yaw) = get_dist_angle(c_pos, self.rig.camera.pos, trig);
        // Prevent the camera from lagging behind too much.
        if d > 50.0 {
            d = 50.0;
            self.rig.camera.pos = set_dist_and_angle(c_pos, d, temp_pitch, temp_yaw, trig);
        }
        if self.geometry.curr.floor_type != c::SURFACE_DEATH_PLANE {
            let focus = self.rig.camera.focus;
            let (d, temp_pitch, temp_yaw) = get_dist_angle(focus, self.rig.camera.pos, trig);
            if d > zoom_dist {
                self.rig.camera.pos =
                    set_dist_and_angle(focus, zoom_dist, temp_pitch, temp_yaw, trig);
            }
        }
        if ceil_height != CELL_HEIGHT_LIMIT {
            ceil_height -= 150.0;
            if self.rig.camera.pos[1] > ceil_height {
                let ceil = ceil.map(|i| world.surface(i));
                if is_range_behind_surface(self.rig.camera.pos, m, ceil, 0, -1, trig) {
                    self.rig.camera.pos[1] = ceil_height;
                }
            }
        }
        if self.radial.area == c::AREA_WDW_TOWN {
            yaw = clamp_positions_and_find_yaw(
                &mut self.rig.camera.pos,
                self.rig.camera.focus,
                2254.0,
                -3789.0,
                3790.0,
                -2253.0,
                trig,
            );
        }
        yaw
    }

    fn mode_default_camera(&mut self, f: &mut Frame<'_, '_>) {
        self.fov.func = c::CAM_FOV_DEFAULT;
        self.rig.camera.next_yaw = self.update_default_camera(f);
        self.pan_ahead_of_player(f);
    }

    /// mode_lakitu_camera: CLOSE and FREE_ROAM.
    fn mode_lakitu_camera(&mut self, f: &mut Frame<'_, '_>) {
        self.zoom_dist = 800.0;
        self.mode_default_camera(f);
    }

    /// mode_mario_camera: the R-button Mario camera.
    fn mode_mario_camera(&mut self, f: &mut Frame<'_, '_>) {
        self.zoom_dist = 350.0;
        self.mode_default_camera(f);
    }

    // ---- Boss fight ----

    fn set_environmental_camera_shake(&mut self, shake: i16) {
        let r = &mut self.rig;
        match shake {
            c::SHAKE_ENV_EXPLOSION => r.set_pitch_shake(0x60, 0x8, 0x4000),
            c::SHAKE_ENV_BOWSER_THROW_BOUNCE => r.set_pitch_shake(0xC0, 0x8, 0x4000),
            c::SHAKE_ENV_BOWSER_JUMP => r.set_pitch_shake(0x100, 0x8, 0x3000),
            c::SHAKE_ENV_UNUSED_6 => r.set_roll_shake(0x80, 0x10, 0x3000),
            c::SHAKE_ENV_UNUSED_7 => r.set_pitch_shake(0x20, 0x8, i16::MIN),
            c::SHAKE_ENV_PYRAMID_EXPLODE => r.set_pitch_shake(0x40, 0x8, i16::MIN),
            c::SHAKE_ENV_JRB_SHIP_DRAIN => {
                r.set_pitch_shake(0x20, 0x8, i16::MIN);
                r.set_roll_shake(0x400, 0x10, 0x100);
            }
            c::SHAKE_ENV_FALLING_BITS_PLAT => r.set_pitch_shake(0x40, 0x2, i16::MIN),
            c::SHAKE_ENV_UNUSED_5 => r.set_yaw_shake(-0x200, 0x80, 0x200),
            _ => {}
        }
    }

    /// update_boss_fight_camera without a boss: it circles the area center.
    fn update_boss_fight_camera(
        &mut self,
        f: &mut Frame<'_, '_>,
        focus: &mut [f32; 3],
        pos: &mut [f32; 3],
    ) -> i16 {
        let trig = f.trig;
        let m = f.mario_cam.pos;
        self.handle_c_button_movement(f);
        if f.mario_cam.camera_event == c::CAM_EVENT_BOWSER_JUMP {
            self.set_environmental_camera_shake(c::SHAKE_ENV_BOWSER_JUMP);
            f.mario_cam.camera_event = 0;
        }
        if f.mario_cam.camera_event == c::CAM_EVENT_BOWSER_THROW_BOUNCE {
            self.set_environmental_camera_shake(c::SHAKE_ENV_BOWSER_THROW_BOUNCE);
            f.mario_cam.camera_event = 0;
        }
        let yaw = self.rig.mode_offset_yaw.wrapping_add(degrees(45));
        // gSecondCameraFocus is the boss object; none is simulated.
        let second_focus = [self.radial.center[0], m[1], self.radial.center[1]];
        let mut focus_distance = calc_abs_dist(m, second_focus) * 1.6;
        if focus_distance < 800.0 {
            focus_distance = 800.0;
        }
        if focus_distance > 5000.0 {
            focus_distance = 5000.0;
        }
        focus[0] = (m[0] + second_focus[0]) / 2.0;
        focus[1] = (m[1] + second_focus[1]) / 2.0 + 125.0;
        focus[2] = (m[2] + second_focus[2]) / 2.0;
        *pos = set_dist_and_angle(*focus, focus_distance, 0x1000, yaw, trig);
        let (height, floor) = f.world.find_floor(
            self.radial.center[0],
            CELL_HEIGHT_LIMIT,
            self.radial.center[1],
            f.flags,
        );
        pos[1] = height;
        if let Some(floor) = floor {
            let floor = f.world.surface(floor);
            let [nx, ny, nz] = floor.normal;
            pos[1] = 300.0 - (nx * pos[0] + nz * pos[2] + floor.origin_offset) / ny;
            match self.radial.area {
                // BOB falls through to WF's offset, rising twice as high.
                c::AREA_BOB => {
                    pos[1] += 125.0;
                    pos[1] += 125.0;
                }
                c::AREA_WF => pos[1] += 125.0,
                _ => {}
            }
        }
        if self.level.level_num == c::LEVEL_BBH {
            pos[1] = 2047.0;
        }
        if self.c_side_button_yaw < 0 {
            self.rig.mode_offset_yaw = self.rig.mode_offset_yaw.wrapping_add(0x200);
            self.c_side_button_yaw = self.c_side_button_yaw.wrapping_add(0x100);
            if self.c_side_button_yaw > 0 {
                self.c_side_button_yaw = 0;
            }
        }
        if self.c_side_button_yaw > 0 {
            self.rig.mode_offset_yaw = self.rig.mode_offset_yaw.wrapping_sub(0x200);
            self.c_side_button_yaw = self.c_side_button_yaw.wrapping_sub(0x100);
            if self.c_side_button_yaw < 0 {
                self.c_side_button_yaw = 0;
            }
        }
        focus[1] = (m[1] + second_focus[1]) / 2.0 + 100.0;
        if focus_distance < 400.0 {
            focus_distance = 400.0;
        }
        self.radial.zoom(&mut self.rig, focus_distance, 0x1800);
        *pos = set_dist_and_angle(
            *pos,
            f32::from(self.rig.lakitu_dist),
            self.rig.lakitu_pitch.wrapping_add(0x1000),
            yaw,
            trig,
        );
        yaw
    }

    fn mode_boss_fight_camera(&mut self, f: &mut Frame<'_, '_>) {
        let (mut focus, mut pos) = (self.rig.camera.focus, self.rig.camera.pos);
        self.rig.camera.next_yaw = self.update_boss_fight_camera(f, &mut focus, &mut pos);
        self.rig.camera.focus = focus;
        self.rig.camera.pos = pos;
    }

    // ---- C-Up ----

    /// update_c_up: first person, looking where Mario's head points.
    fn update_c_up(&mut self, f: &Frame<'_, '_>, focus: &mut [f32; 3], pos: &mut [f32; 3]) -> i16 {
        let pitch = self.rig.c_up_pitch;
        let face = f.mario_cam.face_angle[1];
        let yaw = face
            .wrapping_add(self.rig.mode_offset_yaw)
            .wrapping_add(degrees(180));
        (*focus, *pos) = focus_on_mario(
            f.mario_cam.pos,
            125.0,
            125.0,
            250.0,
            pitch,
            yaw,
            self.rig.lakitu_pitch,
            f.trig,
        );
        face
    }

    fn move_mario_head_c_up(&mut self, f: &mut Frame<'_, '_>) {
        let r = &mut self.rig;
        r.c_up_pitch = r
            .c_up_pitch
            .wrapping_add((f.controller.stick_y * 10.0) as i32 as i16);
        r.mode_offset_yaw = r
            .mode_offset_yaw
            .wrapping_sub((f.controller.stick_x * 10.0) as i32 as i16);
        if r.c_up_pitch > 0x38E3 {
            r.c_up_pitch = 0x38E3;
        }
        if r.c_up_pitch < -0x2000 {
            r.c_up_pitch = -0x2000;
        }
        if r.mode_offset_yaw > 0x5555 {
            r.mode_offset_yaw = 0x5555;
        }
        if r.mode_offset_yaw < -0x5555 {
            r.mode_offset_yaw = -0x5555;
        }
        f.mario_cam.head_rotation[0] = (i32::from(r.c_up_pitch) * 3 / 4) as i16;
        f.mario_cam.head_rotation[1] = (i32::from(r.mode_offset_yaw) * 3 / 4) as i16;
    }

    /// move_into_c_up: interpolate the transition's polar coordinates.
    fn move_into_c_up(&mut self, f: &mut Frame<'_, '_>) {
        let start = self.mode_info.start;
        let end = self.mode_info.end;
        let frame = self.mode_info.frame;
        let max = self.mode_info.max;
        assert!(
            max != 0,
            "move_into_c_up divides by a zero transition length"
        );
        let dist = end.dist - start.dist;
        let pitch = end.pitch.wrapping_sub(start.pitch);
        let yaw = end.yaw.wrapping_sub(start.yaw);
        let dist = start.dist + dist * f32::from(frame) / f32::from(max);
        let lerp = |a: i16, d: i16| {
            (i32::from(a) + i32::from(d) * i32::from(frame) / i32::from(max)) as i16
        };
        let pitch = lerp(start.pitch, pitch);
        let yaw = lerp(start.yaw, yaw);
        let m = f.mario_cam.pos;
        let mut focus: [f32; 3] = std::array::from_fn(|i| {
            start.focus[i] + (end.focus[i] - start.focus[i]) * f32::from(frame) / f32::from(max)
        });
        for i in 0..3 {
            focus[i] += m[i];
        }
        self.rig.camera.focus = focus;
        self.rig.camera.pos = set_dist_and_angle(focus, dist, pitch, yaw, f.trig);
        f.mario_cam.head_rotation[0] = 0;
        f.mario_cam.head_rotation[1] = 0;
        self.mode_info.frame = self.mode_info.frame.wrapping_add(1);
        if self.mode_info.frame == self.mode_info.max {
            self.rig.movement &= !c::CAM_MOVING_INTO_MODE;
        }
    }

    /// exit_c_up: zoom back out, searching for a direction without walls when
    /// returning to a close mode.
    fn exit_c_up(&mut self, f: &mut Frame<'_, '_>) {
        let movement = self.rig.movement;
        if movement & c::CAM_MOVE_C_UP_MODE == 0 || movement & c::CAM_MOVE_STARTED_EXITING_C_UP != 0
        {
            return;
        }
        let trig = f.trig;
        let world = f.world;
        let m = f.mario_cam.pos;
        let mut check_foc = self.rig.camera.focus;
        check_foc[0] = m[0];
        check_foc[2] = m[2];
        let (_, cur_pitch, cur_yaw) = get_dist_angle(check_foc, self.rig.camera.pos, trig);
        let cur_dist = 80.0f32;
        let mut check_yaw: i16 = 0;
        let pass_vanish_walls = f.mario.pass_vanish_walls;
        let last_mode = self.rig.last_mode;
        if last_mode == c::CAMERA_MODE_SPIRAL_STAIRS
            || last_mode == c::CAMERA_MODE_CLOSE
            || last_mode == c::CAMERA_MODE_FREE_ROAM
        {
            let mut searching = 1;
            let wall = |pos: &mut [f32; 3], flags: &CollisionFlags| -> i32 {
                let mut data = WallCollisionData::new(*pos, 20.0, 50.0);
                let n = world.find_wall_collisions(&mut data, *flags, pass_vanish_walls);
                *pos = [data.x, data.y, data.z];
                n
            };
            let mut sector = 0;
            while sector < 16 && searching == 1 {
                let mut cur_pos = set_dist_and_angle(
                    check_foc,
                    cur_dist,
                    0,
                    cur_yaw.wrapping_add(check_yaw),
                    trig,
                );
                if wall(&mut cur_pos, f.flags) == 0 {
                    let mut d = cur_dist;
                    while d < self.zoom_dist {
                        cur_pos = set_dist_and_angle(
                            check_foc,
                            d,
                            0,
                            cur_yaw.wrapping_add(check_yaw),
                            trig,
                        );
                        let (ceil, surface) =
                            world.find_ceil(cur_pos[0], cur_pos[1] - 150.0, cur_pos[2], *f.flags);
                        if surface.is_some() && ceil + -10.0 < cur_pos[1] {
                            break;
                        }
                        let (floor, surface) =
                            world.find_floor(cur_pos[0], cur_pos[1] + 150.0, cur_pos[2], f.flags);
                        if surface.is_some() && floor + 10.0 > cur_pos[1] {
                            break;
                        }
                        if wall(&mut cur_pos, f.flags) == 1 {
                            break;
                        }
                        d += 20.0;
                    }
                    if d >= self.zoom_dist {
                        searching = 0;
                    }
                }
                if searching == 1 {
                    check_yaw = check_yaw.wrapping_neg();
                    if check_yaw < 0 {
                        check_yaw = check_yaw.wrapping_sub(0x1000);
                    } else {
                        check_yaw = check_yaw.wrapping_add(0x1000);
                    }
                }
                sector += 1;
            }
            if searching == 0 {
                self.store_c_up.pos = set_dist_and_angle(
                    check_foc,
                    self.zoom_dist,
                    0,
                    cur_yaw.wrapping_add(check_yaw),
                    trig,
                );
                self.store_c_up.focus = check_foc;
                for (i, m) in m.iter().enumerate() {
                    self.store_c_up.pos[i] -= m;
                    self.store_c_up.focus[i] -= m;
                }
            }
            self.rig.movement |= c::CAM_MOVE_STARTED_EXITING_C_UP;
            self.rig.transition_next_state(15);
        } else {
            self.rig.movement &= !(c::CAM_MOVE_STARTED_EXITING_C_UP | c::CAM_MOVE_C_UP_MODE);
            self.rig.camera.pos = set_dist_and_angle(
                check_foc,
                cur_dist,
                cur_pitch,
                cur_yaw.wrapping_add(check_yaw),
                trig,
            );
        }
        Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_OUT);
    }

    /// mode_c_up_camera.
    fn mode_c_up_camera(&mut self, f: &mut Frame<'_, '_>) {
        if self.sound_flags & c::CAM_SOUND_C_UP_PLAYED == 0 {
            Self::sound(f.events, c::SOUND_MENU_CAMERA_ZOOM_IN);
            self.sound_flags |= c::CAM_SOUND_C_UP_PLAYED;
        }
        if self.rig.movement & c::CAM_MOVING_INTO_MODE != 0 {
            self.rig.movement |= c::CAM_MOVE_C_UP_MODE;
            self.move_into_c_up(f);
            return;
        }
        if self.rig.movement & c::CAM_MOVE_STARTED_EXITING_C_UP == 0 {
            self.move_mario_head_c_up(f);
            let (mut focus, mut pos) = (self.rig.camera.focus, self.rig.camera.pos);
            self.update_c_up(f, &mut focus, &mut pos);
            self.rig.camera.focus = focus;
            self.rig.camera.pos = pos;
        } else if self.rig.status & c::CAM_FLAG_TRANSITION_OUT_OF_C_UP != 0 {
            let m = f.mario_cam.pos;
            self.rig.camera.pos = std::array::from_fn(|i| self.store_c_up.pos[i] + m[i]);
            self.rig.camera.focus = std::array::from_fn(|i| self.store_c_up.focus[i] + m[i]);
            camera_approach_s16_symmetric_bool(&mut f.mario_cam.head_rotation[0], 0, 1024);
            camera_approach_s16_symmetric_bool(&mut f.mario_cam.head_rotation[1], 0, 1024);
        } else {
            self.rig.movement &= !(c::CAM_MOVE_STARTED_EXITING_C_UP | c::CAM_MOVE_C_UP_MODE);
        }
        self.rig.pan_distance = 0.0;
        if f.controller.button_pressed
            & (A_BUTTON | B_BUTTON | D_CBUTTONS | L_CBUTTONS | R_CBUTTONS)
            != 0
        {
            self.exit_c_up(f);
        }
    }

    // ---- Cutscene detection ----

    /// get_cutscene_from_mario_status: which cutscene Mario's state starts.
    /// It also resets the door status every tick.
    fn get_cutscene_from_mario_status(&mut self, mario_cam: &PlayerCameraState) -> u8 {
        let mut cutscene = self.rig.camera.cutscene;
        if cutscene == 0 {
            cutscene = self.object_cutscene;
            self.object_cutscene = 0;
            let event = mario_cam.camera_event;
            assert!(
                event != c::CAM_EVENT_DOOR,
                "door cutscenes need the door object, which is not simulated"
            );
            if event == c::CAM_EVENT_DOOR_WARP {
                cutscene = c::CUTSCENE_DOOR_WARP;
            }
            if event == c::CAM_EVENT_CANNON {
                cutscene = c::CUTSCENE_ENTER_CANNON;
            }
            let floor_type = self.geometry.curr.floor_type;
            if (0xD3..0xFD).contains(&floor_type) {
                cutscene = c::CUTSCENE_ENTER_PAINTING;
            }
            let from_bowser = [
                u32::from(c::LEVEL_BOWSER_1 as u16),
                u32::from(c::LEVEL_BOWSER_2 as u16),
                u32::from(c::LEVEL_BOWSER_3 as u16),
            ]
            .contains(&self.prev_level);
            cutscene = match mario_cam.action {
                c::ACT_DEATH_EXIT => c::CUTSCENE_DEATH_EXIT,
                c::ACT_EXIT_AIRBORNE => c::CUTSCENE_EXIT_PAINTING_SUCC,
                c::ACT_SPECIAL_EXIT_AIRBORNE if from_bowser => c::CUTSCENE_EXIT_BOWSER_SUCC,
                c::ACT_SPECIAL_EXIT_AIRBORNE => c::CUTSCENE_EXIT_SPECIAL_SUCC,
                c::ACT_SPECIAL_DEATH_EXIT if from_bowser => c::CUTSCENE_EXIT_BOWSER_DEATH,
                c::ACT_SPECIAL_DEATH_EXIT => c::CUTSCENE_NONPAINTING_DEATH,
                c::ACT_ENTERING_STAR_DOOR if self.door_status == c::DOOR_DEFAULT => {
                    c::CUTSCENE_SLIDING_DOORS_OPEN
                }
                c::ACT_ENTERING_STAR_DOOR => c::CUTSCENE_DOOR_PULL_MODE,
                c::ACT_UNLOCKING_KEY_DOOR => c::CUTSCENE_UNLOCK_KEY_DOOR,
                c::ACT_WATER_DEATH => c::CUTSCENE_WATER_DEATH,
                c::ACT_DEATH_ON_BACK => c::CUTSCENE_DEATH_ON_BACK,
                c::ACT_DEATH_ON_STOMACH => c::CUTSCENE_DEATH_ON_STOMACH,
                c::ACT_STANDING_DEATH | c::ACT_ELECTROCUTION => c::CUTSCENE_STANDING_DEATH,
                c::ACT_SUFFOCATION => c::CUTSCENE_SUFFOCATION_DEATH,
                c::ACT_QUICKSAND_DEATH => c::CUTSCENE_QUICKSAND_DEATH,
                c::ACT_STAR_DANCE_EXIT | c::ACT_STAR_DANCE_WATER => {
                    panic!("star dance cutscenes need the star object, which is not simulated")
                }
                c::ACT_STAR_DANCE_NO_EXIT => c::CUTSCENE_DANCE_DEFAULT,
                _ => cutscene,
            };
            cutscene = match event {
                c::CAM_EVENT_START_INTRO => c::CUTSCENE_INTRO_PEACH,
                c::CAM_EVENT_START_GRAND_STAR => c::CUTSCENE_GRAND_STAR,
                c::CAM_EVENT_START_ENDING => c::CUTSCENE_ENDING,
                c::CAM_EVENT_START_END_WAVING => c::CUTSCENE_END_WAVING,
                c::CAM_EVENT_START_CREDITS => c::CUTSCENE_CREDITS,
                _ => cutscene,
            };
        }
        self.door_status = c::DOOR_DEFAULT;
        cutscene
    }

    /// start_cutscene. The cutscene variables it clears are not modelled
    /// (no cutscene runs); starting one is reported as unsupported.
    fn start_cutscene(&mut self, cutscene: u8) {
        if self.rig.camera.cutscene != cutscene {
            self.rig.camera.cutscene = cutscene;
        }
    }

    // ---- Render pass ----

    /// The render pass's camera callbacks, once per tick after the update:
    /// geo_camera_fov on the area's perspective node, then geo_camera_main's
    /// update_graph_node_camera. The FOV function's sleeping flag and the FOV
    /// shake are simulation state (the pan reads the flag), so this is part of
    /// the tick. `mario_action` is gMarioStates[0]'s action and `mario_pos`
    /// sMarioCamState's position.
    pub fn render(&mut self, mario_action: u32, mario_pos: [f32; 3], trig: &TrigTables) {
        // geo_camera_fov.
        let fov = &mut self.fov.fov;
        match self.fov.func {
            c::CAM_FOV_SET_45 => *fov = 45.0,
            c::CAM_FOV_SET_29 => *fov = 29.0,
            c::CAM_FOV_ZOOM_30 => {
                let inc = (30.0 - *fov) / 60.0;
                camera_approach_f32_symmetric_bool(fov, 30.0, inc);
            }
            c::CAM_FOV_DEFAULT => {
                self.rig.status &= !c::CAM_FLAG_SLEEPING;
                if mario_action == c::ACT_SLEEPING || mario_action == c::ACT_START_SLEEPING {
                    let inc = (30.0 - *fov) / 30.0;
                    camera_approach_f32_symmetric_bool(fov, 30.0, inc);
                    self.rig.status |= c::CAM_FLAG_SLEEPING;
                } else {
                    let inc = (45.0 - *fov) / 30.0;
                    camera_approach_f32_symmetric_bool(fov, 45.0, inc);
                    self.fov.unused_is_sleeping = 0;
                }
                if self.rig.camera.cutscene == c::CUTSCENE_0F_UNUSED {
                    *fov = 45.0;
                }
            }
            c::CAM_FOV_BBH => {
                let target = if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_FIXED
                    && self.rig.camera.cutscene == 0
                {
                    60.0
                } else {
                    45.0
                };
                *fov = crate::simulation::math::approach_f32(*fov, target, 2.0, 2.0);
            }
            c::CAM_FOV_APP_45 => {
                *fov = crate::simulation::math::approach_f32(*fov, 45.0, 2.0, 2.0);
            }
            c::CAM_FOV_SET_30 => *fov = 30.0,
            c::CAM_FOV_APP_20 => {
                camera_approach_f32_symmetric_bool(fov, 20.0, 0.3);
            }
            c::CAM_FOV_APP_80 => {
                camera_approach_f32_symmetric_bool(fov, 80.0, 3.5);
            }
            c::CAM_FOV_APP_30 => {
                camera_approach_f32_symmetric_bool(fov, 30.0, 1.0);
            }
            c::CAM_FOV_APP_60 => {
                camera_approach_f32_symmetric_bool(fov, 60.0, 1.0);
            }
            // No default case.
            _ => {}
        }
        self.graph.fov = self.fov.fov;
        // shake_camera_fov.
        let shake = &mut self.rig.fov_shake;
        if shake.amplitude != 0.0 {
            self.fov.offset = trig.coss(i32::from(self.fov.shake_phase)) * shake.amplitude / 256.0;
            self.fov.shake_phase = self.fov.shake_phase.wrapping_add(shake.speed);
            camera_approach_f32_symmetric_bool(&mut shake.amplitude, 0.0, f32::from(shake.decay));
            self.graph.fov += self.fov.offset;
        } else {
            self.fov.shake_phase = 0;
        }
        // update_graph_node_camera.
        self.graph.roll_screen = self.rig.lakitu.roll;
        self.graph.pos = self.rig.lakitu.pos;
        self.graph.focus = self.rig.lakitu.focus;
        self.zoom_out_if_paused_and_outside(mario_pos, trig);
    }

    /// zoom_out_if_paused_and_outside: after two paused ticks in an outdoor
    /// area, the drawn camera pulls back over the area center.
    fn zoom_out_if_paused_and_outside(&mut self, mario: [f32; 3], trig: &TrigTables) {
        let area = self.radial.area;
        let mut mask_index = area / 32;
        let mut area_bit = 1i32 << (((area & 0x10) / 4) + (((area & 0xF) - 1) & 3));
        if mask_index >= i32::from(c::LEVEL_MAX) / 2 {
            mask_index = 0;
            area_bit = 0;
        }
        if self.rig.movement & c::CAM_MOVE_PAUSE_SCREEN != 0 {
            if self.frames_paused >= 2 {
                let mask = i32::from(ZOOM_OUT_AREA_MASKS[mask_index as usize]);
                if mask & area_bit != 0 {
                    let center = self.radial.center;
                    self.graph.focus =
                        [center[0], (mario[1] + self.area_center_y) / 2.0, center[1]];
                    let (_, _, yaw) = get_dist_angle(self.graph.focus, mario, trig);
                    self.graph.pos = set_dist_and_angle(mario, 6000.0, 0x1000, yaw, trig);
                    if self.level.level_num != c::LEVEL_THI {
                        find_in_bounds_yaw_wdw_bob_thi(
                            area,
                            &mut self.graph.pos,
                            self.graph.focus,
                            0,
                            trig,
                        );
                    }
                }
            } else {
                self.frames_paused += 1;
            }
        } else {
            self.frames_paused = 0;
        }
    }
}

/// is_within_100_units_of_mario.
fn is_within_100_units_of_mario(mario: [f32; 3], pos: [f32; 3]) -> i32 {
    i32::from(calc_abs_dist(mario, pos) < 100.0)
}

/// offset_rotated: rotate `to` by the pitch then the yaw of `rotation`
/// (flipping Z, so -Z is forward) and add it to `from`.
pub fn offset_rotated(
    from: [f32; 3],
    to: [f32; 3],
    rotation: [i16; 3],
    trig: &TrigTables,
) -> [f32; 3] {
    let (pitch, yaw) = (i32::from(rotation[0]), i32::from(rotation[1]));
    let rotated = [
        to[0],
        to[2] * trig.sins(pitch) + to[1] * trig.coss(pitch),
        -(to[2] * trig.coss(pitch) - to[1] * trig.sins(pitch)),
    ];
    [
        from[0] + rotated[2] * trig.sins(yaw) + rotated[0] * trig.coss(yaw),
        from[1] + rotated[1],
        from[2] + rotated[2] * trig.coss(yaw) - rotated[0] * trig.sins(yaw),
    ]
}
