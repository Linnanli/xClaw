pub mod auth;
pub mod db;
pub mod error;
pub mod handlers;
pub mod models;
pub mod policy_management;
pub mod routes;

pub use error::{Error, Result};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: deadpool_postgres::Pool,
    /// IronClaw Web Gateway 的 HTTP 客户端
    pub http_client: reqwest::Client,
    /// IronClaw Web Gateway 的 URL
    pub gateway_url: String,
}
