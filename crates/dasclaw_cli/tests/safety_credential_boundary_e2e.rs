//! ADR-153 §1.1 e14 — outbound HTTP credential injection at the host
//! boundary.
//!
//! Asserts that `dasclaw_net_tools::HttpTool`, wired with a
//! `SharedCredentialRegistry` + `InMemorySecretsStore`, injects a
//! secret-backed `Authorization: Bearer …` header into the outbound
//! `reqwest` request **before** the HTTP exchange leaves the host
//! process, while keeping the post-W6.4c invariant that recorders /
//! interceptors never observe the raw host-injected secret.
//!
//! The host `HttpTool` rejects loopback (`127.0.0.1`) via its SSRF
//! blocklist, so we cannot stand up a `wiremock` server. Instead we
//! plug a `RecordingHttpInterceptor` into `JobContext.http_interceptor`,
//! which short-circuits the real network call and captures the
//! pre-injection (caller-only) request descriptor. See
//! `crates/dasclaw_net_tools/src/http.rs` ("Canonical credential
//! injection path") for the production path this test exercises.
//!
//! W6.4b-i delivers the green-path assertion. W6.4c landed the
//! caller-headers snapshot fix (issue #840) so that pre-injection
//! headers are what reach recorders, while the production injection
//! still mutates the outbound `reqwest` builder.
//!
//! Cross-cuts: ADR-114 类 A (no new legacy namespace literals introduced);
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

use credential_test_helpers::{RecordingHttpInterceptor, seed_secret, test_secrets_store};

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

    // Assert (post-W6.4c): the interceptor sees the **pre-injection**
    // snapshot, so the host-injected `Authorization: Bearer <secret>` is
    // **not** observable here. The injection still happens on the
    // outbound `reqwest` builder (covered by the LeakDetector-block
    // companion test), but recorders / replay traces never see the raw
    // secret. See ADR-153 §1.1 e14 and issue #840.
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

    let leaked = request.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("authorization") && value.contains(TEST_SECRET_VALUE)
    });
    assert!(
        !leaked,
        "pre-injection snapshot must not contain the raw bearer secret; captured headers = {:?}",
        request.headers
    );
}

/// Red test tracking W6.4c (issue #840): the interceptor (and any other
/// recorder plugged into the same seam) must observe the **caller's**
/// headers, not the post-injection ones, so that trace/replay artifacts
/// and leak detectors cannot accidentally persist the raw secret.
///
/// W6.4c landed the pre-injection `caller_headers_snapshot` in
/// `dasclaw_net_tools::HttpTool`, so `HttpExchangeRequest.headers`
/// observed by interceptors / recorders now contains only caller-supplied
/// headers — never the host-injected bearer secret.
#[tokio::test(flavor = "multi_thread")]
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
    let leaked = request.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("authorization") && value.contains(TEST_SECRET_VALUE)
    });
    assert!(
        !leaked,
        "recorder snapshot must not contain the raw bearer secret; captured headers = {:?}",
        request.headers
    );
}

/// Boundary snapshot (post-W6.4c): when the caller already supplies an
/// `Authorization` header **and** a `CredentialMapping` matches the same
/// host, the host injection path still appends the host header on the
/// outbound wire (so reqwest sees two `Authorization` headers), but the
/// pre-injection snapshot handed to `HttpInterceptor` only carries the
/// caller's header — the host-injected secret never reaches recorders.
///
/// See ADR-153 §1.1 e14 and issue #840.
#[tokio::test(flavor = "multi_thread")]
async fn req_dasclaw_cli_safety_e14_caller_authorization_header_is_preserved_alongside_injection() {
    const CALLER_TOKEN: &str = "callertokenbbbbbbbbbbbbbbbb";
    const HOST_SECRET_NAME: &str = "host_secret";
    const HOST_SECRET_VALUE: &str = "hosttokenaaaaaaaaaaaaaaaaaa";

    let store = test_secrets_store();
    seed_secret(&store, TEST_USER, HOST_SECRET_NAME, HOST_SECRET_VALUE).await;

    let registry = Arc::new(SharedCredentialRegistry::new());
    registry.add_mappings([CredentialMapping::bearer(HOST_SECRET_NAME, TEST_HOST)]);

    let tool = HttpTool::new().with_credentials(registry, store);

    let interceptor = Arc::new(RecordingHttpInterceptor::new("{\"ok\":true}"));
    let mut ctx = JobContext::with_user(TEST_USER, "e14-coexist", "caller + host authz");
    ctx.http_interceptor = Some(interceptor.clone());

    let caller_bearer = format!("Bearer {}", CALLER_TOKEN);
    let args = json!({
        "method": "GET",
        "url": format!("https://{}/v1/things", TEST_HOST),
        "headers": { "Authorization": caller_bearer.clone() },
    });

    let output = tool
        .execute(args, &mut ctx)
        .await
        .expect("HttpTool::execute should succeed under the interceptor short-circuit");
    assert!(
        output.result.get("status").and_then(|s| s.as_u64()) == Some(200),
        "interceptor short-circuit should surface status 200, got {:?}",
        output.result
    );

    let captured = interceptor.captured();
    assert_eq!(
        captured.len(),
        1,
        "interceptor should observe exactly one outbound request, saw {}",
        captured.len()
    );
    let request = &captured[0];

    let auth_values: Vec<&String> = request
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value)
        .collect();
    assert_eq!(
        auth_values.len(),
        1,
        "expected exactly the caller's Authorization in pre-injection snapshot, saw {} (all headers = {:?})",
        auth_values.len(),
        request.headers
    );

    let host_bearer = format!("Bearer {}", HOST_SECRET_VALUE);
    assert!(
        auth_values.iter().any(|v| **v == caller_bearer),
        "caller's Authorization header missing from pre-injection snapshot; auth_values = {:?}",
        auth_values
    );
    assert!(
        auth_values.iter().all(|v| **v != host_bearer),
        "host-injected Authorization must NOT appear in pre-injection snapshot (would leak raw secret); auth_values = {:?}",
        auth_values
    );
}

/// Fail-safe: when the secret bound to a host mapping is shaped like an
/// OpenAI API key (`sk-(?:proj-)?[A-Za-z0-9]{20,}`), the outbound
/// [`LeakDetector::scan_http_request`] **Block**s the request before any
/// network exchange is handed to interceptors / recorders. This pins the
/// ADR-153 §1.1 e14 Fail-Safe contract: the secret never escapes the
/// host, and the interceptor must see zero requests.
#[tokio::test(flavor = "multi_thread")]
async fn req_dasclaw_cli_safety_e14_leak_detector_blocks_openai_shaped_secret_injection() {
    use dasclaw_tool::ToolError;

    const OPENAI_SHAPED_SECRET_NAME: &str = "openai_key";
    const OPENAI_SHAPED_SECRET_VALUE: &str = "sk-proj-test1234567890abcdefghij";

    let store = test_secrets_store();
    seed_secret(
        &store,
        TEST_USER,
        OPENAI_SHAPED_SECRET_NAME,
        OPENAI_SHAPED_SECRET_VALUE,
    )
    .await;

    let registry = Arc::new(SharedCredentialRegistry::new());
    registry.add_mappings([CredentialMapping::bearer(
        OPENAI_SHAPED_SECRET_NAME,
        TEST_HOST,
    )]);

    let tool = HttpTool::new().with_credentials(registry, store);

    let interceptor = Arc::new(RecordingHttpInterceptor::new("{\"ok\":true}"));
    let mut ctx = JobContext::with_user(TEST_USER, "e14-failsafe", "leak detector blocks openai");
    ctx.http_interceptor = Some(interceptor.clone());

    let result = tool
        .execute(
            json!({
                "method": "GET",
                "url": format!("https://{}/v1/things", TEST_HOST),
            }),
            &mut ctx,
        )
        .await;

    let err = result.expect_err(
        "LeakDetector must Block an openai-shaped secret before the outbound exchange leaves the host",
    );
    assert!(
        matches!(err, ToolError::NotAuthorized(_)),
        "expected ToolError::NotAuthorized from LeakDetector Block, got {:?}",
        err
    );

    assert!(
        interceptor.captured().is_empty(),
        "interceptor must not observe any outbound request when LeakDetector Blocks; captured = {:?}",
        interceptor.captured()
    );
}
