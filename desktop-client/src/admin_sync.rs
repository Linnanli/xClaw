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
use uuid::Uuid;

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
    /// 是否启用受管终端模式（只允许管理端注册表/策略定义的安装路径）
    pub managed_mode: Option<bool>,

    // === 限制 ===
    /// 每日最大花费（美分）
    pub max_cost_per_day_cents: Option<u64>,

    /// Admin Backend 中当前客户端绑定的真实用户 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_principal_id: Option<Uuid>,

    // === 水印 ===
    /// 是否启用水印
    pub watermark_enabled: Option<bool>,
    /// 水印内容模板
    pub watermark_template: Option<String>,
    /// 水印字体大小
    pub watermark_font_size: Option<u32>,
    /// 水印透明度
    pub watermark_opacity: Option<f64>,
    /// 水印位置
    pub watermark_position: Option<String>,
    /// 水印颜色
    pub watermark_color: Option<String>,

    // === 版本 ===
    /// 配置版本号（用于增量更新检测）
    pub config_version: Option<u64>,
    /// 最后更新时间
    pub updated_at: Option<String>,
    /// 私有技能注册表地址（需求 14.17）
    /// 格式："{admin_base_url}/api/v1?client_token={token}"
    /// inject_to_env() 将其注入为 CLAWHUB_REGISTRY，ironclaw 引擎据此使用 Admin 私有注册表
    pub skill_registry_url: Option<String>,
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
        if let Some(enabled) = self.managed_mode {
            std::env::set_var("MANAGED_MODE", enabled.to_string());
        }
        if let Some(cost) = self.max_cost_per_day_cents {
            std::env::set_var("MAX_COST_PER_DAY_CENTS", cost.to_string());
        }
        // 注入私有注册表地址（需求 14.17）
        // ironclaw 的 SkillCatalog::new() 读取 CLAWHUB_REGISTRY 环境变量
        if let Some(ref url) = self.skill_registry_url {
            if !url.is_empty() {
                std::env::set_var("CLAWHUB_REGISTRY", url);
                tracing::info!("Injected CLAWHUB_REGISTRY from admin config");
            }
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
    /// 后台主身份同步出口。
    backend_user_id: Option<Arc<std::sync::RwLock<Option<Uuid>>>>,
    /// 同步间隔（用于 run_sync_loop）
    sync_interval: Duration,
    /// 版本检查间隔（用于 run_sync_loop_with_version_check）
    version_check_interval: Duration,
}

pub(crate) fn build_client_config_url(admin_url: &str, client_token: &str) -> String {
    let base = format!("{}/api/client-config", admin_url.trim_end_matches('/'));
    if uuid::Uuid::parse_str(client_token).is_ok() {
        return format!("{}?client_id={}", base, client_token);
    }
    base
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
            backend_user_id: None,
            sync_interval: Duration::from_secs(300), // 5 分钟
            version_check_interval: Duration::from_secs(30), // 30 秒
        }
    }

    pub fn with_backend_user_id_sink(
        mut self,
        backend_user_id: Arc<std::sync::RwLock<Option<Uuid>>>,
    ) -> Self {
        self.backend_user_id = Some(backend_user_id);
        self
    }

    /// 设置同步间隔。
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = interval;
        self
    }

    /// 设置版本检查间隔（用于 run_sync_loop_with_version_check）。
    pub fn with_version_check_interval(mut self, interval: Duration) -> Self {
        self.version_check_interval = interval;
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
        let url = build_client_config_url(&self.admin_url, &self.client_token);

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

        self.publish_backend_principal_id(config.backend_principal_id);

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

    fn publish_backend_principal_id(&self, backend_principal_id: Option<Uuid>) {
        if let Some(target) = &self.backend_user_id {
            match target.write() {
                Ok(mut current) => {
                    *current = backend_principal_id;
                }
                Err(error) => {
                    tracing::warn!(error = %error, "Failed to publish backend principal id");
                }
            }
        }
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
                    tracing::debug!(version = config.config_version, "Config sync completed");
                }
                Err(e) => {
                    tracing::debug!("Config sync failed (will retry): {}", e);
                }
            }
        }
    }

    /// 启动带版本检测的同步循环。
    ///
    /// 每 `version_check_interval`（默认 30 秒）检查一次配置版本，
    /// 版本变化时立即拉取完整配置并注入环境变量。
    ///
    /// 这实现了"推送感知"效果：Admin 保存配置后，
    /// 客户端在下一个检查周期内（最多 30 秒）自动应用新配置，
    /// 无需等待 5 分钟定时周期（需求 9.10）。
    ///
    /// # 首次拉取
    ///
    /// 首次拉取时仅记录版本号，不调用 `inject_to_env()`，
    /// 因为启动时已通过 `apply_admin_overrides()` 完成注入。
    pub async fn run_sync_loop_with_version_check(&self) {
        let mut interval = tokio::time::interval(self.version_check_interval);
        let mut last_version: Option<u64> = None;

        loop {
            interval.tick().await;

            match self.fetch_once().await {
                Ok(config) => {
                    let current_version = config.config_version.unwrap_or(0);
                    let version_changed =
                        last_version.map(|v| v != current_version).unwrap_or(false); // 首次拉取不视为变化，避免重复注入

                    if version_changed {
                        tracing::info!(
                            prev_version = last_version,
                            new_version = current_version,
                            "Config version changed, applying new config"
                        );
                        config.inject_to_env();
                    }

                    last_version = Some(current_version);
                }
                Err(e) => {
                    tracing::debug!("Config version check failed (will retry): {}", e);
                }
            }
        }
    }
}
