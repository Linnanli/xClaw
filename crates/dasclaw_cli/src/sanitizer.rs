//! Default `ToolOutputSanitizer` adapter for the headless CLI.
//!
//! ADR-153 §1.1 B6 / e22 wiring (issue #882). `dasclaw_runtime` keeps
//! its `ToolOutputSanitizer` trait safety-stack-agnostic; this module
//! is where the CLI binary picks a concrete safety stack — currently
//! [`dasclaw_safety::SafetyLayer::sanitize_for_stash`], the same
//! redact-only path the legacy ironclaw loop calls before stashing
//! tool output. The seam matches the legacy contract byte-for-byte:
//! leak detection + policy enforcement, **no length capping**
//! (`cap_for_llm` is a separate concern and is not wired here).
//!
//! Downstream lib consumers that want a different safety stack should
//! reach for [`dasclaw_runtime::Agent::builder`] directly with their
//! own `ToolOutputSanitizer` impl — this module is the binary's
//! default, not a forced choice.

use std::sync::Arc;

use dasclaw_runtime::ToolOutputSanitizer;
use dasclaw_safety::{SafetyConfig, SafetyLayer};

/// Adapter that delegates to [`SafetyLayer::sanitize_for_stash`].
///
/// Cheap to clone (the inner [`SafetyLayer`] is held by [`Arc`]) so the
/// same instance can be shared across the agent loop and the egress
/// gate without re-initialising the leak-detector regex set on every
/// tool invocation.
pub struct SafetyStashSanitizer {
    layer: Arc<SafetyLayer>,
}

impl SafetyStashSanitizer {
    /// Wrap an existing [`SafetyLayer`] so it can be installed via
    /// [`dasclaw_runtime::AgentBuilder::tool_output_sanitizer`]. Callers
    /// that already hold a shared `Arc<SafetyLayer>` (e.g. because the
    /// same layer powers their `EgressGate`) should prefer this entry
    /// so leak-pattern compilation happens exactly once per process.
    pub fn new(layer: Arc<SafetyLayer>) -> Self {
        Self { layer }
    }
}

impl ToolOutputSanitizer for SafetyStashSanitizer {
    fn sanitize(&self, tool_name: &str, content: &str) -> String {
        self.layer.sanitize_for_stash(tool_name, content).content
    }
}

/// Build the binary's default `ToolOutputSanitizer`.
///
/// Uses the same [`SafetyConfig`] values as the desktop client's
/// production wiring (`max_output_length = 100_000`,
/// `injection_check_enabled = true`) so the redact-only path here stays
/// consistent with the parallel ironclaw loop. `max_output_length` is
/// only consulted by `cap_for_llm` and is irrelevant to
/// `sanitize_for_stash`, but it is plumbed through anyway to keep the
/// config single-source-of-truth for the day someone wires capping at
/// this seam too.
pub fn default_safety_sanitizer() -> Arc<dyn ToolOutputSanitizer> {
    let layer = Arc::new(SafetyLayer::new(&SafetyConfig {
        max_output_length: 100_000,
        injection_check_enabled: true,
    }));
    Arc::new(SafetyStashSanitizer::new(layer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_redacts_bearer_token_in_tool_output() {
        let sanitizer = default_safety_sanitizer();
        let raw = "found credential: Bearer abcdef0123456789ABCDEFXYZ (please rotate)";
        let out = sanitizer.sanitize("read_file", raw);
        assert!(
            !out.contains("Bearer abcdef0123456789ABCDEFXYZ"),
            "raw token leaked through default sanitizer: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "expected `[REDACTED]` marker in sanitized output, got {out}"
        );
        assert!(
            out.contains("please rotate"),
            "non-secret context must survive redaction: {out}"
        );
    }

    #[test]
    fn sanitizer_passes_clean_output_through_unchanged() {
        let sanitizer = default_safety_sanitizer();
        let raw = "ok: 42 rows scanned";
        assert_eq!(sanitizer.sanitize("query", raw), raw);
    }

    #[test]
    fn shared_layer_is_reused_across_calls() {
        let layer = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: false,
        }));
        let sanitizer = SafetyStashSanitizer::new(Arc::clone(&layer));
        // Two back-to-back calls share the same underlying layer; the
        // test passes simply by virtue of compiling + not panicking,
        // but also pins the redact-on-second-call behaviour so a
        // future refactor that caches per-call state would surface.
        let first = sanitizer.sanitize("a", "Bearer abcdef0123456789ABCDEFXYZ");
        let second = sanitizer.sanitize("b", "Bearer abcdef0123456789ABCDEFXYZ");
        assert_eq!(first, second);
        assert!(first.contains("[REDACTED]"));
    }
}
