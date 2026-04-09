//! API 处理器模块
//!
//! 提供策略查询和管理的 HTTP API 端点

pub mod alerts;
pub mod approvals;
pub mod compliance;
pub mod conversations;
pub mod departments;
pub mod extensions;
pub mod knowledge_base;
pub mod quota;
pub mod reports;

use crate::db::Database;
use crate::error::Result;
use crate::models::{DlpRule, SensitiveOperationRule};
use crate::policy_management::{PolicyManagementService, PolicyVersionInfo};
use crate::AppState;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

/// 策略查询请求参数
#[derive(Debug, Deserialize)]
pub struct PolicyQueryParams {
    /// 是否包含禁用的规则
    #[serde(default)]
    pub include_disabled: bool,
}

/// 策略响应
#[derive(Debug, Serialize, Deserialize)]
pub struct PoliciesResponse {
    pub dlp_rules: Vec<DlpRule>,
    pub sensitive_ops_rules: Vec<SensitiveOperationRule>,
    pub version: PolicyVersionInfo,
}

/// GET /api/policies - 获取所有策略
#[instrument(skip(state))]
pub async fn get_policies_handler(
    State(state): State<AppState>,
    Query(params): Query<PolicyQueryParams>,
) -> Result<Json<PoliciesResponse>> {
    info!(
        "Fetching all policies, include_disabled: {}",
        params.include_disabled
    );

    let db = Database::new(state.db_pool.clone());
    let service = PolicyManagementService::new(db);

    let dlp_rules = service.get_dlp_rules(params.include_disabled).await?;
    let sensitive_ops_rules = service
        .get_sensitive_op_rules(params.include_disabled)
        .await?;
    let version = service.get_policy_version_info().await?;

    debug!(
        dlp_count = dlp_rules.len(),
        sensitive_ops_count = sensitive_ops_rules.len(),
        "Policies fetched successfully"
    );

    Ok(Json(PoliciesResponse {
        dlp_rules,
        sensitive_ops_rules,
        version,
    }))
}

/// GET /api/policies/dlp - 获取所有 DLP 规则
#[instrument(skip(state))]
pub async fn get_dlp_policies_handler(
    State(state): State<AppState>,
    Query(params): Query<PolicyQueryParams>,
) -> Result<Json<Vec<DlpRule>>> {
    info!(
        "Fetching DLP policies, include_disabled: {}",
        params.include_disabled
    );

    let db = Database::new(state.db_pool.clone());
    let service = PolicyManagementService::new(db);

    let rules = service.get_dlp_rules(params.include_disabled).await?;

    debug!(count = rules.len(), "DLP policies fetched successfully");

    Ok(Json(rules))
}

/// GET /api/policies/sensitive-ops - 获取所有敏感操作规则
#[instrument(skip(state))]
pub async fn get_sensitive_ops_policies_handler(
    State(state): State<AppState>,
    Query(params): Query<PolicyQueryParams>,
) -> Result<Json<Vec<SensitiveOperationRule>>> {
    info!(
        "Fetching sensitive operation policies, include_disabled: {}",
        params.include_disabled
    );

    let db = Database::new(state.db_pool.clone());
    let service = PolicyManagementService::new(db);

    let rules = service
        .get_sensitive_op_rules(params.include_disabled)
        .await?;

    debug!(
        count = rules.len(),
        "Sensitive operation policies fetched successfully"
    );

    Ok(Json(rules))
}

/// GET /api/policies/version - 获取策略版本信息
#[instrument(skip(state))]
pub async fn get_policy_version_handler(
    State(state): State<AppState>,
) -> Result<Json<PolicyVersionInfo>> {
    info!("Fetching policy version information");

    let db = Database::new(state.db_pool.clone());
    let service = PolicyManagementService::new(db);

    let version = service.get_policy_version_info().await?;

    debug!(
        dlp_version = version.dlp_rules_version,
        sensitive_ops_version = version.sensitive_ops_version,
        "Policy version fetched successfully"
    );

    Ok(Json(version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_query_params_default() {
        let json = r#"{}"#;
        let params: PolicyQueryParams = serde_json::from_str(json).unwrap();
        assert!(!params.include_disabled);
    }

    #[test]
    fn test_policy_query_params_with_disabled() {
        let json = r#"{"include_disabled": true}"#;
        let params: PolicyQueryParams = serde_json::from_str(json).unwrap();
        assert!(params.include_disabled);
    }

    #[test]
    fn test_policies_response_serialization() {
        let response = PoliciesResponse {
            dlp_rules: vec![],
            sensitive_ops_rules: vec![],
            version: PolicyVersionInfo {
                dlp_rules_version: 1,
                sensitive_ops_version: 1,
                last_updated: chrono::Utc::now(),
                total_dlp_rules: 0,
                total_sensitive_ops: 0,
                active_dlp_rules: 0,
                active_sensitive_ops: 0,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("dlp_rules"));
        assert!(json.contains("sensitive_ops_rules"));
        assert!(json.contains("version"));
    }
}
