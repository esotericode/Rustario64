//! Camera cutscenes translated from pinned CC0 camera.c: play_cutscene's
//! shot timing, cutscene_event, the object cutscene requests
//! (start_object_cutscene, cutscene_object_with_dialog), store_info_star /
//! retrieve_info_star and the dialog cutscene (sCutsceneDialog: the dialog
//! shot, its flag shot and the end). Every other cutscene, including the race
//! dialog that shares the dialog's shots, is still reported as
//! [`system::Unsupported::Cutscene`] and does not run.
//!
//! The dialog cutscene stops time (gTimeStopState), opens the dialog box with
//! sCutsceneDialogID and copies the box's response for the object that asked;
//! those globals live in the game world and reach the camera through
//! [`Frame`].
use super::{
    lakitu::get_dist_angle,
    system::{CameraSystem, Frame},
    *,
};
use crate::simulation::{mario::Event, object::ObjectId};

/// struct CutsceneVariable.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CutsceneVariable {
    pub unused1: i32,
    pub point: [f32; 3],
    pub unused_point: [f32; 3],
    pub angle: [i16; 3],
    pub unused2: i16,
}

/// What the camera reads from its cutscene focus object (gCutsceneFocus).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FocusView {
    pub pos: [f32; 3],
    pub hitbox_height: f32,
}

/// The camera.c globals objects use to request cutscenes: the camera copies
/// them into the world before the object update and takes them back after.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectCutsceneState {
    /// gCamera->cutscene (objects only read it).
    pub camera_cutscene: u8,
    /// sObjectCutscene.
    pub object_cutscene: u8,
    /// gRecentCutscene.
    pub recent_cutscene: u8,
    /// gCutsceneFocus.
    pub focus: Option<ObjectId>,
    /// gObjCutsceneDone.
    pub obj_cutscene_done: i32,
    /// sCutsceneDialogID.
    pub dialog_id: i16,
    /// sCutsceneDialogResponse.
    pub dialog_response: u8,
}

impl ObjectCutsceneState {
    /// start_object_cutscene.
    pub fn start_object_cutscene(&mut self, cutscene: u8, focus: ObjectId) {
        self.object_cutscene = cutscene;
        self.recent_cutscene = 0;
        self.focus = Some(focus);
        self.obj_cutscene_done = 0;
    }

    /// cutscene_object_with_dialog: starts the cutscene once and returns
    /// the dialog's response after it has played (0 meanwhile).
    pub fn cutscene_object_with_dialog(
        &mut self,
        cutscene: u8,
        focus: ObjectId,
        dialog_id: i16,
    ) -> i16 {
        let mut response = c::DIALOG_RESPONSE_NONE as i16;
        if self.camera_cutscene == 0 && self.object_cutscene == 0 {
            if self.recent_cutscene != cutscene {
                self.start_object_cutscene(cutscene, focus);
                self.dialog_id = if dialog_id != c::DIALOG_NONE {
                    dialog_id
                } else {
                    c::DIALOG_001
                };
            } else {
                response = i16::from(self.dialog_response);
            }
            self.recent_cutscene = 0;
        }
        response
    }
}

/// One shot of a cutscene: its function and duration (struct Cutscene).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shot {
    Dialog,
    DialogSetFlag,
    DialogEnd,
}

/// sCutsceneDialog.
const CUTSCENE_DIALOG_SHOTS: [(Shot, i16); 3] = [
    (Shot::Dialog, c::CUTSCENE_LOOP),
    (Shot::DialogSetFlag, 12),
    (Shot::DialogEnd, 0),
];

impl CameraSystem {
    /// The object-request globals, as objects see them.
    pub fn object_cutscene_state(&self) -> ObjectCutsceneState {
        ObjectCutsceneState {
            camera_cutscene: self.rig.camera.cutscene,
            object_cutscene: self.object_cutscene,
            recent_cutscene: self.recent_cutscene,
            focus: self.cutscene_focus,
            obj_cutscene_done: self.obj_cutscene_done,
            dialog_id: self.cutscene_dialog_id,
            dialog_response: self.cutscene_dialog_response,
        }
    }

    /// Takes back what objects changed during their update.
    pub fn take_object_cutscene_state(&mut self, s: &ObjectCutsceneState) {
        assert_eq!(
            s.camera_cutscene, self.rig.camera.cutscene,
            "objects do not write gCamera->cutscene"
        );
        self.object_cutscene = s.object_cutscene;
        self.recent_cutscene = s.recent_cutscene;
        self.cutscene_focus = s.focus;
        self.obj_cutscene_done = s.obj_cutscene_done;
        self.cutscene_dialog_id = s.dialog_id;
        self.cutscene_dialog_response = s.dialog_response;
    }

    /// Whether play_cutscene runs this cutscene here.
    pub fn cutscene_is_ported(cutscene: u8) -> bool {
        cutscene == c::CUTSCENE_DIALOG
    }

    /// clear_cutscene_vars.
    pub(super) fn clear_cutscene_vars(&mut self) {
        self.cutscene_vars = [CutsceneVariable::default(); 10];
    }

    /// play_cutscene for the ported cutscenes; the caller reports others.
    pub(super) fn play_cutscene(&mut self, f: &mut Frame<'_, '_>) {
        let old_cutscene = self.rig.camera.cutscene;
        self.rig.status &= !c::CAM_FLAG_SMOOTH_MOVEMENT;
        self.rig.movement &= !c::CAM_MOVING_INTO_MODE;
        let shots = match self.rig.camera.cutscene {
            c::CUTSCENE_DIALOG => &CUTSCENE_DIALOG_SHOTS,
            other => unreachable!("cutscene {other:#x} is not ported"),
        };
        let (shot, duration) = *shots.get(self.cutscene_shot as usize).unwrap_or_else(|| {
            panic!(
                "cutscene shot {} is outside the cutscene's table",
                self.cutscene_shot
            )
        });
        match shot {
            Shot::Dialog => self.cutscene_dialog(f),
            Shot::DialogSetFlag => self.rig.status |= c::CAM_FLAG_UNUSED_CUTSCENE_ACTIVE,
            Shot::DialogEnd => self.cutscene_dialog_end(f),
        }
        if duration != 0 && self.cutscene_timer & c::CUTSCENE_STOP == 0 {
            // The original checks 0x3FFF rather than CUTSCENE_LOOP.
            if self.cutscene_timer < 0x3FFF {
                self.cutscene_timer += 1;
            }
            if self.cutscene_timer == duration {
                self.cutscene_shot += 1;
                self.cutscene_timer = 0;
            }
        } else {
            f.mario_cam.camera_event = 0;
            self.cutscene_shot = 0;
            self.cutscene_timer = 0;
        }
        self.rig.area_yaw_change = 0;
        if self.rig.camera.cutscene == 0 && old_cutscene != 0 {
            self.recent_cutscene = old_cutscene;
        }
    }

    /// cutscene_event's window: start <= gCutsceneTimer <= end (-1: the
    /// rest of the shot).
    fn cutscene_event_due(&self, start: i16, end: i16) -> bool {
        start <= self.cutscene_timer && (end == -1 || end >= self.cutscene_timer)
    }

    /// store_info_star.
    fn store_info_star(&mut self, mario_pos: [f32; 3]) {
        self.rig.pan_distance = 0.0;
        self.store_cutscene.pos = self.rig.camera.pos;
        self.store_cutscene.focus = [mario_pos[0], self.rig.camera.focus[1], mario_pos[2]];
    }

    /// retrieve_info_star.
    fn retrieve_info_star(&mut self) {
        self.rig.camera.pos = self.store_cutscene.pos;
        self.rig.camera.focus = self.store_cutscene.focus;
    }

    /// cutscene_dialog.
    fn cutscene_dialog(&mut self, f: &mut Frame<'_, '_>) {
        if self.cutscene_event_due(0, 0) {
            self.cutscene_dialog_start(f);
        }
        if self.cutscene_event_due(0, -1) {
            self.cutscene_dialog_move_mario_shoulder(f);
        }
        if self.cutscene_event_due(10, 10) {
            // cutscene_dialog_create_dialog_box; the race dialog asks for a
            // response.
            f.dialog.create(self.cutscene_dialog_id);
            self.cutscene_vars[8].angle[0] = c::DIALOG_RESPONSE_NOT_DEFINED as i16;
        }
        self.rig.status |= c::CAM_FLAG_SMOOTH_MOVEMENT;
        if f.dialog.response != c::DIALOG_RESPONSE_NONE {
            self.cutscene_dialog_response = f.dialog.response as u8;
        }
        if f.dialog.id() == c::DIALOG_NONE && self.cutscene_vars[8].angle[0] != 0 {
            self.cutscene_dialog_response = c::DIALOG_RESPONSE_NOT_DEFINED as u8;
            self.cutscene_timer = c::CUTSCENE_LOOP;
            self.retrieve_info_star();
            self.rig.transition_next_state(15);
            self.rig.status |= c::CAM_FLAG_UNUSED_CUTSCENE_ACTIVE;
            // cutscene_unsoften_music.
            f.events.push(Event::SeqUnlowerVolume {
                player: c::SEQ_PLAYER_LEVEL,
                duration: 60,
            });
        }
    }

    /// cutscene_dialog_start: cvar8 holds Mario's head, cvar9 the focus
    /// object's top and the yaw the camera swings to.
    fn cutscene_dialog_start(&mut self, f: &mut Frame<'_, '_>) {
        // cutscene_soften_music.
        f.events.push(Event::SeqLowerVolume {
            player: c::SEQ_PLAYER_LEVEL,
            duration: 60,
            percentage: 40,
        });
        *f.time_stop_state |= c::TIME_STOP_ENABLED | c::TIME_STOP_DIALOG;
        if i16::from(self.rig.camera.mode) == c::CAMERA_MODE_BOSS_FIGHT {
            self.store_cutscene.focus = self.rig.camera.focus;
            self.store_cutscene.pos = self.rig.camera.pos;
        } else {
            self.store_info_star(f.mario_cam.pos);
        }
        let mario = f.mario_cam.pos;
        let vars = &mut self.cutscene_vars;
        vars[8].angle[0] = 0;
        vars[8].point = [mario[0], mario[1] + 125.0, mario[2]];
        let focus = f
            .focus
            .expect("the dialog cutscene reads gCutsceneFocus, which is NULL");
        vars[9].point = focus.pos;
        vars[9].point[1] += focus.hitbox_height + 200.0;
        vars[9].angle[1] = calculate_yaw(vars[8].point, vars[9].point, f.trig);
        let yaw = calculate_yaw(mario, self.rig.lakitu.cur_pos, f.trig);
        if yaw.wrapping_sub(vars[9].angle[1]) as u16 & 0x8000 != 0 {
            vars[9].angle[1] = vars[9].angle[1].wrapping_sub(0x6000);
        } else {
            vars[9].angle[1] = vars[9].angle[1].wrapping_add(0x6000);
        }
    }

    /// cutscene_dialog_move_mario_shoulder.
    fn cutscene_dialog_move_mario_shoulder(&mut self, f: &mut Frame<'_, '_>) {
        let trig = f.trig;
        let cvar8 = self.cutscene_vars[8];
        let cvar9 = self.cutscene_vars[9];
        let c = &mut self.rig.camera;
        let mut focus = scale_along_line(cvar9.point, f.mario_cam.pos, 0.7);
        // The original's first vec3f_set_dist_and_angle writes `pos`, which
        // the copy below replaces.
        let _ = get_dist_angle(c.pos, focus, trig);
        focus[1] += (cvar9.point[1] - focus[1]) * 0.1;
        approach_vec3f_asymptotic(&mut c.focus, focus, [0.2, 0.2, 0.2]);
        let mut pos = c.pos;
        pos[1] = cvar8.point[1];
        let (mut dist, pitch, mut yaw) = get_dist_angle(cvar8.point, pos, trig);
        approach_s16_asymptotic_bool(&mut yaw, cvar9.angle[1], 0x10);
        approach_f32_asymptotic_bool(&mut dist, 180.0, 0.05);
        pos = set_dist_and_angle(cvar8.point, dist, pitch, yaw, trig);
        pos[1] = cvar8.point[1]
            + trig.sins(i32::from(calculate_pitch(cvar9.point, cvar8.point, trig))) * 100.0;
        approach_f32_asymptotic_bool(&mut c.pos[1], pos[1], 0.05);
        c.pos[0] = pos[0];
        c.pos[2] = pos[2];
    }

    /// cutscene_dialog_end.
    fn cutscene_dialog_end(&mut self, f: &mut Frame<'_, '_>) {
        self.rig.status |= c::CAM_FLAG_UNUSED_CUTSCENE_ACTIVE;
        self.rig.camera.cutscene = 0;
        *f.time_stop_state &= !(c::TIME_STOP_ENABLED | c::TIME_STOP_DIALOG);
    }
}
