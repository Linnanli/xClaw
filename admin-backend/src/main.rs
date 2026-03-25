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
    let app = routes::create_router(state);

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

    axum::serve(listener, app).await?;

    Ok(())
}
