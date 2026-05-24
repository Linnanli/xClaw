//! Built-in tools that come with the agent.

mod code_edit;
pub mod extension_tools;
mod file;
pub mod file_guard;
pub mod git;
mod glob_search;
mod grep_search;
mod http;
mod job;
pub mod lsp;
pub mod memory;
mod message;
pub mod path_utils;
pub mod routine;
pub mod secrets_tools;
pub(crate) mod shell;
pub use shell::classify_command_risk;
pub mod skill_tools;
pub mod sub_agent;
mod tool_info;
mod web_fetch;
mod web_search;

// F4.6.1 — echo/time/json/plan_mode/restart/session_fork were extracted to
// `crates/dasclaw_misc_tools` per ADR-156 §6.3. The thin re-export below
// keeps every existing `crate::tools::builtin::EchoTool` (etc.) call site
// compiling unchanged.
pub use dasclaw_misc_tools::{
    EchoTool, JsonTool, PlanModeTool, RestartTool, SessionForkTool, TimeTool,
};

pub use code_edit::CodeEditTool;
pub use extension_tools::{
    ExtensionInfoTool, ToolActivateTool, ToolAuthTool, ToolInstallTool, ToolListTool,
    ToolRemoveTool, ToolSearchTool, ToolUpgradeTool,
};
pub use file::{ApplyPatchTool, ListDirTool, ReadFileTool, WriteFileTool};
pub use git::{
    GitBranchTool, GitCommitTool, GitDiffTool, GitLogTool, GitPushTool, GitStaleCheckTool,
    GitStatusTool,
};
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
pub use secrets_tools::{SecretDeleteTool, SecretListTool};
pub use shell::ShellTool;
pub use skill_tools::{SkillInstallTool, SkillListTool, SkillRemoveTool, SkillSearchTool};
pub use sub_agent::{SubAgentRole, SubAgentTool};
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
