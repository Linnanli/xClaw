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

    // Database configuration
    let db_host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let db_port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let db_user = env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
    let db_password = env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string());
    let db_name = env::var("DB_NAME").unwrap_or_else(|_| "ironclaw".to_string());

    let db_config = Config {
        host: Some(db_host),
        port: Some(db_port.parse()?),
        user: Some(db_user),
        password: Some(db_password),
        dbname: Some(db_name),
        ..Default::default()
    };

    let pool = db_config.create_pool(None, tokio_postgres::NoTls)?;

    // IronClaw Gateway URL
    let gateway_url = env::var("IRONCLAW_GATEWAY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:38080".to_string());

    // HTTP client for proxying requests to IronClaw Gateway
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // Create app state
    let state = AppState {
        db_pool: pool,
        http_client,
        gateway_url,
    };

    // Create router
    let app = routes::create_router(state.clone());

    // CORS — 允许 Desktop Client 前端跨域访问
    use tower_http::cors::{CorsLayer, Any};
    use axum::http::Method;
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
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
                            let v: Option<String> = row.get(0);
                            v.and_then(|s| s.parse::<i64>().ok())
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

    tracing::info!(count = rows.len(), "notification retry: retrying failed notifications");

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
