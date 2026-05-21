//! Workspace search configuration data.
//!
//! This module hosts the data structure that callers (typically the host
//! process's configuration layer) resolve from environment variables or
//! settings. The runtime [`crate::search::SearchConfig`] is constructed
//! from these fields by `Workspace::with_search_config`.
//!
//! Environment-variable resolution (`resolve()`) lives in the host crate
//! (`ironclaw::config::search`) because it depends on host-specific
//! helpers and error types; this crate only owns the pure data shape.

use crate::search::FusionStrategy;

/// Workspace search configuration resolved from environment variables.
#[derive(Debug, Clone)]
pub struct WorkspaceSearchConfig {
    /// Fusion strategy: "rrf" or "weighted".
    pub fusion_strategy: FusionStrategy,
    /// RRF constant k (default 60).
    pub rrf_k: u32,
    /// FTS weight for fusion.
    ///
    /// [`Default`] uses 0.5. When the configuration is resolved, per-strategy
    /// defaults are applied: 0.5 (RRF) or 0.3 (weighted).
    pub fts_weight: f32,
    /// Vector weight for fusion.
    ///
    /// [`Default`] uses 0.5. When the configuration is resolved, per-strategy
    /// defaults are applied: 0.5 (RRF) or 0.7 (weighted).
    pub vector_weight: f32,
}

impl Default for WorkspaceSearchConfig {
    fn default() -> Self {
        Self {
            fusion_strategy: FusionStrategy::default(),
            rrf_k: 60,
            fts_weight: 0.5,
            vector_weight: 0.5,
        }
    }
}
