//! Multi-channel input system.
//!
//! Channels receive messages from external sources (CLI, HTTP, etc.)
//! and convert them to a unified message format for the agent to process.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         ChannelManager                              │
//! │                                                                     │
//! │   ┌──────────────┐   ┌─────────────┐   ┌─────────────┐             │
//! │   │ ReplChannel  │   │ HttpChannel │   │ WasmChannel │   ...       │
//! │   └──────┬───────┘   └──────┬──────┘   └──────┬──────┘             │
//! │          │                 │                 │                      │
//! │          └─────────────────┴─────────────────┘                      │
//! │                            │                                        │
//! │                   select_all (futures)                              │
//! │                            │                                        │
//! │                            ▼                                        │
//! │                     MessageStream                                   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # WASM Channels
//!
//! WASM channels allow dynamic loading of channel implementations at runtime.
//! See the [`wasm`] module for details.
//!
//! # F4.5 split
//!
//! Core trait + manager + relay live in `dasclaw_channels` (ADR-152 §3 F4.5);
//! this module re-exports them for backward compatibility and hosts the
//! transport modules (`http` / `repl` / `signal` / `webhook_server` / `web` /
//! `wasm`) plus desktop-only helpers (`status::build_tool_completed`).

mod http;
mod repl;
mod signal;
pub mod status;
pub mod wasm;
pub mod web;
mod webhook_server;

pub use dasclaw_channels::{
    AttachmentKind, Channel, ChannelManager, ChannelSecretUpdater, IncomingAttachment,
    IncomingMessage, MessageStream, OutgoingResponse, StatusUpdate, ToolDecision, relay,
    routing_target_from_metadata, tool_enriched_metadata,
};
pub use http::{HttpChannel, HttpChannelState};
pub use repl::ReplChannel;
pub use signal::SignalChannel;
pub use web::GatewayChannel;
pub use webhook_server::{WebhookServer, WebhookServerConfig};
