//! The dialog box's state, translated from pinned CC0 src/game/ingame_menu.c
//! for the US version: create_dialog_box and its variants, get_dialog_id,
//! and the dialog half of render_menus_and_dialogs (render_dialog_entries'
//! state machine, the page breaks handle_dialog_text_and_pages computes from
//! the entry's text, the yes/no choice's handle_menu_scrolling and
//! handle_special_dialog_text). The original advances this state while it
//! draws, once per frame after the area's render pass; the gameplay effects
//! (the response, the open/close timing that the camera's dialog cutscene and
//! objects wait on) are simulation state. Drawing is presentation, which
//! reads [`DialogBox`].
//!
//! Sounds and music calls are recorded events. The Peach letter (DIALOG_020)
//! has its own printer and the pause/course-complete menus are not ported;
//! reaching them panics.
use crate::{
    content::dialog::{DIALOG_CHAR_NEWLINE, DIALOG_CHAR_TERMINATOR, DialogTable},
    simulation::{
        controller::{A_BUTTON, B_BUTTON, Controller},
        mario::{Event, constants::*},
    },
};

/// The dialog box's menu states (ingame_menu.c's file-local enum).
pub const MENU_STATE_DIALOG_OPENING: i8 = 0;
pub const MENU_STATE_DIALOG_OPEN: i8 = 1;
pub const MENU_STATE_DIALOG_SCROLLING: i8 = 2;
pub const MENU_STATE_DIALOG_CLOSING: i8 = 3;
pub const MENU_STATE_DEFAULT: i8 = MENU_STATE_DIALOG_OPENING;
/// DIALOG_TYPE_ROTATE (NPCs, level messages) and DIALOG_TYPE_ZOOM (signs).
pub const DIALOG_TYPE_ROTATE: i8 = 0;
pub const DIALOG_TYPE_ZOOM: i8 = 1;
pub const DIALOG_BOX_ANGLE_DEFAULT: f32 = 90.0;
pub const DIALOG_BOX_SCALE_DEFAULT: f32 = 19.0;
/// DIAG_VAL1 (US): the scroll distance per line.
const DIAG_VAL1: i16 = 16;
const DIALOG_PAGE_STATE_NONE: i8 = 0;
const DIALOG_PAGE_STATE_SCROLL: i8 = 1;
const DIALOG_PAGE_STATE_END: i8 = 2;
const MENU_SCROLL_HORIZONTAL: i8 = 2;

/// ingame_menu.c's dialog globals, with their initial values.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogBox {
    /// gMenuState.
    pub menu_state: i8,
    /// gDialogBoxAngle.
    pub angle: f32,
    /// gDialogBoxScale.
    pub scale: f32,
    /// gDialogScrollOffsetY.
    pub scroll_offset_y: i16,
    /// gDialogBoxType.
    pub box_type: i8,
    /// gDialogID.
    pub id: i16,
    /// gNextDialogPageStartStrIndex (-1 after the last page).
    pub next_page_start: i16,
    /// gDialogPageStartStrIndex.
    pub page_start: i16,
    /// gMenuLineNum: the highlighted choice, and the response.
    pub menu_line_num: i8,
    /// gDialogWithChoice.
    pub with_choice: i8,
    /// gMenuHoldKeyIndex and gMenuHoldKeyTimer.
    pub hold_key_index: u8,
    pub hold_key_timer: u8,
    /// gDialogResponse.
    pub response: i32,
    /// gLastDialogLineNum.
    pub last_line_num: i8,
    /// gDialogVariable.
    pub variable: i32,
    /// gMenuTextColorTransTimer (presentation colors).
    pub text_color_timer: u16,
    /// The lower bound of the lines drawn this frame (render_dialog_entries'
    /// lowerBound; presentation).
    pub lower_bound: i8,
}

impl Default for DialogBox {
    fn default() -> Self {
        Self {
            menu_state: MENU_STATE_DEFAULT,
            angle: DIALOG_BOX_ANGLE_DEFAULT,
            scale: DIALOG_BOX_SCALE_DEFAULT,
            scroll_offset_y: 0,
            box_type: DIALOG_TYPE_ROTATE,
            id: DIALOG_NONE,
            next_page_start: 0,
            page_start: 0,
            menu_line_num: 1,
            with_choice: 0,
            hold_key_index: 0,
            hold_key_timer: 0,
            response: DIALOG_RESPONSE_NONE,
            last_line_num: 0,
            variable: 0,
            text_color_timer: 0,
            lower_bound: 0,
        }
    }
}

impl DialogBox {
    /// get_dialog_id.
    pub fn id(&self) -> i16 {
        self.id
    }

    /// create_dialog_box.
    pub fn create(&mut self, dialog: i16) {
        if self.id == DIALOG_NONE {
            self.id = dialog;
            self.box_type = DIALOG_TYPE_ROTATE;
        }
    }

    /// create_dialog_box_with_var.
    pub fn create_with_var(&mut self, dialog: i16, var: i32) {
        if self.id == DIALOG_NONE {
            self.id = dialog;
            self.variable = var;
            self.box_type = DIALOG_TYPE_ROTATE;
        }
    }

    /// create_dialog_inverted_box.
    pub fn create_inverted(&mut self, dialog: i16) {
        if self.id == DIALOG_NONE {
            self.id = dialog;
            self.box_type = DIALOG_TYPE_ZOOM;
        }
    }

    /// create_dialog_box_with_response.
    pub fn create_with_response(&mut self, dialog: i16) {
        if self.id == DIALOG_NONE {
            self.id = dialog;
            self.box_type = DIALOG_TYPE_ROTATE;
            self.with_choice = 1;
        }
    }

    /// render_menus_and_dialogs with no menu mode (gMenuMode is
    /// MENU_MODE_NONE while no pause or course-complete screen runs).
    /// `controller` is gPlayer3Controller, which read_controller_inputs
    /// copies from player 1.
    pub fn render(
        &mut self,
        table: &DialogTable,
        controller: &Controller,
        events: &mut Vec<Event>,
    ) {
        if self.id == DIALOG_NONE {
            return;
        }
        assert!(
            self.id != DIALOG_020,
            "the Peach letter (DIALOG_020) is printed by print_peach_letter_message, which is not ported"
        );
        self.render_dialog_entries(table, controller, events);
        self.text_color_timer = self.text_color_timer.wrapping_add(0x1000);
    }

    /// render_dialog_entries (US).
    fn render_dialog_entries(
        &mut self,
        table: &DialogTable,
        controller: &Controller,
        events: &mut Vec<Event>,
    ) {
        // dialogTable[gDialogID]; the table's NULL terminator ends dialogs.
        if usize::try_from(self.id).ok() == Some(table.entries.len()) {
            self.id = DIALOG_NONE;
            return;
        }
        let dialog = table.get(self.id).unwrap_or_else(|| {
            panic!(
                "dialog {} indexes outside seg2_dialog_table ({} entries)",
                self.id,
                table.entries.len()
            )
        });
        let lines_per_box = dialog.lines_per_box;
        let lower_bound: i8 = match self.menu_state {
            MENU_STATE_DIALOG_OPENING => {
                if self.angle == DIALOG_BOX_ANGLE_DEFAULT {
                    events.push(Event::DialogSound(self.id as u8));
                    events.push(Event::Sound(SOUND_MENU_MESSAGE_APPEAR));
                }
                if self.box_type == DIALOG_TYPE_ROTATE {
                    self.angle -= 7.5;
                    self.scale -= 1.5;
                } else {
                    self.angle -= 10.0;
                    self.scale -= 2.0;
                }
                if self.angle == 0.0 {
                    self.menu_state = MENU_STATE_DIALOG_OPEN;
                    self.menu_line_num = 1;
                }
                1
            }
            MENU_STATE_DIALOG_OPEN => {
                self.angle = 0.0;
                if controller.button_pressed & (A_BUTTON | B_BUTTON) != 0 {
                    if self.next_page_start == -1 {
                        self.handle_special_dialog_text(events);
                        self.menu_state = MENU_STATE_DIALOG_CLOSING;
                    } else {
                        self.menu_state = MENU_STATE_DIALOG_SCROLLING;
                        events.push(Event::Sound(SOUND_MENU_MESSAGE_NEXT_PAGE));
                    }
                }
                1
            }
            MENU_STATE_DIALOG_SCROLLING => {
                self.scroll_offset_y = self
                    .scroll_offset_y
                    .wrapping_add(i16::from(lines_per_box) * 2);
                if self.scroll_offset_y >= i16::from(lines_per_box) * DIAG_VAL1 {
                    self.page_start = self.next_page_start;
                    self.menu_state = MENU_STATE_DIALOG_OPEN;
                    self.scroll_offset_y = 0;
                }
                (self.scroll_offset_y / DIAG_VAL1 + 1) as i8
            }
            MENU_STATE_DIALOG_CLOSING => {
                if self.angle == 20.0 {
                    // level_set_transition(0, NULL): no transition is
                    // pending while a dialog is open on the paths ported.
                    events.push(Event::Sound(SOUND_MENU_MESSAGE_DISAPPEAR));
                    assert!(
                        self.box_type != DIALOG_TYPE_ZOOM,
                        "closing a sign's box triggers the read-message cutscene's next state, which is not ported"
                    );
                    self.response = i32::from(self.menu_line_num);
                }
                self.angle += 10.0;
                self.scale += 2.0;
                if self.angle == DIALOG_BOX_ANGLE_DEFAULT {
                    self.menu_state = MENU_STATE_DEFAULT;
                    self.id = DIALOG_NONE;
                    self.page_start = 0;
                    self.with_choice = 0;
                    self.next_page_start = 0;
                    self.response = DIALOG_RESPONSE_NONE;
                }
                1
            }
            state => panic!("gMenuState {state} is not a dialog state"),
        };
        self.lower_bound = lower_bound;
        // render_dialog_box_type draws; handle_dialog_text_and_pages walks the
        // page and records where the next one starts.
        let text = &dialog.text;
        self.handle_dialog_text_and_pages(text, lines_per_box);
        if self.next_page_start == -1 && self.with_choice == 1 {
            // render_dialog_triangle_choice.
            if self.menu_state == MENU_STATE_DIALOG_OPEN {
                self.handle_menu_scrolling(MENU_SCROLL_HORIZONTAL, 1, 2, controller, events);
            }
        }
    }

    /// handle_dialog_text_and_pages (US): only line breaks and the
    /// terminator change the page state; every other character only draws.
    fn handle_dialog_text_and_pages(&mut self, text: &[u8], lines_per_box: i8) {
        let total_lines = if self.menu_state == MENU_STATE_DIALOG_SCROLLING {
            lines_per_box.wrapping_mul(2).wrapping_add(1)
        } else {
            lines_per_box.wrapping_add(1)
        };
        let mut str_index = self.page_start;
        let mut line_num: i8 = 1;
        let mut page_state = DIALOG_PAGE_STATE_NONE;
        while page_state == DIALOG_PAGE_STATE_NONE {
            let ch = *text
                .get(str_index as usize)
                .unwrap_or_else(|| panic!("dialog text read past its terminator at {str_index}"));
            match ch {
                DIALOG_CHAR_TERMINATOR => page_state = DIALOG_PAGE_STATE_END,
                DIALOG_CHAR_NEWLINE => {
                    line_num = line_num.wrapping_add(1);
                    // handle_dialog_scroll_page_state.
                    if line_num == total_lines {
                        page_state = DIALOG_PAGE_STATE_SCROLL;
                    }
                }
                _ => {}
            }
            str_index = str_index.wrapping_add(1);
        }
        if self.menu_state == MENU_STATE_DIALOG_OPEN {
            self.next_page_start = if page_state == DIALOG_PAGE_STATE_END {
                -1
            } else {
                str_index
            };
        }
        self.last_line_num = line_num;
    }

    /// handle_menu_scrolling for gMenuLineNum.
    fn handle_menu_scrolling(
        &mut self,
        direction: i8,
        min: i8,
        max: i8,
        controller: &Controller,
        events: &mut Vec<Event>,
    ) {
        let mut index: u8 = 0;
        if direction == MENU_SCROLL_HORIZONTAL {
            if controller.raw_stick[0] > 60 {
                index += 2;
            }
            if controller.raw_stick[0] < -60 {
                index += 1;
            }
        } else {
            if controller.raw_stick[1] > 60 {
                index += 1;
            }
            if controller.raw_stick[1] < -60 {
                index += 2;
            }
        }
        // The original's "== max" branch leaves the index unchanged.
        if (index ^ self.hold_key_index) & index == 2 && self.menu_line_num != max {
            events.push(Event::Sound(SOUND_MENU_CHANGE_SELECT));
            self.menu_line_num += 1;
        }
        if (index ^ self.hold_key_index) & index == 1 && self.menu_line_num != min {
            events.push(Event::Sound(SOUND_MENU_CHANGE_SELECT));
            self.menu_line_num -= 1;
        }
        if self.hold_key_timer == 10 {
            self.hold_key_timer = 8;
            self.hold_key_index = 0;
        } else {
            self.hold_key_timer = self.hold_key_timer.wrapping_add(1);
            self.hold_key_index = index;
        }
        if index & 3 == 0 {
            self.hold_key_timer = 0;
        }
    }

    /// handle_special_dialog_text: boss music, race fanfares and star sounds.
    fn handle_special_dialog_text(&self, events: &mut Vec<Event>) {
        let id = self.id;
        let yes = i32::from(self.menu_line_num) == DIALOG_RESPONSE_YES;
        if [DIALOG_017, DIALOG_114, DIALOG_128, DIALOG_117, DIALOG_150].contains(&id) {
            events.push(Event::SeqUnlowerVolume {
                player: SEQ_PLAYER_LEVEL,
                duration: 60,
            });
            events.push(Event::PlayMusic {
                player: SEQ_PLAYER_LEVEL,
                seq_args: SEQUENCE_ARGS_4_SEQ_EVENT_BOSS,
                fade: 0,
            });
            return;
        }
        if [DIALOG_005, DIALOG_009, DIALOG_055, DIALOG_164].contains(&id) && yes {
            events.push(Event::RaceFanfare);
            return;
        }
        if [DIALOG_010, DIALOG_011, DIALOG_012, DIALOG_013, DIALOG_014].contains(&id) && yes {
            events.push(Event::Sound(SOUND_MENU_STAR_SOUND));
            return;
        }
        if [DIALOG_017, DIALOG_115, DIALOG_116, DIALOG_118, DIALOG_152].contains(&id) {
            events.push(Event::SeqFadeOut {
                player: SEQ_PLAYER_LEVEL,
                duration: 1,
            });
        }
    }
}

/// SEQUENCE_ARGS(4, SEQ_EVENT_BOSS).
pub const SEQUENCE_ARGS_4_SEQ_EVENT_BOSS: u16 = (4 << 8) | SEQ_EVENT_BOSS as u16;
