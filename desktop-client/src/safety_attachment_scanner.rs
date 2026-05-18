//! Attachment DLP gate (issue #92, ADR-148 §6.4).
//!
//! Wraps the project-wide [`EgressGate`] trait so chat attachments
//! (`document` / `image` / `audio`) flow through the same DLP / leak
//! detector chain as ordinary user text **before** they reach the agent
//! loop or the persistence path.
//!
//! # Threat model addressed
//!
//! Before this gate, [`desktop-client/src/ipc/chat.rs`](../ipc/chat.rs)
//! scanned only the chat textbox content. An attacker (or a careless
//! user) could drop a PDF / Word / image / audio file whose
//! `extracted_text` carried `sk-*` API keys, `BEGIN PRIVATE KEY` PEM
//! bodies, or PII; the attachment was forwarded into the agent message
//! verbatim and persisted into the audit/admin-backend report path.
//!
//! # Two egress kinds, two gate calls
//!
//! For each attachment we ask the same gate two questions:
//!
//! 1. **`EgressKind::LlmRequest`** — is it safe to ship to the LLM?
//! 2. **`EgressKind::Persistence`** — is it safe to write to the local
//!    DB / admin-backend report?
//!
//! The strictest of the two decisions wins (Block > Ask > Redact >
//! Allow). When either gate `Block`s, the whole attachment is rejected
//! and the agent never sees it.
//!
//! # Binary MIME policy (fail-closed)
//!
//! Binary `data` cannot be text-scanned (it is opaque to the leak
//! detector). We allow only a small whitelist of MIME types whose
//! semantics are well understood by downstream consumers (image render
//! / audio transcription / document extraction):
//!
//! - `image/{jpeg,png,webp,gif}`
//! - `audio/{ogg,mpeg,mp4,wav,webm,flac,x-m4a}`
//! - `application/{pdf, vnd.openxmlformats-officedocument.*, msword,
//!    vnd.ms-excel, vnd.ms-powerpoint}`
//! - `text/*`
//!
//! Anything else is **blocked** rather than forwarded — per AGENTS.md
//! red-line "安全功能必须 Fail-Safe".

use std::sync::Arc;

use x_claw_agent::{EgressDecision, EgressGate, EgressKind};

/// Per-attachment scan decision returned by [`AttachmentScanner::scan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentDecision {
    /// Attachment is safe to forward unchanged.
    Allow,
    /// Attachment text was rewritten by the gate; callers MUST replace
    /// the original `extracted_text` with `sanitized_text` before
    /// forwarding the attachment to the agent **and** to persistence.
    Redact {
        sanitized_text: String,
        /// Which egress kinds fired a Redact (for audit stats only).
        kinds_redacted: Vec<EgressKind>,
    },
    /// Attachment must be rejected. `reason` is safe to surface to the
    /// user (never contains the original secret).
    Block { reason: String },
}

/// Async-safe scanner that routes one attachment through the configured
/// [`EgressGate`].
pub struct AttachmentScanner {
    gate: Arc<dyn EgressGate>,
}

impl AttachmentScanner {
    pub fn new(gate: Arc<dyn EgressGate>) -> Self {
        Self { gate }
    }

    /// Run both [`EgressKind::LlmRequest`] and [`EgressKind::Persistence`]
    /// gates against `extracted_text`, plus the binary MIME whitelist on
    /// `mime_type` when the attachment carries raw `data`.
    ///
    /// `extracted_text` may be `None` for attachments that have no text
    /// representation yet (e.g. an image without OCR). In that case
    /// only the binary MIME check runs.
    ///
    /// `has_binary_payload` MUST be `true` whenever the caller is about
    /// to forward non-empty raw bytes to the LLM (multimodal) or
    /// persist them; the whitelist check is skipped only for purely
    /// metadata-only attachments to avoid blocking benign cases.
    pub async fn scan(
        &self,
        mime_type: &str,
        extracted_text: Option<&str>,
        has_binary_payload: bool,
    ) -> AttachmentDecision {
        // ── 1. Binary MIME whitelist (fail-closed) ─────────────────
        if has_binary_payload && !is_binary_mime_allowed(mime_type) {
            return AttachmentDecision::Block {
                reason: format!(
                    "Attachment MIME `{mime_type}` is not on the egress whitelist; refusing to forward binary payload (fail-safe)"
                ),
            };
        }

        // ── 2. extracted_text gate (LlmRequest + Persistence) ──────
        let text = match extracted_text {
            Some(t) if !t.is_empty() => t,
            _ => return AttachmentDecision::Allow,
        };

        let llm_decision = self.gate.check(&EgressKind::LlmRequest, text).await;
        let persist_decision = self.gate.check(&EgressKind::Persistence, text).await;

        merge_decisions(llm_decision, persist_decision)
    }
}

/// Combine the two per-kind decisions; strictest wins.
///
/// Precedence: `Block` > `Ask` (mapped to Block in headless) >
/// `Redact` > `Allow` / `Passthrough`.
fn merge_decisions(llm: EgressDecision, persist: EgressDecision) -> AttachmentDecision {
    // Fast path: both Allow / Passthrough.
    if is_allow_or_passthrough(&llm) && is_allow_or_passthrough(&persist) {
        return AttachmentDecision::Allow;
    }

    // Any Block / Ask → fail-closed.
    if let Some(reason) = block_reason(&llm).or_else(|| block_reason(&persist)) {
        return AttachmentDecision::Block { reason };
    }

    // Otherwise at least one side Redacted. Use the persistence-side
    // sanitized payload if available (it is the stricter scan in
    // IronclawEgressGate — full leak detector), else the LLM-side one.
    let mut kinds_redacted = Vec::new();
    let sanitized = if let EgressDecision::Redact { sanitized, .. } = &persist {
        kinds_redacted.push(EgressKind::Persistence);
        if matches!(llm, EgressDecision::Redact { .. }) {
            kinds_redacted.push(EgressKind::LlmRequest);
        }
        sanitized.clone()
    } else if let EgressDecision::Redact { sanitized, .. } = &llm {
        kinds_redacted.push(EgressKind::LlmRequest);
        sanitized.clone()
    } else {
        // Unreachable: the fast path above ruled out all-Allow, and
        // block_reason ruled out Block/Ask, so at least one side must
        // be Redact. Fail-safe Block defensively.
        return AttachmentDecision::Block {
            reason: "attachment gate returned an unexpected decision combination (fail-safe)"
                .to_string(),
        };
    };

    AttachmentDecision::Redact {
        sanitized_text: sanitized,
        kinds_redacted,
    }
}

fn is_allow_or_passthrough(d: &EgressDecision) -> bool {
    matches!(d, EgressDecision::Allow | EgressDecision::Passthrough)
}

fn block_reason(d: &EgressDecision) -> Option<String> {
    match d {
        EgressDecision::Block { reason, .. } => Some(reason.clone()),
        // Ask in headless desktop chat context = Block (Fail-Safe).
        EgressDecision::Ask { reason, .. } => Some(format!(
            "attachment gate requested user confirmation; treated as block in headless context: {reason}"
        )),
        _ => None,
    }
}

/// Whitelist of binary MIME types whose semantics downstream consumers
/// understand. Anything outside this list with raw bytes is **blocked**.
fn is_binary_mime_allowed(mime: &str) -> bool {
    let lower = mime.to_ascii_lowercase();
    // text/* always allowed (already covered by text scan path; binary
    // path can also accept e.g. text/plain attachments uploaded as raw
    // bytes).
    if lower.starts_with("text/") {
        return true;
    }
    matches!(
        lower.as_str(),
        // Images
        "image/jpeg"
            | "image/png"
            | "image/webp"
            | "image/gif"
            // Audio
            | "audio/ogg"
            | "audio/mpeg"
            | "audio/mp4"
            | "audio/wav"
            | "audio/x-wav"
            | "audio/webm"
            | "audio/flac"
            | "audio/x-m4a"
            | "audio/x-flac"
            // Documents
            | "application/pdf"
            | "application/msword"
            | "application/vnd.ms-excel"
            | "application/vnd.ms-powerpoint"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/json"
            | "application/xml"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use x_claw_agent::RedactionStats;

    /// Programmable test gate keyed by `EgressKind` discriminant.
    struct ScriptedGate {
        on_llm: EgressDecision,
        on_persist: EgressDecision,
    }

    #[async_trait]
    impl EgressGate for ScriptedGate {
        async fn check(&self, kind: &EgressKind, _payload: &str) -> EgressDecision {
            match kind {
                EgressKind::LlmRequest => self.on_llm.clone(),
                EgressKind::Persistence => self.on_persist.clone(),
                _ => EgressDecision::Allow,
            }
        }
    }

    fn scanner(llm: EgressDecision, persist: EgressDecision) -> AttachmentScanner {
        AttachmentScanner::new(Arc::new(ScriptedGate {
            on_llm: llm,
            on_persist: persist,
        }))
    }

    fn block(reason: &str) -> EgressDecision {
        EgressDecision::Block {
            reason: reason.into(),
            stats: RedactionStats::default(),
        }
    }

    fn redact(sanitized: &str) -> EgressDecision {
        EgressDecision::Redact {
            sanitized: sanitized.into(),
            stats: RedactionStats::default(),
        }
    }

    // ── #92 acceptance tests (req_attach_92_*) ────────────────────

    #[tokio::test]
    async fn req_attach_92_a_document_extracted_secret_blocks() {
        // DOCX extracted_text contains an OpenAI key → Block, never
        // reaches msg_sender.send.
        let s = scanner(block("secret detected: sk-xxxx"), EgressDecision::Allow);
        let payload = format!("user notes: sk-{}", "A".repeat(48));
        let decision = s
            .scan(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                Some(&payload),
                true,
            )
            .await;
        assert!(
            matches!(decision, AttachmentDecision::Block { .. }),
            "expected Block, got {decision:?}",
        );
    }

    #[tokio::test]
    async fn req_attach_92_b_image_extracted_pii_redacts() {
        // Image OCR result contains a Chinese ID — must be redacted
        // (not blocked) so the user's intent is preserved.
        let sanitized = "客户身份证：330***618";
        let s = scanner(redact(sanitized), redact(sanitized));
        let decision = s
            .scan("image/png", Some("客户身份证：330106199001011234"), true)
            .await;
        match decision {
            AttachmentDecision::Redact { sanitized_text, .. } => {
                assert_eq!(sanitized_text, sanitized);
                assert!(!sanitized_text.contains("330106199001011234"));
            }
            other => panic!("expected Redact, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn req_attach_92_c_audio_transcript_secret_blocks() {
        let s = scanner(block("secret in transcript"), EgressDecision::Allow);
        let payload = format!("the key is sk-{}", "B".repeat(48));
        let decision = s.scan("audio/ogg", Some(&payload), true).await;
        assert!(matches!(decision, AttachmentDecision::Block { .. }));
    }

    #[tokio::test]
    async fn req_attach_92_d_clean_attachment_passes_through() {
        let s = scanner(EgressDecision::Allow, EgressDecision::Allow);
        let decision = s
            .scan("application/pdf", Some("Quarterly report contents."), true)
            .await;
        assert_eq!(decision, AttachmentDecision::Allow);
    }

    #[tokio::test]
    async fn req_attach_92_e_persistence_path_redacts_before_report() {
        // Persistence side redacts even though LLM side allows. The
        // attachment must be Redacted (not Allowed) so the persistence
        // call site picks up the sanitized text.
        let sanitized = "redacted-for-persistence";
        let s = scanner(EgressDecision::Allow, redact(sanitized));
        let decision = s
            .scan(
                "text/plain",
                Some("data with bank card 6225 7600 1234 5678"),
                true,
            )
            .await;
        match decision {
            AttachmentDecision::Redact {
                sanitized_text,
                kinds_redacted,
            } => {
                assert_eq!(sanitized_text, sanitized);
                assert!(kinds_redacted.contains(&EgressKind::Persistence));
            }
            other => panic!("expected Redact (persistence), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn req_attach_92_f_thread_restore_blocks_historical_secret() {
        // When threads.rs replays a historical attachment whose
        // extracted_text still contains a secret, the same scanner
        // call site MUST block. (Same scan API, different caller.)
        let s = scanner(block("historical secret"), EgressDecision::Allow);
        let decision = s
            .scan("application/pdf", Some("legacy key sk-AAA"), true)
            .await;
        assert!(matches!(decision, AttachmentDecision::Block { .. }));
    }

    #[tokio::test]
    async fn req_attach_92_g_dlp_stats_in_metadata_only() {
        // Redact decision carries kinds_redacted (for audit metadata)
        // but never the original payload — only the sanitized copy.
        let sanitized = "***";
        let s = scanner(redact(sanitized), EgressDecision::Allow);
        let decision = s
            .scan("text/plain", Some("original secret payload sk-xxx"), false)
            .await;
        match decision {
            AttachmentDecision::Redact {
                sanitized_text,
                kinds_redacted,
            } => {
                assert_eq!(sanitized_text, sanitized);
                assert!(!sanitized_text.contains("sk-xxx"));
                assert!(kinds_redacted.contains(&EgressKind::LlmRequest));
            }
            other => panic!("expected Redact, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn req_attach_92_h_binary_data_classification() {
        // Unknown binary MIME with payload → Block.
        let s = scanner(EgressDecision::Allow, EgressDecision::Allow);
        let decision = s
            .scan("application/octet-stream", Some("anything"), true)
            .await;
        assert!(
            matches!(decision, AttachmentDecision::Block { .. }),
            "unknown binary MIME must be blocked (fail-safe), got {decision:?}",
        );

        // Whitelist MIME with payload → reaches text gate (Allow here).
        let decision = s.scan("image/png", Some("metadata only"), true).await;
        assert_eq!(decision, AttachmentDecision::Allow);

        // Metadata-only (no binary payload) skips MIME check even for
        // unknown MIME — the text gate still runs.
        let decision = s
            .scan("application/octet-stream", Some("safe text"), false)
            .await;
        assert_eq!(decision, AttachmentDecision::Allow);
    }

    // ── Security regression tests ─────────────────────────────────

    #[tokio::test]
    async fn test_security_attachment_bypass_via_kind_swap() {
        // Attacker tries to slip a secret through by labelling a PDF as
        // image/png. The scanner uses extracted_text content — not the
        // claimed kind — so the gate still fires.
        let s = scanner(block("secret"), EgressDecision::Allow);
        let payload = format!("sk-{}", "Z".repeat(48));
        let decision = s.scan("image/png", Some(&payload), true).await;
        assert!(
            matches!(decision, AttachmentDecision::Block { .. }),
            "claimed-MIME swap must not bypass content scanning",
        );
    }

    #[tokio::test]
    async fn test_security_attachment_bypass_via_oversize() {
        // Even if the upstream extractor truncated input to 100K chars,
        // anything that does reach the scanner is fully checked — the
        // gate does not short-circuit on size. Verify by feeding a
        // large input where the secret is at the tail.
        let mut payload = String::with_capacity(110_000);
        payload.push_str(&"a".repeat(100_000));
        payload.push_str(" sk-XYZ");
        let s = scanner(block("secret at tail"), EgressDecision::Allow);
        let decision = s.scan("text/plain", Some(&payload), false).await;
        assert!(
            matches!(decision, AttachmentDecision::Block { .. }),
            "oversize input must not bypass the gate",
        );
    }

    // ── Composite-decision merge semantics ────────────────────────

    #[tokio::test]
    async fn ask_decision_is_treated_as_block_in_headless_context() {
        let s = scanner(
            EgressDecision::Ask {
                reason: "needs user confirmation".into(),
                suggestions: Vec::new(),
            },
            EgressDecision::Allow,
        );
        let decision = s
            .scan("text/plain", Some("borderline content"), false)
            .await;
        assert!(matches!(decision, AttachmentDecision::Block { .. }));
    }

    #[tokio::test]
    async fn passthrough_combines_to_allow() {
        let s = scanner(EgressDecision::Passthrough, EgressDecision::Passthrough);
        let decision = s.scan("text/plain", Some("ok"), false).await;
        assert_eq!(decision, AttachmentDecision::Allow);
    }

    #[tokio::test]
    async fn no_extracted_text_with_whitelist_mime_allows() {
        // Pure image/audio with no OCR result yet — binary MIME ok,
        // no text to scan.
        let s = scanner(EgressDecision::Allow, EgressDecision::Allow);
        let decision = s.scan("image/jpeg", None, true).await;
        assert_eq!(decision, AttachmentDecision::Allow);
    }
}
