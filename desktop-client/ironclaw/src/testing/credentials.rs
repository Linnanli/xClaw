//! Centralized fake credential constants for tests.
//!
//! All values here are intentionally fake. Centralizing them makes security
//! audits trivial (one file to verify) and eliminates duplication across
//! the test suite.

// WASM-tool-relevant fixtures live upstream in `dasclaw_wasm_tools::test_credentials`
// so the moved wrapper.rs tests do not depend on the desktop crate. We re-export
// them here so existing desktop callers (oauth_defaults, loader, channels, config,
// orchestrator tests) continue to work unchanged.
pub use dasclaw_wasm_tools::test_credentials::{
    TEST_BEARER_TOKEN_123, TEST_CRYPTO_KEY, TEST_GOOGLE_OAUTH_FRESH, TEST_GOOGLE_OAUTH_LEGACY,
    TEST_GOOGLE_OAUTH_TOKEN, TEST_OAUTH_CLIENT_ID, TEST_OAUTH_CLIENT_SECRET,
    TEST_OPENAI_API_KEY_SHORT, test_secrets_store,
};

// ── Encryption keys ──────────────────────────────────────────────────────

/// 32+ char key for web gateway `SecretsCrypto` in tests.
pub const TEST_GATEWAY_CRYPTO_KEY: &str = "test-key-at-least-32-chars-long!!";

// ── OpenAI-style API keys ────────────────────────────────────────────────

/// Generic OpenAI-style test API key.
pub const TEST_OPENAI_API_KEY: &str = "sk-test123";

/// OpenAI API key with longer format (config round-trip tests).
pub const TEST_OPENAI_API_KEY_LONG: &str = "sk-test-key-1234567890";

/// OpenAI API key used in embeddings config issue-129 test.
pub const TEST_OPENAI_API_KEY_ISSUE_129: &str = "sk-test-key-for-issue-129";

// ── Anthropic keys ───────────────────────────────────────────────────────

/// Anthropic OAuth token for config tests.
pub const TEST_ANTHROPIC_OAUTH_TOKEN: &str = "sk-ant-oat01-test-token";

/// Anthropic API key for priority tests.
pub const TEST_ANTHROPIC_API_KEY: &str = "sk-ant-priority-key";

/// Anthropic OAuth token for sandbox config parse tests.
pub const TEST_ANTHROPIC_OAUTH_BASIC: &str = "sk-ant-oat01-basic";

/// Anthropic OAuth token in nested JSON parse test.
pub const TEST_ANTHROPIC_OAUTH_NESTED: &str = "sk-ant-oat01-primary-token";

// ── Google OAuth ─────────────────────────────────────────────────────────
// `TEST_GOOGLE_OAUTH_TOKEN`, `TEST_GOOGLE_OAUTH_FRESH`, `TEST_GOOGLE_OAUTH_LEGACY`
// are re-exported from `dasclaw_wasm_tools::test_credentials` at the top.

// ── GitHub ───────────────────────────────────────────────────────────────

/// GitHub personal access token (test).
pub const TEST_GITHUB_TOKEN: &str = "ghp_test123";

// ── Telegram ────────────────────────────────────────────────────────────

/// Telegram bot token for credential redaction tests.
pub const TEST_TELEGRAM_BOT_TOKEN: &str = "telegram-test-bot-token-not-a-real-token";

// ── OAuth client credentials ────────────────────────────────────────────
// `TEST_OAUTH_CLIENT_ID`, `TEST_OAUTH_CLIENT_SECRET` are re-exported from
// `dasclaw_wasm_tools::test_credentials` at the top.

// ── Bearer/auth tokens ──────────────────────────────────────────────────

/// Generic test bearer token.
pub const TEST_BEARER_TOKEN: &str = "test-token";

// `TEST_BEARER_TOKEN_123` is re-exported from `dasclaw_wasm_tools::test_credentials`
// at the top.

/// Auth token used by web gateway middleware tests.
pub const TEST_AUTH_SECRET_TOKEN: &str = "secret-token";

// ── Stripe ──────────────────────────────────────────────────────────────

/// Stripe-style test key.
pub const TEST_STRIPE_KEY: &str = "sk_test_fake123";

// ── Redaction test values ───────────────────────────────────────────────

/// Secret-prefixed key for redaction/sanitization tests.
pub const TEST_REDACT_SECRET: &str = "sk-secret";

/// Secret-prefixed key with suffix for redaction tests.
pub const TEST_REDACT_SECRET_123: &str = "sk-secret-123";

// ── Session tokens ──────────────────────────────────────────────────────

/// Generic session token for persistence tests.
pub const TEST_SESSION_TOKEN: &str = "test_token_123";

/// NEAR AI session token variant A.
pub const TEST_SESSION_NEARAI_ABC: &str = "sess_abc123";

/// NEAR AI session token variant B.
pub const TEST_SESSION_NEARAI_XYZ: &str = "sess_xyz789";

// ── Generic ──────────────────────────────────────────────────────────────

/// Generic test API key for LLM config, embedding config, nearai tests.
pub const TEST_API_KEY: &str = "test-key";

/// Stored secret value for create-and-get tests.
pub const TEST_SECRET_VALUE: &str = "sk-test-12345";

/// HTTP webhook secret for channel tests.
pub const TEST_HTTP_SECRET: &str = "test-secret-123";

// `test_secrets_store` is re-exported from `dasclaw_wasm_tools::test_credentials`
// at the top.
