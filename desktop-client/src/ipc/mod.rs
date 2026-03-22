//! Tauri IPC 命令模块。
//!
//! 所有 Tauri Command 直接操作 `AppState` 中的 IronClaw 组件，
//! 无需 HTTP 转发。每个子模块对应一个功能域。

pub mod approval;
pub mod chat;
pub mod dlp;
pub mod extensions;
pub mod memory;
pub mod skills;
pub mod threads;

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

// 统一 re-export 所有 Tauri Command
pub use approval::*;
pub use chat::*;
pub use dlp::*;
pub use extensions::*;
pub use memory::*;
pub use skills::*;
pub use threads::*;
