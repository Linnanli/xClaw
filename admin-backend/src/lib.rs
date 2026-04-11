pub mod auth;
pub mod db;
pub mod error;
pub mod extensions_state;
pub mod extensions_validation;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod policy_management;
pub mod routes;
pub mod scanner;
pub mod skill_package;

pub use error::{Error, Result};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: deadpool_postgres::Pool,
    /// SQLx 连接池 — 新模块使用，逐步替换 db_pool
    pub sqlx_pool: sqlx::PgPool,
    /// IronClaw Web Gateway 的 HTTP 客户端
    pub http_client: reqwest::Client,
    /// IronClaw Web Gateway 的 URL
    pub gateway_url: String,
}
