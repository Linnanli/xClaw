//! Webhook auth/signature capability descriptor (verbatim port from
//! `desktop-client/ironclaw/src/tools/wasm/capabilities.rs`).
//!
//! Per [ADR-154 §3.5], the `WebhookCapability` struct is a pure 7-field POD
//! (all `Option<String>`) with zero ironclaw dependency, and is moved here
//! verbatim so that `Tool::webhook_capability()` can stop pointing into the
//! ironclaw wasm module. The ironclaw side keeps a `pub use` shim so all
//! existing call sites (including `wasm::capabilities::Capabilities.webhook`)
//! continue to compile.

/// Webhook auth/signature capability configuration for tools.
#[derive(Debug, Clone, Default)]
pub struct WebhookCapability {
    /// Optional header name for shared-secret validation.
    pub secret_header: Option<String>,
    /// Secret name in secrets store for shared-secret validation.
    pub secret_name: Option<String>,
    /// Secret name in secrets store containing Ed25519 public key (Discord-style).
    pub signature_key_secret_name: Option<String>,
    /// Secret name in secrets store for HMAC-SHA256 signing validation.
    pub hmac_secret_name: Option<String>,
    /// Header containing signature (e.g. X-Hub-Signature-256 or X-Slack-Signature).
    pub hmac_signature_header: Option<String>,
    /// Optional timestamp header. When present, Slack-style v0 signature is used.
    pub hmac_timestamp_header: Option<String>,
    /// Optional signature prefix (default: "sha256=" or "v0=" for timestamped mode).
    pub hmac_prefix: Option<String>,
}
