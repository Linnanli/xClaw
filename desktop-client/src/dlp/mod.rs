//! DLP (Data Loss Prevention) 脱敏引擎
//!
//! 提供实时敏感信息检测和脱敏功能，支持：
//! - 中国身份证号码检测和脱敏
//! - 手机号码检测和脱敏  
//! - API密钥和令牌检测和脱敏
//! - 自定义规则配置
//! - 实时消息流脱敏

pub mod detector;
pub mod integration;
pub mod patterns;
pub mod sanitizer;

// 测试模块
#[cfg(test)]
pub mod change_coverage_tests;
#[cfg(test)]
pub mod code_coverage_tests;
#[cfg(test)]
pub mod data_coverage_tests;
#[cfg(test)]
pub mod integration_tests;
#[cfg(test)]
pub mod reliability_tests;
#[cfg(test)]
pub mod requirements_tests;
#[cfg(test)]
pub mod security_tests;

pub use detector::{DlpAction, DlpDetectionResult, DlpDetector, DlpMatch, DlpSeverity};
pub use integration::{CustomPatternConfig, DlpIntegration, DlpIntegrationConfig, DlpStatistics};
pub use patterns::{ApiKeyPatterns, ChinesePatterns, CustomPattern};
pub use sanitizer::{DlpSanitizer, SanitizationConfig, SanitizationResult};

use thiserror::Error;

/// DLP 相关错误类型
#[derive(Debug, Error)]
pub enum DlpError {
    #[error("Pattern compilation failed: {0}")]
    PatternCompilation(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Sanitization failed: {0}")]
    Sanitization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Policy sync error: {0}")]
    PolicySync(#[from] crate::error::Error),
}

pub type DlpResult<T> = Result<T, DlpError>;
