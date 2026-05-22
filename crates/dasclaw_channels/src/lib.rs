//! Channel core abstractions (fork ironclaw 0.24 channels module promotion).
//!
//! F4.5（第一刀）：`channel.rs` / `manager.rs` / `relay/` 与 `ChannelError`
//! 已从 `desktop-client/ironclaw` 按 verbatim 方式搬迁过来
//! （ADR-129 §1.3 / ADR-152 §3 F4.5）。
//!
//! 仍留在 `desktop-client/ironclaw/src/channels/` 的模块（`http` / `repl` /
//! `signal` / `webhook_server` / `web` / `wasm`）因重度耦合桌面端 `bootstrap` /
//! `db` / `extensions` / `pairing` / `safety` / `tools::wasm`，
//! 留待后续 F4.5 子切片解耦后再搬。

pub mod channel;
pub mod error;
pub mod manager;
pub mod relay;

pub use channel::{
    AttachmentKind, Channel, ChannelSecretUpdater, IncomingAttachment, IncomingMessage,
    MessageStream, OutgoingResponse, StatusUpdate, ToolDecision, routing_target_from_metadata,
    tool_enriched_metadata,
};
pub use error::ChannelError;
pub use manager::ChannelManager;
