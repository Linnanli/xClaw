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
