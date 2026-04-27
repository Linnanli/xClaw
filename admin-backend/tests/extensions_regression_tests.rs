mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{DateTime, Utc};
use common::{build_app, get, post_json, response_json, try_connect_db, unique_name};
use serde_json::Value;
use tokio_postgres::Client as DbClient;
use tower::ServiceExt;
use uuid::Uuid;

fn registry_search_path(client_token: Uuid) -> String {
    format!("/api/v1/search?q=&client_token={client_token}")
}

fn registry_download_path(skill_id: Uuid, client_token: Uuid) -> String {
    format!("/api/v1/download?slug={skill_id}&client_token={client_token}")
}

fn registry_detail_path(skill_id: Uuid, client_token: Uuid) -> String {
    format!("/api/v1/skills/{skill_id}?client_token={client_token}")
}

fn extract_skill_slugs(body: &Value) -> Vec<String> {
    body["results"]
        .as_array()
        .expect("results should be array")
        .iter()
        .filter_map(|item| item["slug"].as_str().map(str::to_owned))
        .collect()
}

async fn insert_department(
    client: &DbClient,
    now: DateTime<Utc>,
    department_id: Uuid,
    department_name: &str,
    description: &str,
    parent_id: Option<Uuid>,
) {
    client
        .execute(
            "INSERT INTO departments (id, name, description, parent_id, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES ($1, $2, $3, $4, false, NULL, $5, $5)",
            &[&department_id, &department_name, &description, &parent_id, &now],
        )
        .await
        .expect("insert department");
}

async fn insert_user(
    client: &DbClient,
    now: DateTime<Utc>,
    user_id: Uuid,
    username: &str,
    email: &str,
    department_id: Uuid,
) {
    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, department_id, created_at, updated_at)
             VALUES ($1, $2, $3, 'hash', $4, $5, $5)",
            &[&user_id, &username, &email, &department_id, &now],
        )
        .await
        .expect("insert user");
}

async fn insert_registered_client(
    client: &DbClient,
    now: DateTime<Utc>,
    client_token: Uuid,
    user_id: Uuid,
    username: &str,
    client_name: &str,
) {
    client
        .execute(
            "INSERT INTO registered_clients (id, user_id, username, client_name, version, os, ip_address, last_activity, online, policy_version, registered_at, updated_at)
             VALUES ($1, $2, $3, $4, '1.0.0', 'macOS', '127.0.0.1', $5, true, 'v1', $5, $5)",
            &[&client_token, &user_id, &username, &client_name, &now],
        )
        .await
        .expect("insert registered client");
}

async fn insert_skill(
    client: &DbClient,
    now: DateTime<Utc>,
    skill_id: Uuid,
    skill_name: &str,
    description: &str,
    file_path: Option<&str>,
    enabled: bool,
    review_status: &str,
) {
    client
        .execute(
            "INSERT INTO skills (id, name, description, version, author, file_path, enabled, source, review_status, is_builtin, invoke_count, created_at, updated_at)
             VALUES ($1, $2, $3, '1.0.0', 'regression', $4, $5, 'admin_upload', $6, false, 0, $7, $7)",
            &[&skill_id, &skill_name, &description, &file_path, &enabled, &review_status, &now],
        )
        .await
        .expect("insert skill");
}

async fn insert_skill_whitelist(
    client: &DbClient,
    now: DateTime<Utc>,
    department_id: Uuid,
    skill_id: Uuid,
) {
    client
        .execute(
            "INSERT INTO department_skill_whitelist (department_id, skill_id, created_at)
             VALUES ($1, $2, $3)",
            &[&department_id, &skill_id, &now],
        )
        .await
        .expect("insert whitelist");
}

async fn cleanup_conversation_report_data(client: &DbClient, user_id: Uuid) {
    client
        .execute(
            "DELETE FROM conversation_messages WHERE conversation_id IN (SELECT id FROM conversations WHERE user_id = $1)",
            &[&user_id],
        )
        .await
        .expect("cleanup conversation messages");
    client
        .execute("DELETE FROM conversations WHERE user_id = $1", &[&user_id])
        .await
        .expect("cleanup conversations");
    client
        .execute("DELETE FROM usage_records WHERE user_id = $1", &[&user_id])
        .await
        .expect("cleanup usage records");
}

async fn cleanup_skill_whitelist(client: &DbClient, department_id: Uuid) {
    client
        .execute(
            "DELETE FROM department_skill_whitelist WHERE department_id = $1",
            &[&department_id],
        )
        .await
        .expect("cleanup whitelist");
}

async fn cleanup_registered_client(client: &DbClient, client_token: Uuid) {
    client
        .execute(
            "DELETE FROM registered_clients WHERE id = $1",
            &[&client_token],
        )
        .await
        .expect("cleanup client");
}

async fn cleanup_skills(client: &DbClient, skill_ids: &[Uuid]) {
    for skill_id in skill_ids {
        client
            .execute("DELETE FROM skills WHERE id = $1", &[skill_id])
            .await
            .expect("cleanup skill");
    }
}

async fn cleanup_user(client: &DbClient, user_id: Uuid) {
    client
        .execute("DELETE FROM users WHERE id = $1", &[&user_id])
        .await
        .expect("cleanup user");
}

async fn cleanup_departments(client: &DbClient, department_ids: &[Uuid]) {
    for department_id in department_ids {
        client
            .execute("DELETE FROM departments WHERE id = $1", &[department_id])
            .await
            .expect("cleanup department");
    }
}

async fn post_client_report_with_token(
    app: axum::Router,
    client_token: Uuid,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/client-reports")
            .header("authorization", format!("Bearer {client_token}"))
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&body).expect("serialize POST body"),
            ))
            .expect("build client report request"),
    )
    .await
    .expect("execute client report request")
}

#[tokio::test]
async fn test_conversation_report_used_skills_do_not_mutate_invoke_count() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let invoked_skill_id = Uuid::new_v4();
    let untouched_skill_id = Uuid::new_v4();
    let dept_name = unique_name("conv_skill_dept");
    let username = unique_name("conv_skill_user");
    let email = format!("{}@example.com", username);
    let invoked_skill_name = unique_name("invoked_skill");
    let untouched_skill_name = unique_name("untouched_skill");

    let client = pool.get().await.expect("get db client");

    insert_department(
        &client,
        now,
        dept_id,
        &dept_name,
        "conversation skill department",
        None,
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, dept_id).await;
    insert_skill(
        &client,
        now,
        invoked_skill_id,
        &invoked_skill_name,
        "invoked skill",
        None,
        true,
        "approved",
    )
    .await;
    insert_skill(
        &client,
        now,
        untouched_skill_id,
        &untouched_skill_name,
        "untouched skill",
        None,
        true,
        "approved",
    )
    .await;

    let body = serde_json::json!([
        {
            "type": "conversation",
            "client_conversation_id": format!("{}-thread-1", user_id),
            "user_id": user_id,
            "topic": "skill invoke test",
            "model_id": "gpt-4o-mini",
            "used_skills": [invoked_skill_name, invoked_skill_name],
            "messages": [
                {"role": "user", "content": "please use skill"},
                {"role": "assistant", "content": "done", "input_tokens": 5, "output_tokens": 8}
            ]
        }
    ]);

    let resp = post_json(build_app(pool.clone()), "/api/client-reports", body).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let invoked_count: i64 = client
        .query_one(
            "SELECT invoke_count FROM skills WHERE id = $1",
            &[&invoked_skill_id],
        )
        .await
        .expect("query invoked skill count")
        .get(0);
    let untouched_count: i64 = client
        .query_one(
            "SELECT invoke_count FROM skills WHERE id = $1",
            &[&untouched_skill_id],
        )
        .await
        .expect("query untouched skill count")
        .get(0);

    assert_eq!(
        invoked_count, 0,
        "client 侧预判 used_skills 不应直接增加调用计数"
    );
    assert_eq!(untouched_count, 0, "未使用技能不应增加计数");

    cleanup_conversation_report_data(&client, user_id).await;
    cleanup_skills(&client, &[invoked_skill_id, untouched_skill_id]).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[dept_id]).await;
}

#[tokio::test]
async fn test_conversation_report_falls_back_to_registered_client_user_for_list_visibility() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let client_token = Uuid::new_v4();
    let dept_name = unique_name("conv_audit_dept");
    let username = unique_name("conv_audit_user");
    let email = format!("{}@example.com", username);

    let client = pool.get().await.expect("get db client");

    insert_department(
        &client,
        now,
        dept_id,
        &dept_name,
        "conversation audit department",
        None,
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, dept_id).await;
    insert_registered_client(
        &client,
        now,
        client_token,
        user_id,
        &username,
        "conversation-audit-client",
    )
    .await;

    let body = serde_json::json!([
        {
            "type": "conversation",
            "client_conversation_id": format!("default-thread-{}", client_token),
            "user_id": "default",
            "topic": "fallback audit test",
            "model_id": "gpt-4o-mini",
            "messages": [
                {"role": "user", "content": "hello admin audit"},
                {"role": "assistant", "content": "hello", "input_tokens": 3, "output_tokens": 5}
            ]
        }
    ]);

    let resp = post_client_report_with_token(build_app(pool.clone()), client_token, body).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let stored_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM conversations WHERE user_id = $1",
            &[&user_id],
        )
        .await
        .expect("query conversations count")
        .get(0);
    assert_eq!(stored_count, 1, "conversation 应写入真实用户 ID");

    let list_resp = get(
        build_app(pool.clone()),
        "/api/conversations?page=1&page_size=20",
    )
    .await;
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = response_json(list_resp).await;
    let items = list_body["data"]
        .as_array()
        .expect("conversation list should be array");
    assert!(
        items
            .iter()
            .any(|item| item["username"] == username && item["topic"] == "fallback audit test"),
        "对话审计列表应能看到客户端上报的对话"
    );

    cleanup_conversation_report_data(&client, user_id).await;
    cleanup_registered_client(&client, client_token).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[dept_id]).await;
}

#[tokio::test]
async fn test_client_events_report_type_health_status_not_filtered_by_default_exclusion() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let marker = format!("health-status-marker-{}", Uuid::new_v4());
    let body = serde_json::json!([
        {
            "type": "health_status",
            "timestamp": "2026-04-16T10:00:00Z",
            "client_version": "0.1.0",
            "uptime_secs": 120,
            "active_extensions": [],
            "marker": marker
        }
    ]);

    let post_resp = post_json(build_app(pool.clone()), "/api/client-reports", body).await;
    assert_eq!(post_resp.status(), StatusCode::CREATED);

    let get_resp = get(
        build_app(pool.clone()),
        "/api/client-events?page=1&page_size=20&report_type=health_status",
    )
    .await;
    assert_eq!(get_resp.status(), StatusCode::OK);

    let list_body = response_json(get_resp).await;
    let events = list_body["events"]
        .as_array()
        .expect("events should be array");
    assert!(
        events.iter().any(|event| {
            event["report_type"] == "health_status" && event["payload"]["marker"] == marker
        }),
        "显式 report_type=health_status 时应返回 health_status 事件"
    );

    let client = pool.get().await.expect("get db client");
    client
        .execute(
            "DELETE FROM client_reports WHERE payload::text LIKE $1",
            &[&format!("%{}%", marker)],
        )
        .await
        .expect("cleanup client reports");
}

#[tokio::test]
async fn test_department_whitelist_backward_compatibility() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let client_token = Uuid::new_v4();
    let allowed_skill_id = Uuid::new_v4();
    let blocked_skill_id = Uuid::new_v4();
    let dept_name = unique_name("reg_dept");
    let username = unique_name("reg_user");
    let email = format!("{}@example.com", username);
    let allowed_name = unique_name("reg_allowed_skill");
    let blocked_name = unique_name("reg_blocked_skill");

    let client = pool.get().await.expect("get db client");

    insert_department(
        &client,
        now,
        dept_id,
        &dept_name,
        "regression test department",
        None,
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, dept_id).await;
    insert_registered_client(
        &client,
        now,
        client_token,
        user_id,
        &username,
        "reg-test-client",
    )
    .await;
    insert_skill(
        &client,
        now,
        allowed_skill_id,
        &allowed_name,
        "allowed skill",
        None,
        true,
        "approved",
    )
    .await;
    insert_skill(
        &client,
        now,
        blocked_skill_id,
        &blocked_name,
        "blocked skill",
        None,
        true,
        "approved",
    )
    .await;
    insert_skill_whitelist(&client, now, dept_id, allowed_skill_id).await;

    let path = registry_search_path(client_token);
    let resp = get(build_app(pool.clone()), &path).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    let slugs = extract_skill_slugs(&body);

    assert!(
        slugs.contains(&allowed_skill_id.to_string()),
        "白名单技能应出现在搜索结果"
    );
    assert!(
        !slugs.contains(&blocked_skill_id.to_string()),
        "非白名单技能不应出现在搜索结果"
    );

    cleanup_skill_whitelist(&client, dept_id).await;
    cleanup_registered_client(&client, client_token).await;
    cleanup_skills(&client, &[allowed_skill_id, blocked_skill_id]).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[dept_id]).await;
}

#[tokio::test]
async fn test_yanked_skill_not_in_registry_search_backward_compatibility() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let client_token = Uuid::new_v4();
    let visible_skill_id = Uuid::new_v4();
    let yanked_skill_id = Uuid::new_v4();
    let dept_name = unique_name("yanked_dept");
    let username = unique_name("yanked_user");
    let email = format!("{}@example.com", username);
    let visible_name = unique_name("reg_visible_skill");
    let yanked_name = unique_name("reg_yanked_skill");
    let client = pool.get().await.expect("get db client");

    insert_department(
        &client,
        now,
        dept_id,
        &dept_name,
        "yanked visibility department",
        None,
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, dept_id).await;
    insert_registered_client(
        &client,
        now,
        client_token,
        user_id,
        &username,
        "yanked-test-client",
    )
    .await;
    insert_skill(
        &client,
        now,
        visible_skill_id,
        &visible_name,
        "visible skill",
        None,
        true,
        "approved",
    )
    .await;
    insert_skill(
        &client,
        now,
        yanked_skill_id,
        &yanked_name,
        "yanked skill",
        None,
        false,
        "yanked",
    )
    .await;

    let path = registry_search_path(client_token);
    let resp = get(build_app(pool.clone()), &path).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    let slugs = extract_skill_slugs(&body);

    assert!(slugs.contains(&visible_skill_id.to_string()));
    assert!(
        !slugs.contains(&yanked_skill_id.to_string()),
        "已下架技能不应在注册表搜索结果中出现"
    );

    cleanup_skills(&client, &[visible_skill_id, yanked_skill_id]).await;
    cleanup_registered_client(&client, client_token).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[dept_id]).await;
}

#[tokio::test]
async fn test_registry_download_enforces_department_whitelist() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let client_token = Uuid::new_v4();
    let allowed_skill_id = Uuid::new_v4();
    let blocked_skill_id = Uuid::new_v4();
    let dept_name = unique_name("download_dept");
    let username = unique_name("download_user");
    let email = format!("{}@example.com", username);
    let allowed_name = unique_name("download_allowed_skill");
    let blocked_name = unique_name("download_blocked_skill");

    let client = pool.get().await.expect("get db client");

    insert_department(
        &client,
        now,
        dept_id,
        &dept_name,
        "download whitelist department",
        None,
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, dept_id).await;
    insert_registered_client(
        &client,
        now,
        client_token,
        user_id,
        &username,
        "download-test-client",
    )
    .await;
    insert_skill(
        &client,
        now,
        allowed_skill_id,
        &allowed_name,
        "allowed skill",
        Some("# allowed"),
        true,
        "approved",
    )
    .await;
    insert_skill(
        &client,
        now,
        blocked_skill_id,
        &blocked_name,
        "blocked skill",
        Some("# blocked"),
        true,
        "approved",
    )
    .await;
    insert_skill_whitelist(&client, now, dept_id, allowed_skill_id).await;

    let blocked_path = registry_download_path(blocked_skill_id, client_token);
    let blocked_resp = get(build_app(pool.clone()), &blocked_path).await;
    assert_eq!(blocked_resp.status(), StatusCode::NOT_FOUND);

    let allowed_path = registry_download_path(allowed_skill_id, client_token);
    let allowed_resp = get(build_app(pool.clone()), &allowed_path).await;
    assert_eq!(allowed_resp.status(), StatusCode::OK);

    cleanup_skill_whitelist(&client, dept_id).await;
    cleanup_registered_client(&client, client_token).await;
    cleanup_skills(&client, &[allowed_skill_id, blocked_skill_id]).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[dept_id]).await;
}

#[tokio::test]
async fn test_department_skill_whitelist_search_and_download_end_to_end() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let client_token = Uuid::new_v4();
    let allowed_skill_id = Uuid::new_v4();
    let blocked_skill_id = Uuid::new_v4();
    let dept_name = unique_name("e2e_dept");
    let username = unique_name("e2e_user");
    let email = format!("{}@example.com", username);
    let allowed_name = unique_name("e2e_allowed_skill");
    let blocked_name = unique_name("e2e_blocked_skill");

    let client = pool.get().await.expect("get db client");

    insert_department(
        &client,
        now,
        dept_id,
        &dept_name,
        "whitelist e2e department",
        None,
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, dept_id).await;
    insert_registered_client(
        &client,
        now,
        client_token,
        user_id,
        &username,
        "e2e-test-client",
    )
    .await;
    insert_skill(
        &client,
        now,
        allowed_skill_id,
        &allowed_name,
        "allowed skill",
        Some("# allowed"),
        true,
        "approved",
    )
    .await;
    insert_skill(
        &client,
        now,
        blocked_skill_id,
        &blocked_name,
        "blocked skill",
        Some("# blocked"),
        true,
        "approved",
    )
    .await;

    let anonymous_search_resp = get(build_app(pool.clone()), "/api/v1/search?q=").await;
    assert_eq!(anonymous_search_resp.status(), StatusCode::OK);
    let anonymous_search_body = response_json(anonymous_search_resp).await;
    assert_eq!(
        anonymous_search_body["results"]
            .as_array()
            .map(|items| items.len()),
        Some(0),
        "未携带 client_token 的注册表搜索不应返回任何技能"
    );

    let anonymous_download = format!("/api/v1/download?slug={}", allowed_skill_id);
    let anonymous_download_resp = get(build_app(pool.clone()), &anonymous_download).await;
    assert_eq!(anonymous_download_resp.status(), StatusCode::NOT_FOUND);

    let anonymous_detail = format!("/api/v1/skills/{}", allowed_skill_id);
    let anonymous_detail_resp = get(build_app(pool.clone()), &anonymous_detail).await;
    assert_eq!(anonymous_detail_resp.status(), StatusCode::NOT_FOUND);

    let all_search_path = registry_search_path(client_token);
    let all_resp = get(build_app(pool.clone()), &all_search_path).await;
    assert_eq!(all_resp.status(), StatusCode::OK);

    let all_body = response_json(all_resp).await;
    let all_slugs = extract_skill_slugs(&all_body);

    assert!(all_slugs.contains(&allowed_skill_id.to_string()));
    assert!(all_slugs.contains(&blocked_skill_id.to_string()));

    insert_skill_whitelist(&client, now, dept_id, allowed_skill_id).await;

    let filtered_search_path = registry_search_path(client_token);
    let filtered_resp = get(build_app(pool.clone()), &filtered_search_path).await;
    assert_eq!(filtered_resp.status(), StatusCode::OK);

    let filtered_body = response_json(filtered_resp).await;
    let filtered_slugs = extract_skill_slugs(&filtered_body);

    assert!(filtered_slugs.contains(&allowed_skill_id.to_string()));
    assert!(!filtered_slugs.contains(&blocked_skill_id.to_string()));

    let blocked_download = registry_download_path(blocked_skill_id, client_token);
    let blocked_resp = get(build_app(pool.clone()), &blocked_download).await;
    assert_eq!(blocked_resp.status(), StatusCode::NOT_FOUND);

    let allowed_download = registry_download_path(allowed_skill_id, client_token);
    let allowed_resp = get(build_app(pool.clone()), &allowed_download).await;
    assert_eq!(allowed_resp.status(), StatusCode::OK);

    let blocked_detail = registry_detail_path(blocked_skill_id, client_token);
    let blocked_detail_resp = get(build_app(pool.clone()), &blocked_detail).await;
    assert_eq!(blocked_detail_resp.status(), StatusCode::NOT_FOUND);

    let allowed_detail = registry_detail_path(allowed_skill_id, client_token);
    let allowed_detail_resp = get(build_app(pool.clone()), &allowed_detail).await;
    assert_eq!(allowed_detail_resp.status(), StatusCode::OK);
    let allowed_detail_body = response_json(allowed_detail_resp).await;
    let allowed_slug = allowed_skill_id.to_string();
    assert_eq!(
        allowed_detail_body["skill"]["slug"].as_str(),
        Some(allowed_slug.as_str()),
        "白名单技能详情应对合法 client_token 可见"
    );

    cleanup_skill_whitelist(&client, dept_id).await;
    cleanup_registered_client(&client, client_token).await;
    cleanup_skills(&client, &[allowed_skill_id, blocked_skill_id]).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[dept_id]).await;
}

#[tokio::test]
async fn test_registry_whitelist_inherits_from_parent_department() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️ 数据库不可用，跳过");
            return;
        }
    };

    let now = Utc::now();
    let root_dept_id = Uuid::new_v4();
    let child_dept_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let client_token = Uuid::new_v4();
    let allowed_skill_id = Uuid::new_v4();
    let blocked_skill_id = Uuid::new_v4();
    let root_name = unique_name("inherit_root");
    let child_name = unique_name("inherit_child");
    let username = unique_name("inherit_user");
    let email = format!("{}@example.com", username);
    let allowed_name = unique_name("inherit_allowed_skill");
    let blocked_name = unique_name("inherit_blocked_skill");

    let client = pool.get().await.expect("get db client");

    insert_department(&client, now, root_dept_id, &root_name, "root", None).await;
    insert_department(
        &client,
        now,
        child_dept_id,
        &child_name,
        "child",
        Some(root_dept_id),
    )
    .await;
    insert_user(&client, now, user_id, &username, &email, child_dept_id).await;
    insert_registered_client(
        &client,
        now,
        client_token,
        user_id,
        &username,
        "inherit-test-client",
    )
    .await;
    insert_skill(
        &client,
        now,
        allowed_skill_id,
        &allowed_name,
        "allowed skill",
        Some("# allowed"),
        true,
        "approved",
    )
    .await;
    insert_skill(
        &client,
        now,
        blocked_skill_id,
        &blocked_name,
        "blocked skill",
        Some("# blocked"),
        true,
        "approved",
    )
    .await;
    insert_skill_whitelist(&client, now, root_dept_id, allowed_skill_id).await;

    let search_path = registry_search_path(client_token);
    let search_resp = get(build_app(pool.clone()), &search_path).await;
    assert_eq!(search_resp.status(), StatusCode::OK);

    let search_body = response_json(search_resp).await;
    let slugs = extract_skill_slugs(&search_body);

    assert!(slugs.contains(&allowed_skill_id.to_string()));
    assert!(
        !slugs.contains(&blocked_skill_id.to_string()),
        "子部门应继承父部门技能白名单"
    );

    let blocked_download = registry_download_path(blocked_skill_id, client_token);
    let blocked_download_resp = get(build_app(pool.clone()), &blocked_download).await;
    assert_eq!(blocked_download_resp.status(), StatusCode::NOT_FOUND);

    let allowed_download = registry_download_path(allowed_skill_id, client_token);
    let allowed_download_resp = get(build_app(pool.clone()), &allowed_download).await;
    assert_eq!(allowed_download_resp.status(), StatusCode::OK);

    let blocked_detail = registry_detail_path(blocked_skill_id, client_token);
    let blocked_detail_resp = get(build_app(pool.clone()), &blocked_detail).await;
    assert_eq!(blocked_detail_resp.status(), StatusCode::NOT_FOUND);

    let allowed_detail = registry_detail_path(allowed_skill_id, client_token);
    let allowed_detail_resp = get(build_app(pool.clone()), &allowed_detail).await;
    assert_eq!(allowed_detail_resp.status(), StatusCode::OK);

    cleanup_skill_whitelist(&client, root_dept_id).await;
    cleanup_registered_client(&client, client_token).await;
    cleanup_skills(&client, &[allowed_skill_id, blocked_skill_id]).await;
    cleanup_user(&client, user_id).await;
    cleanup_departments(&client, &[child_dept_id, root_dept_id]).await;
}
