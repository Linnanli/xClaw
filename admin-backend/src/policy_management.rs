//! 策略管理模块
//!
//! 提供DLP策略和敏感操作策略的集中管理功能

use crate::db::Database;
use crate::error::{Error, Result};
use crate::models::{DlpRule, SensitiveOperationRule};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// 策略管理请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDlpRuleRequest {
    pub name: String,
    pub pattern: String,
    pub replacement: String,
    pub severity: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub category: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSensitiveOpRuleRequest {
    pub name: String,
    pub operation_type: String,
    pub requires_approval: bool,
    pub risk_level: String,
    pub description: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSensitiveOpRuleRequest {
    pub name: Option<String>,
    pub operation_type: Option<String>,
    pub requires_approval: Option<bool>,
    pub risk_level: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

/// 策略版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVersionInfo {
    pub dlp_rules_version: u64,
    pub sensitive_ops_version: u64,
    pub last_updated: chrono::DateTime<Utc>,
    pub total_dlp_rules: usize,
    pub total_sensitive_ops: usize,
    pub active_dlp_rules: usize,
    pub active_sensitive_ops: usize,
}

/// 策略统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyStatistics {
    pub total_rules: usize,
    pub active_rules: usize,
    pub rules_by_category: HashMap<String, usize>,
    pub rules_by_severity: HashMap<String, usize>,
    pub recent_changes: Vec<PolicyChangeRecord>,
}

/// 策略变更记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyChangeRecord {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub change_type: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub changed_by: String,
    pub changed_at: chrono::DateTime<Utc>,
    pub reason: Option<String>,
}

/// 策略管理服务
pub struct PolicyManagementService {
    db: Database,
}

impl PolicyManagementService {
    /// 创建新的策略管理服务
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 创建DLP规则
    #[instrument(skip(self, request), fields(rule_name = %request.name))]
    pub async fn create_dlp_rule(&self, request: CreateDlpRuleRequest, created_by: Uuid) -> Result<DlpRule> {
        info!("Creating new DLP rule: {}", request.name);

        // 验证正则表达式
        if let Err(e) = regex::Regex::new(&request.pattern) {
            warn!(pattern = %request.pattern, error = %e, "Invalid regex pattern");
            return Err(Error::Validation(format!("Invalid regex pattern: {}", e)));
        }

        // 验证严重程度
        if !["low", "medium", "high", "critical"].contains(&request.severity.as_str()) {
            return Err(Error::Validation("Invalid severity level".to_string()));
        }

        let rule = self.db.create_dlp_rule(
            &request.name,
            &request.pattern,
            &request.replacement,
            &request.severity,
            request.description.as_deref(),
            request.enabled,
            &request.category,
            created_by,
        ).await?;

        // 记录变更
        self.record_policy_change(
            rule.id,
            "created".to_string(),
            None,
            Some(serde_json::to_value(&rule).unwrap()),
            created_by,
            Some("Initial creation".to_string()),
        ).await?;

        info!(rule_id = %rule.id, "DLP rule created successfully");
        Ok(rule)
    }

    /// 更新DLP规则
    #[instrument(skip(self, request), fields(rule_id = %rule_id))]
    pub async fn update_dlp_rule(
        &self,
        rule_id: Uuid,
        request: UpdateDlpRuleRequest,
        updated_by: Uuid,
    ) -> Result<DlpRule> {
        info!("Updating DLP rule: {}", rule_id);

        // 获取现有规则
        let existing_rule = self.db.get_dlp_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("DLP rule not found".to_string()))?;

        // 验证新的正则表达式（如果提供）
        if let Some(ref pattern) = request.pattern {
            if let Err(e) = regex::Regex::new(pattern) {
                warn!(pattern = %pattern, error = %e, "Invalid regex pattern");
                return Err(Error::Validation(format!("Invalid regex pattern: {}", e)));
            }
        }

        // 验证严重程度（如果提供）
        if let Some(ref severity) = request.severity {
            if !["low", "medium", "high", "critical"].contains(&severity.as_str()) {
                return Err(Error::Validation("Invalid severity level".to_string()));
            }
        }

        let updated_rule = self.db.update_dlp_rule(rule_id, request, updated_by).await?;

        // 记录变更
        self.record_policy_change(
            rule_id,
            "updated".to_string(),
            Some(serde_json::to_value(&existing_rule).unwrap()),
            Some(serde_json::to_value(&updated_rule).unwrap()),
            updated_by,
            None,
        ).await?;

        info!(rule_id = %rule_id, "DLP rule updated successfully");
        Ok(updated_rule)
    }

    /// 删除DLP规则
    #[instrument(skip(self), fields(rule_id = %rule_id))]
    pub async fn delete_dlp_rule(&self, rule_id: Uuid, deleted_by: Uuid) -> Result<()> {
        info!("Deleting DLP rule: {}", rule_id);

        // 获取现有规则用于记录
        let existing_rule = self.db.get_dlp_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("DLP rule not found".to_string()))?;

        self.db.delete_dlp_rule(rule_id).await?;

        // 记录变更
        self.record_policy_change(
            rule_id,
            "deleted".to_string(),
            Some(serde_json::to_value(&existing_rule).unwrap()),
            None,
            deleted_by,
            None,
        ).await?;

        info!(rule_id = %rule_id, "DLP rule deleted successfully");
        Ok(())
    }

    /// 获取所有DLP规则
    #[instrument(skip(self))]
    pub async fn get_dlp_rules(&self, include_disabled: bool) -> Result<Vec<DlpRule>> {
        debug!("Fetching DLP rules, include_disabled: {}", include_disabled);
        self.db.get_dlp_rules(include_disabled).await
    }

    /// 根据ID获取DLP规则
    #[instrument(skip(self), fields(rule_id = %rule_id))]
    pub async fn get_dlp_rule_by_id(&self, rule_id: Uuid) -> Result<Option<DlpRule>> {
        debug!("Fetching DLP rule by ID: {}", rule_id);
        self.db.get_dlp_rule_by_id(rule_id).await
    }

    /// 创建敏感操作规则
    #[instrument(skip(self, request), fields(rule_name = %request.name))]
    pub async fn create_sensitive_op_rule(
        &self,
        request: CreateSensitiveOpRuleRequest,
        created_by: Uuid,
    ) -> Result<SensitiveOperationRule> {
        info!("Creating new sensitive operation rule: {}", request.name);

        // 验证风险级别
        if !["low", "medium", "high", "critical"].contains(&request.risk_level.as_str()) {
            return Err(Error::Validation("Invalid risk level".to_string()));
        }

        let rule = self.db.create_sensitive_op_rule(
            &request.name,
            &request.operation_type,
            request.requires_approval,
            &request.risk_level,
            request.description.as_deref(),
            request.enabled,
            created_by,
        ).await?;

        // 记录变更
        self.record_policy_change(
            rule.id,
            "created".to_string(),
            None,
            Some(serde_json::to_value(&rule).unwrap()),
            created_by,
            Some("Initial creation".to_string()),
        ).await?;

        info!(rule_id = %rule.id, "Sensitive operation rule created successfully");
        Ok(rule)
    }

    /// 更新敏感操作规则
    #[instrument(skip(self, request), fields(rule_id = %rule_id))]
    pub async fn update_sensitive_op_rule(
        &self,
        rule_id: Uuid,
        request: UpdateSensitiveOpRuleRequest,
        updated_by: Uuid,
    ) -> Result<SensitiveOperationRule> {
        info!("Updating sensitive operation rule: {}", rule_id);

        // 获取现有规则
        let existing_rule = self.db.get_sensitive_op_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("Sensitive operation rule not found".to_string()))?;

        // 验证风险级别（如果提供）
        if let Some(ref risk_level) = request.risk_level {
            if !["low", "medium", "high", "critical"].contains(&risk_level.as_str()) {
                return Err(Error::Validation("Invalid risk level".to_string()));
            }
        }

        let updated_rule = self.db.update_sensitive_op_rule(rule_id, request, updated_by).await?;

        // 记录变更
        self.record_policy_change(
            rule_id,
            "updated".to_string(),
            Some(serde_json::to_value(&existing_rule).unwrap()),
            Some(serde_json::to_value(&updated_rule).unwrap()),
            updated_by,
            None,
        ).await?;

        info!(rule_id = %rule_id, "Sensitive operation rule updated successfully");
        Ok(updated_rule)
    }

    /// 删除敏感操作规则
    #[instrument(skip(self), fields(rule_id = %rule_id))]
    pub async fn delete_sensitive_op_rule(&self, rule_id: Uuid, deleted_by: Uuid) -> Result<()> {
        info!("Deleting sensitive operation rule: {}", rule_id);

        // 获取现有规则用于记录
        let existing_rule = self.db.get_sensitive_op_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("Sensitive operation rule not found".to_string()))?;

        self.db.delete_sensitive_op_rule(rule_id).await?;

        // 记录变更
        self.record_policy_change(
            rule_id,
            "deleted".to_string(),
            Some(serde_json::to_value(&existing_rule).unwrap()),
            None,
            deleted_by,
            None,
        ).await?;

        info!(rule_id = %rule_id, "Sensitive operation rule deleted successfully");
        Ok(())
    }

    /// 获取所有敏感操作规则
    #[instrument(skip(self))]
    pub async fn get_sensitive_op_rules(&self, include_disabled: bool) -> Result<Vec<SensitiveOperationRule>> {
        debug!("Fetching sensitive operation rules, include_disabled: {}", include_disabled);
        self.db.get_sensitive_op_rules(include_disabled).await
    }

    /// 根据ID获取敏感操作规则
    #[instrument(skip(self), fields(rule_id = %rule_id))]
    pub async fn get_sensitive_op_rule_by_id(&self, rule_id: Uuid) -> Result<Option<SensitiveOperationRule>> {
        debug!("Fetching sensitive operation rule by ID: {}", rule_id);
        self.db.get_sensitive_op_rule_by_id(rule_id).await
    }

    /// 获取策略版本信息
    #[instrument(skip(self))]
    pub async fn get_policy_version_info(&self) -> Result<PolicyVersionInfo> {
        debug!("Fetching policy version information");

        let dlp_rules = self.db.get_dlp_rules(true).await?;
        let sensitive_ops = self.db.get_sensitive_op_rules(true).await?;

        let active_dlp_rules = dlp_rules.iter().filter(|r| r.enabled).count();
        let active_sensitive_ops = sensitive_ops.iter().filter(|r| r.enabled).count();

        // 获取最后更新时间
        let last_updated = self.db.get_last_policy_update_time().await?
            .unwrap_or_else(Utc::now);

        Ok(PolicyVersionInfo {
            dlp_rules_version: self.db.get_dlp_rules_version().await?,
            sensitive_ops_version: self.db.get_sensitive_ops_version().await?,
            last_updated,
            total_dlp_rules: dlp_rules.len(),
            total_sensitive_ops: sensitive_ops.len(),
            active_dlp_rules,
            active_sensitive_ops,
        })
    }

    /// 获取策略统计信息
    #[instrument(skip(self))]
    pub async fn get_policy_statistics(&self) -> Result<PolicyStatistics> {
        debug!("Fetching policy statistics");

        let dlp_rules = self.db.get_dlp_rules(true).await?;
        let active_rules = dlp_rules.iter().filter(|r| r.enabled).count();

        // 按类别统计
        let mut rules_by_category = HashMap::new();
        for rule in &dlp_rules {
            *rules_by_category.entry(rule.category.clone()).or_insert(0) += 1;
        }

        // 按严重程度统计
        let mut rules_by_severity = HashMap::new();
        for rule in &dlp_rules {
            *rules_by_severity.entry(rule.severity.clone()).or_insert(0) += 1;
        }

        // 获取最近的变更记录
        let recent_changes = self.db.get_recent_policy_changes(10).await?;

        Ok(PolicyStatistics {
            total_rules: dlp_rules.len(),
            active_rules,
            rules_by_category,
            rules_by_severity,
            recent_changes,
        })
    }

    /// 批量启用/禁用规则
    #[instrument(skip(self), fields(rule_ids_count = rule_ids.len()))]
    pub async fn bulk_update_rule_status(
        &self,
        rule_ids: Vec<Uuid>,
        enabled: bool,
        updated_by: Uuid,
    ) -> Result<usize> {
        info!("Bulk updating rule status for {} rules", rule_ids.len());

        let updated_count = self.db.bulk_update_dlp_rule_status(rule_ids, enabled, updated_by).await?;

        info!(updated_count = updated_count, enabled = enabled, "Bulk rule status update completed");
        Ok(updated_count)
    }

    /// 测试DLP规则
    #[instrument(skip(self, test_content), fields(rule_id = %rule_id, content_len = test_content.len()))]
    pub async fn test_dlp_rule(&self, rule_id: Uuid, test_content: &str) -> Result<DlpTestResult> {
        debug!("Testing DLP rule: {}", rule_id);

        let rule = self.db.get_dlp_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("DLP rule not found".to_string()))?;

        let regex = regex::Regex::new(&rule.pattern)
            .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?;

        let matches: Vec<DlpTestMatch> = regex.find_iter(test_content)
            .map(|m| DlpTestMatch {
                start: m.start(),
                end: m.end(),
                matched_text: m.as_str().to_string(),
                replacement: rule.replacement.clone(),
            })
            .collect();

        let sanitized_content = regex.replace_all(test_content, &rule.replacement).to_string();

        Ok(DlpTestResult {
            rule_id: rule.id,
            rule_name: rule.name.clone(),
            matches,
            sanitized_content,
            has_matches: !matches.is_empty(),
        })
    }

    /// 记录策略变更
    async fn record_policy_change(
        &self,
        rule_id: Uuid,
        change_type: String,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
        changed_by: Uuid,
        reason: Option<String>,
    ) -> Result<()> {
        self.db.record_policy_change(
            rule_id,
            change_type,
            old_value,
            new_value,
            changed_by,
            reason,
        ).await
    }
}

/// DLP规则测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpTestResult {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub matches: Vec<DlpTestMatch>,
    pub sanitized_content: String,
    pub has_matches: bool,
}

/// DLP测试匹配项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpTestMatch {
    pub start: usize,
    pub end: usize,
    pub matched_text: String,
    pub replacement: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use deadpool_postgres::Pool;

    // 注意：这些测试需要数据库连接，在实际环境中需要配置测试数据库
    
    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_create_dlp_rule() {
        // 这里需要设置测试数据库
        // let pool = setup_test_db().await;
        // let db = Database::new(pool);
        // let service = PolicyManagementService::new(db);
        
        // let request = CreateDlpRuleRequest {
        //     name: "Test Rule".to_string(),
        //     pattern: r"\d{3}-\d{2}-\d{4}".to_string(),
        //     replacement: "[REDACTED]".to_string(),
        //     severity: "high".to_string(),
        //     description: Some("Test description".to_string()),
        //     enabled: true,
        //     category: "test".to_string(),
        // };
        
        // let created_by = Uuid::new_v4();
        // let result = service.create_dlp_rule(request, created_by).await;
        // assert!(result.is_ok());
    }

    #[test]
    fn test_dlp_rule_validation() {
        // 测试正则表达式验证
        let invalid_pattern = "[invalid regex(";
        let result = regex::Regex::new(invalid_pattern);
        assert!(result.is_err());

        let valid_pattern = r"\d{3}-\d{2}-\d{4}";
        let result = regex::Regex::new(valid_pattern);
        assert!(result.is_ok());
    }

    #[test]
    fn test_severity_validation() {
        let valid_severities = ["low", "medium", "high", "critical"];
        for severity in &valid_severities {
            assert!(valid_severities.contains(severity));
        }

        let invalid_severity = "invalid";
        assert!(!valid_severities.contains(&invalid_severity));
    }

    #[tokio::test]
    async fn test_dlp_rule_testing() {
        // 模拟测试DLP规则功能
        let pattern = r"\d{3}-\d{2}-\d{4}";
        let replacement = "[SSN]";
        let test_content = "My SSN is 123-45-6789 and yours is 987-65-4321";

        let regex = regex::Regex::new(pattern).unwrap();
        let matches: Vec<_> = regex.find_iter(test_content).collect();
        
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].as_str(), "123-45-6789");
        assert_eq!(matches[1].as_str(), "987-65-4321");

        let sanitized = regex.replace_all(test_content, replacement);
        assert_eq!(sanitized, "My SSN is [SSN] and yours is [SSN]");
    }

    #[test]
    fn test_policy_change_record_serialization() {
        let record = PolicyChangeRecord {
            id: Uuid::new_v4(),
            rule_id: Uuid::new_v4(),
            change_type: "created".to_string(),
            old_value: None,
            new_value: Some(serde_json::json!({"name": "test"})),
            changed_by: "admin".to_string(),
            changed_at: Utc::now(),
            reason: Some("Initial creation".to_string()),
        };

        let serialized = serde_json::to_string(&record).unwrap();
        let deserialized: PolicyChangeRecord = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(record.id, deserialized.id);
        assert_eq!(record.change_type, deserialized.change_type);
    }
}