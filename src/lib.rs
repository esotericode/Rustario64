//! ROM-owned content, deterministic tick scheduling, and read-only presentation.
//! Original collision, math, and Mario's complete update (Mario's object only)
//! are ported and compared tick by tick with the natively compiled decomp;
//! `play` drives that tick from held controls.

/// Shared project version, inherited by all workspace packages.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Application name and version for window titles and desktop UI.
pub const APP_TITLE: &str = concat!("Rustario64 v", env!("CARGO_PKG_VERSION"));

pub mod content;
pub mod diagnostics;
pub mod import;
pub mod play;
pub mod presentation;
pub mod simulation;
pub mod trace;
