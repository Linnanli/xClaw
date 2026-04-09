//! 策略同步模块
//!
//! 提供基础的策略同步功能

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// DLP策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpPolicy {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
    pub severity: String,
}

/// 敏感操作策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveOpPolicy {
    pub id: String,
    pub operation: String,
    pub requires_approval: bool,
    pub risk_level: String,
}

/// 策略版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVersion {
    pub dlp_rules_version: u64,
    pub sensitive_ops_version: u64,
    pub last_sync: u64,
}

/// 策略同步管理器
pub struct PolicySyncManager {
    dlp_policies: HashMap<String, DlpPolicy>,
    sensitive_ops_policies: HashMap<String, SensitiveOpPolicy>,
    version: PolicyVersion,
}

impl PolicySyncManager {
    /// 创建新的策略同步管理器
    pub fn new() -> Self {
        Self {
            dlp_policies: HashMap::new(),
            sensitive_ops_policies: HashMap::new(),
            version: PolicyVersion {
                dlp_rules_version: 0,
                sensitive_ops_version: 0,
                last_sync: 0,
            },
        }
    }

    /// 更新DLP策略
    pub fn update_dlp_policies(&mut self, policies: Vec<DlpPolicy>, version: u64) -> Result<()> {
        self.dlp_policies.clear();
        for policy in policies {
            self.dlp_policies.insert(policy.id.clone(), policy);
        }
        self.version.dlp_rules_version = version;
        Ok(())
    }

    /// 更新敏感操作策略
    pub fn update_sensitive_ops_policies(
        &mut self,
        policies: Vec<SensitiveOpPolicy>,
        version: u64,
    ) -> Result<()> {
        self.sensitive_ops_policies.clear();
        for policy in policies {
            self.sensitive_ops_policies
                .insert(policy.id.clone(), policy);
        }
        self.version.sensitive_ops_version = version;
        Ok(())
    }

    /// 获取DLP策略
    pub fn get_dlp_policies(&self) -> Vec<DlpPolicy> {
        self.dlp_policies.values().cloned().collect()
    }

    /// 获取敏感操作策略
    pub fn get_sensitive_ops_policies(&self) -> Vec<SensitiveOpPolicy> {
        self.sensitive_ops_policies.values().cloned().collect()
    }

    /// 检查是否需要同步
    pub fn needs_sync(&self, remote_version: &PolicyVersion) -> bool {
        self.version.dlp_rules_version < remote_version.dlp_rules_version
            || self.version.sensitive_ops_version < remote_version.sensitive_ops_version
    }

    /// 获取当前版本
    pub fn get_version(&self) -> &PolicyVersion {
        &self.version
    }

    /// 应用DLP策略（简化实现）
    pub fn apply_dlp_policy(&self, text: &str) -> String {
        // 简化的DLP策略应用
        let mut result = text.to_string();

        // 应用所有DLP策略
        for policy in self.dlp_policies.values() {
            if let Ok(regex) = regex::Regex::new(&policy.pattern) {
                result = regex.replace_all(&result, &policy.replacement).to_string();
            }
        }

        result
    }
}

impl Default for PolicySyncManager {
    fn default() -> Self {
        Self::new()
    }
}
