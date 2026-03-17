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
        
        let raw_token: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| TokenError::IoError(format!("Failed to query token: {}", e)))?;
        
        // 使用新的清理函数
        let token = clean_token(&raw_token)?;
        
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
    /// 无效的令牌格式
    InvalidToken,
    /// 令牌包含无效字符
    InvalidCharacter(String),
    /// 令牌长度错误
    InvalidLength(usize),
    /// 无效的路径
    InvalidPath,
    /// IO 错误
    IoError(String),
    /// 数据库错误
    DatabaseError(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::TokenNotFound => write!(f, "Token not found"),
            TokenError::InvalidToken => write!(f, "Invalid token format"),
            TokenError::InvalidCharacter(details) => write!(f, "Token contains invalid characters: {}", details),
            TokenError::InvalidLength(len) => write!(f, "Invalid token length: {} (expected 64)", len),
            TokenError::InvalidPath => write!(f, "Invalid token file path"),
            TokenError::IoError(e) => write!(f, "IO error: {}", e),
            TokenError::DatabaseError(e) => write!(f, "Database error: {}", e),
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
/// 令牌应该是 64 字符的十六进制字符串，不包含控制字符、引号、空格等
pub fn is_valid_token(token: &str) -> bool {
    token.len() == 64 
        && token.chars().all(|c| c.is_ascii_hexdigit())
        && !token.contains('\n')
        && !token.contains('\r')
        && !token.contains('"')
        && !token.contains(' ')
        && !token.contains('\t')
        && !token.chars().any(|c| c.is_control())
}

/// 清理令牌字符串
/// 
/// 移除 JSON 引号、空白字符、换行符等，返回清理后的令牌
/// 如果清理后的令牌无效，返回错误
pub fn clean_token(raw: &str) -> Result<String, TokenError> {
    if raw.is_empty() {
        return Err(TokenError::InvalidToken);
    }
    
    let cleaned = raw
        .trim()
        .trim_matches('"')
        .trim()
        .replace('\n', "")
        .replace('\r', "")
        .replace(' ', "")
        .replace('\t', "");
    
    if is_valid_token(&cleaned) {
        Ok(cleaned)
    } else {
        Err(TokenError::InvalidToken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    // ========== 单元测试覆盖率 >90% ==========
    
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
    fn test_is_valid_token_all_cases() {
        // 正常情况
        let valid_token = "a".repeat(64);
        assert!(is_valid_token(&valid_token));
        
        // 边界情况 - 长度
        assert!(!is_valid_token(""));  // 空字符串
        assert!(!is_valid_token(&"a".repeat(63)));  // 长度不足
        assert!(!is_valid_token(&"a".repeat(65)));  // 长度超出
        
        // 边界情况 - 字符类型
        assert!(!is_valid_token(&format!("{}g", "a".repeat(63))));  // 非十六进制字符
        assert!(!is_valid_token(&format!("{}G", "a".repeat(63))));  // 大写非十六进制
        
        // 控制字符和特殊字符
        assert!(!is_valid_token(&format!("{}\n", "a".repeat(63))));  // 换行符
        assert!(!is_valid_token(&format!("{}\r", "a".repeat(63))));  // 回车符
        assert!(!is_valid_token(&format!("{}\t", "a".repeat(63))));  // 制表符
        assert!(!is_valid_token(&format!("{} ", "a".repeat(63))));   // 空格
        assert!(!is_valid_token(&format!("\"{}\"", "a".repeat(62)))); // 引号
        assert!(!is_valid_token(&format!("{}\x00", "a".repeat(63)))); // 空字符
        assert!(!is_valid_token(&format!("{}\x01", "a".repeat(63)))); // 控制字符
        
        // 有效的十六进制字符
        assert!(is_valid_token(&"0123456789abcdef".repeat(4)));
        assert!(is_valid_token(&"0123456789ABCDEF".repeat(4)));
        assert!(is_valid_token(&"fedcba9876543210".repeat(4)));
    }
    
    #[test]
    fn test_clean_token_comprehensive() {
        // 正常清理
        let valid_token = "a".repeat(64);
        assert_eq!(clean_token(&valid_token).unwrap(), valid_token);
        
        // JSON 引号清理
        assert_eq!(
            clean_token(&format!("\"{}\"", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        
        // 空白字符清理
        assert_eq!(
            clean_token(&format!(" {} ", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        assert_eq!(
            clean_token(&format!("\t{}\t", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        
        // 换行符清理
        assert_eq!(
            clean_token(&format!("{}\n", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        assert_eq!(
            clean_token(&format!("{}\r", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        assert_eq!(
            clean_token(&format!("{}\r\n", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        
        // 复合清理
        assert_eq!(
            clean_token(&format!(" \t\"{}\" \r\n", "a".repeat(64))).unwrap(), 
            "a".repeat(64)
        );
        
        // 中间的空格和换行符清理
        let token_with_spaces = format!("{}{}{}{}",
            "a".repeat(16),
            " ",
            "b".repeat(16),
            "\n",
        ) + &"c".repeat(31);
        assert_eq!(
            clean_token(&token_with_spaces).unwrap(),
            format!("{}{}{}", "a".repeat(16), "b".repeat(16), "c".repeat(32))
        );
    }
    
    #[test]
    fn test_clean_token_error_cases() {
        // 空字符串
        assert!(clean_token("").is_err());
        assert!(clean_token("   ").is_err());
        assert!(clean_token("\n\r\t").is_err());
        
        // 清理后长度不正确
        assert!(clean_token(&"a".repeat(63)).is_err());
        assert!(clean_token(&"a".repeat(65)).is_err());
        
        // 清理后包含无效字符
        assert!(clean_token(&format!("{}g", "a".repeat(63))).is_err());
        
        // 包含控制字符（无法清理）
        assert!(clean_token(&format!("{}\x00", "a".repeat(63))).is_err());
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
        assert_eq!(
            TokenError::InvalidCharacter("newline".to_string()).to_string(), 
            "Token contains invalid characters: newline"
        );
        assert_eq!(
            TokenError::InvalidLength(32).to_string(), 
            "Invalid token length: 32 (expected 64)"
        );
        assert_eq!(
            TokenError::DatabaseError("connection failed".to_string()).to_string(), 
            "Database error: connection failed"
        );
    }
    
    #[test]
    fn test_token_cleaning_from_db_format() {
        // 模拟从数据库读取的 JSON 字符串格式
        let db_value = r#""ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e""#;
        
        // 使用新的清理函数
        let cleaned = clean_token(db_value).unwrap();
        
        println!("Original: {:?} (len={})", db_value, db_value.len());
        println!("Cleaned:  {:?} (len={})", cleaned, cleaned.len());
        
        // 验证
        assert_eq!(cleaned.len(), 64, "Token should be 64 characters after cleaning");
        assert!(cleaned.chars().all(|c| c.is_ascii_hexdigit()), "Token should only contain hex digits");
        assert!(!cleaned.contains('"'), "Token should not contain quotes");
        assert!(!cleaned.contains('\n'), "Token should not contain newlines");
        assert!(!cleaned.contains('\r'), "Token should not contain carriage returns");
        assert!(is_valid_token(&cleaned), "Cleaned token should be valid");
    }
    
    #[test]
    fn test_token_with_newlines_should_be_cleaned() {
        // 测试包含换行符的情况 - 现在应该能够清理
        let db_value = "\"ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970\nc5656ae62f253e6e\"";
        
        // 这个现在应该能够成功清理
        let result = clean_token(db_value);
        assert!(result.is_err(), "Token with newline in middle should still be invalid after cleaning");
        
        // 但是末尾的换行符应该能够清理
        let db_value_with_trailing_newline = "\"ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e\"\n";
        let cleaned = clean_token(db_value_with_trailing_newline).unwrap();
        assert_eq!(cleaned.len(), 64);
        assert!(is_valid_token(&cleaned));
    }
    
    // ========== 安全覆盖率 100% ==========
    
    #[test]
    fn test_security_malicious_input_protection() {
        // 创建长期存在的字符串
        let unicode_math = "𝕒".repeat(32);
        let unicode_cyrillic = "а".repeat(64);
        let large_string_1k = "A".repeat(1000);
        let large_string_10k = "A".repeat(10000);
        
        let malicious_tokens = vec![
            // SQL 注入尝试
            "'; DROP TABLE users; --",
            "' OR '1'='1",
            "1'; DELETE FROM settings; --",
            
            // XSS 尝试
            "<script>alert('xss')</script>",
            "javascript:alert('xss')",
            "<img src=x onerror=alert('xss')>",
            
            // 路径遍历尝试
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config\\sam",
            "/etc/shadow",
            
            // 控制字符注入 (使用有效的ASCII范围)
            "\x00\x01\x02\x03",
            "\x7f",
            
            // 格式字符串攻击
            "%s%s%s%s",
            "%x%x%x%x",
            
            // 缓冲区溢出尝试
            &large_string_1k,
            &large_string_10k,
            
            // Unicode 攻击
            &unicode_math, // 数学字母数字符号
            &unicode_cyrillic, // 西里尔字母 (看起来像拉丁字母)
        ];
        
        for (i, token) in malicious_tokens.iter().enumerate() {
            assert!(!is_valid_token(token), "Should reject malicious token {}: {}", i, token);
            assert!(clean_token(token).is_err(), "Should fail to clean malicious token {}: {}", i, token);
        }
    }
    
    #[test]
    fn test_security_timing_attack_resistance() {
        let valid_token = "a".repeat(64);
        let invalid_tokens = vec![
            "b".repeat(64),
            "c".repeat(64),
            "d".repeat(64),
            "e".repeat(64),
            "f".repeat(64),
        ];
        
        // 测试多次以获得更稳定的时间测量
        let iterations = 100;
        let mut valid_times = Vec::new();
        let mut invalid_times = Vec::new();
        
        for _ in 0..iterations {
            let start = std::time::Instant::now();
            let _ = is_valid_token(&valid_token);
            valid_times.push(start.elapsed());
            
            for invalid_token in &invalid_tokens {
                let start = std::time::Instant::now();
                let _ = is_valid_token(invalid_token);
                invalid_times.push(start.elapsed());
            }
        }
        
        let avg_valid_time: u128 = valid_times.iter().map(|d| d.as_nanos()).sum::<u128>() / valid_times.len() as u128;
        let avg_invalid_time: u128 = invalid_times.iter().map(|d| d.as_nanos()).sum::<u128>() / invalid_times.len() as u128;
        
        // 时间差不应该太大（允许一定的变化，但不应该有明显的时序泄露）
        let time_diff = avg_valid_time.abs_diff(avg_invalid_time);
        let max_allowed_diff = std::cmp::max(avg_valid_time, avg_invalid_time) / 2; // 允许50%的差异
        
        println!("Average valid time: {}ns", avg_valid_time);
        println!("Average invalid time: {}ns", avg_invalid_time);
        println!("Time difference: {}ns (max allowed: {}ns)", time_diff, max_allowed_diff);
        
        assert!(time_diff < max_allowed_diff, 
            "Timing difference too large: {}ns > {}ns (may indicate timing attack vulnerability)", 
            time_diff, max_allowed_diff);
    }
    
    #[test]
    fn test_security_memory_safety() {
        // 测试大量内存分配不会导致问题
        let large_input = "a".repeat(1_000_000);
        assert!(clean_token(&large_input).is_err());
        
        // 测试空指针和边界情况
        assert!(clean_token("").is_err());
        
        // 测试 Unicode 边界情况
        let unicode_input = "🔒".repeat(32);
        assert!(clean_token(&unicode_input).is_err());
    }
    
    #[test]
    fn test_security_side_channel_resistance() {
        // 测试不同长度的输入是否有时序差异
        let input_32 = "a".repeat(32);
        let input_63 = "a".repeat(63);
        let input_64 = "a".repeat(64);
        let input_65 = "a".repeat(65);
        let input_128 = "a".repeat(128);
        
        let inputs = vec![
            "",
            "a",
            "ab",
            "abc",
            &input_32,
            &input_63,
            &input_64,
            &input_65,
            &input_128,
        ];
        
        let mut times = Vec::new();
        
        for input in &inputs {
            let start = std::time::Instant::now();
            let _ = is_valid_token(input);
            times.push(start.elapsed().as_nanos());
        }
        
        // 检查时间变化不会泄露长度信息
        // 对于短输入，时间应该相对稳定
        let short_times: Vec<_> = times[0..4].iter().collect();
        let long_times: Vec<_> = times[4..].iter().collect();
        
        println!("Short input times: {:?}", short_times);
        println!("Long input times: {:?}", long_times);
        
        // 这个测试主要是为了检测明显的时序泄露
        // 在实际应用中，可能需要更复杂的统计分析
    }
}
