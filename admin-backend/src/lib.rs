pub mod auth;
pub mod db;
pub mod error;
pub mod models;
pub mod policy_management;
pub mod routes;

pub use error::{Error, Result};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: deadpool_postgres::Pool,
}
