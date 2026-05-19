//! Re-export shim for the migrated `dasclaw_core::SecretProvider` adapter.
//!
//! The `AgentSecrets` adapter now lives in
//! `dasclaw_runtime::secrets::agent_provider` (F3.2 phase 2 PR 4c, #641).
//! This shim only exists so existing ironclaw call sites (`hooks::*`,
//! `worker::*`, `agent::*`) keep resolving `crate::secrets::AgentSecrets`
//! and `crate::secrets::agent_provider::AgentSecrets` without an
//! ironclaw-wide import sweep.
//!
//! # Wiring status (Phase 3, retained for context)
//!
//! - ✅ Wired for interactive chat via `Agent::run_agentic_loop`
//!   (`hook_bundle_with_safety_and_secrets` with `message.user_id`).
//! - ✅ Wired for the local job worker via `Worker::execution_loop`
//!   (`hook_bundle_with_safety_and_secrets` with `JobContext.user_id`).
//! - ⚠️  Intentionally NOT wired for the container worker — secrets there
//!   are injected at container-provisioning time, not through the per-call
//!   hook bundle.

pub use dasclaw_runtime::secrets::AgentSecrets;
