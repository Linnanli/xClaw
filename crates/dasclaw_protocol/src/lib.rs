//! Verbatim file-level slice of `codex-cli-main/codex-rs/protocol/`.
//!
//! **Current scope** (Step C1.2, ADR-136 §3 layered plan):
//! Layer 1 zero-`crate::`-deps leaves (14 modules) + `parse_command` from C1.1.
//!
//! **Planned expansion** (ADR-136 §3 Step C1.3 ~ C1.5):
//! Layer 2 — `config_types` + `openai_models` (mutual cycle, ~1.5 KLOC);
//! Layer 3 — `protocol`, `permissions`, `models`, `approvals`, `network_policy`,
//! `items`, `request_permissions` (~12 KLOC, hub crate);
//! Layer 4 — `error` + `error_tests` (~1.2 KLOC).
//!
//! Verbatim red line per [ADR-129 §1.3](../../docs/plans/architecture-refactor/adr-129-sandbox-windows-windows-crate-adoption.md);
//! file-level slicing rationale per [ADR-136 §3.1](../../docs/plans/architecture-refactor/adr-136-protocol-expansion-plan.md).

pub mod account;
mod agent_path;
pub mod auth;
mod thread_id;
mod tool_name;
pub use agent_path::AgentPath;
pub use thread_id::ThreadId;
pub use tool_name::ToolName;
pub mod dynamic_tools;
pub mod exec_output;
pub mod mcp;
pub mod memory_citation;
pub mod message_history;
pub mod num_format;
pub mod parse_command;
pub mod plan_tool;
pub mod request_user_input;
pub mod user_input;
