use crate::error::{Error, Result};
use crate::models::{User, AuditLog, DlpRule, SensitiveOperationRule, PolicyChangeRecord};
use crate::policy_management::{UpdateDlpRuleRequest, UpdateSensitiveOpRuleRequest};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use uuid::Uuid;

pub struct Database {
    pool: Pool,
}

impl Database {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    // User operations
    pub async fn create_user(&self, username: &str, email: &str, password_hash: &str) -> Result<User> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        client
            .execute(
                "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) 
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[&id, &username, &email, &password_hash, &now, &now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(User {
            id,
            username: username.to_string(),
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt(
                "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE username = $1",
                &[&username],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| User {
            id: r.get(0),
            username: r.get(1),
            email: r.get(2),
            password_hash: r.get(3),
            created_at: r.get(4),
            updated_at: r.get(5),
        }))
    }

    pub async fn get_user_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt(
                "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| User {
            id: r.get(0),
            username: r.get(1),
            email: r.get(2),
            password_hash: r.get(3),
            created_at: r.get(4),
            updated_at: r.get(5),
        }))
    }

    // Audit log operations
    pub async fn add_audit_log(&self, user_id: Uuid, action: &str, details: &str) -> Result<AuditLog> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        client
            .execute(
                "INSERT INTO audit_logs (id, user_id, action, details, created_at) 
                 VALUES ($1, $2, $3, $4, $5)",
                &[&id, &user_id, &action, &details, &now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(AuditLog {
            id,
            user_id,
            action: action.to_string(),
            details: details.to_string(),
            created_at: now,
        })
    }

    pub async fn get_audit_logs(&self, limit: i64) -> Result<Vec<AuditLog>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = client
            .query(
                "SELECT id, user_id, action, details, created_at FROM audit_logs ORDER BY created_at DESC LIMIT $1",
                &[&limit],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| AuditLog {
                id: r.get(0),
                user_id: r.get(1),
                action: r.get(2),
                details: r.get(3),
                created_at: r.get(4),
            })
            .collect())
    }

    // DLP Rule operations
    pub async fn create_dlp_rule(
        &self,
        name: &str,
        pattern: &str,
        replacement: &str,
        severity: &str,
        description: Option<&str>,
        enabled: bool,
        category: &str,
        created_by: Uuid,
    ) -> Result<DlpRule> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        client
            .execute(
                "INSERT INTO dlp_rules (id, name, pattern, replacement, severity, description, enabled, category, created_by, created_at, updated_at) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                &[&id, &name, &pattern, &replacement, &severity, &description, &enabled, &category, &created_by, &now, &now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(DlpRule {
            id,
            name: name.to_string(),
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            severity: severity.to_string(),
            description: description.map(|s| s.to_string()),
            enabled,
            category: category.to_string(),
            created_by,
            updated_by: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn update_dlp_rule(
        &self,
        rule_id: Uuid,
        request: UpdateDlpRuleRequest,
        updated_by: Uuid,
    ) -> Result<DlpRule> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let now = Utc::now();

        // 简化实现：分别处理每个字段
        if let Some(name) = request.name {
            client.execute(
                "UPDATE dlp_rules SET name = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&name, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(pattern) = request.pattern {
            client.execute(
                "UPDATE dlp_rules SET pattern = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&pattern, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(replacement) = request.replacement {
            client.execute(
                "UPDATE dlp_rules SET replacement = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&replacement, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(severity) = request.severity {
            client.execute(
                "UPDATE dlp_rules SET severity = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&severity, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(description) = request.description {
            client.execute(
                "UPDATE dlp_rules SET description = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&description, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(enabled) = request.enabled {
            client.execute(
                "UPDATE dlp_rules SET enabled = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&enabled, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(category) = request.category {
            client.execute(
                "UPDATE dlp_rules SET category = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&category, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        // 获取更新后的规则
        self.get_dlp_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("DLP rule not found after update".to_string()))
    }

    pub async fn delete_dlp_rule(&self, rule_id: Uuid) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        client
            .execute("DELETE FROM dlp_rules WHERE id = $1", &[&rule_id])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    pub async fn get_dlp_rules(&self, include_disabled: bool) -> Result<Vec<DlpRule>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let query = if include_disabled {
            "SELECT id, name, pattern, replacement, severity, description, enabled, category, created_by, updated_by, created_at, updated_at FROM dlp_rules ORDER BY created_at DESC"
        } else {
            "SELECT id, name, pattern, replacement, severity, description, enabled, category, created_by, updated_by, created_at, updated_at FROM dlp_rules WHERE enabled = true ORDER BY created_at DESC"
        };

        let rows = client
            .query(query, &[])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| DlpRule {
                id: r.get(0),
                name: r.get(1),
                pattern: r.get(2),
                replacement: r.get(3),
                severity: r.get(4),
                description: r.get(5),
                enabled: r.get(6),
                category: r.get(7),
                created_by: r.get(8),
                updated_by: r.get(9),
                created_at: r.get(10),
                updated_at: r.get(11),
            })
            .collect())
    }

    pub async fn get_dlp_rule_by_id(&self, rule_id: Uuid) -> Result<Option<DlpRule>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt(
                "SELECT id, name, pattern, replacement, severity, description, enabled, category, created_by, updated_by, created_at, updated_at FROM dlp_rules WHERE id = $1",
                &[&rule_id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| DlpRule {
            id: r.get(0),
            name: r.get(1),
            pattern: r.get(2),
            replacement: r.get(3),
            severity: r.get(4),
            description: r.get(5),
            enabled: r.get(6),
            category: r.get(7),
            created_by: r.get(8),
            updated_by: r.get(9),
            created_at: r.get(10),
            updated_at: r.get(11),
        }))
    }

    // Sensitive Operation Rule operations
    pub async fn create_sensitive_op_rule(
        &self,
        name: &str,
        operation_type: &str,
        requires_approval: bool,
        risk_level: &str,
        description: Option<&str>,
        enabled: bool,
        created_by: Uuid,
    ) -> Result<SensitiveOperationRule> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        client
            .execute(
                "INSERT INTO sensitive_operation_rules (id, name, operation_type, requires_approval, risk_level, description, enabled, created_by, created_at, updated_at) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                &[&id, &name, &operation_type, &requires_approval, &risk_level, &description, &enabled, &created_by, &now, &now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(SensitiveOperationRule {
            id,
            name: name.to_string(),
            operation_type: operation_type.to_string(),
            requires_approval,
            risk_level: risk_level.to_string(),
            description: description.map(|s| s.to_string()),
            enabled,
            created_by,
            updated_by: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn update_sensitive_op_rule(
        &self,
        rule_id: Uuid,
        request: UpdateSensitiveOpRuleRequest,
        updated_by: Uuid,
    ) -> Result<SensitiveOperationRule> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let now = Utc::now();

        // 简化实现：分别处理每个字段
        if let Some(name) = request.name {
            client.execute(
                "UPDATE sensitive_operation_rules SET name = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&name, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(operation_type) = request.operation_type {
            client.execute(
                "UPDATE sensitive_operation_rules SET operation_type = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&operation_type, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(requires_approval) = request.requires_approval {
            client.execute(
                "UPDATE sensitive_operation_rules SET requires_approval = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&requires_approval, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(risk_level) = request.risk_level {
            client.execute(
                "UPDATE sensitive_operation_rules SET risk_level = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&risk_level, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(description) = request.description {
            client.execute(
                "UPDATE sensitive_operation_rules SET description = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&description, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        if let Some(enabled) = request.enabled {
            client.execute(
                "UPDATE sensitive_operation_rules SET enabled = $1, updated_by = $2, updated_at = $3 WHERE id = $4",
                &[&enabled, &updated_by, &now, &rule_id],
            ).await.map_err(|e| Error::Database(e.to_string()))?;
        }

        // 获取更新后的规则
        self.get_sensitive_op_rule_by_id(rule_id).await?
            .ok_or_else(|| Error::NotFound("Sensitive operation rule not found after update".to_string()))
    }

    pub async fn delete_sensitive_op_rule(&self, rule_id: Uuid) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        client
            .execute("DELETE FROM sensitive_operation_rules WHERE id = $1", &[&rule_id])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    pub async fn get_sensitive_op_rules(&self, include_disabled: bool) -> Result<Vec<SensitiveOperationRule>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let query = if include_disabled {
            "SELECT id, name, operation_type, requires_approval, risk_level, description, enabled, created_by, updated_by, created_at, updated_at FROM sensitive_operation_rules ORDER BY created_at DESC"
        } else {
            "SELECT id, name, operation_type, requires_approval, risk_level, description, enabled, created_by, updated_by, created_at, updated_at FROM sensitive_operation_rules WHERE enabled = true ORDER BY created_at DESC"
        };

        let rows = client
            .query(query, &[])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| SensitiveOperationRule {
                id: r.get(0),
                name: r.get(1),
                operation_type: r.get(2),
                requires_approval: r.get(3),
                risk_level: r.get(4),
                description: r.get(5),
                enabled: r.get(6),
                created_by: r.get(7),
                updated_by: r.get(8),
                created_at: r.get(9),
                updated_at: r.get(10),
            })
            .collect())
    }

    pub async fn get_sensitive_op_rule_by_id(&self, rule_id: Uuid) -> Result<Option<SensitiveOperationRule>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt(
                "SELECT id, name, operation_type, requires_approval, risk_level, description, enabled, created_by, updated_by, created_at, updated_at FROM sensitive_operation_rules WHERE id = $1",
                &[&rule_id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| SensitiveOperationRule {
            id: r.get(0),
            name: r.get(1),
            operation_type: r.get(2),
            requires_approval: r.get(3),
            risk_level: r.get(4),
            description: r.get(5),
            enabled: r.get(6),
            created_by: r.get(7),
            updated_by: r.get(8),
            created_at: r.get(9),
            updated_at: r.get(10),
        }))
    }

    // Policy version and change tracking operations
    pub async fn get_dlp_rules_version(&self) -> Result<u64> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt("SELECT version FROM policy_versions WHERE policy_type = 'dlp_rules'", &[])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| r.get::<_, i64>(0) as u64).unwrap_or(0))
    }

    pub async fn get_sensitive_ops_version(&self) -> Result<u64> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt("SELECT version FROM policy_versions WHERE policy_type = 'sensitive_ops'", &[])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.map(|r| r.get::<_, i64>(0) as u64).unwrap_or(0))
    }

    pub async fn increment_dlp_rules_version(&self) -> Result<u64> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let now = Utc::now();
        let row = client
            .query_one(
                "INSERT INTO policy_versions (policy_type, version, updated_at) 
                 VALUES ('dlp_rules', 1, $1) 
                 ON CONFLICT (policy_type) 
                 DO UPDATE SET version = policy_versions.version + 1, updated_at = $1 
                 RETURNING version",
                &[&now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.get::<_, i64>(0) as u64)
    }

    pub async fn increment_sensitive_ops_version(&self) -> Result<u64> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let now = Utc::now();
        let row = client
            .query_one(
                "INSERT INTO policy_versions (policy_type, version, updated_at) 
                 VALUES ('sensitive_ops', 1, $1) 
                 ON CONFLICT (policy_type) 
                 DO UPDATE SET version = policy_versions.version + 1, updated_at = $1 
                 RETURNING version",
                &[&now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.get::<_, i64>(0) as u64)
    }

    pub async fn get_last_policy_update_time(&self) -> Result<Option<DateTime<Utc>>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let row = client
            .query_opt(
                "SELECT MAX(updated_at) FROM policy_versions",
                &[],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.and_then(|r| r.get(0)))
    }

    pub async fn record_policy_change(
        &self,
        rule_id: Uuid,
        change_type: String,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
        changed_by: Uuid,
        reason: Option<String>,
    ) -> Result<()> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        client
            .execute(
                "INSERT INTO policy_change_records (id, rule_id, change_type, old_value, new_value, changed_by, changed_at, reason) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[&id, &rule_id, &change_type, &old_value, &new_value, &changed_by, &now, &reason],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    pub async fn get_recent_policy_changes(&self, limit: i64) -> Result<Vec<PolicyChangeRecord>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = client
            .query(
                "SELECT id, rule_id, change_type, old_value, new_value, changed_by, changed_at, reason 
                 FROM policy_change_records 
                 ORDER BY changed_at DESC 
                 LIMIT $1",
                &[&limit],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| PolicyChangeRecord {
                id: r.get(0),
                rule_id: r.get(1),
                change_type: r.get(2),
                old_value: r.get(3),
                new_value: r.get(4),
                changed_by: r.get(5),
                changed_at: r.get(6),
                reason: r.get(7),
            })
            .collect())
    }

    pub async fn bulk_update_dlp_rule_status(
        &self,
        rule_ids: Vec<Uuid>,
        enabled: bool,
        updated_by: Uuid,
    ) -> Result<usize> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let now = Utc::now();

        // 构建 IN 子句的占位符
        let placeholders: Vec<String> = (1..=rule_ids.len())
            .map(|i| format!("${}", i))
            .collect();
        let in_clause = placeholders.join(", ");

        // 构建参数列表 - 使用 Box 来延长生命周期
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = Vec::new();
        for rule_id in rule_ids {
            params.push(Box::new(rule_id));
        }
        params.push(Box::new(enabled));
        params.push(Box::new(updated_by));
        params.push(Box::new(now));

        // 转换为引用
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = 
            params.iter().map(|p| p.as_ref()).collect();

        let query = format!(
            "UPDATE dlp_rules SET enabled = ${}, updated_by = ${}, updated_at = ${} WHERE id IN ({})",
            params.len() - 2,
            params.len() - 1,
            params.len(),
            in_clause
        );

        let updated_count = client
            .execute(&query, &param_refs[..])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(updated_count as usize)
    }
}