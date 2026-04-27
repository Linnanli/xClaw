//! Task types for the scheduler — Phase 3 D-1：实现已搬到 [`x_claw_agent::task`]。
//!
//! `TaskHandler::run` 签名从 `Result<TaskOutput, crate::error::Error>` 放宽为
//! `Result<TaskOutput, Box<dyn std::error::Error + Send + Sync>>`，避免把 ironclaw
//! 应用层的 `Error` 类型拖进 agent runtime crate。当前 ironclaw 代码中 **无任何
//! `impl TaskHandler`**，签名放宽零破坏。

pub use x_claw_agent::task::*;
