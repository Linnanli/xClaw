//! ADR-148 R2 cleanup helper: replace `SafetyLayer::sanitize_tool_output`
//! call sites with the unified Layer-B [`EgressGate`] using the
//! [`EgressKind::UserDisplay`] kind.
//!
//! ## Why a single helper
//!
//! Before this helper, nine call sites across `dispatcher.rs`,
//! `worker/job.rs`, and `routines/routine_engine.rs` each called
//! `SafetyLayer::sanitize_tool_output(tool, payload).content`. That bypassed
//! the ADR-148 Layer-B chain entirely (no leak detector, no audit signal,
//! no shared rule set with the agentic loop's egress checks).
//!
//! Threading a new `egress: Arc<dyn EgressGate>` field through
//! `AgentDeps` / `WorkerDeps` / `EngineContext` would ripple to ~14
//! constructors including 8 test mocks — pure plumbing for what is
//! semantically a single operation: "sanitise this tool output via the
//! unified Layer-B gate".
//!
//! Instead, [`sanitize_tool_output_via_egress`] takes the same
//! `Arc<SafetyLayer>` the caller already holds, wraps it in an
//! [`IronclawEgressGate`] (the canonical adapter from `SafetyLayer` to
//! [`EgressGate`]), runs the gate, and applies the decision via
//! [`dasclaw_core::egress_apply::apply_egress_decision`] — the same helper
//! the agentic loop's `LlmRequest`/`UserDisplay` sites use. One Fail-Safe
//! semantics surface, nine identical call-site rewrites, zero ctor changes.
//!
//! ## EgressKind choice — `UserDisplay`, never `ToolExecution`
//!
//! All nine sites take **already-executed tool output** (plain text) and
//! channel it back into either the user-facing UI or the LLM context.
//! That fits the `UserDisplay` contract (leak detector + scan_and_clean).
//!
//! `EgressKind::ToolExecution` would route through
//! `IronclawEgressGate::validate_tool_params` which JSON-parses the
//! payload and rejects any non-JSON. Tool outputs are arbitrary text →
//! parse fails → Fail-Safe `Block` → every tool output dropped.
//! Catastrophic regression; the helper enforces `UserDisplay` so the
//! choice cannot be made wrong at a call site.
//!
//! ## Fail-Safe on `Block` / `Ask`
//!
//! If the gate refuses (Block or, in a headless context, Ask), the
//! returned string is `"[redacted: <reason>]"` so the LLM / UI does not
//! see the original payload. The reason is also surfaced through
//! `tracing::warn!` for audit. This mirrors the agentic loop's behaviour
//! of halting on Block / Ask but keeps the worker / dispatcher pipelines
//! flowing (a single redacted line in a tool result is recoverable;
//! aborting the entire job because one tool's output tripped a leak rule
//! is not the desired UX here — that is the role of the LlmRequest gate
//! upstream).

use std::sync::Arc;

use dasclaw_core::egress_apply::{EgressApply, apply_egress_decision};
use dasclaw_governance::egress::{EgressGate, EgressKind};
use dasclaw_safety::SafetyLayer;
use dasclaw_safety::egress_gate::IronclawEgressGate;

/// Run `payload` through the ADR-148 Layer-B egress gate and return the
/// sanitised string ready to be embedded in LLM context, channel events,
/// or persisted job records.
///
/// On `Allow` / `Passthrough` returns the original payload unchanged. On
/// `Redact` returns the gate-sanitised payload. On `Block` / `Ask` /
/// future variants returns `"[redacted: <reason>]"` and logs the
/// Fail-Safe halt at `WARN`.
///
/// `tool_name` is used purely as a tracing label (`ToolOutput:<tool>`);
/// the gate itself never inspects it.
pub async fn sanitize_tool_output_via_egress(
    safety: &Arc<SafetyLayer>,
    tool_name: &str,
    payload: &str,
) -> String {
    // Cheap wrapper: holds `Arc<SafetyLayer>` only. Constructing per call
    // avoids ctor-threading 14 construction sites for what is functionally
    // a method on `SafetyLayer`.
    let gate = IronclawEgressGate::new(Arc::clone(safety));
    let mut buf = payload.to_string();
    let decision = gate.check(&EgressKind::UserDisplay, &buf).await;
    let label = format!("ToolOutput:{tool_name}");
    match apply_egress_decision(decision, &mut buf, &label) {
        EgressApply::Continue => buf,
        EgressApply::Halt(reason) => {
            tracing::warn!(
                tool = tool_name,
                %reason,
                "tool output halted by egress gate; substituting [redacted] placeholder"
            );
            format!("[redacted: {reason}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dasclaw_safety::SafetyConfig;

    fn make_safety() -> Arc<SafetyLayer> {
        Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 1024,
            injection_check_enabled: true,
        }))
    }

    #[tokio::test]
    async fn plain_output_passes_through() {
        let safety = make_safety();
        let out = sanitize_tool_output_via_egress(&safety, "read_file", "Hello world").await;
        assert_eq!(out, "Hello world");
    }

    #[tokio::test]
    async fn empty_payload_is_preserved() {
        let safety = make_safety();
        let out = sanitize_tool_output_via_egress(&safety, "noop_tool", "").await;
        assert_eq!(out, "");
    }

    #[tokio::test]
    async fn unicode_payload_is_preserved_unchanged() {
        // Regression: leak-detector must not mangle multi-byte boundaries
        // on a clean payload.
        let safety = make_safety();
        let payload = "你好，世界 🌏 — café";
        let out = sanitize_tool_output_via_egress(&safety, "translate", payload).await;
        assert_eq!(out, payload);
    }
}
