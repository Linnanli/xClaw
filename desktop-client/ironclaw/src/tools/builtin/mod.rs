//! Built-in tools that come with the agent.

pub mod extension_tools;
mod file;
mod http;
mod job;
pub mod lsp;
pub mod memory;
mod message;
pub mod routine;
pub(crate) mod shell;
pub use shell::classify_command_risk;
pub mod skill_tools;
mod tool_info;
mod web_fetch;
mod web_search;

// F4.6.6-a/-b (ADR-156 §6.3): `path_utils` + `file_guard` + `code_edit` +
// `glob_search` + `grep_search` were extracted to `dasclaw_fs_tools`.
// Re-exported at the original module paths so existing in-tree consumers
// (`tools::builtin::path_utils::*` / `tools::builtin::file_guard::*` /
// `tools::builtin::code_edit::*` / `tools::builtin::glob_search::*` /
// `tools::builtin::grep_search::*`) and external `tests/parity_gate_*`
// imports keep working without changes. F4.6.6-c will migrate `file`
// once `workspace::paths` extraction is decided.
pub use dasclaw_fs_tools::{code_edit, file_guard, glob_search, grep_search, path_utils};

// F4.6.1 — echo/time/json/plan_mode/restart/session_fork were extracted to
// `crates/dasclaw_misc_tools` per ADR-156 §6.3. F4.6.1b added
// secrets_tools (SecretListTool / SecretDeleteTool) into the same crate.
// The thin re-export below keeps every existing
// `crate::tools::builtin::EchoTool` (etc.) call site compiling unchanged.
pub use dasclaw_misc_tools::{
    EchoTool, JsonTool, PlanModeTool, RestartTool, SecretDeleteTool, SecretListTool,
    SessionForkTool, TimeTool,
};

// F4.6.5 — git tools were extracted to `crates/dasclaw_git_tools` per
// ADR-156 §6.3. Re-exported here so `crate::tools::builtin::GitStatusTool`
// (etc.) call sites keep compiling unchanged.
pub use dasclaw_git_tools::{
    GitBranchTool, GitCommitTool, GitDiffTool, GitLogTool, GitPushTool, GitStaleCheckTool,
    GitStatusTool,
};

// F4.6.4 — SubAgentTool / SubAgentRole were extracted to
// `crates/dasclaw_sub_agent_tools` per ADR-156 §6.3. Thin re-export keeps
// existing call sites compiling unchanged.
pub use dasclaw_sub_agent_tools::{SubAgentRole, SubAgentTool};

pub use code_edit::CodeEditTool;
pub use extension_tools::{
    ExtensionInfoTool, ToolActivateTool, ToolAuthTool, ToolInstallTool, ToolListTool,
    ToolRemoveTool, ToolSearchTool, ToolUpgradeTool,
};
pub use file::{ApplyPatchTool, ListDirTool, ReadFileTool, WriteFileTool};
pub use glob_search::GlobSearchTool;
pub use grep_search::GrepSearchTool;
pub use http::HttpTool;
pub use job::{
    CancelJobTool, CreateJobTool, JobEventsTool, JobPromptTool, JobStatusTool, ListJobsTool,
    PromptQueue, SchedulerSlot,
};
pub use lsp::LspQueryTool;
pub use memory::{MemoryReadTool, MemorySearchTool, MemoryTreeTool, MemoryWriteTool};
pub use message::MessageTool;
pub use routine::{
    EventEmitTool, RoutineCreateTool, RoutineDeleteTool, RoutineFireTool, RoutineHistoryTool,
    RoutineListTool, RoutineUpdateTool,
};
pub use shell::ShellTool;
pub use skill_tools::{SkillInstallTool, SkillListTool, SkillRemoveTool, SkillSearchTool};
pub use tool_info::ToolInfoTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
mod html_converter;
pub mod image_analyze;
pub mod image_edit;
pub mod image_gen;

pub use html_converter::convert_html_to_markdown;
pub use image_analyze::ImageAnalyzeTool;
pub use image_edit::ImageEditTool;
pub use image_gen::ImageGenerateTool;

/// Detect image media type from file extension via `mime_guess`.
/// Falls back to `image/jpeg` for unrecognized or non-image extensions.
pub(crate) fn media_type_from_path(path: &str) -> String {
    mime_guess::from_path(path)
        .first_raw()
        .filter(|m| m.starts_with("image/"))
        .unwrap_or("image/jpeg")
        .to_string()
}
