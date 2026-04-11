mod common;

use axum::http::StatusCode;
use chrono::Utc;
use common::{
    build_app, configure_scanner_env, get, post_json, response_json, spawn_scanner_server,
    try_connect_db, unique_name,
};
use serde_json::json;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[tokio::test]
async fn req_extensions_001_upload_valid_skill_returns_201_and_persists() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let _scanner_guard = configure_scanner_env(false, None, None);
    let skill_name = unique_name("it_skill_upload");
    let content = format!(
        "---\nname: {}\nversion: 1.0.0\ndescription: 集成测试技能\nactivation:\n  keywords:\n    - integration\nkeywords:\n  - integration\n---\n# Integration Skill",
        skill_name
    );

    let resp = post_json(
        build_app(pool.clone()),
        "/api/skills/upload",
        json!({
            "name": skill_name,
            "content": content,
            "version": "1.0.0",
            "description": "集成测试技能",
            "author": "integration-test"
        }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = response_json(resp).await;
    assert_eq!(body["review_status"], "pending");

    let client = pool.get().await.expect("get db client");
    let row = client
        .query_one(
            "SELECT review_status, file_path FROM skills WHERE name = $1",
            &[&body["name"].as_str().unwrap_or_default()],
        )
        .await
        .expect("query inserted skill");

    let review_status: String = row.get(0);
    let file_path: Option<String> = row.get(1);
    assert_eq!(review_status, "pending");
    assert!(
        file_path.unwrap_or_default().contains("# Integration Skill"),
        "file_path 应保存上传内容"
    );

    client
        .execute(
            "DELETE FROM skills WHERE name = $1",
            &[&body["name"].as_str().unwrap_or_default()],
        )
        .await
        .expect("cleanup inserted skill");
}

#[tokio::test]
async fn req_extensions_001b_upload_with_scanner_enabled_returns_scanning_then_pending() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let (scanner_url, handle) = spawn_scanner_server(
        json!({
            "scanner_type": "integration-scanner",
            "verdict": "SAFE",
            "is_safe": true,
            "findings_count": 0,
            "findings": [],
            "scan_duration_ms": 6
        }),
        0,
    )
    .await;
    let _scanner_guard = configure_scanner_env(true, Some(&scanner_url), Some(500));

    let skill_name = unique_name("it_skill_upload_async");
    let content = format!(
        "---\nname: {}\nversion: 1.0.0\ndescription: 异步扫描测试\nactivation:\n  keywords:\n    - async\nkeywords:\n  - async\n---\n# Async Skill",
        skill_name
    );

    let resp = post_json(
        build_app(pool.clone()),
        "/api/skills/upload",
        json!({
            "name": skill_name,
            "content": content,
            "version": "1.0.0",
            "description": "异步扫描测试",
            "author": "integration-test"
        }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = response_json(resp).await;
    assert_eq!(body["review_status"], "scanning");

    let skill_id = Uuid::parse_str(body["id"].as_str().unwrap_or_default())
        .expect("response should include valid skill id");
    let client = pool.get().await.expect("get db client");

    let mut final_status = String::new();
    for _ in 0..20 {
        let row = client
            .query_one("SELECT review_status FROM skills WHERE id = $1", &[&skill_id])
            .await
            .expect("query uploaded skill status");
        final_status = row.get::<_, String>(0);

        if final_status != "scanning" {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    assert_eq!(final_status, "pending");

    let scan_row = client
        .query_one(
            "SELECT verdict, is_safe, findings_count
             FROM scan_results
             WHERE target_type = 'skill' AND target_id = $1
             ORDER BY created_at DESC
             LIMIT 1",
            &[&skill_id],
        )
        .await
        .expect("query async scan result");
    let verdict: String = scan_row.get(0);
    let is_safe: bool = scan_row.get(1);
    let findings_count: i32 = scan_row.get(2);

    assert_eq!(verdict, "SAFE");
    assert!(is_safe);
    assert_eq!(findings_count, 0);

    handle.abort();
    client
        .execute("DELETE FROM scan_results WHERE target_id = $1", &[&skill_id])
        .await
        .expect("cleanup scan results");
    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup uploaded skill");
}

#[tokio::test]
async fn req_extensions_002_get_scan_results_returns_latest_record() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let old_id = Uuid::new_v4();
    let new_id = Uuid::new_v4();
    let now = Utc::now();
    let old_time = now - chrono::Duration::seconds(20);

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO scan_results (
                id, target_type, target_id, scanner_type, verdict, is_safe,
                max_severity, findings_count, findings, scan_duration_ms, scanned_at, created_at
             ) VALUES ($1, 'skill', $2, 'unit-scanner', 'SAFE', true, 'LOW', 0, $3::jsonb, 10, $4, $4)",
            &[&old_id, &skill_id, &json!([]), &old_time],
        )
        .await
        .expect("seed old scan result");

    client
        .execute(
            "INSERT INTO scan_results (
                id, target_type, target_id, scanner_type, verdict, is_safe,
                max_severity, findings_count, findings, scan_duration_ms, scanned_at, created_at
             ) VALUES ($1, 'skill', $2, 'unit-scanner', 'BLOCKED', false, 'HIGH', 2, $3::jsonb, 20, $4, $4)",
            &[&new_id, &skill_id, &json!([{"severity":"HIGH"}]), &now],
        )
        .await
        .expect("seed scan results");

    let path = format!("/api/skills/{}/scan-results", skill_id);
    let resp = get(build_app(pool.clone()), &path).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    assert_eq!(body["skill_id"], skill_id.to_string());
    assert_eq!(body["scan_result"]["verdict"], "BLOCKED");
    assert_eq!(body["scan_result"]["findings_count"], 2);

    client
        .execute("DELETE FROM scan_results WHERE target_id = $1", &[&skill_id])
        .await
        .expect("cleanup scan results");
}

#[tokio::test]
async fn req_extensions_003_yank_skill_sets_status_and_disables() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("it_yank_skill");
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES ($1, $2, 'integration yank', '1.0.0', 'integration', true,
                       'admin_upload', 'approved', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name],
        )
        .await
        .expect("seed skill");

    let path = format!("/api/skills/{}/yank", skill_id);
    let resp = post_json(
        build_app(pool.clone()),
        &path,
        json!({"note": "security incident"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    assert_eq!(body["review_status"], "yanked");
    assert_eq!(body["enabled"], false);

    let row = client
        .query_one(
            "SELECT review_status, enabled, review_note FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query yanked skill");
    let review_status: String = row.get(0);
    let enabled: bool = row.get(1);
    let review_note: Option<String> = row.get(2);

    assert_eq!(review_status, "yanked");
    assert!(!enabled);
    assert_eq!(review_note.as_deref(), Some("security incident"));

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn req_extensions_004_rescan_safe_marks_pending_and_stores_scan_result() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("it_rescan_safe");
    let skill_content = format!(
        "---\nname: {}\nversion: 1.0.0\ndescription: rescan safe\nactivation:\n  keywords:\n    - safe\n---\n# safe",
        skill_name
    );

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'rescan safe', '1.0.0', 'integration', $3, false, 'admin_upload',
                'scan_failed', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name, &skill_content],
        )
        .await
        .expect("seed skill");

    let (scanner_url, handle) = spawn_scanner_server(
        json!({
            "scanner_type": "integration-scanner",
            "verdict": "SAFE",
            "is_safe": true,
            "findings_count": 0,
            "findings": [],
            "scan_duration_ms": 8
        }),
        0,
    )
    .await;
    let _scanner_guard = configure_scanner_env(true, Some(&scanner_url), Some(500));

    let path = format!("/api/skills/{}/rescan", skill_id);
    let resp = post_json(build_app(pool.clone()), &path, json!({})).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    assert_eq!(body["review_status"], "pending");
    assert_eq!(body["enabled"], false);
    assert_eq!(body["is_safe"], true);

    let skill_row = client
        .query_one(
            "SELECT review_status, enabled FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query skill after rescan");
    let review_status: String = skill_row.get(0);
    let enabled: bool = skill_row.get(1);
    assert_eq!(review_status, "pending");
    assert!(!enabled);

    let scan_row = client
        .query_one(
            "SELECT verdict, is_safe, findings_count
             FROM scan_results
             WHERE target_type = 'skill' AND target_id = $1
             ORDER BY created_at DESC
             LIMIT 1",
            &[&skill_id],
        )
        .await
        .expect("query latest scan result");
    let verdict: String = scan_row.get(0);
    let is_safe: bool = scan_row.get(1);
    let findings_count: i32 = scan_row.get(2);
    assert_eq!(verdict, "SAFE");
    assert!(is_safe);
    assert_eq!(findings_count, 0);

    handle.abort();
    client
        .execute("DELETE FROM scan_results WHERE target_id = $1", &[&skill_id])
        .await
        .expect("cleanup scan results");
    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn req_extensions_005_rescan_blocked_keeps_scan_failed_and_disables_skill() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("it_rescan_blocked");
    let skill_content = format!(
        "---\nname: {}\nversion: 1.0.0\ndescription: rescan blocked\nactivation:\n  keywords:\n    - blocked\n---\n# blocked",
        skill_name
    );

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'rescan blocked', '1.0.0', 'integration', $3, true, 'admin_upload',
                'scan_failed', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name, &skill_content],
        )
        .await
        .expect("seed skill");

    let (scanner_url, handle) = spawn_scanner_server(
        json!({
            "scanner_type": "integration-scanner",
            "verdict": "BLOCKED",
            "is_safe": false,
            "findings_count": 1,
            "findings": [{"rule_id": "R1", "severity": "HIGH"}],
            "scan_duration_ms": 9
        }),
        0,
    )
    .await;
    let _scanner_guard = configure_scanner_env(true, Some(&scanner_url), Some(500));

    let path = format!("/api/skills/{}/rescan", skill_id);
    let resp = post_json(build_app(pool.clone()), &path, json!({})).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    assert_eq!(body["review_status"], "scan_failed");
    assert_eq!(body["enabled"], false);
    assert_eq!(body["is_safe"], false);
    assert_eq!(body["findings_count"], 1);

    let row = client
        .query_one(
            "SELECT review_status, enabled FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query skill after blocked rescan");
    let review_status: String = row.get(0);
    let enabled: bool = row.get(1);
    assert_eq!(review_status, "scan_failed");
    assert!(!enabled);

    handle.abort();
    client
        .execute("DELETE FROM scan_results WHERE target_id = $1", &[&skill_id])
        .await
        .expect("cleanup scan results");
    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn req_extensions_006_get_plugin_scan_results_returns_latest_record() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let plugin_id = Uuid::new_v4();
    let old_id = Uuid::new_v4();
    let new_id = Uuid::new_v4();
    let now = Utc::now();
    let old_time = now - chrono::Duration::seconds(10);

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO scan_results (
                id, target_type, target_id, scanner_type, verdict, is_safe,
                max_severity, findings_count, findings, scan_duration_ms, scanned_at, created_at
             ) VALUES ($1, 'plugin', $2, 'unit-scanner', 'SAFE', true, 'LOW', 0, $3::jsonb, 8, $4, $4)",
            &[&old_id, &plugin_id, &json!([]), &old_time],
        )
        .await
        .expect("seed old plugin scan result");

    client
        .execute(
            "INSERT INTO scan_results (
                id, target_type, target_id, scanner_type, verdict, is_safe,
                max_severity, findings_count, findings, scan_duration_ms, scanned_at, created_at
             ) VALUES ($1, 'plugin', $2, 'unit-scanner', 'BLOCKED', false, 'HIGH', 1, $3::jsonb, 12, $4, $4)",
            &[&new_id, &plugin_id, &json!([{"severity":"HIGH"}]), &now],
        )
        .await
        .expect("seed new plugin scan result");

    let path = format!("/api/plugins/{}/scan-results", plugin_id);
    let resp = get(build_app(pool.clone()), &path).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    assert_eq!(body["plugin_id"], plugin_id.to_string());
    assert_eq!(body["scan_result"]["verdict"], "BLOCKED");
    assert_eq!(body["scan_result"]["findings_count"], 1);

    client
        .execute("DELETE FROM scan_results WHERE target_id = $1", &[&plugin_id])
        .await
        .expect("cleanup plugin scan results");
}

#[tokio::test]
async fn req_extensions_007_yank_plugin_sets_status_and_disables() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let plugin_id = Uuid::new_v4();
    let plugin_name = unique_name("it_yank_plugin");
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO plugins (
                id, name, description, version, author, enabled, source,
                review_status, plugin_type, is_builtin, invoke_count, requires_sandbox,
                created_at, updated_at
             ) VALUES (
                $1, $2, 'integration yank plugin', '1.0.0', 'integration', true,
                'admin_upload', 'approved', 'http', false, 0, false, NOW(), NOW())",
            &[&plugin_id, &plugin_name],
        )
        .await
        .expect("seed plugin");

    let path = format!("/api/plugins/{}/yank", plugin_id);
    let resp = post_json(
        build_app(pool.clone()),
        &path,
        json!({"note": "plugin security incident"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    assert_eq!(body["review_status"], "yanked");
    assert_eq!(body["enabled"], false);

    let row = client
        .query_one(
            "SELECT review_status, enabled, review_note FROM plugins WHERE id = $1",
            &[&plugin_id],
        )
        .await
        .expect("query yanked plugin");
    let review_status: String = row.get(0);
    let enabled: bool = row.get(1);
    let review_note: Option<String> = row.get(2);

    assert_eq!(review_status, "yanked");
    assert!(!enabled);
    assert_eq!(review_note.as_deref(), Some("plugin security incident"));

    client
        .execute("DELETE FROM plugins WHERE id = $1", &[&plugin_id])
        .await
        .expect("cleanup plugin");
}
