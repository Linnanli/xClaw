use crate::policy_sync::PolicySyncManager;
use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpScanResult {
    pub is_clean: bool,
    pub matches: Vec<DlpMatch>,
    pub sanitized_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpMatch {
    pub pattern_id: String,
    pub matched_text: String,
    pub severity: String,
    pub position: usize,
}

pub struct DlpIntegration {
    policy_manager: PolicySyncManager,
}

impl DlpIntegration {
    pub fn new(policy_manager: PolicySyncManager) -> Self {
        Self { policy_manager }
    }

    pub async fn scan_user_input(&self, text: &str) -> Result<DlpScanResult> {
        let mut matches = Vec::new();

        for policy in self.policy_manager.get_dlp_policies() {
            if let Ok(re) = regex::Regex::new(&policy.pattern) {
                for cap in re.captures_iter(text) {
                    if let Some(matched) = cap.get(0) {
                        matches.push(DlpMatch {
                            pattern_id: policy.id.clone(),
                            matched_text: matched.as_str().to_string(),
                            severity: policy.severity.clone(),
                            position: matched.start(),
                        });
                    }
                }
            }
        }

        let sanitized = self.policy_manager.apply_dlp_policy(text);

        Ok(DlpScanResult {
            is_clean: matches.is_empty(),
            matches,
            sanitized_text: sanitized,
        })
    }

    pub async fn scan_outbound_request(&self, request_body: &str) -> Result<DlpScanResult> {
        self.scan_user_input(request_body).await
    }

    pub async fn sanitize_for_storage(&self, text: &str) -> Result<String> {
        Ok(self.policy_manager.apply_dlp_policy(text))
    }

    pub fn update_policies_from_storage(
        &mut self,
        policy_manager: PolicySyncManager,
    ) -> Result<()> {
        self.policy_manager = policy_manager;
        Ok(())
    }

    pub fn get_policy_version(&self) -> u64 {
        self.policy_manager.get_version().dlp_rules_version
    }
}

impl Default for DlpIntegration {
    fn default() -> Self {
        Self::new(PolicySyncManager::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_sync::DlpPolicy;

    #[tokio::test]
    async fn test_scan_user_input() {
        let mut policy_manager = PolicySyncManager::new();
        let policies = vec![DlpPolicy {
            id: "ssn".to_string(),
            pattern: r"\d{3}-\d{2}-\d{4}".to_string(),
            replacement: "[REDACTED]".to_string(),
            severity: "high".to_string(),
        }];

        policy_manager.update_dlp_policies(policies, 1).unwrap();
        let dlp = DlpIntegration::new(policy_manager);

        let result = dlp.scan_user_input("My SSN is 123-45-6789").await.unwrap();
        assert!(!result.is_clean);
        assert!(!result.matches.is_empty());
        assert!(result.sanitized_text.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn test_clean_input() {
        let dlp = DlpIntegration::default();
        let result = dlp.scan_user_input("Hello world").await.unwrap();
        assert!(result.is_clean);
        assert!(result.matches.is_empty());
    }

    #[tokio::test]
    async fn test_sanitize_for_storage() {
        let mut policy_manager = PolicySyncManager::new();
        let policies = vec![DlpPolicy {
            id: "email".to_string(),
            pattern: r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".to_string(),
            replacement: "[EMAIL]".to_string(),
            severity: "medium".to_string(),
        }];

        policy_manager.update_dlp_policies(policies, 1).unwrap();
        let dlp = DlpIntegration::new(policy_manager);

        let sanitized = dlp
            .sanitize_for_storage("Contact: user@example.com")
            .await
            .unwrap();
        assert!(sanitized.contains("[EMAIL]"));
    }
}
