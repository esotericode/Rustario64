//! ROM-owned content, deterministic tick scheduling, and read-only presentation.
//! Original collision, math, and Mario's complete update (Mario's object only)
//! are ported and compared tick by tick with the natively compiled decomp;
//! `play` drives that tick from held controls.
pub mod content;
pub mod diagnostics;
pub mod import;
pub mod play;
pub mod presentation;
pub mod simulation;
pub mod trace;
