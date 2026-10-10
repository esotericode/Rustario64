//! What the dialog box shows, read from the simulated box after a frame:
//! the lines of the current page (and of the page scrolling in), how far the
//! box has opened, and the next-page and yes/no markers. The original draws
//! the box rotating and scaling with gDialogBoxAngle/gDialogBoxScale and its
//! text with the ROM font; this view supplies the same text and timing to a
//! development overlay. Drawing never writes back to the simulation.
use crate::{
    content::dialog::{DIALOG_CHAR_NEWLINE, DIALOG_CHAR_TERMINATOR, DialogTable, us_char},
    simulation::dialog::{
        DIALOG_BOX_ANGLE_DEFAULT, DialogBox, MENU_STATE_DIALOG_OPEN, MENU_STATE_DIALOG_SCROLLING,
    },
};

/// DIALOG_CHAR_STAR_COUNT: the dialog variable, as render_star_count_dialog_text prints it.
const DIALOG_CHAR_STAR_COUNT: u8 = 0xE0;

#[derive(Debug, Clone, PartialEq)]
pub struct DialogView {
    pub dialog_id: i16,
    /// The page's lines (lines_per_box of them at most); while scrolling,
    /// the incoming page's lines follow.
    pub lines: Vec<String>,
    pub lines_per_box: usize,
    /// 0 closed .. 1 open, from the box's angle.
    pub open: f32,
    /// Whether the text is scrolling to the next page.
    pub scrolling: bool,
    /// The fraction of a page scrolled (0..1).
    pub scroll: f32,
    /// A further page waits for A or B.
    pub more: bool,
    /// The highlighted answer of a yes/no box (1 yes, 2 no).
    pub choice: Option<i8>,
}

/// The lines from `start`, up to `count` of them.
pub fn page_lines(text: &[u8], start: usize, count: usize, star_count: i32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for &byte in text.iter().skip(start) {
        match byte {
            DIALOG_CHAR_TERMINATOR => break,
            DIALOG_CHAR_NEWLINE => {
                if lines.len() == count {
                    break;
                }
                lines.push(String::new());
            }
            DIALOG_CHAR_STAR_COUNT => lines.last_mut().unwrap().push_str(&star_count.to_string()),
            other => lines
                .last_mut()
                .unwrap()
                .push_str(us_char(other).unwrap_or("?")),
        }
    }
    lines
}

/// The box's view, or None while no dialog is open.
pub fn dialog_view(d: &DialogBox, table: &DialogTable) -> Option<DialogView> {
    let entry = table.get(d.id())?;
    let lines_per_box = usize::try_from(entry.lines_per_box).ok()?.max(1);
    let start = usize::try_from(d.page_start).unwrap_or(0);
    let scrolling = d.menu_state == MENU_STATE_DIALOG_SCROLLING;
    let mut lines = page_lines(&entry.text, start, lines_per_box, d.variable);
    if scrolling && d.next_page_start > 0 {
        let next = page_lines(
            &entry.text,
            d.next_page_start as usize,
            lines_per_box,
            d.variable,
        );
        lines.resize(lines_per_box, String::new());
        lines.extend(next);
    }
    let scroll = if scrolling {
        f32::from(d.scroll_offset_y) / (f32::from(entry.lines_per_box) * 16.0)
    } else {
        0.0
    };
    Some(DialogView {
        dialog_id: d.id(),
        lines,
        lines_per_box,
        open: (1.0 - d.angle / DIALOG_BOX_ANGLE_DEFAULT).clamp(0.0, 1.0),
        scrolling,
        scroll: scroll.clamp(0.0, 1.0),
        more: d.next_page_start != -1 && d.menu_state == MENU_STATE_DIALOG_OPEN,
        choice: (d.with_choice != 0 && d.next_page_start == -1).then_some(d.menu_line_num),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::dialog::DialogEntry;

    #[test]
    fn pages_follow_the_simulated_page_start() {
        // "AB\nC\nD the\nE[%]\nF", three lines per box.
        let text = vec![
            0x0A, 0x0B, 0xFE, 0x0C, 0xFE, 0x0D, 0x9E, 0xD1, 0xFE, 0x0E, 0xE0, 0xFE, 0x0F, 0xFF,
        ];
        assert_eq!(page_lines(&text, 0, 3, 0), ["AB", "C", "D the"]);
        assert_eq!(page_lines(&text, 9, 3, 12), ["E12", "F"]);
        let table = DialogTable {
            entries: vec![DialogEntry {
                unused: 1,
                lines_per_box: 3,
                left_offset: 95,
                width: 200,
                text,
            }],
        };
        let mut d = DialogBox::default();
        assert!(dialog_view(&d, &table).is_none());
        d.create(0);
        let opening = dialog_view(&d, &table).unwrap();
        assert_eq!(opening.open, 0.0);
        d.angle = 0.0;
        d.menu_state = MENU_STATE_DIALOG_OPEN;
        d.next_page_start = 9;
        let open = dialog_view(&d, &table).unwrap();
        assert!(open.more && open.open == 1.0);
        d.menu_state = MENU_STATE_DIALOG_SCROLLING;
        d.scroll_offset_y = 24;
        let scrolling = dialog_view(&d, &table).unwrap();
        assert_eq!(scrolling.lines.len(), 5);
        assert_eq!(scrolling.scroll, 0.5);
    }
}
