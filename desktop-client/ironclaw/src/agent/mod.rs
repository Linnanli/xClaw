//! Core agent logic.
//!
//! The agent orchestrates:
//! - Message routing from channels
//! - Job scheduling and execution
//! - Tool invocation with safety
//! - Self-repair for stuck jobs
//! - Proactive heartbeat execution
//! - Routine-based scheduled and reactive jobs
//! - Turn-based session management with undo
//! - Context compaction for long conversations

mod agent_loop;
pub mod agentic_loop;
mod attachments;
mod commands;
pub mod compaction;
mod desktop_dispatcher;
mod desktop_responder;
mod dispatcher;
mod router;
pub mod session;
pub mod submission;
pub mod task;
mod thread_ops;
mod traits_impl;
pub use traits_impl::ReasoningCompleter;
pub mod undo;

// Phase 3 plan H'' (2026-04-23): The routines / guardrails subsystem
// (routine, routine_engine, scheduler, self_repair, cost_guard, heartbeat,
// job_monitor) now lives under `crate::routines::*`. The following
// re-exports preserve the historical `crate::agent::{...}` module paths so
// downstream call sites (channels/web, db, tools/builtin, cli, etc.) do not
// need to be rewritten.
pub use crate::routines::cost_guard;
pub use crate::routines::heartbeat;
pub use crate::routines::job_monitor;
pub use crate::routines::routine;
pub use crate::routines::routine_engine;
pub use crate::routines::self_repair;

pub use crate::worker::job_dispatcher::{JobDispatcher, JobDispatcherDeps};
pub(crate) use agent_loop::truncate_for_preview;
pub use agent_loop::{Agent, AgentDeps};
pub use compaction::{CompactionResult, ContextCompactor};
pub use dasclaw_core::context_monitor::{CompactionStrategy, ContextBreakdown, ContextMonitor};
pub(crate) use dispatcher::strip_suggestions;
pub use heartbeat::{
    HeartbeatConfig, HeartbeatResult, HeartbeatRunner, spawn_heartbeat, spawn_multi_user_heartbeat,
};
pub use router::{MessageIntent, Router};
pub use routine::{Routine, RoutineAction, RoutineRun, Trigger};
pub use routine_engine::{RoutineEngine, SandboxReadiness};
pub use self_repair::{BrokenTool, RepairResult, RepairTask, SelfRepair, StuckJob};
pub use session::{PendingApproval, PendingAuth, Session, Thread, ThreadState, Turn, TurnState};
// `SessionManager` now lives in the `dasclaw_core` runtime crate (Phase 3
// Step D-5, plan variant C'). Re-exported here for backward compatibility so
// existing `crate::agent::SessionManager` call sites keep compiling.
pub use dasclaw_core::SessionManager;
pub use submission::{Submission, SubmissionParser, SubmissionResult};
pub use task::{Task, TaskContext, TaskHandler, TaskOutput};
pub use undo::{Checkpoint, UndoManager};
