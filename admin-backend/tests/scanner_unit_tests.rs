use admin_backend::scanner::{ScanError, ScanUploadOptions, SecurityVerdict, SkillScanner};
use axum::{routing::post, Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

async fn spawn_scanner_server(response: Value, delay: Duration) -> (String, JoinHandle<()>) {
    let body = Arc::new(response);
    let app = {
        let body = body.clone();
        Router::new().route(
            "/scan-upload",
            post(move || {
                let body = body.clone();
                async move {
                    sleep(delay).await;
                    Json((*body).clone())
                }
            }),
        )
    };

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test scanner server");
    let addr: SocketAddr = listener.local_addr().expect("get test scanner addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve scanner app");
    });

    (format!("http://{}", addr), handle)
}

fn scan_options() -> ScanUploadOptions {
    ScanUploadOptions {
        use_llm: false,
        llm_provider: String::new(),
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        llm_api_version: None,
    }
}

#[tokio::test]
async fn scan_upload_success() {
    let response = json!({
        "scanner_type": "unit-test-scanner",
        "verdict": "SAFE",
        "findings": [],
        "scan_duration_ms": 10
    });
    let (base_url, handle) = spawn_scanner_server(response, Duration::from_millis(0)).await;

    let scanner = SkillScanner::new(&base_url, 500).expect("create scanner client");
    let result = scanner
        .scan_upload(
            "review-checklist",
            "---\nname: review-checklist\n---",
            &scan_options(),
        )
        .await
        .expect("scan should succeed");

    assert_eq!(result.verdict, SecurityVerdict::Safe);
    assert!(result.is_safe);
    assert_eq!(result.findings_count, 0);

    handle.abort();
}

#[tokio::test]
async fn scan_upload_timeout_error() {
    let response = json!({
        "verdict": "SAFE",
        "findings": []
    });
    let (base_url, handle) = spawn_scanner_server(response, Duration::from_millis(200)).await;

    let scanner = SkillScanner::new(&base_url, 50).expect("create scanner client");
    let err = scanner
        .scan_upload(
            "review-checklist",
            "---\nname: review-checklist\n---",
            &scan_options(),
        )
        .await
        .expect_err("scan should timeout");

    assert!(matches!(err, ScanError::Request(_)));

    handle.abort();
}

#[tokio::test]
async fn scan_upload_unreachable_error() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("get ephemeral addr");
    drop(listener);

    let scanner = SkillScanner::new(&format!("http://{}", addr), 100).expect("create scanner");
    let err = scanner
        .scan_upload(
            "review-checklist",
            "---\nname: review-checklist\n---",
            &scan_options(),
        )
        .await
        .expect_err("unreachable scanner should fail");

    assert!(matches!(err, ScanError::Request(_)));
}