//! Lifecycle hooks compatibility shim — thin reexport of [`dasclaw_hooks`].
//!
//! ## ADR-113 — single hook front-door
//!
//! The hook engine, declarative bundle parser, and trait seams have all
//! moved to the `dasclaw_hooks` crate (P0-3, see ADR-113). This module is
//! retained for backwards compatibility so existing call sites in ironclaw
//! continue to compile against `crate::hooks::*` paths. PR #48 will remove
//! this shim once every caller switches to `dasclaw_hooks::*` directly.
//!
//! New code SHOULD import from `dasclaw_hooks` directly.

pub mod bootstrap;

pub use bootstrap::{HookBootstrapSummary, bootstrap_hooks};

// Reexport everything `dasclaw_hooks` publishes so legacy
// `crate::hooks::HookRegistry` etc. paths continue to resolve.
pub use dasclaw_hooks::*;
