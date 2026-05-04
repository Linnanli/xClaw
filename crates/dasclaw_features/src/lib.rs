//! 4-stage feature flag lifecycle (codex port).
//!
//! # W4 — 治理 6 件套默认关闭契约
//!
//! 注册 `dasclaw_governance` 自治套件的 6 个 feature flag，并保证 **默认全部 disabled**。
//! 这是 W4 默认关闭策略的运行期闸门：即便 `dasclaw_governance` 编译时打开了若干
//! cargo feature，运行期是否真正启用对应治理逻辑，仍须由本注册表确认。
//!
//! 来源：`docs/plans/architecture-refactor/32-execution-plan.md` W4 默认关闭策略。
//!
//! ## 6 件套清单
//!
//! | Flag                | 模块                       | 描述                                       |
//! |---------------------|----------------------------|--------------------------------------------|
//! | `policy_engine`     | dasclaw_governance         | 决策引擎（红线评估）                       |
//! | `recovery_recipes`  | dasclaw_governance         | 异常恢复 recipe 库                         |
//! | `trust_resolver`    | dasclaw_governance         | 信任解析（身份 + capability）              |
//! | `branch_lock`       | dasclaw_governance         | 分支独占检测（#68 已落地）                 |
//! | `stale_base`        | dasclaw_governance         | base commit 漂移检测（#69 已落地）         |
//! | `green_contract`    | dasclaw_governance         | "全绿" 提交契约                            |
//!
//! ## 默认关闭契约
//!
//! - `FeatureFlagRegistry::default()` 返回 6 个 flag 全部 `false`。
//! - `default_build_has_no_governance()` 在内部测试中守护"默认 build 不启用任何
//!   治理逻辑"，对应验收第 2 条。
//! - 单独 `enable()` 一个 flag **不会** 影响其它 flag 的状态，对应验收第 3 条
//!   "flag 启用后链路通"。
//!
//! 移除 / 改名某个 flag 必须同步更新 `GovernanceFlag::ALL` 与对应字符串映射。

use std::collections::BTreeMap;
use std::str::FromStr;

/// Errors surfaced by the feature flag registry.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FeatureFlagError {
    /// The given flag string did not match any registered governance flag.
    #[error("unknown governance feature flag: {0}")]
    UnknownFlag(String),
}

/// 治理 6 件套 flag 枚举。
///
/// 任何新增 / 移除必须同时更新 [`GovernanceFlag::ALL`]、[`GovernanceFlag::as_str`]
/// 与 [`GovernanceFlag::parse`]，否则 `req_features_74_*` 测试会失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GovernanceFlag {
    /// dasclaw_governance::policy_engine — 决策引擎（红线评估）。
    PolicyEngine,
    /// dasclaw_governance::recovery_recipes — 异常恢复 recipe 库。
    RecoveryRecipes,
    /// dasclaw_governance::trust_resolver — 信任解析。
    TrustResolver,
    /// dasclaw_governance::branch_lock — 分支独占检测（#68）。
    BranchLock,
    /// dasclaw_governance::stale_base — base commit 漂移检测（#69）。
    StaleBase,
    /// dasclaw_governance::green_contract — "全绿"提交契约。
    GreenContract,
}

impl GovernanceFlag {
    /// 6 件套全集，顺序固定，方便 `default()` / 审计枚举。
    pub const ALL: [GovernanceFlag; 6] = [
        GovernanceFlag::PolicyEngine,
        GovernanceFlag::RecoveryRecipes,
        GovernanceFlag::TrustResolver,
        GovernanceFlag::BranchLock,
        GovernanceFlag::StaleBase,
        GovernanceFlag::GreenContract,
    ];

    /// Stable string identifier (与 `dasclaw_governance` cargo feature 同名).
    pub fn as_str(self) -> &'static str {
        match self {
            GovernanceFlag::PolicyEngine => "policy_engine",
            GovernanceFlag::RecoveryRecipes => "recovery_recipes",
            GovernanceFlag::TrustResolver => "trust_resolver",
            GovernanceFlag::BranchLock => "branch_lock",
            GovernanceFlag::StaleBase => "stale_base",
            GovernanceFlag::GreenContract => "green_contract",
        }
    }

    /// Parse a flag identifier coming from config / CLI.
    ///
    /// Available via `GovernanceFlag::from_str` thanks to the
    /// [`std::str::FromStr`] impl.
    pub fn parse(s: &str) -> Result<Self, FeatureFlagError> {
        match s {
            "policy_engine" => Ok(GovernanceFlag::PolicyEngine),
            "recovery_recipes" => Ok(GovernanceFlag::RecoveryRecipes),
            "trust_resolver" => Ok(GovernanceFlag::TrustResolver),
            "branch_lock" => Ok(GovernanceFlag::BranchLock),
            "stale_base" => Ok(GovernanceFlag::StaleBase),
            "green_contract" => Ok(GovernanceFlag::GreenContract),
            other => Err(FeatureFlagError::UnknownFlag(other.to_string())),
        }
    }
}

impl FromStr for GovernanceFlag {
    type Err = FeatureFlagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GovernanceFlag::parse(s)
    }
}

/// Primary trait used by callers that only know the flag identifier as a string
/// (config files, env vars, CLI). Returns `false` for unknown flags so callers
/// always fail closed.
pub trait FeatureFlags {
    /// Return whether the named flag is currently enabled. Unknown flags
    /// **must** return `false`. This is the runtime fail-closed invariant —
    /// 默认 build 不启用任何治理逻辑.
    fn is_enabled(&self, flag: &str) -> bool;
}

/// W4 治理 6 件套 flag 注册表。默认所有 flag 关闭。
#[derive(Debug, Clone)]
pub struct FeatureFlagRegistry {
    states: BTreeMap<GovernanceFlag, bool>,
}

impl FeatureFlagRegistry {
    /// Build a registry with all 6 governance flags registered and disabled.
    pub fn new() -> Self {
        let mut states = BTreeMap::new();
        for flag in GovernanceFlag::ALL {
            states.insert(flag, false);
        }
        Self { states }
    }

    /// Returns `true` iff the typed flag is currently enabled.
    pub fn is_flag_enabled(&self, flag: GovernanceFlag) -> bool {
        // ALL 件套都在 `new()` 中注册过，缺失视为 false（fail closed）。
        self.states.get(&flag).copied().unwrap_or(false)
    }

    /// Enable a single flag. Other flags are unaffected.
    pub fn enable(&mut self, flag: GovernanceFlag) -> &mut Self {
        self.states.insert(flag, true);
        self
    }

    /// Disable a single flag. Other flags are unaffected.
    pub fn disable(&mut self, flag: GovernanceFlag) -> &mut Self {
        self.states.insert(flag, false);
        self
    }

    /// Enumerate the currently-enabled flags. Order matches
    /// [`GovernanceFlag::ALL`]. Useful for audit log emission.
    pub fn enabled_flags(&self) -> Vec<GovernanceFlag> {
        GovernanceFlag::ALL
            .into_iter()
            .filter(|f| self.is_flag_enabled(*f))
            .collect()
    }

    /// Returns `true` iff **no** governance flag is enabled. This is the
    /// W4 默认关闭契约 of "默认 build 不启用任何治理逻辑".
    pub fn default_build_has_no_governance(&self) -> bool {
        self.enabled_flags().is_empty()
    }
}

impl Default for FeatureFlagRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureFlags for FeatureFlagRegistry {
    fn is_enabled(&self, flag: &str) -> bool {
        match GovernanceFlag::from_str(flag) {
            Ok(f) => self.is_flag_enabled(f),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_features_74_default_all_disabled() {
        let registry = FeatureFlagRegistry::default();
        for flag in GovernanceFlag::ALL {
            assert!(
                !registry.is_flag_enabled(flag),
                "{} must be disabled by default",
                flag.as_str()
            );
        }
    }

    #[test]
    fn req_features_74_default_build_has_no_governance() {
        let registry = FeatureFlagRegistry::default();
        assert!(registry.default_build_has_no_governance());
        assert!(registry.enabled_flags().is_empty());
    }

    #[test]
    fn req_features_74_six_flags_registered() {
        // 防漏注册：ALL 必须为 6，且每一项的字符串必须 roundtrip。
        assert_eq!(GovernanceFlag::ALL.len(), 6);
        for flag in GovernanceFlag::ALL {
            let s = flag.as_str();
            let parsed = GovernanceFlag::from_str(s).expect("ALL entry must parse");
            assert_eq!(parsed, flag);
        }
    }

    #[test]
    fn req_features_74_each_flag_enables_independently() {
        for target in GovernanceFlag::ALL {
            let mut registry = FeatureFlagRegistry::default();
            registry.enable(target);
            for other in GovernanceFlag::ALL {
                let expected = other == target;
                assert_eq!(
                    registry.is_flag_enabled(other),
                    expected,
                    "enabling {} must not affect {}",
                    target.as_str(),
                    other.as_str()
                );
            }
            assert_eq!(registry.enabled_flags(), vec![target]);
        }
    }

    #[test]
    fn req_features_74_disable_returns_to_default() {
        let mut registry = FeatureFlagRegistry::default();
        registry.enable(GovernanceFlag::BranchLock);
        assert!(registry.is_flag_enabled(GovernanceFlag::BranchLock));
        registry.disable(GovernanceFlag::BranchLock);
        assert!(!registry.is_flag_enabled(GovernanceFlag::BranchLock));
        assert!(registry.default_build_has_no_governance());
    }

    #[test]
    fn req_features_74_unknown_flag_returns_false() {
        let registry = FeatureFlagRegistry::default();
        // FeatureFlags trait fail-closed contract.
        assert!(!registry.is_enabled("does_not_exist"));
        assert!(!registry.is_enabled(""));
        // GovernanceFlag::from_str surfaces the typed error.
        assert_eq!(
            GovernanceFlag::from_str("does_not_exist"),
            Err(FeatureFlagError::UnknownFlag("does_not_exist".to_string()))
        );
    }

    #[test]
    fn req_features_74_trait_is_enabled_matches_typed_view() {
        let mut registry = FeatureFlagRegistry::default();
        registry.enable(GovernanceFlag::PolicyEngine);
        registry.enable(GovernanceFlag::StaleBase);
        // String API 与类型化 API 一致。
        assert!(registry.is_enabled("policy_engine"));
        assert!(registry.is_enabled("stale_base"));
        assert!(!registry.is_enabled("branch_lock"));
        assert!(!registry.is_enabled("green_contract"));
    }

    #[test]
    fn req_features_74_enabled_flags_preserves_all_order() {
        let mut registry = FeatureFlagRegistry::default();
        // Enable in reverse order to prove the audit list follows ALL ordering.
        for flag in GovernanceFlag::ALL.iter().rev() {
            registry.enable(*flag);
        }
        assert_eq!(registry.enabled_flags(), GovernanceFlag::ALL.to_vec());
    }
}
