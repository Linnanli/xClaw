//! Shared test helpers for outbound HTTP credential-injection tests.
//!
//! Backs `tests/safety_credential_boundary_e2e.rs` (ADR-153 §1.1 e14,
//! W6.4b-i). The host `HttpTool` SSRF blocklist rejects loopback
//! (`127.0.0.1`) before any request is dispatched, so `wiremock` cannot
//! be used here. Instead we provide a `RecordingHttpInterceptor` that
//! satisfies the `HttpInterceptor` short-circuit contract on the
//! `JobContext` and captures the post-injection `HttpExchangeRequest`
//! before any TLS handshake occurs.
//!
//! Cross-cuts: ADR-114 类 A (no new `.ironclaw` literals);
//! ADR-129 verbatim discipline (no behaviour changes to the production
//! injection path under test).

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use secrecy::SecretString;

use dasclaw_runtime::recording::{HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor};
use dasclaw_runtime::secrets::{
    CreateSecretParams, InMemorySecretsStore, SecretsCrypto, SecretsStore,
};

/// Crypto key used by the in-memory secrets store under test.
///
/// `SecretsCrypto::new` rejects keys shorter than 32 bytes, so this is
/// exactly 32 ASCII bytes.
const TEST_MASTER_KEY: &str = "0123456789abcdef0123456789abcdef";

/// Build a fresh `InMemorySecretsStore` wired to a 32-byte test master key.
pub fn test_secrets_store() -> Arc<InMemorySecretsStore> {
    let crypto =
        SecretsCrypto::new(SecretString::from(TEST_MASTER_KEY.to_string())).expect(
            "32-byte master key must satisfy SecretsCrypto::new minimum-length check",
        );
    Arc::new(InMemorySecretsStore::new(Arc::new(crypto)))
}

/// Seed a single secret on the store for `user_id`.
///
/// `CreateSecretParams::new` lowercases the secret name, so callers
/// should pass `name` already in the lowercase form that
/// `CredentialMapping::bearer(name, ...)` will look up.
pub async fn seed_secret(
    store: &Arc<InMemorySecretsStore>,
    user_id: &str,
    name: &str,
    value: &str,
) {
    let store: &dyn SecretsStore = store.as_ref();
    store
        .create(user_id, CreateSecretParams::new(name, value))
        .await
        .expect("seeding a fresh secret in InMemorySecretsStore must succeed");
}

/// `HttpInterceptor` that captures every outbound request and short-circuits
/// the real network with a canned 200 response.
///
/// The recorded `HttpExchangeRequest` carries the URL, method, headers
/// and body **after** the production injection block has appended any
/// matched credentials (see `crates/dasclaw_net_tools/src/http.rs`,
/// "Canonical credential injection path"). Tests read `captured()` to
/// assert injection happened on the host boundary.
#[derive(Debug)]
pub struct RecordingHttpInterceptor {
    captured: Mutex<Vec<HttpExchangeRequest>>,
    response_body: String,
}

impl RecordingHttpInterceptor {
    pub fn new(response_body: impl Into<String>) -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
            response_body: response_body.into(),
        }
    }

    /// Snapshot the requests captured so far.
    pub fn captured(&self) -> Vec<HttpExchangeRequest> {
        self.captured
            .lock()
            .expect("RecordingHttpInterceptor mutex poisoned")
            .clone()
    }
}

#[async_trait]
impl HttpInterceptor for RecordingHttpInterceptor {
    async fn before_request(
        &self,
        request: &HttpExchangeRequest,
    ) -> Option<HttpExchangeResponse> {
        self.captured
            .lock()
            .expect("RecordingHttpInterceptor mutex poisoned")
            .push(request.clone());
        Some(HttpExchangeResponse {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: self.response_body.clone(),
        })
    }

    async fn after_response(
        &self,
        _request: &HttpExchangeRequest,
        _response: &HttpExchangeResponse,
    ) {
    }
}
