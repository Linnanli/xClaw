//! 跨平台工具库
//! 
//! 处理不同操作系统的差异，包括：
//! - 文件路径处理
//! - 权限管理
//! - 系统调用

use std::path::{Path, PathBuf};
use std::env;

/// 获取应用数据目录
/// 
/// 不同操作系统的路径：
/// - macOS: ~/Library/Application Support/ironclaw
/// - Windows: C:\Users\<user>\AppData\Local\ironclaw
/// - Linux: ~/.local/share/ironclaw
pub fn get_app_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PathBuf::from(".ironclaw")
    }
}

/// 获取应用配置目录
pub fn get_config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PathBuf::from(".config/ironclaw")
    }
}

/// 获取应用缓存目录
pub fn get_cache_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(target_os = "windows")]
    {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw")
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PathBuf::from(".cache/ironclaw")
    }
}

/// 获取数据库路径
pub fn get_database_path() -> PathBuf {
    get_app_data_dir().join("ironclaw.db")
}

/// 获取配置文件路径
pub fn get_config_file_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

/// 获取认证令牌文件路径
pub fn get_auth_token_path() -> PathBuf {
    get_app_data_dir().join(".auth_token")
}

/// 获取日志文件路径
pub fn get_log_file_path() -> PathBuf {
    get_app_data_dir().join("ironclaw.log")
}

/// 规范化路径
/// 
/// 处理不同操作系统的路径差异
pub fn normalize_path(path: &str) -> PathBuf {
    let path = path.replace("\\", "/");
    
    if path.starts_with("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(&path[2..])
    } else {
        PathBuf::from(path)
    }
}

/// 获取当前操作系统
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingSystem {
    MacOS,
    Windows,
    Linux,
    Unknown,
}

pub fn get_os() -> OperatingSystem {
    #[cfg(target_os = "macos")]
    {
        OperatingSystem::MacOS
    }
    
    #[cfg(target_os = "windows")]
    {
        OperatingSystem::Windows
    }
    
    #[cfg(target_os = "linux")]
    {
        OperatingSystem::Linux
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        OperatingSystem::Unknown
    }
}

/// 获取操作系统名称
pub fn get_os_name() -> &'static str {
    match get_os() {
        OperatingSystem::MacOS => "macOS",
        OperatingSystem::Windows => "Windows",
        OperatingSystem::Linux => "Linux",
        OperatingSystem::Unknown => "Unknown",
    }
}

/// 获取路径分隔符
pub fn get_path_separator() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "\\"
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        "/"
    }
}

/// 检查文件是否存在
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// 检查目录是否存在
pub fn dir_exists(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

/// 创建目录（如果不存在）
pub fn create_dir_if_not_exists(path: &Path) -> std::io::Result<()> {
    if !dir_exists(path) {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// 获取用户主目录
pub fn get_home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// 获取用户名
pub fn get_username() -> Option<String> {
    env::var("USER")
        .ok()
        .or_else(|| env::var("USERNAME").ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_app_data_dir() {
        let dir = get_app_data_dir();
        assert!(dir.to_string_lossy().contains("ironclaw"));
    }
    
    #[test]
    fn test_get_config_dir() {
        let dir = get_config_dir();
        assert!(dir.to_string_lossy().contains("ironclaw"));
    }
    
    #[test]
    fn test_get_cache_dir() {
        let dir = get_cache_dir();
        assert!(dir.to_string_lossy().contains("ironclaw"));
    }
    
    #[test]
    fn test_get_database_path() {
        let path = get_database_path();
        assert!(path.to_string_lossy().contains("ironclaw.db"));
    }
    
    #[test]
    fn test_normalize_path() {
        let path = normalize_path("~/test/file.txt");
        assert!(path.to_string_lossy().contains("test"));
    }
    
    #[test]
    fn test_get_os() {
        let os = get_os();
        assert_ne!(os, OperatingSystem::Unknown);
    }
    
    #[test]
    fn test_get_os_name() {
        let name = get_os_name();
        assert!(!name.is_empty());
    }
    
    #[test]
    fn test_get_path_separator() {
        let sep = get_path_separator();
        assert!(!sep.is_empty());
    }
    
    #[test]
    fn test_get_home_dir() {
        let home = get_home_dir();
        assert!(home.is_some());
    }
    
    #[test]
    fn test_get_username() {
        let username = get_username();
        assert!(username.is_some());
    }
}
