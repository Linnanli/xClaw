use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVersion {
    pub dlp_rules_version: u64,
    pub sensitive_ops_version: u64,
    pub last_sync: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpPolicy {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveOpPolicy {
    pub id: String,
    pub operation: String,
    pub requires_approval: bool,
    pub risk_level: String,
}

pub struct PolicySyncManager {
    local_version: PolicyVersion,
    dlp_policies: Vec<DlpPolicy>,
    sensitive_ops_policies: Vec<SensitiveOpPolicy>,
}

impl PolicySyncManager {
    pub fn new() -> Self {
        Self {
            local_version: PolicyVersion {
                dlp_rules_version: 0,
                sensitive_ops_version: 0,
                last_sync: 0,
            },
            dlp_policies: Vec::new(),
            sensitive_ops_policies: Vec::new(),
        }
    }

    pub fn get_local_version(&self) -> &PolicyVersion {
        &self.local_version
    }

    pub fn needs_sync(&self, remote_version: &PolicyVersion) -> bool {
        remote_version.dlp_rules_version > self.local_version.dlp_rules_version
            || remote_version.sensitive_ops_version > self.local_version.sensitive_ops_version
    }

    pub fn update_dlp_policies(&mut self, policies: Vec<DlpPolicy>, version: u64) -> Result<()> {
        self.dlp_policies = policies;
        self.local_version.dlp_rules_version = version;
        self.update_sync_time();
        Ok(())
    }

    pub fn update_sensitive_ops_policies(
        &mut self,
        policies: Vec<SensitiveOpPolicy>,
        version: u64,
    ) -> Result<()> {
        self.sensitive_ops_policies = policies;
        self.local_version.sensitive_ops_version = version;
        self.update_sync_time();
        Ok(())
    }

    pub fn get_dlp_policies(&self) -> &[DlpPolicy] {
        &self.dlp_policies
    }

    pub fn get_sensitive_ops_policies(&self) -> &[SensitiveOpPolicy] {
        &self.sensitive_ops_policies
    }

    pub fn apply_dlp_policy(&self, text: &str) -> String {
        let mut result = text.to_string();

        for policy in &self.dlp_policies {
            if let Ok(re) = regex::Regex::new(&policy.pattern) {
                result = re.replace_all(&result, &policy.replacement).to_string();
            }
        }

        result
    }

    pub fn check_sensitive_operation(&self, operation: &str) -> Option<&SensitiveOpPolicy> {
        self.sensitive_ops_policies
            .iter()
            .find(|p| p.operation == operation)
    }

    fn update_sync_time(&mut self) {
        self.local_version.last_sync = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

impl Default for PolicySyncManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_version_check() {
        let manager = PolicySyncManager::new();
        let local = manager.get_local_version();

        assert_eq!(local.dlp_rules_version, 0);
        assert_eq!(local.sensitive_ops_version, 0);
    }

    #[test]
    fn test_needs_sync() {
        let manager = PolicySyncManager::new();
        let remote = PolicyVersion {
            dlp_rules_version: 1,
            sensitive_ops_version: 0,
            last_sync: 0,
        };

        assert!(manager.needs_sync(&remote));
    }

    #[test]
    fn test_dlp_policy_application() {
        let mut manager = PolicySyncManager::new();
        let policies = vec![DlpPolicy {
            id: "1".to_string(),
            pattern: r"\d{3}-\d{2}-\d{4}".to_string(),
            replacement: "[REDACTED]".to_string(),
            severity: "high".to_string(),
        }];

        manager.update_dlp_policies(policies, 1).unwrap();

        let text = "My SSN is 123-45-6789";
        let sanitized = manager.apply_dlp_policy(text);
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn test_sensitive_operation_check() {
        let mut manager = PolicySyncManager::new();
        let policies = vec![SensitiveOpPolicy {
            id: "1".to_string(),
            operation: "delete_file".to_string(),
            requires_approval: true,
            risk_level: "high".to_string(),
        }];

        manager.update_sensitive_ops_policies(policies, 1).unwrap();

        let policy = manager.check_sensitive_operation("delete_file");
        assert!(policy.is_some());
        assert!(policy.unwrap().requires_approval);
    }
}
