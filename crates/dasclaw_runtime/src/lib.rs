//! Application-layer runtime types shared across dasclaw hosts (ironclaw,
//! admin-backend, claw-code, ...).
//!
//! ## Two-layer `ToolError` model
//!
//! There are *two* `ToolError` types in the codebase and that is intentional:
//!
//! - [`dasclaw_tool::ToolError`] is the **tool implementation layer** error.
//!   It is what an individual tool's `execute()` returns. It carries no tool
//!   name — the tool does not need to repeat its own identity.
//! - [`ToolError`] (this crate) is the **application / dispatcher layer**
//!   error. The dispatcher knows *which* tool failed, so every variant here
//!   carries the failing tool's `name` (and any additional runtime context
//!   such as `retry_after` for rate limiting).
//!
//! The conversion from the implementation layer to the application layer is
//! intentionally **not** a blanket [`From`] impl: a free `From` could not
//! supply the tool name and would silently fall back to `"<unknown>"` — a
//! fail-open behaviour incompatible with the security stance of x-claw.
//! Instead, callers must attach the name explicitly at the boundary using
//! [`ToolError::from_tool_impl`].

pub mod error;

pub use error::ToolError;
