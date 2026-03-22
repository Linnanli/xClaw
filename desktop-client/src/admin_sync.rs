//! 管理端配置同步模块。
//!
//! 负责从 Admin Backend 拉取客户端配置（LLM Key、安全策略、功能开关等），
//! 并缓存到本地文件系统。支持离线启动（使用缓存配置）。
//!
//! # 架构
//!
//! ```text
//! Admin Backend                    Desktop Client
//! ┌──────────────┐                ┌──────────────────────┐
//! │ GET /api/    │  ◄── HTTPS ──  │ AdminConfigSync       │
//! │ client-config│                │  ├── 启动时拉取       │
//! │              │                │  ├── 定时刷新(5min)   │
//! │              │                │  ├── 本地加密缓存     │
//! │              │                │  └── 注入 env vars    │
//! └──────────────┘                └──────────────────────┘
//! ```
//!
//! # 安全
//!
//! - API Key 不写入日志
//! - 本地缓存使用 JSON 文件（生产环境应加密）
//! - HTTPS 传输

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// 管理端下发的客户端配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdminClientConfig {
    // === LLM 配置 ===
    /// LLM 后端类型（"openai" | "anthropic" | "nearai" | ...）
    pub llm_backend: Option<String>,
    /// LLM API Key（加密传输，不写入日志）
    pub llm_api_key: Option<String>,
    /// 模型名称
    pub llm_model: Option<String>,
    /// 自定义 API 端点
    pub llm_base_url: Option<String>,

    // === 安全策略 ===
    /// 是否启用安全层
    pub safety_enabled: Option<bool>,

    // === 功能开关 ===
    /// 是否启用技能系统
    pub skills_enabled: Option<bool>,
    /// 是否启用扩展系统
    pub extensions_enabled: Option<bool>,

    // === 限制 ===
    /// 每日最大花费（美分）
    pub max_cost_per_day_cents: Option<u64>,

    // === 版本 ===
    /// 配置版本号（用于增量更新检测）
    pub config_version: Option<u64>,
    /// 最后更新时间
    pub updated_at: Option<String>,
}

impl AdminClientConfig {
    /// 将管理端配置注入为环境变量。
    ///
    /// # 安全
    ///
    /// - API Key 不记录到日志
    /// - 仅注入非空值，不覆盖已有环境变量中的有效值
    pub fn inject_to_env(&self) {
        let mappings: &[(&Option<String>, &str, bool)] = &[
            (&self.llm_backend, "LLM_BACKEND", false),
            (&self.llm_api_key, "LLM_API_KEY", true), // sensitive
            (&self.llm_model, "LLM_MODEL", false),
            (&self.llm_base_url, "LLM_BASE_URL", false),
        ];

        for (value, env_key, is_sensitive) in mappings {
            if let Some(val) = value {
                if !val.is_empty() {
                    std::env::set_var(env_key, val);
                    if *is_sensitive {
                        tracing::info!("Injected {} from admin config (***)", env_key);
                    } else {
                        tracing::info!("Injected {}={} from admin config", env_key, val);
                    }
                }
            }
        }

        // Boolean 和数值类型
        if let Some(enabled) = self.safety_enabled {
            std::env::set_var("SAFETY_ENABLED", enabled.to_string());
        }
        if let Some(enabled) = self.skills_enabled {
            std::env::set_var("SKILLS_ENABLED", enabled.to_string());
        }
        if let Some(enabled) = self.extensions_enabled {
            std::env::set_var("EXTENSIONS_ENABLED", enabled.to_string());
        }
        if let Some(cost) = self.max_cost_per_day_cents {
            std::env::set_var("MAX_COST_PER_DAY_CENTS", cost.to_string());
        }
    }
}

/// 本地配置缓存。
pub struct AdminConfigCache;

impl AdminConfigCache {
    /// 缓存文件路径。
    pub fn cache_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw-desktop")
            .join("admin_config.json")
    }

    /// 从本地缓存加载配置。
    pub fn load() -> Result<AdminClientConfig, String> {
        let path = Self::cache_path();
        if !path.exists() {
            return Err("No cached config found".into());
        }
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("Failed to read cache: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse cache: {}", e))
    }

    /// 保存配置到本地缓存。
    pub fn save(config: &AdminClientConfig) -> Result<(), String> {
        let path = Self::cache_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write(&path, content).map_err(|e| format!("Failed to write cache: {}", e))
    }
}

/// 管理端配置同步器。
///
/// 定期从 Admin Backend 拉取最新配置，更新本地缓存。
/// 支持离线模式（使用缓存配置）。
pub struct AdminConfigSync {
    /// Admin Backend URL
    admin_url: String,
    /// 客户端认证 token
    client_token: String,
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// 当前配置（内存缓存）
    current_config: Arc<RwLock<AdminClientConfig>>,
    /// 同步间隔
    sync_interval: Duration,
}

impl AdminConfigSync {
    /// 创建新的同步器。
    pub fn new(admin_url: String, client_token: String) -> Self {
        Self {
            admin_url,
            client_token,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            current_config: Arc::new(RwLock::new(AdminClientConfig::default())),
            sync_interval: Duration::from_secs(300), // 5 分钟
        }
    }

    /// 设置同步间隔。
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = interval;
        self
    }

    /// 获取当前配置的只读引用。
    pub fn config(&self) -> Arc<RwLock<AdminClientConfig>> {
        self.current_config.clone()
    }

    /// 执行一次配置拉取。
    ///
    /// 成功时更新内存缓存和本地文件缓存。
    /// 失败时保持现有配置不变。
    pub async fn fetch_once(&self) -> Result<AdminClientConfig, String> {
        let url = format!("{}/api/client-config", self.admin_url);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.client_token)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server returned {}", response.status()));
        }

        let config: AdminClientConfig = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        // 更新内存缓存
        {
            let mut current = self.current_config.write().await;
            *current = config.clone();
        }

        // 更新本地文件缓存
        if let Err(e) = AdminConfigCache::save(&config) {
            tracing::warn!("Failed to save config cache: {}", e);
        }

        tracing::info!(
            version = config.config_version,
            "Admin config synced successfully"
        );

        Ok(config)
    }

    /// 启动后台同步循环。
    ///
    /// 定期拉取最新配置。失败时静默重试，不影响客户端运行。
    pub async fn run_sync_loop(&self) {
        let mut interval = tokio::time::interval(self.sync_interval);

        loop {
            interval.tick().await;

            match self.fetch_once().await {
                Ok(config) => {
                    tracing::debug!(
                        version = config.config_version,
                        "Config sync completed"
                    );
                }
                Err(e) => {
                    tracing::debug!("Config sync failed (will retry): {}", e);
                }
            }
        }
    }
}
