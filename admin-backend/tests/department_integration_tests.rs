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
        .execute(
            "DELETE FROM departments WHERE id = $1 OR id = $2",
            &[&root_id, &child_id],
        )
        .await
        .expect("cleanup department tree");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Validation error");
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("只允许一个顶级部门"));
}

#[tokio::test]
async fn test_failure_update_model_whitelist_with_disabled_model_rejected() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let client = pool.get().await.expect("get db client");
    let dept_id = Uuid::new_v4();
    let model_id = Uuid::new_v4();
    let now = Utc::now();
    let model_name = format!("disabled_model_{}", model_id.simple());

    client
        .execute(
            "INSERT INTO departments (id, name, description, parent_id, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES ($1, $2, 'for whitelist test', NULL, false, NULL, $3, $3)",
            &[&dept_id, &format!("dept_{}", dept_id.simple()), &now],
        )
        .await
        .expect("insert department");

    client
        .execute(
            "INSERT INTO model_configs (id, model_id, display_name, provider, enabled, is_default, sort_order, capabilities, extra_config)
             VALUES ($1, $2, $3, 'openai', false, false, 9999, '[]'::jsonb, '{}'::jsonb)",
            &[&model_id, &format!("model_{}", model_id.simple()), &model_name],
        )
        .await
        .expect("insert disabled model");

    let resp = put_json(
        build_app(pool.clone()),
        &format!("/api/departments/{}/model-whitelist", dept_id),
        json!({ "model_config_ids": [model_id] }),
    )
    .await;

    let status = resp.status();
    let body = response_json(resp).await;

    client
        .execute(
            "DELETE FROM department_model_whitelist WHERE department_id = $1",
            &[&dept_id],
        )
        .await
        .expect("cleanup whitelist");
    client
        .execute("DELETE FROM model_configs WHERE id = $1", &[&model_id])
        .await
        .expect("cleanup model config");
    client
        .execute("DELETE FROM departments WHERE id = $1", &[&dept_id])
        .await
        .expect("cleanup department");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Validation error");
    let details = body["details"].as_str().unwrap_or_default();
    assert!(details.contains("禁止设置禁用模型"));
    assert!(details.contains(&model_name));
}
