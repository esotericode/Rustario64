pub mod animation;
pub mod bob;
pub mod collision;
pub mod engine;
pub mod geo;
pub mod gfx;
pub mod level;
pub mod macros;
pub mod mario;
pub mod mio0;
pub mod model;
pub mod reader;
pub mod rom;
pub mod segments;
pub mod shadow;
mod special;
pub mod texture;
pub mod version;

use std::fmt;

/// Revision/data matching only; SHA-1 is not an authenticity guarantee.
pub fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError {
    pub context: &'static str,
    pub offset: usize,
    pub detail: String,
}

impl ImportError {
    pub fn new(context: &'static str, offset: usize, detail: impl Into<String>) -> Self {
        Self {
            context,
            offset,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at 0x{:X}: {}",
            self.context, self.offset, self.detail
        )
    }
}

impl std::error::Error for ImportError {}
pub type Result<T> = std::result::Result<T, ImportError>;
