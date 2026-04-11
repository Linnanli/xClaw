mod common;

use axum::http::StatusCode;
use chrono::Utc;
use common::{
    build_app, configure_scanner_env, get, post_json, post_json_with_headers, response_json,
    try_connect_db, unique_name,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_audit_reviewer_id_is_persisted_from_header() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("sec_review_skill");
    let reviewer_id = Uuid::new_v4();
    let reviewer_username = unique_name("sec_reviewer");
    let reviewer_email = format!("{}@example.com", reviewer_username);
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, created_at, updated_at)
             VALUES ($1, $2, $3, 'hash', $4, $4)",
            &[&reviewer_id, &reviewer_username, &reviewer_email, &Utc::now()],
        )
        .await
        .expect("seed reviewer user");

    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES ($1, $2, 'for security audit', '1.0.0', 'security-test', false,
                       'admin_upload', 'pending', false, 0, $3, $3)",
            &[&skill_id, &skill_name, &Utc::now()],
        )
        .await
        .expect("seed pending skill");

    let path = format!("/api/skills/{}/review", skill_id);
    let reviewer_header_value = reviewer_id.to_string();
    let resp = post_json_with_headers(
        build_app(pool.clone()),
        &path,
        json!({"approved": true, "note": "security review passed"}),
        &[("x-admin-user-id", reviewer_header_value.as_str())],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    let serialized = serde_json::to_string(&body).expect("serialize response body");
    assert!(
        !serialized.contains(&reviewer_id.to_string()),
        "API 响应不应回显 reviewer_id"
    );

    let row = client
        .query_one(
            "SELECT reviewed_by FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query reviewed skill");
    let reviewed_by: Option<Uuid> = row.get(0);
    assert_eq!(reviewed_by, Some(reviewer_id));

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
    client
        .execute("DELETE FROM users WHERE id = $1", &[&reviewer_id])
        .await
        .expect("cleanup reviewer user");
}

#[tokio::test]
async fn test_audit_validation_error_does_not_leak_raw_secret_content() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    std::env::set_var("SCANNER_ENABLED", "false");
    let secret_marker = "TOP_SECRET_MARKER_42";
    let invalid_content = format!(
        "---\nname: broken-skill\nversion: 1.0.0\ndescription: bad yaml\nactivation:\n  keywords: [a\n---\n{}",
        secret_marker
    );

    let resp = post_json(
        build_app(pool),
        "/api/skills/upload",
        json!({
            "name": "broken-skill",
            "content": invalid_content,
            "version": "1.0.0",
            "description": "bad",
            "author": "security-test"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = response_json(resp).await;
    let body_text = serde_json::to_string(&body).expect("serialize response body");
    assert!(
        !body_text.contains(secret_marker),
        "错误响应不应泄露原始敏感内容"
    );
}

#[tokio::test]
async fn test_audit_yanked_skill_not_downloadable_and_no_content_leak() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let skill_id = Uuid::new_v4();
    let skill_name = unique_name("sec_yanked_skill");
    let secret_marker = "LEAK_MARKER_DOWNLOAD_77";
    let skill_content = format!(
        "---\nname: {}\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - audit\n---\n{}",
        skill_name, secret_marker
    );

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO skills (
                id, name, description, version, author, file_path, enabled, source,
                review_status, is_builtin, invoke_count, created_at, updated_at
             ) VALUES (
                $1, $2, 'security', '1.0.0', 'security-test', $3, false, 'admin_upload',
                'yanked', false, 0, NOW(), NOW())",
            &[&skill_id, &skill_name, &skill_content],
        )
        .await
        .expect("seed yanked skill");

    let path = format!("/api/v1/download?slug={}", skill_id);
    let resp = get(build_app(pool.clone()), &path).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body = response_json(resp).await;
    let body_text = serde_json::to_string(&body).expect("serialize response body");
    assert!(
        !body_text.contains(secret_marker),
        "下载被拒绝时不应泄露技能原始内容"
    );

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
}

#[tokio::test]
async fn test_audit_uploader_id_is_persisted_from_header() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let _scanner_guard = configure_scanner_env(false, None, None);
    let uploader_id = Uuid::new_v4();
    let uploader_username = unique_name("sec_uploader");
    let uploader_email = format!("{}@example.com", uploader_username);
    let skill_name = unique_name("sec_upload_skill");
    let skill_content = format!(
        "---\nname: {}\nversion: 1.0.0\ndescription: x\nactivation:\n  keywords:\n    - upload\n---\n# skill",
        skill_name
    );

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, created_at, updated_at)
             VALUES ($1, $2, $3, 'hash', $4, $4)",
            &[&uploader_id, &uploader_username, &uploader_email, &Utc::now()],
        )
        .await
        .expect("seed uploader user");

    let uploader_header = uploader_id.to_string();
    let resp = post_json_with_headers(
        build_app(pool.clone()),
        "/api/skills/upload",
        json!({
            "name": skill_name,
            "content": skill_content,
            "version": "1.0.0",
            "description": "security",
            "author": "security-test"
        }),
        &[("x-admin-user-id", uploader_header.as_str())],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = response_json(resp).await;
    let skill_id = Uuid::parse_str(body["id"].as_str().unwrap_or_default())
        .expect("response should contain valid skill id");

    let serialized = serde_json::to_string(&body).expect("serialize response body");
    assert!(
        !serialized.contains(&uploader_id.to_string()),
        "API 响应不应回显 uploader_id"
    );

    let row = client
        .query_one(
            "SELECT uploaded_by FROM skills WHERE id = $1",
            &[&skill_id],
        )
        .await
        .expect("query uploaded skill");
    let uploaded_by: Option<Uuid> = row.get(0);
    assert_eq!(uploaded_by, Some(uploader_id));

    client
        .execute("DELETE FROM skills WHERE id = $1", &[&skill_id])
        .await
        .expect("cleanup skill");
    client
        .execute("DELETE FROM users WHERE id = $1", &[&uploader_id])
        .await
        .expect("cleanup uploader user");
}
