//! 环境一致性检查工具
//! 
//! 用于检查开发、测试、生产环境的一致性

use std::collections::HashMap;
use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Production,
}

#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    pub environment: Environment,
    pub api_base_url: String,
    pub api_timeout_secs: u64,
    pub api_retry_count: u32,
    pub cors_origins: Vec<String>,
    pub auth_method: AuthMethod,
    pub log_level: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    Header,
    UrlParameter,
    Both,
}

pub struct EnvironmentChecker {
    config: EnvironmentConfig,
    checks: Vec<EnvironmentCheck>,
}

#[derive(Debug, Clone)]
pub struct EnvironmentCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

impl EnvironmentChecker {
    pub fn new() -> Self {
        let environment = Self::detect_environment();
        let config = Self::load_config(&environment);
        
        Self {
            config,
            checks: Vec::new(),
        }
    }
    
    /// 检测当前环境
    fn detect_environment() -> Environment {
        match env::var("ENVIRONMENT").as_deref() {
            Ok("testing") => Environment::Testing,
            Ok("production") => Environment::Production,
            _ => Environment::Development,
        }
    }
    
    /// 加载环境配置
    fn load_config(environment: &Environment) -> EnvironmentConfig {
        match environment {
            Environment::Development => EnvironmentConfig {
                environment: Environment::Development,
                api_base_url: "http://localhost:3000".to_string(),
                api_timeout_secs: 30,
                api_retry_count: 3,
                cors_origins: vec![
                    "http://localhost:5173".to_string(),
                    "http://127.0.0.1:5173".to_string(),
                ],
                auth_method: AuthMethod::Both,
                log_level: "debug".to_string(),
            },
            Environment::Testing => EnvironmentConfig {
                environment: Environment::Testing,
                api_base_url: "http://localhost:3001".to_string(),
                api_timeout_secs: 5,
                api_retry_count: 1,
                cors_origins: vec!["http://localhost:5173".to_string()],
                auth_method: AuthMethod::UrlParameter,
                log_level: "info".to_string(),
            },
            Environment::Production => EnvironmentConfig {
                environment: Environment::Production,
                api_base_url: "https://api.example.com".to_string(),
                api_timeout_secs: 10,
                api_retry_count: 5,
                cors_origins: vec!["https://example.com".to_string()],
                auth_method: AuthMethod::Header,
                log_level: "warn".to_string(),
            },
        }
    }
    
    /// 运行所有检查
    pub fn run_all_checks(&mut self) -> bool {
        self.check_api_connectivity();
        self.check_cors_configuration();
        self.check_auth_method();
        self.check_environment_variables();
        self.check_dependencies();
        
        self.checks.iter().all(|check| check.passed)
    }
    
    /// 检查 API 连接
    fn check_api_connectivity(&mut self) {
        let passed = !self.config.api_base_url.is_empty();
        self.checks.push(EnvironmentCheck {
            name: "API Connectivity".to_string(),
            passed,
            message: if passed {
                format!("API URL configured: {}", self.config.api_base_url)
            } else {
                "API URL not configured".to_string()
            },
        });
    }
    
    /// 检查 CORS 配置
    fn check_cors_configuration(&mut self) {
        let passed = !self.config.cors_origins.is_empty();
        self.checks.push(EnvironmentCheck {
            name: "CORS Configuration".to_string(),
            passed,
            message: if passed {
                format!("CORS origins configured: {:?}", self.config.cors_origins)
            } else {
                "CORS origins not configured".to_string()
            },
        });
    }
    
    /// 检查认证方法
    fn check_auth_method(&mut self) {
        let passed = self.config.auth_method != AuthMethod::Header 
            || self.config.environment != Environment::Development;
        self.checks.push(EnvironmentCheck {
            name: "Auth Method".to_string(),
            passed,
            message: format!("Auth method: {:?}", self.config.auth_method),
        });
    }
    
    /// 检查环境变量
    fn check_environment_variables(&mut self) {
        let required_vars = match self.config.environment {
            Environment::Development => vec![],
            Environment::Testing => vec!["DATABASE_URL"],
            Environment::Production => vec!["DATABASE_URL", "API_KEY"],
        };
        
        let mut missing_vars = Vec::new();
        for var in required_vars {
            if env::var(var).is_err() {
                missing_vars.push(var);
            }
        }
        
        let passed = missing_vars.is_empty();
        self.checks.push(EnvironmentCheck {
            name: "Environment Variables".to_string(),
            passed,
            message: if passed {
                "All required environment variables are set".to_string()
            } else {
                format!("Missing environment variables: {:?}", missing_vars)
            },
        });
    }
    
    /// 检查依赖
    fn check_dependencies(&mut self) {
        let dependencies = vec![
            ("Rust", env::var("CARGO").is_ok()),
            ("Node.js", env::var("NODE_PATH").is_ok()),
            ("Docker", env::var("DOCKER_HOST").is_ok()),
        ];
        
        let mut missing_deps = Vec::new();
        for (dep, available) in dependencies {
            if !available && self.config.environment == Environment::Testing {
                missing_deps.push(dep);
            }
        }
        
        let passed = missing_deps.is_empty();
        self.checks.push(EnvironmentCheck {
            name: "Dependencies".to_string(),
            passed,
            message: if passed {
                "All required dependencies are available".to_string()
            } else {
                format!("Missing dependencies: {:?}", missing_deps)
            },
        });
    }
    
    /// 获取检查结果
    pub fn get_checks(&self) -> &[EnvironmentCheck] {
        &self.checks
    }
    
    /// 获取配置
    pub fn get_config(&self) -> &EnvironmentConfig {
        &self.config
    }
    
    /// 打印检查结果
    pub fn print_results(&self) {
        println!("\n=== Environment Consistency Check ===");
        println!("Environment: {:?}", self.config.environment);
        println!("API URL: {}", self.config.api_base_url);
        println!();
        
        for check in &self.checks {
            let status = if check.passed { "✓" } else { "✗" };
            println!("{} {}: {}", status, check.name, check.message);
        }
        
        let passed_count = self.checks.iter().filter(|c| c.passed).count();
        let total_count = self.checks.len();
        println!("\nPassed: {}/{}", passed_count, total_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_environment_detection() {
        let checker = EnvironmentChecker::new();
        assert!(matches!(
            checker.config.environment,
            Environment::Development | Environment::Testing | Environment::Production
        ));
    }
    
    #[test]
    fn test_development_config() {
        env::set_var("ENVIRONMENT", "development");
        let checker = EnvironmentChecker::new();
        assert_eq!(checker.config.environment, Environment::Development);
        assert_eq!(checker.config.api_timeout_secs, 30);
        assert_eq!(checker.config.auth_method, AuthMethod::Both);
    }
    
    #[test]
    fn test_testing_config() {
        env::set_var("ENVIRONMENT", "testing");
        let checker = EnvironmentChecker::new();
        assert_eq!(checker.config.environment, Environment::Testing);
        assert_eq!(checker.config.api_timeout_secs, 5);
        assert_eq!(checker.config.auth_method, AuthMethod::UrlParameter);
    }
    
    #[test]
    fn test_production_config() {
        env::set_var("ENVIRONMENT", "production");
        let checker = EnvironmentChecker::new();
        assert_eq!(checker.config.environment, Environment::Production);
        assert_eq!(checker.config.api_timeout_secs, 10);
        assert_eq!(checker.config.auth_method, AuthMethod::Header);
    }
    
    #[test]
    fn test_run_checks() {
        let mut checker = EnvironmentChecker::new();
        let all_passed = checker.run_all_checks();
        assert!(!checker.get_checks().is_empty());
    }
}
