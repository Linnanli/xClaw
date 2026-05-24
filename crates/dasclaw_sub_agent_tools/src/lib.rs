//! F4.6.4 — Sub-agent builtin tool extracted from
//! `desktop-client/ironclaw/src/tools/builtin/` per
//! [ADR-156 §6.3](../../../docs/plans/architecture-refactor/adr-156-f46-builtin-tools-landing-decision.md).
//!
//! Verbatim port (ADR-129 §1.3): the file body is a byte-for-byte copy of
//! the desktop source, with only the `use` line rewritten from
//! `crate::tools::tool::{...}` to `dasclaw_runtime::Tool` +
//! `dasclaw_tool::{...}` to follow the workspace dependency direction
//! (ADR-154 §3.1 amendment).

mod sub_agent;

pub use sub_agent::{SubAgentRole, SubAgentTool};
