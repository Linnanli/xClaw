//! 认证令牌管理器
//! 
//! 负责令牌的持久化存储、加载和生成

use crate::platform_utils;
use std::fs;
use std::path::PathBuf;

/// 认证令牌管理器
pub struct AuthTokenManager {
    token_file: PathBuf,
}

impl AuthTokenManager {
    /// 创建新的令牌管理器
    pub fn new() -> Self {
        Self {
            token_file: platform_utils::get_auth_token_path(),
        }
    }
    
    /// 加载或生成令牌
    /// 
    /// 优先级：
    /// 1. 从后端数据库读取 gateway_auth_token
    /// 2. 从本地文件加载
    /// 3. 生成新令牌并保存
    pub fn load_or_generate(&self) -> Result<String, TokenError> {
        // 1. 尝试从后端数据库读取 gateway_auth_token
        if let Ok(token) = self.load_from_backend_db() {
            if !token.is_empty() && is_valid_token(&token) {
                // 保存到本地文件以便下次快速加载
                let _ = self.save(&token);
                return Ok(token);
            }
        }
        
        // 2. 确保目录存在
        let dir = self.token_file.parent().ok_or(TokenError::InvalidPath)?;
        platform_utils::create_dir_if_not_exists(dir)
            .map_err(|e| TokenError::IoError(e.to_string()))?;
        
        // 3. 尝试加载现有令牌
        if platform_utils::file_exists(&self.token_file) {
            let token = fs::read_to_string(&self.token_file)
                .map_err(|e| TokenError::IoError(e.to_string()))?;
            
            let token = token.trim().to_string();
            if !token.is_empty() && is_valid_token(&token) {
                return Ok(token);
            }
        }
        
        // 4. 生成新令牌
        let token = generate_random_token();
        fs::write(&self.token_file, &token)
            .map_err(|e| TokenError::IoError(e.to_string()))?;
        
        Ok(token)
    }
    
    /// 从后端数据库读取 gateway_auth_token
    /// 
    /// 读取 ~/.ironclaw/ironclaw.db 中的 channels.gateway_auth_token 配置
    fn load_from_backend_db(&self) -> Result<String, TokenError> {
        use std::env;
        
        // 获取后端数据库路径
        let home_dir = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .map_err(|_| TokenError::IoError("Cannot determine home directory".to_string()))?;
        
        let db_path = PathBuf::from(home_dir)
            .join(".ironclaw")
            .join("ironclaw.db");
        
        if !db_path.exists() {
            return Err(TokenError::TokenNotFound);
        }
        
        // 使用 rusqlite 读取数据库
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| TokenError::IoError(format!("Failed to open database: {}", e)))?;
        
        let token: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| TokenError::IoError(format!("Failed to query token: {}", e)))?;
        
        // 移除 JSON 字符串的引号和空白字符
        let token = token
            .trim()
            .trim_matches('"')
            .trim()
            .to_string();
        
        if token.is_empty() {
            return Err(TokenError::TokenNotFound);
        }
        
        Ok(token)
    }
    
    /// 保存令牌
    pub fn save(&self, token: &str) -> Result<(), TokenError> {
        if !is_valid_token(token) {
            return Err(TokenError::InvalidToken);
        }
        
        // 确保目录存在
        let dir = self.token_file.parent().ok_or(TokenError::InvalidPath)?;
        platform_utils::create_dir_if_not_exists(dir)
            .map_err(|e| TokenError::IoError(e.to_string()))?;
        
        fs::write(&self.token_file, token)
            .map_err(|e| TokenError::IoError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 加载令牌（如果不存在则返回错误）
    pub fn load(&self) -> Result<String, TokenError> {
        if !platform_utils::file_exists(&self.token_file) {
            return Err(TokenError::TokenNotFound);
        }
        
        let token = fs::read_to_string(&self.token_file)
            .map_err(|e| TokenError::IoError(e.to_string()))?;
        
        let token = token.trim().to_string();
        if !is_valid_token(&token) {
            return Err(TokenError::InvalidToken);
        }
        
        Ok(token)
    }
    
    /// 删除令牌
    pub fn delete(&self) -> Result<(), TokenError> {
        if platform_utils::file_exists(&self.token_file) {
            fs::remove_file(&self.token_file)
                .map_err(|e| TokenError::IoError(e.to_string()))?;
        }
        Ok(())
    }
    
    /// 检查令牌是否存在
    pub fn exists(&self) -> bool {
        platform_utils::file_exists(&self.token_file)
    }
    
    /// 获取令牌文件路径
    pub fn get_token_file_path(&self) -> &PathBuf {
        &self.token_file
    }
    
    /// 生成新的随机令牌（不保存）
    /// 
    /// 用于刷新令牌或测试目的
    pub fn generate_new_token() -> String {
        generate_random_token()
    }
}

impl Default for AuthTokenManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 令牌错误类型
#[derive(Debug, Clone)]
pub enum TokenError {
    /// 令牌未找到
    TokenNotFound,
    /// 无效的令牌
    InvalidToken,
    /// 无效的路径
    InvalidPath,
    /// IO 错误
    IoError(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::TokenNotFound => write!(f, "Token not found"),
            TokenError::InvalidToken => write!(f, "Invalid token format"),
            TokenError::InvalidPath => write!(f, "Invalid token file path"),
            TokenError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for TokenError {}

/// 生成随机令牌
/// 
/// 使用 UUID v4 生成 64 字符的十六进制字符串
/// 这比自定义哈希更安全、更标准
fn generate_random_token() -> String {
    use uuid::Uuid;
    
    // 生成 UUID v4 并移除连字符，得到 32 字符的十六进制字符串
    // 然后重复两次得到 64 字符
    let uuid1 = Uuid::new_v4().to_string().replace("-", "");
    let uuid2 = Uuid::new_v4().to_string().replace("-", "");
    format!("{}{}", uuid1, uuid2)
}

/// 验证令牌格式
/// 
/// 令牌应该是 64 字符的十六进制字符串
fn is_valid_token(token: &str) -> bool {
    token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    #[test]
    fn test_generate_random_token() {
        let token1 = generate_random_token();
        let token2 = generate_random_token();
        
        assert_eq!(token1.len(), 64);
        assert_eq!(token2.len(), 64);
        assert_ne!(token1, token2);
        assert!(token1.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(token2.chars().all(|c| c.is_ascii_hexdigit()));
    }
    
    #[test]
    fn test_is_valid_token() {
        let valid_token = "a".repeat(64);
        let invalid_token_short = "a".repeat(32);
        let invalid_token_chars = "g".repeat(64);
        
        assert!(is_valid_token(&valid_token));
        assert!(!is_valid_token(&invalid_token_short));
        assert!(!is_valid_token(&invalid_token_chars));
    }
    
    #[test]
    fn test_save_and_load_token() {
        let temp_dir = TempDir::new().unwrap();
        let token_file = temp_dir.path().join(".auth_token");
        
        let token = generate_random_token();
        fs::write(&token_file, &token).unwrap();
        
        let loaded_token = fs::read_to_string(&token_file).unwrap();
        assert_eq!(token, loaded_token.trim());
    }
    
    #[test]
    fn test_token_error_display() {
        assert_eq!(TokenError::TokenNotFound.to_string(), "Token not found");
        assert_eq!(TokenError::InvalidToken.to_string(), "Invalid token format");
        assert_eq!(TokenError::InvalidPath.to_string(), "Invalid token file path");
    }
    
    #[test]
    fn test_token_cleaning_from_db_format() {
        // 模拟从数据库读取的 JSON 字符串格式
        let db_value = r#""ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e""#;
        
        // 清理逻辑
        let cleaned = db_value
            .trim()
            .trim_matches('"')
            .trim()
            .to_string();
        
        println!("Original: {:?} (len={})", db_value, db_value.len());
        println!("Cleaned:  {:?} (len={})", cleaned, cleaned.len());
        
        // 验证
        assert_eq!(cleaned.len(), 64, "Token should be 64 characters after cleaning");
        assert!(cleaned.chars().all(|c| c.is_ascii_hexdigit()), "Token should only contain hex digits");
        assert!(!cleaned.contains('"'), "Token should not contain quotes");
        assert!(!cleaned.contains('\n'), "Token should not contain newlines");
        assert!(!cleaned.contains('\r'), "Token should not contain carriage returns");
    }
    
    #[test]
    fn test_token_with_newlines() {
        // 测试包含换行符的情况
        let db_value = "\"ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970\nc5656ae62f253e6e\"";
        
        let cleaned = db_value
            .trim()
            .trim_matches('"')
            .trim()
            .to_string();
        
        println!("With newline - Original: {:?}", db_value);
        println!("With newline - Cleaned:  {:?}", cleaned);
        
        // 这个应该失败，因为包含换行符
        assert!(cleaned.contains('\n'), "This test should detect newline in token");
    }
}
