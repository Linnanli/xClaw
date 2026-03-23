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
fn deserialize_optional_uuid<'de, D>(deserializer: D) -> std::result::Result<Option<Option<Uuid>>, D::Error>
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
    #[serde(default)]
    pub token_quota_enabled: Option<bool>,
    pub token_quota_per_day: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDepartmentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub token_quota_enabled: Option<bool>,
    pub token_quota_per_day: Option<Option<i32>>,
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
