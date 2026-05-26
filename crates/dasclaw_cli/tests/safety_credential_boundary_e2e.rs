//! ADR-153 §1.1 e14 — outbound HTTP credential injection at the host
//! boundary.
//!
//! Asserts that `dasclaw_net_tools::HttpTool`, wired with a
//! `SharedCredentialRegistry` + `InMemorySecretsStore`, injects a
//! secret-backed `Authorization: Bearer …` header into the outbound
//! request **before** the HTTP exchange leaves the host process, and
//! that the caller-supplied `params` JSON never contained the raw
//! secret in the first place.
//!
//! The host `HttpTool` rejects loopback (`127.0.0.1`) via its SSRF
//! blocklist, so we cannot stand up a `wiremock` server. Instead we
//! plug a `RecordingHttpInterceptor` into `JobContext.http_interceptor`,
//! which short-circuits the real network call and captures the
//! post-injection request descriptor. See
//! `crates/dasclaw_net_tools/src/http.rs` ("Canonical credential
//! injection path") for the production path this test exercises.
//!
//! W6.4b-i delivers the green-path assertion. The companion red test
//! `req_dasclaw_cli_safety_e14_recorder_must_not_see_raw_token` is
//! `#[ignore]`-tracked under W6.4c (issue #840), which will land the
//! caller-headers snapshot fix so that pre-injection headers are
//! reported to recorders and leak detectors.
//!
//! Cross-cuts: ADR-114 类 A (no new `.ironclaw` literals);
//! ADR-129 verbatim (tests only — production path untouched);
//! ADR-153 §1.1 e14.

use std::sync::Arc;

use dasclaw_net_tools::HttpTool;
use dasclaw_runtime::Tool;
use dasclaw_runtime::context::JobContext;
use dasclaw_runtime::secrets::CredentialMapping;
use dasclaw_wasm_tools::SharedCredentialRegistry;
use serde_json::json;

#[path = "fixtures/credential_test_helpers.rs"]
mod credential_test_helpers;

use credential_test_helpers::{seed_secret, test_secrets_store, RecordingHttpInterceptor};

const TEST_USER: &str = "test-user-e14";
const TEST_SECRET_NAME: &str = "openai_api_key";
const TEST_SECRET_VALUE: &str = "test-bearer-token-12345";
// Use a literal public IP so the test bypasses DNS entirely: `validate_url`
// accepts IP literals that are not loopback/private, and
// `validate_and_resolve_url` parses the IP without contacting a resolver.
// The interceptor short-circuits the request before any TCP connect, so no
// packet ever leaves the host.
const TEST_HOST: &str = "1.1.1.1";

#[tokio::test(flavor = "multi_thread")]
async fn req_dasclaw_cli_safety_e14_outbound_http_credential_injection_works() {
    // Arrange: secrets store + matching credential mapping for the host.
    let store = test_secrets_store();
    seed_secret(&store, TEST_USER, TEST_SECRET_NAME, TEST_SECRET_VALUE).await;

    let registry = Arc::new(SharedCredentialRegistry::new());
    registry.add_mappings([CredentialMapping::bearer(TEST_SECRET_NAME, TEST_HOST)]);

    let tool = HttpTool::new().with_credentials(registry, store);

    let interceptor = Arc::new(RecordingHttpInterceptor::new("{\"ok\":true}"));
    let mut ctx = JobContext::with_user(TEST_USER, "e14", "outbound credential injection");
    ctx.http_interceptor = Some(interceptor.clone());

    let args = json!({
        "method": "GET",
        "url": format!("https://{}/v1/things", TEST_HOST),
    });

    // Sanity: the caller never put the raw secret in the args.
    let args_str = serde_json::to_string(&args).expect("serialize args");
    assert!(
        !args_str.contains(TEST_SECRET_VALUE),
        "caller args must not embed the raw secret: {}",
        args_str
    );

    // Act: drive the tool through the production injection path.
    let output = tool
        .execute(args, &mut ctx)
        .await
        .expect("HttpTool::execute should succeed under the interceptor short-circuit");
    assert!(
        output.result.get("status").and_then(|s| s.as_u64()) == Some(200),
        "interceptor short-circuit should surface status 200 to the tool result, got {:?}",
        output.result
    );

    // Assert: the host injected `Authorization: Bearer <secret>` before the
    // outbound exchange was handed to the interceptor.
    let captured = interceptor.captured();
    assert_eq!(
        captured.len(),
        1,
        "interceptor should observe exactly one outbound request, saw {}",
        captured.len()
    );
    let request = &captured[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("https://{}/v1/things", TEST_HOST));

    let expected_bearer = format!("Bearer {}", TEST_SECRET_VALUE);
    let injected = request
        .headers
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("authorization") && value == &expected_bearer);
    assert!(
        injected,
        "Authorization: Bearer <secret> header was not injected; captured headers = {:?}",
        request.headers
    );
}

/// Red test tracking W6.4c (issue #840): the interceptor (and any other
/// recorder plugged into the same seam) must observe the **caller's**
/// headers, not the post-injection ones, so that trace/replay artifacts
/// and leak detectors cannot accidentally persist the raw secret.
///
/// Currently `dasclaw_net_tools::HttpTool` snapshots `headers_vec.clone()`
/// for the `HttpExchangeRequest` **after** the injection block has
/// pushed the bearer header, so this assertion fails. Re-enable once
/// W6.4c lands the pre-injection snapshot.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "tracks W6.4c (issue #840): caller-headers snapshot fix for e14"]
async fn req_dasclaw_cli_safety_e14_recorder_must_not_see_raw_token() {
    let store = test_secrets_store();
    seed_secret(&store, TEST_USER, TEST_SECRET_NAME, TEST_SECRET_VALUE).await;

    let registry = Arc::new(SharedCredentialRegistry::new());
    registry.add_mappings([CredentialMapping::bearer(TEST_SECRET_NAME, TEST_HOST)]);

    let tool = HttpTool::new().with_credentials(registry, store);

    let interceptor = Arc::new(RecordingHttpInterceptor::new("{\"ok\":true}"));
    let mut ctx = JobContext::with_user(TEST_USER, "e14-red", "recorder must not see raw token");
    ctx.http_interceptor = Some(interceptor.clone());

    tool.execute(
        json!({
            "method": "GET",
            "url": format!("https://{}/v1/things", TEST_HOST),
        }),
        &mut ctx,
    )
    .await
    .expect("HttpTool::execute should succeed under the interceptor short-circuit");

    let captured = interceptor.captured();
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    let leaked = request
        .headers
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("authorization") && value.contains(TEST_SECRET_VALUE));
    assert!(
        !leaked,
        "recorder snapshot must not contain the raw bearer secret; captured headers = {:?}",
        request.headers
    );
}
