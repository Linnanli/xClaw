//! Per-job context isolation and state management.
//!
//! Verbatim port from `desktop-client/ironclaw/src/context/` per ADR-152 §3
//! F3.6. Slice 1 of this port brings in [`memory`]; subsequent slices will
//! add `fallback`, `state`, and `manager`. See ADR-152 §3 F3.6 row and §11.7
//! for the boundary analysis.
//!
//! Modules are re-exported from this crate root via
//! `dasclaw_core::context::memory::*` so host code only needs a thin
//! `pub use dasclaw_core::context::memory::*;` shim to consume them.

pub mod memory;

pub use memory::{ActionRecord, ConversationMemory, Memory};
