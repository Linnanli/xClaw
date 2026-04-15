use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export TokenClaims from shared auth crate
pub use ironclaw_auth::TokenClaims;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    /// 登录用户基本信息（供前端展示和权限判断）
    pub user: LoginUserInfo,
}

/// 登录响应中携带的用户基本信息（不含敏感字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginUserInfo {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    /// 用户的角色名称列表
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersion {
    pub id: Uuid,
    pub skill_id: Uuid,
    pub version: String,
    pub changelog: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub action: String,
    pub details: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpRule {
    pub id: Uuid,
    pub name: String,
    pub pattern: String,
    pub replacement: String,
    pub severity: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub category: String,
    pub created_by: Uuid,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveOperationRule {
    pub id: Uuid,
    pub name: String,
    pub operation_type: String,
    pub requires_approval: bool,
    pub risk_level: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_by: Uuid,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyChangeRecord {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub change_type: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub changed_by: Uuid,
    pub changed_at: DateTime<Utc>,
    pub reason: Option<String>,
}

// DLP Rule request models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDlpRuleRequest {
    pub name: String,
    pub pattern: String,
    pub replacement: Option<String>,
    pub severity: String,
    pub description: Option<String>,
    pub category: String,
    /// 规则类型: "regex"（默认）或 "keyword"
    #[serde(default = "default_rule_type")]
    pub rule_type: String,
    /// 规则额外配置（JSON），keyword 类型包含 keywords, match_mode, case_sensitive
    pub rule_config: Option<serde_json::Value>,
    /// 数据分级标签: public / internal / confidential / top_secret
    pub classification_level: Option<String>,
}

fn default_rule_type() -> String {
    "regex".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDlpRuleRequest {
    pub name: Option<String>,
    pub pattern: Option<String>,
    pub replacement: Option<String>,
    pub severity: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub category: Option<String>,
    pub rule_type: Option<String>,
    pub rule_config: Option<serde_json::Value>,
    pub classification_level: Option<String>,
}

// Dictionary models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDictionaryRequest {
    pub name: String,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDictionaryRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<Vec<String>>,
}

// Sensitive Operation request models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSensitiveOperationRequest {
    pub name: String,
    pub operation_type: String,
    pub requires_approval: Option<bool>,
    pub risk_level: Option<String>,
    pub description: Option<String>,
    pub approver_roles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSensitiveOperationRequest {
    pub name: Option<String>,
    pub operation_type: Option<String>,
    pub requires_approval: Option<bool>,
    pub risk_level: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub approver_roles: Option<Vec<String>>,
}

// Plugin model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// System settings request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfigRequest {
    pub dlp_enabled: Option<bool>,
    pub dlp_scan_timeout_ms: Option<i64>,
    pub dlp_fail_open: Option<bool>,
    pub audit_retention_days: Option<i64>,
    pub audit_enabled: Option<bool>,
    pub client_heartbeat_interval_s: Option<i64>,
    pub client_offline_threshold_s: Option<i64>,
    pub policy_sync_interval_s: Option<i64>,
    pub policy_auto_push: Option<bool>,
}

// Role models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<Uuid>,
}

// Permission models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub resource: String,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleWithPermissions {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
    pub user_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// 用户编辑请求
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub password: Option<String>,
    /// None = 不修改, Some(None) = 清除部门, Some(Some(id)) = 设置部门
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub department_id: Option<Option<Uuid>>,
}

/// 自定义反序列化：支持 null（清除）、缺失（不修改）、UUID 值（设置）
fn deserialize_optional_uuid<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<Option<Uuid>> = Option::deserialize(deserializer)?;
    Ok(Some(opt.unwrap_or(None)))
}

// ============================================================================
// 部门管理请求
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub token_quota_enabled: Option<bool>,
    pub token_quota_per_day: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDepartmentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    /// None = 不修改, Some(None) = 清除父部门（提升为根）, Some(Some(id)) = 设置上级部门
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub parent_id: Option<Option<Uuid>>,
    pub token_quota_enabled: Option<bool>,
    pub token_quota_per_day: Option<Option<i32>>,
}

/// 部门查询参数（搜索 + 筛选 + 树形）
#[derive(Debug, Clone, Deserialize)]
pub struct DepartmentQuery {
    pub search: Option<String>,
    /// "enabled" | "disabled" | 不传则不筛选
    pub quota_status: Option<String>,
    /// true 时返回嵌套树形结构，false（默认）返回扁平列表
    #[serde(default)]
    pub tree: Option<bool>,
}

/// 部门模型白名单更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateModelWhitelistRequest {
    /// 模型配置 ID 列表（全量替换）
    pub model_config_ids: Vec<Uuid>,
}

/// 部门成员查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct DepartmentMembersQuery {
    pub search: Option<String>,
}

// ============================================================================
// 审计日志导出查询参数
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct AuditLogExportQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub action: Option<String>,
    pub username: Option<String>,
}

// ============================================================================
// 客户端配置更新请求
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateClientConfigRequest {
    pub llm_backend: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
    pub llm_base_url: Option<String>,
    pub safety_enabled: Option<bool>,
    pub skills_enabled: Option<bool>,
    pub extensions_enabled: Option<bool>,
    pub max_cost_per_day_cents: Option<i64>,
}

// ============================================================================
// 模型配置
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: Uuid,
    pub model_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub provider: String,
    pub api_base_url: Option<String>,
    /// API Key — 列表接口返回脱敏值，详情接口可选返回原文
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub sort_order: i32,
    pub capabilities: serde_json::Value,
    pub extra_config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModelConfigRequest {
    pub model_id: String,
    pub display_name: String,
    pub description: Option<String>,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default = "default_sort_order")]
    pub sort_order: i32,
    pub capabilities: Option<serde_json::Value>,
    pub extra_config: Option<serde_json::Value>,
    /// 单价（分/千Token）
    pub input_price_per_1k_cents: Option<i32>,
    pub output_price_per_1k_cents: Option<i32>,
}

fn default_provider() -> String {
    "custom".to_string()
}

fn default_sort_order() -> i32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateModelConfigRequest {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
    pub sort_order: Option<i32>,
    pub capabilities: Option<serde_json::Value>,
    pub extra_config: Option<serde_json::Value>,
    /// 调用统计（可由外部写入）
    pub total_calls: Option<i64>,
    pub avg_latency_ms: Option<f64>,
    /// 单价（分/千Token）
    pub input_price_per_1k_cents: Option<i32>,
    pub output_price_per_1k_cents: Option<i32>,
}

/// 测试模型连接请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionRequest {
    pub provider: String,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_id: Option<String>,
}

/// 客户端侧模型配置（供 Desktop Client 拉取，包含直连 LLM API 所需的全部信息）。
///
/// 与管理端 `ModelConfigResponse` 的区别：
/// - 包含完整 `api_key`（Desktop Client 直连 LLM API 需要真实 key，传输安全由 HTTPS 保证）
/// - 包含 `source` 字段标记来源（"admin"），客户端合并本地自定义模型时用于区分
/// - `capabilities` 使用 `Vec<String>` 强类型，避免前端收到非数组值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientModelConfig {
    pub model_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub provider: String,
    pub is_default: bool,
    pub capabilities: Vec<String>,
    /// API Base URL（客户端直连时使用）
    pub api_base_url: Option<String>,
    /// API Key（完整下发，客户端直连 LLM API 需要真实 key）
    pub api_key: Option<String>,
    /// API 格式：openai / anthropic
    #[serde(default = "default_api_format")]
    pub api_format: String,
    /// 来源标记：admin（后台下发）
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_api_format() -> String {
    "openai".to_string()
}

fn default_source() -> String {
    "admin".to_string()
}

// ============================================================================
// 告警与通知系统
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlertRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    #[serde(default = "default_empty_json")]
    pub condition: serde_json::Value,
    pub severity: String,
    #[serde(default)]
    pub notify_channels: Vec<String>,
    #[serde(default = "default_silence_minutes")]
    pub silence_minutes: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAlertRuleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub event_type: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub severity: Option<String>,
    pub notify_channels: Option<Vec<String>>,
    pub silence_minutes: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AlertEventQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertEventStatusRequest {
    pub status: String,
    pub note: Option<String>,
}

/// trigger_alert 的输入参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertTrigger {
    pub event_type: String,
    pub severity: String,
    pub detail: String,
    pub event_data: Option<serde_json::Value>,
}

fn default_empty_json() -> serde_json::Value {
    serde_json::json!({})
}

fn default_silence_minutes() -> i32 {
    60
}

fn default_true() -> bool {
    true
}

// ============================================================================
// 对话审计
// ============================================================================

/// 客户端上报对话的 payload 结构（嵌入 client-reports 的 data 字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationReportPayload {
    pub client_conversation_id: String,
    pub user_id: Uuid,
    pub topic: Option<String>,
    pub model_id: Option<String>,
    pub dlp_flagged: Option<bool>,
    pub dlp_details: Option<String>,
    #[serde(default)]
    pub used_skills: Vec<String>,
    pub messages: Vec<ConversationMessagePayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationAttachmentPayload {
    pub id: String,
    pub kind: String,
    pub mime_type: String,
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub extracted_text: Option<String>,
    pub image_data_base64: Option<String>,
    pub duration_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessagePayload {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<ConversationAttachmentPayload>,
    pub model_id: Option<String>,
    #[serde(default)]
    pub input_tokens: i32,
    #[serde(default)]
    pub output_tokens: i32,
}

#[derive(Debug, Deserialize)]
pub struct ConversationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub username: Option<String>,
    pub dlp_flagged: Option<bool>,
    pub search: Option<String>,
}

// ============================================================================
// 操作审批流
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApprovalRequest {
    pub applicant_id: Uuid,
    pub operation_rule_id: Option<Uuid>,
    pub operation_type: String,
    pub operation_name: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewApprovalRequest {
    pub action: String, // "approve" | "reject"
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub status: Option<String>,
}

// ============================================================================
// 数据分类分级与合规
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateReportRequest {
    pub name: String,
    pub report_type: Option<String>,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRetentionPolicyRequest {
    pub retention_days: i32,
}
