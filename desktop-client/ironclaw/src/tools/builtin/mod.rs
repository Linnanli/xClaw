//! Built-in tools that come with the agent.
//!
//! Per ADR-156 §6.4, after the F4.6.1 ~ F4.6.8 sink waves landed this
//! module is now a thin re-export entry point: each builtin tool family
//! that was extracted lives in `crates/dasclaw_*_tools` and is wildcard
//! re-exported here so every historical
//! `crate::tools::builtin::<Tool>` call site keeps compiling unchanged.
//! Only T3/T4 modules that intentionally stay desktop-resident
//! (see ADR-156 §6.3.1 and §8) keep `pub mod` declarations below.

// Sunk crates (F4.6.1 ~ F4.6.8 + ADR-152 F3.3 / dasclaw_lsp).
pub use dasclaw_fs_tools::*;
pub use dasclaw_git_tools::*;
pub use dasclaw_image_tools::*;
pub use dasclaw_lsp::*;
pub use dasclaw_misc_tools::*;
pub use dasclaw_net_tools::*;
pub use dasclaw_shell_tools::*;
pub use dasclaw_sub_agent_tools::*;

// T3/T4 desktop-resident tools (留守).
//
// - `memory` — ADR-156 §6.3.1 explicitly cancels the sink (desktop-only,
//   `runtime` workspace dep) so it stays a local module.
// - `tool_info` — `ToolInfoTool` reflects on `Weak<ToolRegistry>` which
//   lives in desktop (F4.6.8 note in §6.4).
// - `extension_tools` / `skill_tools` — T3 host-bound extension /
//   skill management.
// - `job` / `routine` / `message` — T4 desktop-backend-only orchestrator
//   tools (ADR-155).
pub mod extension_tools;
mod job;
pub mod memory;
mod message;
pub mod routine;
pub mod skill_tools;
mod tool_info;

pub use extension_tools::{
    ExtensionInfoTool, ToolActivateTool, ToolAuthTool, ToolInstallTool, ToolListTool,
    ToolRemoveTool, ToolSearchTool, ToolUpgradeTool,
};
pub use job::{
    CancelJobTool, CreateJobTool, JobDispatcherSlot, JobEventsTool, JobPromptTool, JobStatusTool,
    ListJobsTool, PromptQueue,
};
pub use memory::{MemoryReadTool, MemorySearchTool, MemoryTreeTool, MemoryWriteTool};
pub use message::MessageTool;
pub use routine::{
    EventEmitTool, RoutineCreateTool, RoutineDeleteTool, RoutineFireTool, RoutineHistoryTool,
    RoutineListTool, RoutineUpdateTool,
};
pub use skill_tools::{SkillInstallTool, SkillListTool, SkillRemoveTool, SkillSearchTool};
pub use tool_info::ToolInfoTool;
