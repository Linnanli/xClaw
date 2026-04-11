mod common;

use axum::http::StatusCode;
use common::{build_app, post_json, response_json, try_connect_db, unique_name};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_failure_rescan_skill_not_found() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let missing_skill_id = Uuid::new_v4();
    let path = format!("/api/skills/{}/rescan", missing_skill_id);
    let resp = post_json(build_app(pool), &path, json!({})).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = response_json(resp).await;
    assert_eq!(body["error"], "Resource not found");
}

#[tokio::test]
async fn test_failure_rescan_missing_file_content_rejected() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("failure_rescan_empty");
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'failure', '1.0.0', 'failure-test', NULL, false, 'admin_upload',
                'scan_failed', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name],
        )
        .await
        .expect("seed skill");

    let path = format!("/api/skills/{}/rescan", skill_id);
    let resp = post_json(build_app(pool.clone()), &path, json!({})).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_json(resp).await;
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("技能内容为空"));

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn test_failure_yank_conflict_when_already_yanked() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("failure_yank_conflict");
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'failure', '1.0.0', 'failure-test', false, 'admin_upload',
                'yanked', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name],
        )
        .await
        .expect("seed yanked skill");

    let path = format!("/api/skills/{}/yank", skill_id);
    let resp = post_json(build_app(pool.clone()), &path, json!({"note": "repeat"})).await;

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = response_json(resp).await;
    assert_eq!(body["error"], "Resource conflict");

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn test_failure_upload_rejects_patterns_over_limit() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let content = "---\nname: invalid-patterns\nversion: 1.0.0\ndescription: too many patterns\nactivation:\n  keywords:\n    - trigger\npatterns:\n  - p1\n  - p2\n  - p3\n  - p4\n  - p5\n  - p6\n---\n# body";
    let resp = post_json(
        build_app(pool),
        "/api/skills/upload",
        json!({
            "name": unique_name("invalid_patterns"),
            "content": content,
            "version": "1.0.0",
            "description": "test",
            "author": "failure-test"
        }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_json(resp).await;
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("patterns 最多允许"));
}

#[tokio::test]
async fn test_failure_upload_rejects_tags_over_limit() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let content = "---\nname: invalid-tags\nversion: 1.0.0\ndescription: too many tags\nactivation:\n  keywords:\n    - trigger\ntags:\n  - t1\n  - t2\n  - t3\n  - t4\n  - t5\n  - t6\n  - t7\n  - t8\n  - t9\n  - t10\n  - t11\n---\n# body";
    let resp = post_json(
        build_app(pool),
        "/api/skills/upload",
        json!({
            "name": unique_name("invalid_tags"),
            "content": content,
            "version": "1.0.0",
            "description": "test",
            "author": "failure-test"
        }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_json(resp).await;
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("tags 最多允许"));
}

#[tokio::test]
async fn test_failure_rescan_rejects_non_scan_failed_status() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("failure_rescan_status_guard");
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'failure', '1.0.0', 'failure-test', '# content', false,
                'admin_upload', 'pending', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name],
        )
        .await
        .expect("seed pending skill");

    let path = format!("/api/skills/{}/rescan", skill_id);
    let resp = post_json(build_app(pool.clone()), &path, json!({})).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_json(resp).await;
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("仅 scan_failed 状态的技能可以重扫"));

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}
