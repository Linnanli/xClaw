mod common;

use axum::http::StatusCode;
use chrono::Utc;
use common::{build_app, get, response_json, try_connect_db, unique_name};
use uuid::Uuid;

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

    client
        .execute(
            "INSERT INTO departments (id, name, description, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES ($1, $2, 'regression test department', false, NULL, $3, $3)",
            &[&dept_id, &dept_name, &now],
        )
        .await
        .expect("insert department");

    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, department_id, created_at, updated_at)
             VALUES ($1, $2, $3, 'hash', $4, $5, $5)",
            &[&user_id, &username, &email, &dept_id, &now],
        )
        .await
        .expect("insert user");

    client
        .execute(
            "INSERT INTO registered_clients (id, user_id, username, client_name, version, os, ip_address, last_activity, online, policy_version, registered_at, updated_at)
             VALUES ($1, $2, $3, 'reg-test-client', '1.0.0', 'macOS', '127.0.0.1', $4, true, 'v1', $4, $4)",
            &[&client_token, &user_id, &username, &now],
        )
        .await
        .expect("insert registered client");

    client
        .execute(
            "INSERT INTO skills (id, name, description, version, author, enabled, source, review_status, is_builtin, invoke_count, created_at, updated_at)
             VALUES
             ($1, $2, 'allowed skill', '1.0.0', 'regression', true, 'admin_upload', 'approved', false, 0, $5, $5),
             ($3, $4, 'blocked skill', '1.0.0', 'regression', true, 'admin_upload', 'approved', false, 0, $5, $5)",
            &[&allowed_skill_id, &allowed_name, &blocked_skill_id, &blocked_name, &now],
        )
        .await
        .expect("insert skills");

    client
        .execute(
            "INSERT INTO department_skill_whitelist (department_id, skill_id, created_at)
             VALUES ($1, $2, $3)",
            &[&dept_id, &allowed_skill_id, &now],
        )
        .await
        .expect("insert whitelist");

    let path = format!("/api/v1/search?q=&client_token={}", client_token);
    let resp = get(build_app(pool.clone()), &path).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    let results = body["results"].as_array().expect("results should be array");
    let slugs: Vec<String> = results
        .iter()
        .filter_map(|item| item["slug"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(
        slugs.contains(&allowed_skill_id.to_string()),
        "白名单技能应出现在搜索结果"
    );
    assert!(
        !slugs.contains(&blocked_skill_id.to_string()),
        "非白名单技能不应出现在搜索结果"
    );

    client
        .execute(
            "DELETE FROM department_skill_whitelist WHERE department_id = $1",
            &[&dept_id],
        )
        .await
        .expect("cleanup whitelist");
    client
        .execute(
            "DELETE FROM registered_clients WHERE id = $1",
            &[&client_token],
        )
        .await
        .expect("cleanup client");
    client
        .execute(
            "DELETE FROM skills WHERE id = $1 OR id = $2",
            &[&allowed_skill_id, &blocked_skill_id],
        )
        .await
        .expect("cleanup skills");
    client
        .execute("DELETE FROM users WHERE id = $1", &[&user_id])
        .await
        .expect("cleanup user");
    client
        .execute("DELETE FROM departments WHERE id = $1", &[&dept_id])
        .await
        .expect("cleanup department");
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
    let visible_skill_id = Uuid::new_v4();
    let yanked_skill_id = Uuid::new_v4();
    let visible_name = unique_name("reg_visible_skill");
    let yanked_name = unique_name("reg_yanked_skill");
    let client = pool.get().await.expect("get db client");

    client
        .execute(
            "INSERT INTO skills (id, name, description, version, author, enabled, source, review_status, is_builtin, invoke_count, created_at, updated_at)
             VALUES
             ($1, $2, 'visible skill', '1.0.0', 'regression', true, 'admin_upload', 'approved', false, 0, $5, $5),
             ($3, $4, 'yanked skill', '1.0.0', 'regression', false, 'admin_upload', 'yanked', false, 0, $5, $5)",
            &[&visible_skill_id, &visible_name, &yanked_skill_id, &yanked_name, &now],
        )
        .await
        .expect("insert skills");

    let resp = get(build_app(pool.clone()), "/api/v1/search?q=&client_token=").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = response_json(resp).await;
    let results = body["results"].as_array().expect("results should be array");
    let slugs: Vec<String> = results
        .iter()
        .filter_map(|item| item["slug"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(slugs.contains(&visible_skill_id.to_string()));
    assert!(
        !slugs.contains(&yanked_skill_id.to_string()),
        "已下架技能不应在注册表搜索结果中出现"
    );

    client
        .execute(
            "DELETE FROM skills WHERE id = $1 OR id = $2",
            &[&visible_skill_id, &yanked_skill_id],
        )
        .await
        .expect("cleanup skills");
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

    client
        .execute(
            "INSERT INTO departments (id, name, description, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES ($1, $2, 'download whitelist department', false, NULL, $3, $3)",
            &[&dept_id, &dept_name, &now],
        )
        .await
        .expect("insert department");

    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, department_id, created_at, updated_at)
             VALUES ($1, $2, $3, 'hash', $4, $5, $5)",
            &[&user_id, &username, &email, &dept_id, &now],
        )
        .await
        .expect("insert user");

    client
        .execute(
            "INSERT INTO registered_clients (id, user_id, username, client_name, version, os, ip_address, last_activity, online, policy_version, registered_at, updated_at)
             VALUES ($1, $2, $3, 'download-test-client', '1.0.0', 'macOS', '127.0.0.1', $4, true, 'v1', $4, $4)",
            &[&client_token, &user_id, &username, &now],
        )
        .await
        .expect("insert registered client");

    client
        .execute(
            "INSERT INTO skills (id, name, description, version, author, file_path, enabled, source, review_status, is_builtin, invoke_count, created_at, updated_at)
             VALUES
             ($1, $2, 'allowed skill', '1.0.0', 'regression', '# allowed', true, 'admin_upload', 'approved', false, 0, $5, $5),
             ($3, $4, 'blocked skill', '1.0.0', 'regression', '# blocked', true, 'admin_upload', 'approved', false, 0, $5, $5)",
            &[&allowed_skill_id, &allowed_name, &blocked_skill_id, &blocked_name, &now],
        )
        .await
        .expect("insert skills");

    client
        .execute(
            "INSERT INTO department_skill_whitelist (department_id, skill_id, created_at)
             VALUES ($1, $2, $3)",
            &[&dept_id, &allowed_skill_id, &now],
        )
        .await
        .expect("insert whitelist");

    let blocked_path = format!(
        "/api/v1/download?slug={}&client_token={}",
        blocked_skill_id, client_token
    );
    let blocked_resp = get(build_app(pool.clone()), &blocked_path).await;
    assert_eq!(blocked_resp.status(), StatusCode::NOT_FOUND);

    let allowed_path = format!(
        "/api/v1/download?slug={}&client_token={}",
        allowed_skill_id, client_token
    );
    let allowed_resp = get(build_app(pool.clone()), &allowed_path).await;
    assert_eq!(allowed_resp.status(), StatusCode::OK);

    client
        .execute(
            "DELETE FROM department_skill_whitelist WHERE department_id = $1",
            &[&dept_id],
        )
        .await
        .expect("cleanup whitelist");
    client
        .execute(
            "DELETE FROM registered_clients WHERE id = $1",
            &[&client_token],
        )
        .await
        .expect("cleanup client");
    client
        .execute(
            "DELETE FROM skills WHERE id = $1 OR id = $2",
            &[&allowed_skill_id, &blocked_skill_id],
        )
        .await
        .expect("cleanup skills");
    client
        .execute("DELETE FROM users WHERE id = $1", &[&user_id])
        .await
        .expect("cleanup user");
    client
        .execute("DELETE FROM departments WHERE id = $1", &[&dept_id])
        .await
        .expect("cleanup department");
}
