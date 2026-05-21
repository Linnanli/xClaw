//! Memory layer metadata (sensitivity, scope, writability).
//!
//! Re-export shim. The implementation lives in
//! [`dasclaw_workspace_cap::layer`] so that admin-backend and other
//! capability consumers can use it without depending on the ironclaw
//! desktop crate. See ADR-152 §3 F3.4 and issue #674.

pub use dasclaw_workspace_cap::layer::{LayerSensitivity, MemoryLayer};
