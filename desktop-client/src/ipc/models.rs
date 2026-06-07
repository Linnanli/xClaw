//! 模型配置 IPC 命令。
//!
//! 从 Admin Backend 拉取可用模型列表，并支持本地自定义模型管理。
//! 自定义模型存储在本地 JSON 文件中，与后台下发的模型合并展示。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;
use tracing::{debug, info};

use crate::state::EngineState;

/// 客户端模型配置（与 Admin Backend ClientModel 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_id: String,
    pub display_name: String,
    pub provider: String,
    #[serde(default)]
    pub provider_display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_capabilities")]
    pub capabilities: serde_json::Value,
    /// API Base URL（后台下发，客户端直连时使用）
    #[serde(default)]
    pub api_base_url: Option<String>,
    /// API Key（脱敏，仅用于判断是否已配置）
    #[serde(default)]
    pub api_key: Option<String>,
    /// API 格式：openai / anthropic
    #[serde(default = "default_api_format")]
    pub api_format: String,
    /// 标记来源：admin（后台下发）/ custom（本地自定义）/ builtin（内置兜底）
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_api_format() -> String {
    "openai".to_string()
}

fn default_source() -> String {
    "admin".to_string()
}

fn default_capabilities() -> serde_json::Value {
    serde_json::json!([])
}

/// 本地自定义模型（含 API 配置）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModel {
    pub model_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub provider: String,
    pub api_base_url: String,
    pub api_key: String,
    pub capabilities: serde_json::Value,
    pub extra_config: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

/// 本地自定义模型存储
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CustomModelStore {
    models: Vec<CustomModel>,
}

/// 获取自定义模型存储文件路径
fn custom_models_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ironclaw");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("custom_models.json")
}

/// 读取本地自定义模型
fn load_custom_models() -> CustomModelStore {
    let path = custom_models_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => CustomModelStore::default(),
    }
}

pub(crate) fn model_fetch_client_id() -> Result<uuid::Uuid, String> {
    let token = std::env::var("ADMIN_AUTH_TOKEN")
        .map_err(|_| "ADMIN_AUTH_TOKEN 未配置，无法拉取模型列表".to_string())?;
    uuid::Uuid::parse_str(token.trim())
        .map_err(|_| "ADMIN_AUTH_TOKEN 不是有效客户端 ID，无法拉取模型列表".to_string())
}

pub(crate) fn client_models_url(admin_url: &str, client_id: uuid::Uuid) -> String {
    format!("{}/api/client-models?client_id={}", admin_url, client_id)
}

/// 从本地自定义模型存储中查找指定 model_id 的真实 API key。
///
/// 供 `switch_provider` 在收到脱敏值（`"****"`）时回退调用，
/// 避免因前端不回显 key 而导致跨 provider 切换失败。
pub(crate) fn lookup_custom_api_key(model_id: &str) -> Option<String> {
    let store = load_custom_models();
    store
        .models
        .into_iter()
        .find(|m| m.model_id == model_id)
        .map(|m| m.api_key)
        .filter(|k| !k.is_empty())
}

/// 保存本地自定义模型
fn save_custom_models(store: &CustomModelStore) -> Result<(), String> {
    let path = custom_models_path();
    let content = serde_json::to_string_pretty(store).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(())
}

/// 获取所有可用模型（后台下发）
///
/// Admin Backend 是客户端模型列表的唯一权威来源；只有 Admin 成功返回空列表时
/// 才向 UI 返回空。请求失败必须保留错误，避免把真实问题伪装成“无模型”。
#[tauri::command]
pub async fn get_available_models(
    _engine: State<'_, EngineState>,
) -> Result<Vec<ModelConfig>, String> {
    let client_id = model_fetch_client_id()?;
    let models = fetch_admin_models(client_id).await?;
    debug!(count = models.len(), "从后台获取模型列表");

    Ok(models)
}

/// 获取本地自定义模型列表
#[tauri::command]
pub async fn get_custom_models() -> Result<Vec<CustomModel>, String> {
    let store = load_custom_models();
    Ok(store.models)
}

/// 创建本地自定义模型
#[tauri::command]
pub async fn create_custom_model(
    model_id: String,
    display_name: String,
    description: Option<String>,
    provider: String,
    api_base_url: String,
    api_key: String,
) -> Result<CustomModel, String> {
    if model_id.trim().is_empty() {
        return Err("model_id 不能为空".to_string());
    }
    if display_name.trim().is_empty() {
        return Err("display_name 不能为空".to_string());
    }
    if api_base_url.trim().is_empty() {
        return Err("api_base_url 不能为空".to_string());
    }

    let mut store = load_custom_models();

    // 检查 model_id 唯一性
    if store.models.iter().any(|m| m.model_id == model_id) {
        return Err(format!("model_id '{}' 已存在", model_id));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let model = CustomModel {
        model_id,
        display_name,
        description,
        provider,
        api_base_url,
        api_key,
        capabilities: serde_json::json!([]),
        extra_config: serde_json::json!({}),
        created_at: now.clone(),
        updated_at: now,
    };

    store.models.push(model.clone());
    save_custom_models(&store)?;

    debug!(model_id = %model.model_id, "创建自定义模型");
    Ok(model)
}

/// 更新本地自定义模型
#[tauri::command]
pub async fn update_custom_model(
    model_id: String,
    display_name: Option<String>,
    description: Option<String>,
    provider: Option<String>,
    api_base_url: Option<String>,
    api_key: Option<String>,
) -> Result<CustomModel, String> {
    let mut store = load_custom_models();

    let model = store
        .models
        .iter_mut()
        .find(|m| m.model_id == model_id)
        .ok_or_else(|| format!("模型 '{}' 不存在", model_id))?;

    if let Some(name) = display_name {
        model.display_name = name;
    }
    if let Some(desc) = description {
        model.description = Some(desc);
    }
    if let Some(prov) = provider {
        model.provider = prov;
    }
    if let Some(url) = api_base_url {
        model.api_base_url = url;
    }
    if let Some(key) = api_key {
        model.api_key = key;
    }
    model.updated_at = chrono::Utc::now().to_rfc3339();

    let updated = model.clone();
    save_custom_models(&store)?;

    debug!(model_id = %updated.model_id, "更新自定义模型");
    Ok(updated)
}

/// 删除本地自定义模型
#[tauri::command]
pub async fn delete_custom_model(model_id: String) -> Result<(), String> {
    let mut store = load_custom_models();
    let before = store.models.len();
    store.models.retain(|m| m.model_id != model_id);

    if store.models.len() == before {
        return Err(format!("模型 '{}' 不存在", model_id));
    }

    save_custom_models(&store)?;
    debug!(model_id = %model_id, "删除自定义模型");
    Ok(())
}

/// 测试模型连接
#[tauri::command]
pub async fn test_model_connection(
    api_base_url: String,
    api_key: String,
    model_id: String,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 发送一个简单的 chat completion 请求测试连接
    let url = format!("{}/chat/completions", api_base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model_id,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 5,
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("连接失败: {}", e))?;

    let status = response.status();
    if status.is_success() {
        Ok(serde_json::json!({
            "success": true,
            "message": "连接成功",
            "status": status.as_u16(),
        }))
    } else {
        let error_text = response.text().await.unwrap_or_default();
        Ok(serde_json::json!({
            "success": false,
            "message": format!("连接失败 ({})", status),
            "status": status.as_u16(),
            "error": error_text,
        }))
    }
}

/// 从 Admin Backend 拉取模型列表
pub(crate) async fn fetch_admin_models(client_id: uuid::Uuid) -> Result<Vec<ModelConfig>, String> {
    // 从环境变量获取 admin backend URL
    let admin_url =
        std::env::var("ADMIN_BACKEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let url = client_models_url(&admin_url, client_id);
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let models: Vec<ModelConfig> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let default_model = models
        .iter()
        .find(|model| model.is_default)
        .map(|model| model.model_id.as_str())
        .unwrap_or("<none>");
    let first_model = models
        .first()
        .map(|model| model.model_id.as_str())
        .unwrap_or("<none>");
    info!(
        url = %url,
        count = models.len(),
        default_model,
        first_model,
        "Admin client model list fetched for UI"
    );

    Ok(models)
}
