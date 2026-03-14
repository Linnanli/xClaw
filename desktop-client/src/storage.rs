use crate::{Error, Result};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use libsql::Connection;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub key: String,
    pub value: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    pub action: String,
    pub timestamp: i64,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDlpRule {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedSkill {
    pub id: String,
    pub name: String,
    pub version: String,
    pub binary_hash: String,
    pub downloaded_at: i64,
}

pub struct StorageManager {
    conn: Connection,
    encryption_key: Vec<u8>,
}

impl StorageManager {
    pub async fn new(db_path: &Path, encryption_key: Vec<u8>) -> Result<Self> {
        let db_url = format!("file:{}", db_path.display());
        let conn = libsql::Connection::open(&db_url)
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        let manager = Self {
            conn,
            encryption_key,
        };

        manager.init_schema().await?;
        Ok(manager)
    }

    async fn init_schema(&self) -> Result<()> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS local_config (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    encrypted BOOLEAN NOT NULL
                )",
                [],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS audit_logs (
                    id TEXT PRIMARY KEY,
                    action TEXT NOT NULL,
                    timestamp INTEGER NOT NULL,
                    details TEXT NOT NULL
                )",
                [],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS cached_dlp_rules (
                    id TEXT PRIMARY KEY,
                    pattern TEXT NOT NULL,
                    replacement TEXT NOT NULL,
                    severity TEXT NOT NULL
                )",
                [],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS downloaded_skills (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    version TEXT NOT NULL,
                    binary_hash TEXT NOT NULL,
                    downloaded_at INTEGER NOT NULL
                )",
                [],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn set_config(&self, key: &str, value: &str, encrypt: bool) -> Result<()> {
        let stored_value = if encrypt {
            self.encrypt_value(value)?
        } else {
            value.to_string()
        };

        self.conn
            .execute(
                "INSERT OR REPLACE INTO local_config (key, value, encrypted) VALUES (?, ?, ?)",
                libsql::params![key, stored_value, encrypt],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
        let mut rows = self
            .conn
            .query("SELECT value, encrypted FROM local_config WHERE key = ?", libsql::params![key])
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| Error::DatabaseError(e.to_string()))? {
            let value: String = row.get(0).map_err(|e| Error::DatabaseError(e.to_string()))?;
            let encrypted: bool = row.get(1).map_err(|e| Error::DatabaseError(e.to_string()))?;

            if encrypted {
                Ok(Some(self.decrypt_value(&value)?))
            } else {
                Ok(Some(value))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn add_audit_log(&self, action: &str, details: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn
            .execute(
                "INSERT INTO audit_logs (id, action, timestamp, details) VALUES (?, ?, ?, ?)",
                libsql::params![id, action, timestamp, details],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_audit_logs(&self, limit: i64) -> Result<Vec<AuditLog>> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, action, timestamp, details FROM audit_logs ORDER BY timestamp DESC LIMIT ?",
                libsql::params![limit],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        let mut logs = Vec::new();
        while let Some(row) = rows.next().map_err(|e| Error::DatabaseError(e.to_string()))? {
            logs.push(AuditLog {
                id: row.get(0).map_err(|e| Error::DatabaseError(e.to_string()))?,
                action: row.get(1).map_err(|e| Error::DatabaseError(e.to_string()))?,
                timestamp: row.get(2).map_err(|e| Error::DatabaseError(e.to_string()))?,
                details: row.get(3).map_err(|e| Error::DatabaseError(e.to_string()))?,
            });
        }

        Ok(logs)
    }

    pub async fn cache_dlp_rule(&self, rule: &CachedDlpRule) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO cached_dlp_rules (id, pattern, replacement, severity) VALUES (?, ?, ?, ?)",
                libsql::params![rule.id, rule.pattern, rule.replacement, rule.severity],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_cached_dlp_rules(&self) -> Result<Vec<CachedDlpRule>> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, pattern, replacement, severity FROM cached_dlp_rules",
                [],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        let mut rules = Vec::new();
        while let Some(row) = rows.next().map_err(|e| Error::DatabaseError(e.to_string()))? {
            rules.push(CachedDlpRule {
                id: row.get(0).map_err(|e| Error::DatabaseError(e.to_string()))?,
                pattern: row.get(1).map_err(|e| Error::DatabaseError(e.to_string()))?,
                replacement: row.get(2).map_err(|e| Error::DatabaseError(e.to_string()))?,
                severity: row.get(3).map_err(|e| Error::DatabaseError(e.to_string()))?,
            });
        }

        Ok(rules)
    }

    pub async fn cache_skill(&self, skill: &DownloadedSkill) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO downloaded_skills (id, name, version, binary_hash, downloaded_at) VALUES (?, ?, ?, ?, ?)",
                libsql::params![skill.id, skill.name, skill.version, skill.binary_hash, skill.downloaded_at],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_cached_skills(&self) -> Result<Vec<DownloadedSkill>> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, name, version, binary_hash, downloaded_at FROM downloaded_skills",
                [],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        let mut skills = Vec::new();
        while let Some(row) = rows.next().map_err(|e| Error::DatabaseError(e.to_string()))? {
            skills.push(DownloadedSkill {
                id: row.get(0).map_err(|e| Error::DatabaseError(e.to_string()))?,
                name: row.get(1).map_err(|e| Error::DatabaseError(e.to_string()))?,
                version: row.get(2).map_err(|e| Error::DatabaseError(e.to_string()))?,
                binary_hash: row.get(3).map_err(|e| Error::DatabaseError(e.to_string()))?,
                downloaded_at: row.get(4).map_err(|e| Error::DatabaseError(e.to_string()))?,
            });
        }

        Ok(skills)
    }

    pub async fn delete_skill(&self, skill_id: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM downloaded_skills WHERE id = ?",
                libsql::params![skill_id],
            )
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn encrypt_value(&self, value: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.encryption_key));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, value.as_bytes())
            .map_err(|e| Error::CryptoError(e.to_string()))?;

        let mut encrypted = nonce_bytes.to_vec();
        encrypted.extend_from_slice(&ciphertext);

        Ok(hex::encode(encrypted))
    }

    fn decrypt_value(&self, encrypted: &str) -> Result<String> {
        let encrypted_bytes = hex::decode(encrypted)
            .map_err(|e| Error::CryptoError(e.to_string()))?;

        if encrypted_bytes.len() < 12 {
            return Err(Error::CryptoError("Invalid encrypted data".to_string()));
        }

        let (nonce_bytes, ciphertext) = encrypted_bytes.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.encryption_key));
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::CryptoError(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| Error::CryptoError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encryption_decryption() {
        let key = vec![0u8; 32];
        let manager = StorageManager {
            conn: libsql::Connection::open("file::memory:").await.unwrap(),
            encryption_key: key,
        };

        let original = "sensitive data";
        let encrypted = manager.encrypt_value(original).unwrap();
        let decrypted = manager.decrypt_value(&encrypted).unwrap();

        assert_eq!(original, decrypted);
    }
}
