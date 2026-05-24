//! F4.6.1 — Miscellaneous builtin tools extracted from
//! `desktop-client/ironclaw/src/tools/builtin/` per
//! [ADR-156 §6.3](../../../docs/plans/architecture-refactor/adr-156-f46-builtin-tools-landing-decision.md).
//!
//! Verbatim port (ADR-129 §1.3): file bodies are byte-for-byte copies of the
//! desktop sources, with only the `use` line rewritten from
//! `crate::tools::tool::{...}` to `dasclaw_runtime::Tool` +
//! `dasclaw_tool::{...}` to follow the workspace dependency direction
//! (ADR-154 §3.1 amendment).
//!
//! Not in this PR (deferred to F4.6.1b):
//! - `secrets_tools.rs` — its `#[cfg(test)] mod tests` depends on
//!   `crate::testing::credentials::{TEST_OPENAI_API_KEY_SHORT,
//!   test_secrets_store}`, a desktop-internal fixture that requires a
//!   separate `pub(crate) testing` exposure decision.
//! - `tool_info.rs` — depends on `crate::tools::registry::ToolRegistry`
//!   which still lives in desktop and will be sunk in a later wave.

mod echo;
mod json;
mod plan_mode;
mod restart;
mod session_fork;
mod time;

pub use echo::EchoTool;
pub use json::JsonTool;
pub use plan_mode::PlanModeTool;
pub use restart::RestartTool;
pub use session_fork::SessionForkTool;
pub use time::TimeTool;
