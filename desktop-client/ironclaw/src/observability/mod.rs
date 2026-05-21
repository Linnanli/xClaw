//! Observability subsystem (host-side shim).
//!
//! Production implementation lives in [`dasclaw_observability`] per
//! ADR-152 §3 F3.5 (issue #631). This module is a thin re-export so every
//! existing `crate::observability::*` path keeps resolving.

pub use dasclaw_observability::*;
