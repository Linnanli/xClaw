//! 数据库配置管理
//! 
//! 支持多种数据库后端，包括 SQLite 和 PostgreSQL

use crate::platform_utils;
use std::env;
use std::path::PathBuf;

/// 数据库后端类型
#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseBackend {
    /// SQLite 数据库
    SQLite(String),
    /// PostgreSQL 数据库
    PostgreSQL(String),
}

impl DatabaseBackend {
    /// 从环境变量加载数据库配置
    /// 
    /// 支持的环境变量：
    /// - DATABASE_URL: 完整的数据库连接字符串
    /// - DATABASE_TYPE: 数据库类型 (sqlite, postgresql)
    /// - DATABASE_PATH: SQLite 数据库文件路径
    /// - DATABASE_HOST: PostgreSQL 主机
    /// - DATABASE_PORT: PostgreSQL 端口
    /// - DATABASE_NAME: 数据库名称
    /// - DATABASE_USER: 数据库用户
    /// - DATABASE_PASSWORD: 数据库密码
    pub fn from_env() -> Result<Self, DatabaseError> {
        // 首先检查 DATABASE_URL
        if let Ok(url) = env::var("DATABASE_URL") {
            return Self::from_url(&url);
        }
        
        // 检查 DATABASE_TYPE
        let db_type = env::var("DATABASE_TYPE")
            .unwrap_or_else(|_| "sqlite".to_string())
            .to_lowercase();
        
        match db_type.as_str() {
            "sqlite" => {
                let path = env::var("DATABASE_PATH")
                    .unwrap_or_else(|_| {
                        platform_utils::get_database_path()
                            .to_string_lossy()
                            .to_string()
                    });
                Ok(DatabaseBackend::SQLite(path))
            }
            "postgresql" | "postgres" => {
                let host = env::var("DATABASE_HOST")
                    .unwrap_or_else(|_| "localhost".to_string());
                let port = env::var("DATABASE_PORT")
                    .unwrap_or_else(|_| "5432".to_string());
                let name = env::var("DATABASE_NAME")
                    .map_err(|_| DatabaseError::MissingConfig("DATABASE_NAME"))?;
                let user = env::var("DATABASE_USER")
                    .map_err(|_| DatabaseError::MissingConfig("DATABASE_USER"))?;
                let password = env::var("DATABASE_PASSWORD")
                    .unwrap_or_else(|_| String::new());
                
                let url = if password.is_empty() {
                    format!("postgresql://{}@{}:{}/{}", user, host, port, name)
                } else {
                    format!("postgresql://{}:{}@{}:{}/{}", user, password, host, port, name)
                };
                
                Ok(DatabaseBackend::PostgreSQL(url))
            }
            _ => Err(DatabaseError::UnsupportedBackend(db_type)),
        }
    }
    
    /// 从连接字符串解析数据库配置
    pub fn from_url(url: &str) -> Result<Self, DatabaseError> {
        if url.starts_with("sqlite://") {
            let path = url.strip_prefix("sqlite://")
                .ok_or(DatabaseError::InvalidUrl)?
                .to_string();
            Ok(DatabaseBackend::SQLite(path))
        } else if url.starts_with("postgresql://") || url.starts_with("postgres://") {
            Ok(DatabaseBackend::PostgreSQL(url.to_string()))
        } else {
            Err(DatabaseError::InvalidUrl)
        }
    }
    
    /// 获取数据库连接字符串
    pub fn get_connection_string(&self) -> String {
        match self {
            DatabaseBackend::SQLite(path) => format!("sqlite://{}", path),
            DatabaseBackend::PostgreSQL(url) => url.clone(),
        }
    }
    
    /// 获取数据库类型名称
    pub fn get_type_name(&self) -> &'static str {
        match self {
            DatabaseBackend::SQLite(_) => "SQLite",
            DatabaseBackend::PostgreSQL(_) => "PostgreSQL",
        }
    }
    
    /// 验证数据库配置
    pub fn validate(&self) -> Result<(), DatabaseError> {
        match self {
            DatabaseBackend::SQLite(path) => {
                let path = PathBuf::from(path);
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() && !platform_utils::dir_exists(parent) {
                        return Err(DatabaseError::InvalidPath(path.to_string_lossy().to_string()));
                    }
                }
                Ok(())
            }
            DatabaseBackend::PostgreSQL(url) => {
                if url.is_empty() {
                    Err(DatabaseError::InvalidUrl)
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Default for DatabaseBackend {
    fn default() -> Self {
        DatabaseBackend::SQLite(
            platform_utils::get_database_path()
                .to_string_lossy()
                .to_string()
        )
    }
}

impl std::fmt::Display for DatabaseBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseBackend::SQLite(path) => write!(f, "SQLite({})", path),
            DatabaseBackend::PostgreSQL(url) => {
                // 隐藏密码
                let masked_url = if let Some(at_pos) = url.find('@') {
                    let before_at = &url[..at_pos];
                    let after_at = &url[at_pos..];
                    
                    if let Some(colon_pos) = before_at.rfind(':') {
                        let user_part = &before_at[..colon_pos];
                        format!("{}:***{}", user_part, after_at)
                    } else {
                        url.clone()
                    }
                } else {
                    url.clone()
                };
                write!(f, "PostgreSQL({})", masked_url)
            }
        }
    }
}

/// 数据库配置错误
#[derive(Debug, Clone)]
pub enum DatabaseError {
    /// 不支持的数据库后端
    UnsupportedBackend(String),
    /// 无效的连接 URL
    InvalidUrl,
    /// 无效的路径
    InvalidPath(String),
    /// 缺少配置
    MissingConfig(&'static str),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::UnsupportedBackend(backend) => {
                write!(f, "Unsupported database backend: {}", backend)
            }
            DatabaseError::InvalidUrl => write!(f, "Invalid database URL"),
            DatabaseError::InvalidPath(path) => write!(f, "Invalid database path: {}", path),
            DatabaseError::MissingConfig(var) => write!(f, "Missing configuration: {}", var),
        }
    }
}

impl std::error::Error for DatabaseError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sqlite_from_url() {
        let db = DatabaseBackend::from_url("sqlite:///tmp/test.db").unwrap();
        assert_eq!(db, DatabaseBackend::SQLite("/tmp/test.db".to_string()));
    }
    
    #[test]
    fn test_postgresql_from_url() {
        let url = "postgresql://user:pass@localhost:5432/mydb";
        let db = DatabaseBackend::from_url(url).unwrap();
        assert_eq!(db, DatabaseBackend::PostgreSQL(url.to_string()));
    }
    
    #[test]
    fn test_invalid_url() {
        let result = DatabaseBackend::from_url("invalid://localhost");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_get_connection_string() {
        let db = DatabaseBackend::SQLite("/tmp/test.db".to_string());
        assert_eq!(db.get_connection_string(), "sqlite:///tmp/test.db");
    }
    
    #[test]
    fn test_get_type_name() {
        let sqlite = DatabaseBackend::SQLite("/tmp/test.db".to_string());
        let postgres = DatabaseBackend::PostgreSQL("postgresql://localhost/db".to_string());
        
        assert_eq!(sqlite.get_type_name(), "SQLite");
        assert_eq!(postgres.get_type_name(), "PostgreSQL");
    }
    
    #[test]
    fn test_display_hides_password() {
        let db = DatabaseBackend::PostgreSQL(
            "postgresql://user:secret@localhost:5432/mydb".to_string()
        );
        let display = format!("{}", db);
        assert!(!display.contains("secret"));
        assert!(display.contains("***"));
    }
    
    #[test]
    fn test_default_sqlite() {
        let db = DatabaseBackend::default();
        assert!(matches!(db, DatabaseBackend::SQLite(_)));
    }
}
