//! The game's dialog entries (pinned CC0 src/game/ingame_menu.h's
//! `struct DialogEntry` and the seg2_dialog_table it lists), imported from
//! the owner's ROM. Text stays in the game's encoding; the dialog box's page
//! timing reads its newline and terminator bytes, presentation decodes the
//! rest.

/// DIALOG_CHAR_NEWLINE.
pub const DIALOG_CHAR_NEWLINE: u8 = 0xFE;
/// DIALOG_CHAR_TERMINATOR.
pub const DIALOG_CHAR_TERMINATOR: u8 = 0xFF;

/// One DialogEntry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogEntry {
    pub unused: u32,
    pub lines_per_box: i8,
    pub left_offset: i16,
    pub width: i16,
    /// The string through its terminator.
    pub text: Vec<u8>,
}

/// A table with no entries (worlds without imported text).
pub static NO_DIALOGS: DialogTable = DialogTable {
    entries: Vec::new(),
};

/// seg2_dialog_table: entries by dialog ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogTable {
    pub entries: Vec<DialogEntry>,
}

impl DialogTable {
    /// The entry for a dialog ID; None for IDs past the table, whose
    /// original lookup reads the NULL terminator or beyond.
    pub fn get(&self, id: i16) -> Option<&DialogEntry> {
        usize::try_from(id).ok().and_then(|i| self.entries.get(i))
    }
}

/// The US dialog font's characters for presentation (pinned CC0
/// charmap.txt). Multi-character codes expand; unknown codes are None.
pub fn us_char(code: u8) -> Option<&'static str> {
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    const UPPER: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    const LOWER: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ];
    Some(match code {
        0x00..=0x09 => DIGITS[usize::from(code)],
        0x0A..=0x23 => UPPER[usize::from(code - 0x0A)],
        0x24..=0x3D => LOWER[usize::from(code - 0x24)],
        0x3E => "'",
        0x3F => ".",
        0x50 => "▲",
        0x51 => "▼",
        0x52 => "◀",
        0x53 => "▶",
        0x54 => "[A]",
        0x55 => "[B]",
        0x56 => "[C]",
        0x57 => "[Z]",
        0x58 => "[R]",
        0x6F => ",",
        0x9E => " ",
        0x9F => "-",
        0xD0 => "/",
        0xD1 => "the",
        0xD2 => "you",
        0xE0 => "[%]",
        0xE1 => "(",
        0xE2 => ")(",
        0xE3 => ")",
        0xE4 => "+",
        0xE5 => "&",
        0xE6 => ":",
        0xF2 => "!",
        0xF3 => "%",
        0xF4 => "?",
        0xF5 => "『",
        0xF6 => "』",
        0xF7 => "~",
        0xF9 => "$",
        0xFA => "★",
        0xFB => "×",
        0xFC => "・",
        0xFD => "☆",
        DIALOG_CHAR_NEWLINE => "\n",
        _ => return None,
    })
}

/// A string decoded for display up to its terminator; unknown codes show
/// as `?`.
pub fn decode_us(text: &[u8]) -> String {
    text.iter()
        .take_while(|&&b| b != DIALOG_CHAR_TERMINATOR)
        .map(|&b| us_char(b).unwrap_or("?"))
        .collect()
}
