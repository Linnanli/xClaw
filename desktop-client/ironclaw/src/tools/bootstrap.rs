//! Tool registry bootstrap context — single-entry initialization API.
//!
//! Replaces the legacy 12-call register_*_tools pattern that was scattered
//! across `app.rs`, `main.rs`, `agent_loop.rs`, and `worker/container.rs`.
//!
//! See [`p02-bootstrap-tools-design.md`](../../../../../docs/plans/architecture-refactor/p02-bootstrap-tools-design.md)
//! for the full design rationale and ADR-112 §5 P0-2 reference.
//!
//! # Usage
//!
//! ```ignore
//! use ironclaw::tools::{BootstrapContext, BootstrapMode};
//!
//! // Test convenience (replaces standalone `register_builtin_tools()`):
//! let ctx = BootstrapContext::for_test();
//!
//! // Production orchestrator path:
//! let ctx = BootstrapContext {
//!     mode: BootstrapMode::Orchestrator { allow_local_tools: false },
//!     workspace: Some(workspace.clone()),
//!     extension_manager: Some(manager.clone()),
//!     ..Default::default()
//! };
//!
//! tool_registry.bootstrap_tools(&ctx).await?;
//! ```
//!
//! # Field semantics
//!
//! Each optional field gates one logical tool group. `bootstrap_tools` walks
//! the context fields in dependency order and registers only the groups whose
//! prerequisites are present. Missing prerequisites silently skip their group
//! (no panic, no error) — this lets early-startup callers register the
//! always-available subset (e.g. `mode = Test`) without wiring every
//! production-only resource.
//!
//! # Rollout note
//!
//! This struct landed in PR #2 of the P0-2 stacked series. PR #3 (this PR)
//! adds the `bootstrap_tools()` method and replaces the 9 forward-compat
//! marker traits with concrete types. PR #4 migrates the 12 legacy
//! `register_*_tools` call sites and deletes the public compat wrappers.

use std::sync::Arc;

use crate::agent::routine_engine::RoutineEngine;
use crate::channels::ChannelManager;
use crate::db::Database;
use crate::extensions::ExtensionManager;
use crate::secrets::SecretsStore;
use crate::skills::catalog::SkillCatalog;
use crate::skills::registry::SkillRegistry;
use crate::tools::builtin::memory::WorkspaceResolver;
use crate::workspace::Workspace;

/// Deployment mode that selects which tool groups are registered by default.
///
/// `Orchestrator` is the main process path. `Container` is the sandboxed
/// worker path that needs filesystem/shell access. `Test` is a minimal
/// initialization for unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    /// Main process. By default, only safe (non-filesystem, non-shell) tools
    /// register. Set `allow_local_tools = true` to include dev tools (file ops,
    /// shell) — equivalent to the legacy `register_dev_tools` call site.
    Orchestrator { allow_local_tools: bool },

    /// Sandboxed worker. Registers all dev tools (filesystem, shell, code).
    /// Equivalent to the legacy `register_container_tools` call site.
    Container,

    /// Unit-test mode. Registers only built-in safe tools. Equivalent to the
    /// legacy standalone `register_builtin_tools()` call.
    Test,
}

impl Default for BootstrapMode {
    fn default() -> Self {
        Self::Test
    }
}

/// Errors that can occur during `bootstrap_tools` execution.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BootstrapError {
    /// Catch-all variant for downstream registration failures. Kept as the
    /// sole variant for now: `bootstrap_tools` is intentionally infallible
    /// for the current set of registered tool groups (each `register_*_internal`
    /// dispatches to `register_sync`, which is `HashMap::insert`-style and
    /// cannot fail). The variant exists so future tool groups that need real
    /// fallible registration (e.g. WASM tool integrity checks at bootstrap
    /// time) have a place to slot in without changing the public signature.
    #[error("bootstrap error: {0}")]
    Other(String),
}

// ─── Forward declarations for fields ────────────────────────────────────
// These types are owned by other modules; bootstrap.rs references them via
// `Arc<dyn Trait>` or concrete types from the parent crate. Direct imports
// would create cycles during incremental compilation, so callers fill the
// fields with their own `Arc<...>` values at the call site.

/// Configuration bundle for `register_job_tools`.
#[derive(Debug, Clone, Default)]
pub struct JobToolsConfig {
    pub admin_base_url: Option<String>,
    pub agent_id: Option<String>,
}

/// Configuration bundle for `register_image_tools`.
#[derive(Debug, Clone, Default)]
pub struct ImageApiConfig {
    pub api_base: String,
    pub api_key: String,
    pub gen_model: String,
}

/// Configuration bundle for `register_vision_tools`.
#[derive(Debug, Clone, Default)]
pub struct VisionApiConfig {
    pub api_base: String,
    pub api_key: String,
    pub vision_model: String,
}

/// Single-entry bootstrap context for `ToolRegistry::bootstrap_tools`.
///
/// All fields except `mode` are optional. The bootstrap method registers
/// each tool group only if its required field(s) are `Some`. Use
/// `..Default::default()` to elide unused fields.
///
/// # Field-to-group mapping
///
/// | Field(s)                                        | Tool group registered           |
/// |-------------------------------------------------|---------------------------------|
/// | (always, gated by `mode`)                       | builtin / dev / container       |
/// | `secrets_store`                                 | secrets                         |
/// | `db_pool` (preferred) or `workspace`            | memory                          |
/// | `job_config`                                    | job                             |
/// | `extension_manager`                             | extension                       |
/// | `skill_registry` + `skill_catalog`              | skill                           |
/// | `routine_store` + `routine_engine`              | routine                         |
/// | `image_api`                                     | image                           |
/// | `vision_api`                                    | vision                          |
/// | `channels` + `extension_manager`                | message (async)                 |
#[derive(Default)]
#[non_exhaustive]
pub struct BootstrapContext {
    /// Deployment mode. Required.
    pub mode: BootstrapMode,

    /// Workspace handle (memory tools fallback when `db_pool` is absent).
    pub workspace: Option<Arc<Workspace>>,

    /// Workspace resolver used by memory tools (preferred over `workspace`).
    /// Despite the legacy field name, this is the resolver abstraction —
    /// any implementation of `WorkspaceResolver` (libsql-backed, in-memory,
    /// fixed-workspace) works.
    pub db_pool: Option<Arc<dyn WorkspaceResolver>>,

    /// Secrets store (secrets / authenticated http).
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,

    /// Extension manager (extension / message tools).
    pub extension_manager: Option<Arc<ExtensionManager>>,

    /// Channel manager (message tools).
    pub channels: Option<Arc<ChannelManager>>,

    /// Skill registry handle (skill tools).
    pub skill_registry: Option<Arc<std::sync::RwLock<SkillRegistry>>>,

    /// Skill catalog handle (skill tools).
    pub skill_catalog: Option<Arc<SkillCatalog>>,

    /// Routine store (routine tools, also used as the JobEvents backing store).
    pub routine_store: Option<Arc<dyn Database>>,

    /// Routine engine (routine tools).
    pub routine_engine: Option<Arc<RoutineEngine>>,

    /// Job tools configuration.
    pub job_config: Option<JobToolsConfig>,

    /// Image generation API configuration.
    pub image_api: Option<ImageApiConfig>,

    /// Vision API configuration.
    pub vision_api: Option<VisionApiConfig>,
}

impl BootstrapContext {
    /// Test-only convenience constructor.
    ///
    /// Equivalent to `BootstrapContext { mode: BootstrapMode::Test, ..Default::default() }`.
    /// Use in `#[cfg(test)]` and integration tests that only need the built-in
    /// tool subset (echo, time, json, http).
    pub fn for_test() -> Self {
        Self {
            mode: BootstrapMode::Test,
            ..Default::default()
        }
    }

    /// Returns `true` if the context's mode includes dev tools.
    ///
    /// Useful for callers that need to know whether shell / filesystem tools
    /// will be registered before invoking `bootstrap_tools`.
    pub fn includes_dev_tools(&self) -> bool {
        matches!(
            self.mode,
            BootstrapMode::Orchestrator {
                allow_local_tools: true
            } | BootstrapMode::Container
        )
    }
}

// ─── Removed in PR #3: 9 forward-compat marker traits ───────────────────
//
// PR #2 used empty marker traits (`WorkspaceHandle`, `DbPoolHandle`, …) as
// placeholders so the struct shape could land before concrete types were
// chosen. PR #3 replaces them with concrete `Arc<…>` field types (see
// `BootstrapContext` above), unblocking the actual `bootstrap_tools()`
// implementation.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_p02_pr2_a_default_context_is_test_mode() {
        // Default mode must be Test (safe minimum) — protects callers that
        // forget to set `mode` from accidentally registering dev tools.
        let ctx = BootstrapContext::default();
        assert_eq!(ctx.mode, BootstrapMode::Test);
        assert!(!ctx.includes_dev_tools());
    }

    #[test]
    fn req_p02_pr2_b_for_test_constructor_matches_default() {
        // for_test() must be semantically identical to Default::default()
        // for callers migrating from `register_builtin_tools()`.
        let a = BootstrapContext::for_test();
        let b = BootstrapContext::default();
        assert_eq!(a.mode, b.mode);
        assert_eq!(a.includes_dev_tools(), b.includes_dev_tools());
    }

    #[test]
    fn req_p02_pr2_c_orchestrator_mode_default_no_dev_tools() {
        // The most common production path is Orchestrator { allow_local_tools: false }.
        // It must NOT include dev tools — those belong inside the sandbox.
        let ctx = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            ..Default::default()
        };
        assert!(!ctx.includes_dev_tools());
    }

    #[test]
    fn req_p02_pr2_d_orchestrator_with_local_tools_includes_dev() {
        // Opt-in path for local-development orchestrators.
        let ctx = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: true,
            },
            ..Default::default()
        };
        assert!(ctx.includes_dev_tools());
    }

    #[test]
    fn req_p02_pr2_e_container_mode_includes_dev() {
        // Sandboxed worker always has dev tools.
        let ctx = BootstrapContext {
            mode: BootstrapMode::Container,
            ..Default::default()
        };
        assert!(ctx.includes_dev_tools());
    }

    #[test]
    fn req_p02_pr2_f_optional_fields_default_to_none() {
        // Spec test: every non-mode field is None on Default. Catches accidental
        // field renames or default-value changes that would silently register
        // unwanted tool groups.
        let ctx = BootstrapContext::default();
        assert!(ctx.workspace.is_none());
        assert!(ctx.db_pool.is_none());
        assert!(ctx.secrets_store.is_none());
        assert!(ctx.extension_manager.is_none());
        assert!(ctx.channels.is_none());
        assert!(ctx.skill_registry.is_none());
        assert!(ctx.skill_catalog.is_none());
        assert!(ctx.routine_store.is_none());
        assert!(ctx.routine_engine.is_none());
        assert!(ctx.job_config.is_none());
        assert!(ctx.image_api.is_none());
        assert!(ctx.vision_api.is_none());
    }

    #[test]
    fn req_p02_pr2_g_bootstrap_error_other_renders() {
        // Sanity check the placeholder error variant so future variants
        // can be added without breaking the trait derive contract.
        let err = BootstrapError::Other("boom".to_string());
        assert_eq!(err.to_string(), "bootstrap error: boom");
    }
}
