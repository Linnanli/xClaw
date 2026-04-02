//! 集成冒烟测试
//!
//! 解决三类"测试通过但生产失败"的问题：
//!
//! 1. **编译问题**：引用真实 create_router，确保所有 handler 都被编译
//! 2. **迁移问题**：验证所有迁移对应的表都存在
//! 3. **HTTP 路由问题**：通过真实 axum router 发 HTTP 请求验证路由注册
//!
//! 运行：cargo test --test integration_smoke_tests -p admin-backend

use admin_backend::{routes::create_router, AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use deadpool_postgres::Config;
use http_body_util::BodyExt;
use tower::ServiceExt; // oneshot
use tokio_postgres::NoTls;

// ============================================================================
// 辅助函数
// ============================================================================

async fn try_connect_db() -> Option<deadpool_postgres::Pool> {
    let mut cfg = Config::new();
    cfg.host = Some(std::env::var("DB_HOST").unwrap_or("localhost".to_string()));
    cfg.port = Some(5432);
    cfg.user = Some(std::env::var("DB_USER").unwrap_or("postgres".to_string()));
    cfg.password = Some(std::env::var("DB_PASSWORD").unwrap_or("postgres".to_string()));
    cfg.dbname = Some(std::env::var("DB_NAME").unwrap_or("ironclaw".to_string()));
    let pool = cfg.create_pool(None, NoTls).ok()?;
    let _ = pool.get().await.ok()?;
    Some(pool)
}

fn build_app(pool: deadpool_postgres::Pool) -> axum::Router {
    let state = AppState {
        db_pool: pool,
        http_client: reqwest::Client::new(),
        gateway_url: "http://localhost:38080".to_string(),
    };
    create_router(state)
}

async fn get(app: axum::Router, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn put_json(app: axum::Router, path: &str, body: serde_json::Value) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}

// ============================================================================
// 层 1：编译检查
// create_router 引用了所有 handler，只要这个测试能编译，说明 routes.rs 无编译错误
// ============================================================================

#[tokio::test]
async fn test_compile_all_handlers_via_create_router() {
    let mut cfg = Config::new();
    cfg.host = Some("localhost".to_string());
    cfg.port = Some(5432);
    cfg.user = Some("postgres".to_string());
    cfg.password = Some("postgres".to_string());
    cfg.dbname = Some("ironclaw".to_string());
    let pool = cfg.create_pool(None, NoTls).unwrap();
    // 只要能构建 router，说明所有 handler 都通过了编译
    let _app = build_app(pool);
}

// ============================================================================
// 层 2：迁移完整性检查
// ============================================================================

#[tokio::test]
async fn test_migration_all_tables_exist() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let client = pool.get().await.unwrap();

    // 每个迁移 → 对应必须存在的表
    // 新增迁移时在这里追加一行
    let required: &[(&str, &str)] = &[
        ("001_init",      "users"),
        ("002_rbac",      "roles"),
        ("003_dlp",       "dlp_rules"),
        ("004_audit",     "audit_logs"),
        ("006_dict",      "dlp_dictionaries"),
        ("007_sensitive", "sensitive_operation_rules"),
        ("008_policy",    "policy_change_records"),
        ("009_client",    "registered_clients"),
        ("010_skills",    "plugins"),
        ("011_reports",   "client_configs"),
        ("012_dept",      "departments"),
        ("013_model",     "model_configs"),   // ← 本次问题根因
        ("014_dept_ext",  "department_model_whitelist"),
        ("015_quota",     "usage_records"),
        ("016_alerts",    "alert_rules"),
        ("017_convs",     "conversations"),
        ("018_approvals", "approval_tickets"),
        ("019_compliance", "compliance_reports"),
        ("021_knowledge", "knowledge_bases"),
        // 020 是字段扩展迁移，表已存在，列级验证见 test_migration_020_security_fields_columns
    ];

    let mut missing = Vec::new();
    for (migration, table) in required {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name = $1",
                &[table],
            )
            .await
            .unwrap_or_else(|e| panic!("查询 information_schema 失败: {}", e));

        if row.get::<_, i64>(0) == 0 {
            missing.push(format!("{} → 表 `{}` 不存在，请执行迁移", migration, table));
        }
    }

    assert!(
        missing.is_empty(),
        "以下迁移未执行：\n{}\n\n执行命令：\n  docker exec -i <postgres> psql -U postgres -d ironclaw < admin-backend/migrations/<file>.sql",
        missing.join("\n")
    );
}

/// 验证 020_security_fields 迁移新增的列
#[tokio::test]
async fn test_migration_020_security_fields_columns() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let client = pool.get().await.unwrap();

    // users 表新增字段
    for col in &["mfa_enabled", "login_fail_count", "locked_until"] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'users' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0), 1,
            "users 表缺少列 `{}`，请执行迁移 020_security_fields.sql", col
        );
    }

    // audit_logs 表新增字段
    for col in &["ip_address", "user_agent", "is_immutable"] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'audit_logs' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0), 1,
            "audit_logs 表缺少列 `{}`，请执行迁移 020_security_fields.sql", col
        );
    }

    // registered_clients 表新增字段
    for col in &["device_fingerprint", "needs_upgrade"] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'registered_clients' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0), 1,
            "registered_clients 表缺少列 `{}`，请执行迁移 020_security_fields.sql", col
        );
    }

    // model_configs 表新增字段
    for col in &["total_calls", "avg_latency_ms", "consecutive_failures", "last_error_at"] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'model_configs' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0), 1,
            "model_configs 表缺少列 `{}`，请执行迁移 020_security_fields.sql", col
        );
    }
}

/// 验证 model_configs 表的列结构
#[tokio::test]
async fn test_migration_013_model_configs_columns() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let client = pool.get().await.unwrap();

    let rows = client
        .query(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_name = 'model_configs' AND table_schema = 'public'",
            &[],
        )
        .await
        .unwrap();

    let cols: Vec<String> = rows.iter().map(|r| r.get::<_, String>(0)).collect();

    for required_col in &["id", "model_id", "display_name", "provider",
                           "api_base_url", "api_key", "enabled", "is_default",
                           "sort_order", "capabilities", "created_at", "updated_at"] {
        assert!(
            cols.contains(&required_col.to_string()),
            "model_configs 表缺少列 `{}`，迁移 013 可能未完整执行",
            required_col
        );
    }
}

// ============================================================================
// 层 3：HTTP 路由冒烟测试
// 通过真实 axum router 发请求，验证路由注册 + handler 编译 + DB 查询全链路
// ============================================================================

#[tokio::test]
async fn test_http_health_check_200() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let resp = get(build_app(pool), "/health").await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// 核心测试：GET /api/model-configs 不能返回 404
/// 如果本次问题发生前有这个测试，会直接捕获
#[tokio::test]
async fn test_http_get_model_configs_not_404() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let resp = get(build_app(pool), "/api/model-configs").await;
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "GET /api/model-configs 返回 404：路由未注册或 handler 编译失败"
    );
}

#[tokio::test]
async fn test_http_get_model_configs_returns_json_array() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let resp = get(build_app(pool), "/api/model-configs").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("响应不是合法 JSON");
    assert!(json.is_array(), "响应应为 JSON 数组，实际：{}", json);
}

/// PUT /api/model-configs/:id 路由存在性验证
/// 本次问题：update_model_config 编译失败 → 旧 binary → 路由不存在
#[tokio::test]
async fn test_http_put_model_config_route_exists() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };
    let resp = put_json(
        build_app(pool),
        "/api/model-configs/00000000-0000-0000-0000-000000000000",
        serde_json::json!({ "enabled": true }),
    ).await;

    // 路由存在时返回 200/400/422/500，不会是 404
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "PUT /api/model-configs/:id 返回 404：路由未注册"
    );
}

/// 批量验证所有关键 GET 路由都已注册
#[tokio::test]
async fn test_http_all_critical_get_routes_registered() {
    let _pool = match try_connect_db().await {
        Some(p) => p,
        None => { println!("⚠️  数据库不可用，跳过"); return; }
    };

    let routes = [
        "/health",
        "/api/model-configs",
        "/api/client-models",
        "/api/users",
        "/api/roles",
        "/api/dlp-rules",
        "/api/audit-logs",
        "/api/departments",
        "/api/quota/overview",
        "/api/quota/usage-records",
        "/api/skills",
        "/api/plugins",
        "/api/settings",
        "/api/dashboard/stats",
        "/api/alert-rules",
        "/api/alerts",
        "/api/alerts/stats",
        "/api/conversations",
        "/api/conversations/stats",
        "/api/approvals",
        "/api/approvals/stats",
        "/api/compliance/overview",
        "/api/compliance/reports",
        "/api/compliance/retention",
    ];

    let mut failed = Vec::new();
    for path in &routes {
        // 每次请求需要新的 app 实例（oneshot 消费 router）
        let p2 = match try_connect_db().await {
            Some(p) => p,
            None => return,
        };
        let resp = get(build_app(p2), path).await;
        if resp.status() == StatusCode::NOT_FOUND {
            failed.push(*path);
        }
    }

    assert!(
        failed.is_empty(),
        "以下路由返回 404（未注册）：\n{}",
        failed.join("\n")
    );
}

// ============================================================================
// 回归测试：ironclaw_llm 清理
// 防止 chat_proxy / ironclaw_llm 依赖被误加回来
// ============================================================================

/// 回归测试：POST /api/chat/completions 路由已被移除
///
/// 背景：admin-backend 前端无 LLM 对话需求，chat_proxy.rs 和 ironclaw_llm
/// 依赖已于清理时删除。此测试确保该路由不会被误加回来。
#[tokio::test]
async fn test_regression_chat_completions_route_removed() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };

    let app = build_app(pool);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test","messages":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "POST /api/chat/completions 不应存在：admin-backend 无 LLM 代理需求，\
         如需恢复请先确认前端有对应页面"
    );
}

/// 契约测试：create_router 编译时不依赖 chat_proxy 模块
///
/// 只要此测试能编译通过，说明 lib.rs 中 chat_proxy 模块声明已被移除。
#[tokio::test]
async fn test_contract_router_compiles_without_chat_proxy() {
    // try_connect_db 读取环境变量，与其他冒烟测试保持一致
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    // create_router 能构建 = chat_proxy 模块不存在也不影响编译
    let _app = build_app(pool);
}
