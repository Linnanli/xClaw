//! Centralized fake credential constants for WASM tool tests.
//!
//! All values here are intentionally fake. Centralizing them makes security
//! audits trivial (one file to verify) and eliminates duplication across
//! the test suite.
//!
//! These fixtures were originally defined in
//! `desktop-client/ironclaw/src/testing/credentials.rs`; the WASM-tool-relevant
//! subset is hoisted here so that `crates/dasclaw_wasm_tools/src/wrapper.rs`
//! tests do not depend on the desktop crate. The downstream `testing/credentials`
//! module re-exports these symbols so existing desktop callers continue to work
//! unchanged.

use std::sync::Arc;

use secrecy::SecretString;

use dasclaw_runtime::secrets::{InMemorySecretsStore, SecretsCrypto};

/// 32-character key string for `SecretsCrypto::new()` in tests.
pub const TEST_CRYPTO_KEY: &str = "0123456789abcdef0123456789abcdef";

// ── Google OAuth ─────────────────────────────────────────────────────────

/// Google OAuth access token (standard test).
pub const TEST_GOOGLE_OAUTH_TOKEN: &str = "ya29.test-token";

/// Google OAuth access token (fresh/non-expired variant).
pub const TEST_GOOGLE_OAUTH_FRESH: &str = "ya29.fresh-token";

/// Google OAuth access token (legacy/no-expiry variant).
pub const TEST_GOOGLE_OAUTH_LEGACY: &str = "ya29.legacy-token";

// ── OAuth client credentials ────────────────────────────────────────────

/// OAuth client ID for token refresh tests.
pub const TEST_OAUTH_CLIENT_ID: &str = "test-client-id";

/// OAuth client secret for token refresh tests.
pub const TEST_OAUTH_CLIENT_SECRET: &str = "test-client-secret";

// ── Bearer/auth tokens ──────────────────────────────────────────────────

/// Bearer token with suffix (wasm wrapper credential injection).
pub const TEST_BEARER_TOKEN_123: &str = "test-token-123";

// ── Helpers ──────────────────────────────────────────────────────────────

/// Create an `InMemorySecretsStore` backed by [`TEST_CRYPTO_KEY`].
///
/// Replaces the duplicated `test_store()` pattern found across multiple
/// test modules.
pub fn test_secrets_store() -> InMemorySecretsStore {
    // safety: test-only helper with a 32-byte hard-coded key; cannot fail.
    let crypto =
        Arc::new(SecretsCrypto::new(SecretString::from(TEST_CRYPTO_KEY.to_string())).unwrap());
    InMemorySecretsStore::new(crypto)
}
