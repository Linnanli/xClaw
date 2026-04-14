mod common;

use axum::http::StatusCode;
use chrono::Utc;
use common::{build_app, post_json, put_json, response_json, try_connect_db};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_failure_create_second_root_department_rejected() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let client = pool.get().await.expect("get db client");
    let root_id = Uuid::new_v4();
    let now = Utc::now();

    client
        .execute(
            "INSERT INTO departments (id, name, description, parent_id, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES ($1, $2, 'root', NULL, false, NULL, $3, $3)",
            &[&root_id, &format!("root_dept_{}", root_id.simple()), &now],
        )
        .await
        .expect("insert root department");

    let resp = post_json(
        build_app(pool.clone()),
        "/api/departments",
        json!({
            "name": format!("second_root_{}", Uuid::new_v4().simple()),
            "description": "should fail",
            "parent_id": null,
            "token_quota_enabled": false,
            "token_quota_per_day": null
        }),
    )
    .await;

    let status = resp.status();
    let body = response_json(resp).await;

    client
        .execute("DELETE FROM departments WHERE id = $1", &[&root_id])
        .await
        .expect("cleanup root department");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Validation error");
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("只允许一个顶级部门"));
}

#[tokio::test]
async fn test_failure_update_child_to_second_root_rejected() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let client = pool.get().await.expect("get db client");
    let root_id = Uuid::new_v4();
    let child_id = Uuid::new_v4();
    let now = Utc::now();

    client
        .execute(
            "INSERT INTO departments (id, name, description, parent_id, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES
             ($1, $2, 'root', NULL, false, NULL, $4, $4),
             ($3, $5, 'child', $1, false, NULL, $4, $4)",
            &[
                &root_id,
                &format!("root_dept_{}", root_id.simple()),
                &child_id,
                &now,
                &format!("child_dept_{}", child_id.simple()),
            ],
        )
        .await
        .expect("insert department tree");

    let resp = put_json(
        build_app(pool.clone()),
        &format!("/api/departments/{}", child_id),
        json!({
            "parent_id": null
        }),
    )
    .await;

    let status = resp.status();
    let body = response_json(resp).await;

    client
        .execute("DELETE FROM departments WHERE id = $1 OR id = $2", &[&root_id, &child_id])
        .await
        .expect("cleanup department tree");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Validation error");
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("只允许一个顶级部门"));
}