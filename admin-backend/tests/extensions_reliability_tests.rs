mod common;

use axum::http::StatusCode;
use common::{
    assert_status_ok, build_app, configure_scanner_env, get, post_json, response_json,
    spawn_scanner_server, try_connect_db,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_invalid_client_token_recovery() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let resp = get(
        build_app(pool),
        "/api/v1/search?q=&client_token=not-a-valid-uuid-token",
    )
    .await;
    assert_status_ok(resp.status());

    let body = response_json(resp).await;
    assert!(
        body.get("results").and_then(|v| v.as_array()).is_some(),
        "降级路径下仍应返回 results 数组"
    );
}

#[tokio::test]
async fn test_missing_scan_result_recovery() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = uuid::Uuid::new_v4();
    let path = format!("/api/skills/{}/scan-results", skill_id);

    let resp1 = get(build_app(pool.clone()), &path).await;
    assert_status_ok(resp1.status());
    let body1 = response_json(resp1).await;
    assert!(body1["scan_result"].is_null());

    let resp2 = get(build_app(pool), &path).await;
    assert_status_ok(resp2.status());
    let body2 = response_json(resp2).await;
    assert!(body2["scan_result"].is_null());
}

#[tokio::test]
async fn test_rescan_returns_400_when_scanner_disabled() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let _scanner_guard = configure_scanner_env(false, None, None);

    let skill_id = Uuid::new_v4();
    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'reliability', '1.0.0', 'reliability', $3, false, 'admin_upload',
                'scan_failed', false, 0, NOW(), NOW())",
            &[
                &skill_id,
                &format!("reliability_rescan_{}", skill_id.simple()),
                &"---\nname: reliability\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - abc\n---\n# skill",
            ],
        )
        .await
        .expect("seed skill");

    let path = format!("/api/skills/{}/rescan", skill_id);
    let resp = common::post_json(build_app(pool.clone()), &path, json!({})).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = response_json(resp).await;
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("未启用"), "响应应明确告知扫描器未启用");

    let row = client
        .query_one(
            "SELECT review_status, enabled FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query skill");
    let review_status: String = row.get(0);
    let enabled: bool = row.get(1);
    assert_eq!(review_status, "scan_failed");
    assert!(!enabled);

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn test_rescan_timeout_returns_error_without_state_change() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_content = format!(
        "---\nname: reliability_timeout_{}\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - abc\n---\n# skill",
        skill_id.simple()
    );

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'reliability timeout', '1.0.0', 'reliability', $3, true, 'admin_upload',
                     'scan_failed', false, 0, NOW(), NOW())",
            &[
                &skill_id,
                &format!("reliability_timeout_{}", skill_id.simple()),
                &skill_content,
            ],
        )
        .await
        .expect("seed skill");

    let (scanner_url, handle) = spawn_scanner_server(
        json!({"verdict": "SAFE", "findings": []}),
        200,
    )
    .await;
    let _scanner_guard = configure_scanner_env(true, Some(&scanner_url), Some(50));

    let path = format!("/api/skills/{}/rescan", skill_id);
    let resp = post_json(build_app(pool.clone()), &path, json!({})).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = response_json(resp).await;
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("安全扫描服务未启动或不可用"));

    let row = client
        .query_one(
            "SELECT review_status, enabled FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query skill");
    let review_status: String = row.get(0);
    let enabled: bool = row.get(1);
    assert_eq!(review_status, "scan_failed");
    assert!(enabled);

    let scan_count_row = client
        .query_one(
            "SELECT COUNT(*) FROM scan_results WHERE target_type = 'skill' AND target_id = $1",
            &[&skill_id],
        )
        .await
        .expect("query scan result count");
    let scan_count: i64 = scan_count_row.get(0);
    assert_eq!(scan_count, 0);

    handle.abort();
    client
        .execute("DELETE FROM scan_results WHERE target_id = $1", &[&skill_id])
        .await
        .expect("cleanup scan result");
    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}
