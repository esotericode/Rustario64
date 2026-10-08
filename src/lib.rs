//! ROM-owned content, deterministic tick scheduling, and read-only presentation.
//! Original collision, math, and Mario physics steps are ported and component-tested.
//! Mario actions and per-tick gameplay integration are not implemented yet.
pub mod content;
pub mod diagnostics;
pub mod import;
pub mod presentation;
pub mod simulation;
pub mod trace;
