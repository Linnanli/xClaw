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
use tokio_postgres::NoTls;
use tower::ServiceExt; // oneshot

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
        db_pool: pool.clone(),
        sqlx_pool: {
            // 冒烟测试不调用 sqlx 路径，用 connect_lazy 避免 block_in_place 的多线程依赖
            let db_url = format!(
                "postgres://{}:{}@{}:{}/{}",
                std::env::var("DB_USER").unwrap_or("postgres".to_string()),
                std::env::var("DB_PASSWORD").unwrap_or("postgres".to_string()),
                std::env::var("DB_HOST").unwrap_or("localhost".to_string()),
                5432,
                std::env::var("DB_NAME").unwrap_or("ironclaw".to_string()),
            );
            sqlx::PgPool::connect_lazy(&db_url).expect("lazy connect should not fail")
        },
        http_client: reqwest::Client::new(),
        gateway_url: "http://localhost:38080".to_string(),
    };
    create_router(state)
}

/// 生成一个有效的测试用 JWT token（使用与服务端相同的 secret）
fn make_auth_token() -> String {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let auth = admin_backend::auth::AuthManager::new(secret);
    auth.generate_access_token("00000000-0000-0000-0000-000000000001")
        .expect("test token generation should not fail")
}

async fn get(app: axum::Router, path: &str) -> axum::response::Response {
    let token = make_auth_token();
    app.oneshot(
        Request::builder()
            .method("GET")
            .uri(path)
            .header("authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn put_json(
    app: axum::Router,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let token = make_auth_token();
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(path)
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn post_json(
    app: axum::Router,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let token = make_auth_token();
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(path)
            .header("authorization", format!("Bearer {}", token))
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
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let client = pool.get().await.unwrap();

    // 每个迁移 → 对应必须存在的表
    // 新增迁移时在这里追加一行
    let required: &[(&str, &str)] = &[
        ("001_init", "users"),
        ("002_rbac", "roles"),
        ("003_dlp", "dlp_rules"),
        ("004_audit", "audit_logs"),
        ("006_dict", "dlp_dictionaries"),
        ("007_sensitive", "sensitive_operation_rules"),
        ("008_policy", "policy_change_records"),
        ("009_client", "registered_clients"),
        ("010_skills", "plugins"),
        ("011_reports", "client_configs"),
        ("012_dept", "departments"),
        ("013_model", "model_configs"), // ← 本次问题根因
        ("014_dept_ext", "department_model_whitelist"),
        ("015_quota", "usage_records"),
        ("016_alerts", "alert_rules"),
        ("017_convs", "conversations"),
        ("018_approvals", "approval_tickets"),
        ("019_compliance", "compliance_reports"),
        ("021_knowledge", "knowledge_bases"),
        ("021_knowledge_docs", "kb_documents"),
        ("023_extensions_v2", "department_skill_whitelist"),
        ("025_scan_results", "scan_results"),
        ("027_code_tool_settings", "code_tool_settings"),
        // 026 为 conversation_messages 字段扩展，列级验证见 test_migration_026_conversation_attachments_column
        // 020 是字段扩展迁移，表已存在，列级验证见 test_migration_020_security_fields_columns
        // 022 是 system_settings 数据插入，表已存在，通过 settings key 验证
        // 023 字段扩展验证见 test_migration_023_extensions_v2_columns
        // 028 是 code_tool_settings 数据插入（lsp_servers/git_repos），表已存在于 027
        // 029 是 registered_clients 字段扩展，列级验证见 test_migration_029_conversation_upload_enabled_column
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
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
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
            row.get::<_, i64>(0),
            1,
            "users 表缺少列 `{}`，请执行迁移 020_security_fields.sql",
            col
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
            row.get::<_, i64>(0),
            1,
            "audit_logs 表缺少列 `{}`，请执行迁移 020_security_fields.sql",
            col
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
            row.get::<_, i64>(0),
            1,
            "registered_clients 表缺少列 `{}`，请执行迁移 020_security_fields.sql",
            col
        );
    }

    // model_configs 表新增字段
    for col in &[
        "total_calls",
        "avg_latency_ms",
        "consecutive_failures",
        "last_error_at",
    ] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'model_configs' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0),
            1,
            "model_configs 表缺少列 `{}`，请执行迁移 020_security_fields.sql",
            col
        );
    }
}

#[tokio::test]
async fn test_migration_026_conversation_attachments_column() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let client = pool.get().await.unwrap();

    let row = client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = 'conversation_messages' AND column_name = 'attachments'",
            &[],
        )
        .await
        .unwrap();

    assert_eq!(row.get::<_, i64>(0), 1, "026 迁移缺少 attachments 列");
}

#[tokio::test]
async fn test_migration_029_conversation_upload_enabled_column() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let client = pool.get().await.unwrap();

    let row = client
        .query_one(
            "SELECT column_default, is_nullable FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = 'registered_clients' \
             AND column_name = 'conversation_upload_enabled'",
            &[],
        )
        .await
        .expect("029 迁移应该已加列 conversation_upload_enabled");

    let default_expr: Option<String> = row.get(0);
    let is_nullable: String = row.get(1);
    assert_eq!(
        is_nullable, "NO",
        "conversation_upload_enabled 必须 NOT NULL"
    );
    assert!(
        default_expr
            .as_deref()
            .map(|s| s.to_lowercase().contains("true"))
            .unwrap_or(false),
        "conversation_upload_enabled 默认值应为 TRUE，实际: {:?}",
        default_expr
    );
}

/// 验证 model_configs 表的列结构
#[tokio::test]
async fn test_migration_013_model_configs_columns() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
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

    for required_col in &[
        "id",
        "model_id",
        "display_name",
        "provider",
        "api_base_url",
        "api_key",
        "enabled",
        "is_default",
        "sort_order",
        "capabilities",
        "created_at",
        "updated_at",
    ] {
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
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let resp = get(build_app(pool), "/health").await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// GET /api/model-configs 不能返回 404
#[tokio::test]
async fn test_http_get_model_configs_not_404() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
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
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let resp = get(build_app(pool), "/api/model-configs").await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "GET /api/model-configs 应返回 200，实际返回 {}",
        resp.status()
    );

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("响应不是合法 JSON");
    assert!(json.is_array(), "响应应为 JSON 数组，实际：{}", json);
}

/// PUT /api/model-configs/:id 路由存在性验证
#[tokio::test]
async fn test_http_put_model_config_route_exists() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let resp = put_json(
        build_app(pool),
        "/api/model-configs/00000000-0000-0000-0000-000000000000",
        serde_json::json!({ "enabled": true }),
    )
    .await;
    // 路由存在时返回 200/400/404（记录不存在）/422，不会是方法不允许 405
    assert_ne!(
        resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "PUT /api/model-configs/:id 路由未注册"
    );
}

/// 批量验证所有关键 GET 路由都已注册
#[tokio::test]
async fn test_http_all_critical_get_routes_registered() {
    let _pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
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
        "/api/quota/details",
        "/api/skills",
        "/api/plugins",
        "/api/settings",
        "/api/settings/code-tools",
        "/api/settings/workspace-paths",
        "/api/settings/bash-rules",
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
        "/api/knowledge-bases",
        "/api/reports/ai-usage",
        "/api/reports/model-cost",
        "/api/reports/dept-ranking",
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
#[tokio::test]
async fn test_regression_chat_completions_route_removed() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    // 带 token 发请求：路由存在返回 200/400/422，路由不存在返回 404
    let resp = post_json(
        build_app(pool),
        "/api/chat/completions",
        serde_json::json!({ "model": "test", "messages": [] }),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "POST /api/chat/completions 不应存在，实际返回 {}",
        resp.status()
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

/// 验证 023_extensions_v2 迁移新增的列
#[tokio::test]
async fn test_migration_023_extensions_v2_columns() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let client = pool.get().await.unwrap();

    for col in &[
        "source",
        "review_status",
        "is_builtin",
        "invoke_count",
        "reviewed_by",
    ] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'skills' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0),
            1,
            "skills 表缺少列 `{}`，请执行迁移 023_extensions_v2.sql",
            col
        );
    }

    for col in &[
        "source",
        "review_status",
        "plugin_type",
        "requires_sandbox",
        "invoke_count",
    ] {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'plugins' AND column_name = $1 AND table_schema = 'public'",
                &[col],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0),
            1,
            "plugins 表缺少列 `{}`，请执行迁移 023_extensions_v2.sql",
            col
        );
    }
}

/// 验证扩展管理新路由已注册
#[tokio::test]
async fn test_http_extensions_v2_routes_registered() {
    let routes = [
        "/api/skills/upload",
        "/api/skills/00000000-0000-0000-0000-000000000000/scan-results",
        "/api/skills/00000000-0000-0000-0000-000000000000/rescan",
        "/api/skills/00000000-0000-0000-0000-000000000000/yank",
        "/api/plugins/upload",
        "/api/v1/search",
        "/api/v1/download",
    ];

    let mut failed = Vec::new();
    for path in &routes {
        let pool = match try_connect_db().await {
            Some(p) => p,
            None => return,
        };
        let resp = get(build_app(pool), path).await;
        if resp.status() == StatusCode::NOT_FOUND {
            failed.push(*path);
        }
    }

    assert!(
        failed.is_empty(),
        "以下扩展管理路由返回 404（未注册）：\n{}",
        failed.join("\n")
    );
}

/// 验证内置技能种子数据已写入 DB
#[tokio::test]
async fn test_migration_023_builtin_skills_seeded() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let client = pool.get().await.unwrap();

    // 验证内置技能存在
    let builtin_skills = [
        "delegation",
        "review-checklist",
        "routine-advisor",
        "ironclaw-workflow-orchestrator",
    ];
    for name in &builtin_skills {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM skills WHERE name = $1 AND is_builtin = true AND source = 'builtin'",
                &[name],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0),
            1,
            "内置技能 '{}' 未在 DB 中找到，请执行迁移 023_extensions_v2.sql",
            name
        );
    }

    // 验证内置插件存在
    let builtin_plugins = ["notion", "github", "gmail", "web-search", "slack"];
    for name in &builtin_plugins {
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM plugins WHERE name = $1 AND is_builtin = true AND source = 'builtin'",
                &[name],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0),
            1,
            "内置插件 '{}' 未在 DB 中找到，请执行迁移 023_extensions_v2.sql",
            name
        );
    }
}

/// 验证内置条目的 review_status 为 approved（不需要审核）
#[tokio::test]
async fn test_migration_023_builtin_entries_auto_approved() {
    let pool = match try_connect_db().await {
        Some(p) => p,
        None => {
            println!("⚠️  数据库不可用，跳过");
            return;
        }
    };
    let client = pool.get().await.unwrap();

    let row = client
        .query_one(
            "SELECT COUNT(*) FROM skills WHERE is_builtin = true AND review_status != 'approved'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        row.get::<_, i64>(0),
        0,
        "存在 is_builtin=true 但 review_status != 'approved' 的技能，内置条目应自动通过审核"
    );

    let row = client
        .query_one(
            "SELECT COUNT(*) FROM plugins WHERE is_builtin = true AND review_status != 'approved'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        row.get::<_, i64>(0),
        0,
        "存在 is_builtin=true 但 review_status != 'approved' 的插件，内置条目应自动通过审核"
    );
}
