//! Tauri IPC 命令模块。
//!
//! 所有 Tauri Command 直接操作 `AppState` 中的 IronClaw 组件，
//! 无需 HTTP 转发。每个子模块对应一个功能域。

pub mod approval;
pub mod chat;
pub mod dlp;
pub mod extensions;
pub mod jobs;
pub mod logs;
pub mod memory;
pub mod models;
pub(crate) mod persistence;
pub mod plan_mode;
pub mod routines;
pub mod skills;
pub mod threads;
pub mod file_ops;
pub mod workspace;

#[cfg(test)]
#[path = "chat_tests.rs"]
mod chat_tests;

#[cfg(test)]
#[path = "threads_tests.rs"]
mod threads_tests;

#[cfg(test)]
#[path = "memory_tests.rs"]
mod memory_tests;

#[cfg(test)]
#[path = "skills_tests.rs"]
mod skills_tests;

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod extensions_tests;

#[cfg(test)]
#[path = "approval_tests.rs"]
mod approval_tests;

#[cfg(test)]
#[path = "dlp_tests.rs"]
mod dlp_tests;

#[cfg(test)]
#[path = "routines_tests.rs"]
mod routines_tests;

#[cfg(test)]
#[path = "logs_tests.rs"]
mod logs_tests;

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod jobs_tests;

#[cfg(test)]
#[path = "models_tests.rs"]
mod models_tests;

// 统一 re-export 所有 Tauri Command
pub use approval::*;
pub use chat::*;
pub use dlp::*;
pub use extensions::*;
pub use jobs::*;
pub use logs::*;
pub use memory::*;
pub use models::*;
pub use plan_mode::*;
pub use routines::*;
pub use skills::*;
pub use threads::*;
pub use file_ops::*;
pub use workspace::*;
