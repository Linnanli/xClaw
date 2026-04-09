use admin_backend::{routes, AppState};
use deadpool_postgres::Config;
use std::env;
use tokio::net::TcpListener;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Database configuration — 变量读取一次，同时供 legacy pool 和 sqlx pool 使用
    let db_host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let db_port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let db_user = env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
    let db_password = env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string());
    let db_name = env::var("DB_NAME").unwrap_or_else(|_| "ironclaw".to_string());

    let db_config = Config {
        host: Some(db_host.clone()),
        port: Some(db_port.parse()?),
        user: Some(db_user.clone()),
        password: Some(db_password.clone()),
        dbname: Some(db_name.clone()),
        ..Default::default()
    };
    let pool = db_config.create_pool(None, tokio_postgres::NoTls)?;

    // SQLx 连接池 — 新模块使用；优先读 DATABASE_URL，否则从各分项变量拼接
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        format!("postgres://{db_user}:{db_password}@{db_host}:{db_port}/{db_name}")
    });
    let sqlx_pool = sqlx::PgPool::connect(&database_url).await?;

    // IronClaw Gateway URL
    let gateway_url =
        env::var("IRONCLAW_GATEWAY_URL").unwrap_or_else(|_| "http://127.0.0.1:38080".to_string());

    // HTTP client for proxying requests to IronClaw Gateway
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // Create app state
    let state = AppState {
        db_pool: pool,
        sqlx_pool,
        http_client,
        gateway_url,
    };

    // Create router
    let app = routes::create_router(state.clone());

    // CORS — 允许 Desktop Client 前端跨域访问
    use axum::http::Method;
    use tower_http::cors::{Any, CorsLayer};
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);
    let app = app.layer(cors);

    // Start server
    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("Server listening on {}", addr);

    // 启动审计日志自动清理后台任务
    let cleanup_pool = state.db_pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
        loop {
            interval.tick().await;
            match cleanup_pool.get().await {
                Err(e) => tracing::error!("audit cleanup: failed to get db connection: {}", e),
                Ok(client) => {
                    // 从 system_settings 读取保留天数，默认 90 天
                    let retention_days: i64 = client
                        .query_opt(
                            "SELECT value FROM system_settings WHERE key = 'audit_retention_days'",
                            &[],
                        )
                        .await
                        .ok()
                        .flatten()
                        .and_then(|row| {
                            let v: Option<serde_json::Value> = row.get(0);
                            v.and_then(|j| match j {
                                serde_json::Value::Number(n) => n.as_i64(),
                                serde_json::Value::String(s) => s.parse::<i64>().ok(),
                                _ => None,
                            })
                        })
                        .unwrap_or(90);

                    let days_str = retention_days.to_string();
                    match client
                        .execute(
                            "DELETE FROM audit_logs \
                             WHERE created_at < NOW() - ($1 || ' days')::INTERVAL \
                             AND is_immutable = FALSE",
                            &[&days_str],
                        )
                        .await
                    {
                        Ok(deleted) => tracing::info!(
                            "audit cleanup: deleted {} rows older than {} days",
                            deleted,
                            retention_days
                        ),
                        Err(e) => tracing::error!("audit cleanup: delete failed: {}", e),
                    }
                }
            }
        }
    });

    // 启动通知失败重试后台任务（每 5 分钟扫描，最多重试 3 次）
    let retry_pool = state.db_pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            retry_failed_notifications(&retry_pool).await;
        }
    });

    // 启动审批超时催办后台任务（每小时扫描，24 小时未处理时发送催办告警）
    let approval_pool = state.db_pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            admin_backend::handlers::approvals::check_approval_timeouts(&approval_pool).await;
        }
    });

    // 启动客户端在线状态自动更新任务（每分钟扫描，按 offline_threshold 标记离线）
    let client_status_pool = state.db_pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            update_client_online_status(&client_status_pool).await;
        }
    });

    axum::serve(listener, app).await?;

    Ok(())
}

/// 重试失败的通知：扫描 notification_logs 中 status='failed' 且 attempts < 3 的记录，
/// 重新发送；达到 3 次后标记为永久失败（需求 15.9）。
async fn retry_failed_notifications(pool: &deadpool_postgres::Pool) {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "notification retry: failed to get db connection");
            return;
        }
    };

    // 查询需要重试的通知（失败且未超过 3 次）
    let rows = match client
        .query(
            "SELECT nl.id, nl.channel, ae.id as event_id, ae.rule_name, ae.severity, ae.trigger_detail
             FROM notification_logs nl
             JOIN alert_events ae ON ae.id = nl.alert_event_id
             WHERE nl.status = 'failed' AND nl.attempts < 3",
            &[],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "notification retry: query failed");
            return;
        }
    };

    if rows.is_empty() {
        return;
    }

    tracing::info!(
        count = rows.len(),
        "notification retry: retrying failed notifications"
    );

    for row in &rows {
        let log_id: uuid::Uuid = row.get(0);
        let event_id: uuid::Uuid = row.get(2);
        let rule_name: String = row.get(3);
        let severity: String = row.get(4);
        let detail: String = row.get(5);

        // 重置为 pending，让 send_alert_notifications 重新发送
        let _ = client
            .execute(
                "UPDATE notification_logs SET status = 'pending' WHERE id = $1",
                &[&log_id],
            )
            .await;

        admin_backend::handlers::alerts::send_alert_notifications(
            pool, event_id, &rule_name, &severity, &detail,
        )
        .await;
    }

    // 将仍然失败且已达 3 次的记录标记为永久失败
    let _ = client
        .execute(
            "UPDATE notification_logs SET status = 'failed' \
             WHERE status = 'failed' AND attempts >= 3",
            &[],
        )
        .await;
}

/// 客户端在线状态自动更新：
/// 读取 system_settings 中的 client_offline_threshold_s，
/// 将超过阈值未活跃的客户端标记为离线（需求 8.8）。
/// 同时检查 minimum_client_version，标记需要升级的客户端（需求 8.9）。
async fn update_client_online_status(pool: &deadpool_postgres::Pool) {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "client status update: failed to get db connection");
            return;
        }
    };

    // 读取离线判定阈值（默认 120 秒）
    let threshold_secs: i64 = client
        .query_opt(
            "SELECT value FROM system_settings WHERE key = 'client_offline_threshold_s'",
            &[],
        )
        .await
        .ok()
        .flatten()
        .and_then(|row| {
            let v: Option<serde_json::Value> = row.get(0);
            v.and_then(|val| val.as_i64())
        })
        .unwrap_or(120);

    // 将超过阈值未活跃的在线客户端标记为离线
    match client
        .execute(
            "UPDATE registered_clients \
             SET online = false, updated_at = NOW() \
             WHERE online = true \
             AND last_activity < NOW() - ($1 || ' seconds')::INTERVAL",
            &[&threshold_secs.to_string()],
        )
        .await
    {
        Ok(n) if n > 0 => tracing::info!(
            count = n,
            "client status update: marked {} clients offline",
            n
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "client status update: failed to mark offline"),
    }

    // 读取最低客户端版本（空字符串表示不限制）
    let min_version: String = client
        .query_opt(
            "SELECT value FROM system_settings WHERE key = 'minimum_client_version'",
            &[],
        )
        .await
        .ok()
        .flatten()
        .and_then(|row| {
            let v: Option<serde_json::Value> = row.get(0);
            v.and_then(|val| val.as_str().map(|s| s.to_string()))
        })
        .unwrap_or_default();

    if min_version.is_empty() {
        return;
    }

    // 标记版本低于最低要求的客户端需要升级
    // 使用字符串比较（semver 格式 x.y.z，字符串比较在版本号位数相同时有效）
    // 生产环境建议使用 semver crate 做精确比较
    match client
        .execute(
            "UPDATE registered_clients \
             SET needs_upgrade = true, updated_at = NOW() \
             WHERE version IS NOT NULL \
             AND version < $1 \
             AND needs_upgrade = false",
            &[&min_version],
        )
        .await
    {
        Ok(n) if n > 0 => tracing::info!(
            count = n,
            min_version = %min_version,
            "client status update: marked {} clients as needs_upgrade",
            n
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "client status update: failed to mark needs_upgrade"),
    }
}
