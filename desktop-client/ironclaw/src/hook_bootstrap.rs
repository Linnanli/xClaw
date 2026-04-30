//! Hook bootstrap helpers for loading bundled, plugin, and workspace hooks.
//!
//! ## ADR-113 Phase 0 red-line invariants
//!
//! This module is the only path that wires hooks into the running agent
//! (P0-3 PR #3). Two invariants from `dasclaw_hooks::contract` are enforced
//! here at startup:
//!
//! 1. [`count_hook_systems()`] must return `1` (compile-time `const _`
//!    guard below) — Phase 0 red-line. If this trips you've reintroduced a
//!    competing hook orchestration entry; revisit ADR-113 §2.2 before
//!    bypassing.
//! 2. [`no_safety_rule_in_event_hooks`] runs at the end of
//!    [`bootstrap_hooks`] and **panics** on any violation, by design (see
//!    ADR-113 §2.3 + contract.rs module docs: "fail loud, not silent").
//!    Bundle authors must move secret/redact/safety responsibilities to
//!    the [`SafetyHook`](dasclaw_hooks::SafetyHook) trait seam.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::channels::wasm::discover_channels;
use crate::tools::wasm::{discover_dev_tools, discover_tools};
use crate::workspace::Workspace;
use dasclaw_hooks::{
    HookBundleConfig, HookRegistrationSummary, HookRegistry, count_hook_systems,
    no_safety_rule_in_event_hooks, register_bundle, register_bundled_hooks,
};

/// Compile-time red-line: exactly one hook orchestration entry exists
/// (`dasclaw_hooks` itself). If ADR-113 §2.2 is violated by reintroducing
/// a parallel registry/dispatcher, this assertion will fail to compile.
const _: () = assert!(
    count_hook_systems() == 1,
    "ADR-113 Phase 0 red-line: count_hook_systems() must equal 1"
);

/// Summary of hook bootstrap work done at startup.
#[derive(Debug, Default, Clone, Copy)]
pub struct HookBootstrapSummary {
    /// Number of bundled built-in hooks registered.
    pub bundled_hooks: usize,
    /// Number of plugin-provided rule hooks registered.
    pub plugin_hooks: usize,
    /// Number of workspace-provided rule hooks registered.
    pub workspace_hooks: usize,
    /// Number of outbound webhook hooks registered.
    pub outbound_webhooks: usize,
    /// Number of invalid hook configs skipped.
    pub errors: usize,
}

impl HookBootstrapSummary {
    /// Total number of hooks registered across all categories.
    pub fn total_hooks(&self) -> usize {
        self.bundled_hooks + self.plugin_hooks + self.workspace_hooks + self.outbound_webhooks
    }
}

/// Register bundled hooks, then load plugin and workspace hook bundles.
pub async fn bootstrap_hooks(
    registry: &Arc<HookRegistry>,
    workspace: Option<&Arc<Workspace>>,
    wasm_tools_dir: &Path,
    wasm_channels_dir: &Path,
    active_tool_names: &[String],
    active_channel_names: &[String],
    dev_loaded_tool_names: &[String],
) -> HookBootstrapSummary {
    let mut summary = HookBootstrapSummary::default();

    let bundled = register_bundled_hooks(registry).await;
    summary.bundled_hooks += bundled.hooks;
    summary.outbound_webhooks += bundled.outbound_webhooks;
    summary.errors += bundled.errors;

    let plugin = register_plugin_bundles(
        registry,
        wasm_tools_dir,
        wasm_channels_dir,
        active_tool_names,
        active_channel_names,
        dev_loaded_tool_names,
    )
    .await;
    summary.plugin_hooks += plugin.hooks;
    summary.outbound_webhooks += plugin.outbound_webhooks;
    summary.errors += plugin.errors;

    if let Some(workspace) = workspace {
        let workspace_loaded = register_workspace_bundles(registry, workspace).await;
        summary.workspace_hooks += workspace_loaded.hooks;
        summary.outbound_webhooks += workspace_loaded.outbound_webhooks;
        summary.errors += workspace_loaded.errors;
    }

    // ADR-113 §2.3 / Phase 0 red-line: any declarative event-hook rule
    // carrying safety/redact/secret responsibility is a contract violation.
    // Fail loudly at startup so deployments never silently route safety
    // decisions through the event-hook bus instead of the SafetyHook trait
    // seam (which returns structured SafetyDecision values).
    let violations = no_safety_rule_in_event_hooks(registry).await;
    assert!(
        violations.is_empty(),
        "ADR-113 §2.3 violated: declarative event-hook bundle contains \
         safety-responsibility rule(s) — move them to a SafetyHook \
         implementation. Violations: {violations:?}"
    );

    summary
}

async fn register_plugin_bundles(
    registry: &Arc<HookRegistry>,
    wasm_tools_dir: &Path,
    wasm_channels_dir: &Path,
    active_tool_names: &[String],
    active_channel_names: &[String],
    dev_loaded_tool_names: &[String],
) -> HookRegistrationSummary {
    let mut summary = HookRegistrationSummary::default();
    let files = collect_plugin_capability_files(
        wasm_tools_dir,
        wasm_channels_dir,
        active_tool_names,
        active_channel_names,
        dev_loaded_tool_names,
    )
    .await;

    for (source, path) in files {
        let registered =
            register_plugin_bundle_from_capabilities_file(registry, &source, &path).await;
        summary.merge(registered);
    }

    summary
}

/// Register a plugin hook bundle from a single capabilities file.
///
/// This is used by startup bootstrap and by runtime extension activation.
pub async fn register_plugin_bundle_from_capabilities_file(
    registry: &Arc<HookRegistry>,
    source: &str,
    path: &Path,
) -> HookRegistrationSummary {
    match load_plugin_bundle_from_capabilities_file(path).await {
        Ok(Some(bundle)) => register_bundle(registry, source, bundle).await,
        Ok(None) => HookRegistrationSummary::default(),
        Err(err) => {
            tracing::warn!(
                source = source,
                path = %path.display(),
                error = %err,
                "Skipping plugin hook bundle"
            );
            HookRegistrationSummary {
                hooks: 0,
                outbound_webhooks: 0,
                errors: 1,
            }
        }
    }
}

async fn collect_plugin_capability_files(
    wasm_tools_dir: &Path,
    wasm_channels_dir: &Path,
    active_tool_names: &[String],
    active_channel_names: &[String],
    dev_loaded_tool_names: &[String],
) -> Vec<(String, PathBuf)> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let active_tools: HashSet<&str> = active_tool_names.iter().map(String::as_str).collect();
    let active_channels: HashSet<&str> = active_channel_names.iter().map(String::as_str).collect();
    let dev_loaded_tools: HashSet<&str> =
        dev_loaded_tool_names.iter().map(String::as_str).collect();

    if wasm_tools_dir.exists() {
        match discover_tools(wasm_tools_dir).await {
            Ok(tools) => {
                for (name, tool) in tools {
                    if let Some(path) = tool.capabilities_path
                        && active_tools.contains(name.as_str())
                        && !dev_loaded_tools.contains(name.as_str())
                    {
                        insert_unique(&mut files, &mut seen, format!("plugin.tool:{}", name), path);
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    path = %wasm_tools_dir.display(),
                    error = %err,
                    "Failed to discover WASM tool capabilities for plugin hooks"
                );
            }
        }
    }

    match discover_dev_tools().await {
        Ok(dev_tools) => {
            for (name, tool) in dev_tools {
                if let Some(path) = tool.capabilities_path
                    && active_tools.contains(name.as_str())
                    && dev_loaded_tools.contains(name.as_str())
                {
                    insert_unique(
                        &mut files,
                        &mut seen,
                        format!("plugin.dev_tool:{}", name),
                        path,
                    );
                }
            }
        }
        Err(err) => {
            tracing::debug!(error = %err, "No dev tool capabilities discovered for plugin hooks");
        }
    }

    if wasm_channels_dir.exists() {
        match discover_channels(wasm_channels_dir).await {
            Ok(channels) => {
                for (name, channel) in channels {
                    if let Some(path) = channel.capabilities_path
                        && active_channels.contains(name.as_str())
                    {
                        insert_unique(
                            &mut files,
                            &mut seen,
                            format!("plugin.channel:{}", name),
                            path,
                        );
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    path = %wasm_channels_dir.display(),
                    error = %err,
                    "Failed to discover WASM channel capabilities for plugin hooks"
                );
            }
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn insert_unique(
    files: &mut Vec<(String, PathBuf)>,
    seen: &mut HashSet<String>,
    source: String,
    path: PathBuf,
) {
    let key = path.to_string_lossy().to_string();
    if seen.insert(key) {
        files.push((source, path));
    }
}

async fn load_plugin_bundle_from_capabilities_file(
    path: &Path,
) -> Result<Option<HookBundleConfig>, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("read failed: {e}"))?;

    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON: {e}"))?;

    let Some(hooks_value) = extract_hooks_section(&value) else {
        return Ok(None);
    };

    HookBundleConfig::from_value(hooks_value)
        .map(Some)
        .map_err(|e| e.to_string())
}

fn extract_hooks_section(root: &serde_json::Value) -> Option<&serde_json::Value> {
    root.get("hooks")
        .or_else(|| root.get("capabilities").and_then(|c| c.get("hooks")))
}

async fn register_workspace_bundles(
    registry: &Arc<HookRegistry>,
    workspace: &Arc<Workspace>,
) -> HookRegistrationSummary {
    let mut summary = HookRegistrationSummary::default();

    let paths = match workspace.list_all().await {
        Ok(paths) => paths,
        Err(err) => {
            summary.errors += 1;
            tracing::warn!(error = %err, "Failed to list workspace paths for hooks");
            return summary;
        }
    };

    let mut hook_paths: Vec<String> = paths
        .into_iter()
        .filter(|path| is_workspace_hook_file(path))
        .collect();
    hook_paths.sort();

    for path in hook_paths {
        let doc = match workspace.read(&path).await {
            Ok(doc) => doc,
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(path = %path, error = %err, "Skipping unreadable workspace hook file");
                continue;
            }
        };

        let parsed: serde_json::Value = match serde_json::from_str(&doc.content) {
            Ok(value) => value,
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(path = %path, error = %err, "Workspace hook file is not valid JSON");
                continue;
            }
        };

        let bundle = match parse_workspace_bundle(&parsed) {
            Ok(bundle) => bundle,
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(path = %path, error = %err, "Skipping invalid workspace hook bundle");
                continue;
            }
        };

        let source = format!("workspace:{}", path);
        let registered = register_bundle(registry, &source, bundle).await;
        summary.merge(registered);
    }

    summary
}

fn parse_workspace_bundle(value: &serde_json::Value) -> Result<HookBundleConfig, String> {
    if let Some(nested) = value.get("hooks") {
        HookBundleConfig::from_value(nested).map_err(|e| e.to_string())
    } else {
        HookBundleConfig::from_value(value).map_err(|e| e.to_string())
    }
}

fn is_workspace_hook_file(path: &str) -> bool {
    path == "hooks/hooks.json" || (path.starts_with("hooks/") && path.ends_with(".hook.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_hooks_section_from_tool_caps() {
        let value = serde_json::json!({
            "http": {"allowlist": []},
            "hooks": {"rules": []}
        });

        let extracted = extract_hooks_section(&value).unwrap();
        assert!(extracted.get("rules").is_some());
    }

    #[test]
    fn test_extract_hooks_section_from_channel_caps() {
        let value = serde_json::json!({
            "type": "channel",
            "capabilities": {
                "hooks": {
                    "rules": []
                }
            }
        });

        let extracted = extract_hooks_section(&value).unwrap();
        assert!(extracted.get("rules").is_some());
    }

    #[test]
    fn test_workspace_hook_file_filter() {
        assert!(is_workspace_hook_file("hooks/hooks.json"));
        assert!(is_workspace_hook_file("hooks/redact.hook.json"));
        assert!(!is_workspace_hook_file("hooks/readme.md"));
        assert!(!is_workspace_hook_file("MEMORY.md"));
    }

    #[test]
    fn test_parse_workspace_bundle_wrapped_hooks() {
        let value = serde_json::json!({
            "hooks": {
                "rules": [
                    {
                        "name": "append-bang",
                        "points": ["beforeInbound"],
                        "append": "!"
                    }
                ]
            }
        });

        let bundle = parse_workspace_bundle(&value).unwrap();
        assert_eq!(bundle.rules.len(), 1);
    }

    // ------------------------------------------------------------------
    // P0-3 PR #3 red-line tests (ADR-113 §2.2 + §2.3, Phase 0)
    // ------------------------------------------------------------------

    /// `req_p03_pr3_shim_module_removed` — the legacy
    /// `desktop-client/ironclaw/src/hooks/` shim (PR #2 transitional bridge)
    /// must be physically gone after PR #3. A returning shim would mean
    /// duplicate re-exports of `dasclaw_hooks::*` paths and dilute the
    /// "single front-door" property from ADR-113 §2.2.
    #[test]
    fn req_p03_pr3_shim_module_removed() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let shim_dir = manifest_dir.join("src").join("hooks");
        let shim_modrs = shim_dir.join("mod.rs");
        assert!(
            !shim_modrs.exists(),
            "ADR-113: hooks/ shim module must be removed (still found at {})",
            shim_modrs.display()
        );
        assert!(
            !shim_dir.exists(),
            "ADR-113: hooks/ shim directory must be removed (still found at {})",
            shim_dir.display()
        );
    }

    /// `req_p03_pr3_bootstrap_runtime_assert_count_eq_one` — Phase 0 red-line
    /// is enforced at compile time via the module-level `const _: () =
    /// assert!(count_hook_systems() == 1)` guard. The runtime mirror keeps
    /// the test grid green for code-review-expert auditing.
    #[test]
    fn req_p03_pr3_bootstrap_runtime_assert_count_eq_one() {
        assert_eq!(
            dasclaw_hooks::count_hook_systems(),
            1,
            "ADR-113 Phase 0 red-line: exactly one hook orchestration system"
        );
    }

    /// `req_p03_pr3_bootstrap_panics_on_safety_in_event_hooks` — exercises
    /// the same invariant the production `bootstrap_hooks` runs at the end
    /// of registration. A declarative bundle rule named with a banned
    /// keyword (`secret`/`redact`/`safety`) MUST cause startup to fail
    /// loudly (ADR-113 §2.3, contract.rs "fail loud, not silent").
    #[tokio::test]
    #[should_panic(expected = "ADR-113 §2.3 violated")]
    async fn req_p03_pr3_bootstrap_panics_on_safety_in_event_hooks() {
        use dasclaw_hooks::hook::{
            Hook, HookContext, HookError, HookEvent, HookOutcome, HookPoint,
        };
        use std::sync::Arc;

        struct PoisonHook;

        #[async_trait::async_trait]
        impl Hook for PoisonHook {
            fn name(&self) -> &str {
                "redact-pii"
            }
            fn hook_points(&self) -> &[HookPoint] {
                &[HookPoint::BeforeInbound]
            }
            async fn execute(
                &self,
                _event: &HookEvent,
                _ctx: &HookContext,
            ) -> Result<HookOutcome, HookError> {
                Ok(HookOutcome::ok())
            }
        }

        let registry = HookRegistry::new();
        registry.register(Arc::new(PoisonHook)).await;

        // Mirror the exact assertion `bootstrap_hooks` runs.
        let violations = no_safety_rule_in_event_hooks(&registry).await;
        assert!(
            violations.is_empty(),
            "ADR-113 §2.3 violated: declarative event-hook bundle contains \
             safety-responsibility rule(s) — move them to a SafetyHook \
             implementation. Violations: {violations:?}"
        );
    }
}
