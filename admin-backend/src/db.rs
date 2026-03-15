use crate::error::{Error, Result};
use crate::models::{User, AuditLog, DlpRule, SensitiveOperationRule};
use chrono::Utc;
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
    pub async fn get_dlp_rules(&self) -> Result<Vec<DlpRule>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = client
            .query(
                "SELECT id, pattern, replacement, severity, created_at FROM dlp_rules",
                &[],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| DlpRule {
                id: r.get(0),
                pattern: r.get(1),
                replacement: r.get(2),
                severity: r.get(3),
                created_at: r.get(4),
            })
            .collect())
    }

    // Sensitive Operation Rule operations
    pub async fn get_sensitive_operation_rules(&self) -> Result<Vec<SensitiveOperationRule>> {
        let client = self.pool.get().await
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = client
            .query(
                "SELECT id, operation_type, requires_approval, created_at FROM sensitive_operation_rules",
                &[],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| SensitiveOperationRule {
                id: r.get(0),
                operation_type: r.get(1),
                requires_approval: r.get(2),
                created_at: r.get(3),
            })
            .collect())
    }
}
