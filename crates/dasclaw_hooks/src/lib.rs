//! Unified hook engine for the x-claw agent runtime.
//!
//! ## ADR-113 — single front-door for hooks
//!
//! `dasclaw_hooks` is the **single orchestration entry** (`HookRegistry`) for
//! all event-style lifecycle hooks (audit log, declarative regex transforms,
//! outbound webhook notifications, plugin/workspace bundles).
//!
//! Trait seams — `SafetyHook`, `SandboxExecutor`, `SecretProvider`,
//! `ApprovalGate`, `SessionHooks` — are owned by [`x_claw_agent`] (per
//! ADR-001 crate-independence rule) and **reexported here** so callers only
//! need to depend on `dasclaw_hooks`.
//!
//! See [`docs/plans/architecture-refactor/adr-113-hook-engine-unification.md`]
//! for the design rationale, including:
//! - why the two-layer split (event hooks + trait seams) is preserved
//! - the responsibility contract (`no_safety_rule_in_event_hooks`)
//! - the Phase 0 red-line definition (`count_hook_systems() == 1`).

pub mod bash_validation_hook;
pub mod bundled;
pub mod contract;
pub mod hook;
pub mod registry;

pub use bash_validation_hook::{BashValidationHook, DEFAULT_BASH_TOOL_NAMES};

pub use bundled::{
    HookBundleConfig, HookBundleError, HookRegistrationSummary, HookRuleConfig,
    OutboundWebhookConfig, RegexReplacementConfig, register_bundle, register_bundled_hooks,
};
pub use contract::{ContractViolation, count_hook_systems, no_safety_rule_in_event_hooks};
pub use hook::{Hook, HookContext, HookError, HookEvent, HookFailureMode, HookOutcome, HookPoint};
pub use registry::HookRegistry;

// Reexport trait seams from `x_claw_agent` so external callers never have to
// import both crates. The reexports are deliberately type-identity-preserving
// (`pub use`), not newtype wrappers.
pub use x_claw_agent::{
    ApprovalError, ApprovalGate, ApprovalOutcome, ApprovalRequest, AutoApproveGate, DenyAllGate,
    HookBundle, InMemorySecrets, NoopSafetyHook, NoopSandboxExecutor, NoopSessionHooks, RuleAction,
    RuleSuggestion, SafetyDecision, SafetyError, SafetyHook, SandboxError, SandboxExecOutput,
    SandboxExecRequest, SandboxExecutor, SandboxNetworkHint, SecretError, SecretProvider,
    SecretString, SessionHooks,
};

/// Bridge [`HookRegistry`] into the `x_claw_agent::SessionHooks` trait so
/// `SessionManager` (in the runtime crate) can fire `OnSessionStart` /
/// `OnSessionEnd` events without depending on a concrete hook engine.
///
/// Errors from hook execution are logged and swallowed — session lifecycle
/// must never be blocked by hook failures (fire-and-forget contract).
///
/// Lives in `dasclaw_hooks` (rather than the runtime or ironclaw) because
/// `HookRegistry` is defined here; placing the impl elsewhere would violate
/// Rust's orphan rule.
#[async_trait::async_trait]
impl x_claw_agent::SessionHooks for HookRegistry {
    async fn on_session_start(&self, user_id: &str, session_id: &str) {
        let event = HookEvent::SessionStart {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        };
        if let Err(e) = self.run(&event).await {
            tracing::warn!("OnSessionStart hook error: {}", e);
        }
    }

    async fn on_session_end(&self, user_id: &str, session_id: &str) {
        let event = HookEvent::SessionEnd {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        };
        if let Err(e) = self.run(&event).await {
            tracing::warn!("OnSessionEnd hook error: {}", e);
        }
    }
}

#[cfg(test)]
mod acceptance_tests {
    //! ADR-113 §6.1 acceptance tests for P0-3 PR #46.

    use super::*;

    /// `req_p03_pr1_count_hook_systems_is_one` — Phase 0 red-line: exactly
    /// one orchestration entry exists for event-style hooks.
    #[test]
    fn req_p03_pr1_count_hook_systems_is_one() {
        assert_eq!(count_hook_systems(), 1);
    }

    /// `req_p03_pr1_reexport_trait_seams_identity` — the trait seams reexported
    /// from `x_claw_agent` must be the *same* types (no newtype wrappers), so
    /// implementors of `x_claw_agent::SafetyHook` are accepted wherever
    /// `dasclaw_hooks::SafetyHook` is expected.
    #[test]
    fn req_p03_pr1_reexport_trait_seams_identity() {
        fn _assert_same<T: ?Sized>() {}
        // The cast below only type-checks when the two paths resolve to the
        // identical trait object type.
        let _: fn(&dyn x_claw_agent::SafetyHook) -> &dyn SafetyHook = |x| x;
        let _: fn(&dyn x_claw_agent::ApprovalGate) -> &dyn ApprovalGate = |x| x;
        let _: fn(&dyn x_claw_agent::SandboxExecutor) -> &dyn SandboxExecutor = |x| x;
        let _: fn(&dyn x_claw_agent::SecretProvider) -> &dyn SecretProvider = |x| x;
        let _: fn(&dyn x_claw_agent::SessionHooks) -> &dyn SessionHooks = |x| x;
    }

    /// `req_p03_pr1_registry_implements_session_hooks` — `HookRegistry` is
    /// usable as `&dyn SessionHooks` (orphan-rule bridge stays inside this
    /// crate).
    #[test]
    fn req_p03_pr1_registry_implements_session_hooks() {
        let registry = HookRegistry::new();
        let _: &dyn x_claw_agent::SessionHooks = &registry;
    }

    // ───────────────────── ADR-113 §6.2 PR #2 acceptance ─────────────────────
    //
    // PR #2 落地 Strategy A（实证修订 ADR §2.4）：保留 `HookRegistry.run(Inbound)`
    // dispatch（承载 declarative bundle 横切层），SafetyLayer 仅经 `HookBundle.safety`
    // 走 agent loop 直调路径——双调用在代码层面**不存在**，由以下 5 个测试钉死。

    /// `req_p03_pr2_dispatcher_no_double_call_safety` — `SafetyHook` 与
    /// `Hook`（事件型）是两个互不兼容的 trait，编译器静态保证 `IronclawSafetyHook`
    /// 这种 `SafetyHook` 实现**无法**作为 `Arc<dyn Hook>` 注册到 `HookRegistry`。
    /// 这把"双调用风险"在类型系统层封死——比运行期 contract 检查更强。
    #[test]
    fn req_p03_pr2_dispatcher_no_double_call_safety() {
        // 静态：两个 trait object 不是同一类型，无法相互赋值/转换。
        // 若有人把 SafetyHook 误适配为 Hook 注册到 Registry，编译会失败。
        fn _assert_disjoint_traits() {
            // SafetyHook 必须实现 before_prompt / after_completion / before_tool_call /
            // after_tool_output（结构化决策）；Hook 只有 execute(HookEvent)（字符串语义）。
            // 任何"把 dyn SafetyHook 当 dyn Hook 用"的 fn 都不存在编译路径。
            //
            // 反向 sanity：HookRegistry 不实现 SafetyHook（它只代理事件型 hook）。
            fn _registry_is_not_safety_hook() {
                fn _take_safety<T: SafetyHook>(_: &T) {}
                // 下面这一行被注释——保留作为契约文档：若 HookRegistry 误实现 SafetyHook，
                // 取消注释后会编译通过，但语义上 HookRegistry 不该承担 safety 决策。
                // _take_safety(&HookRegistry::new());
            }
        }
        // 同时 contract 兜底：含 secret/redact/safety 关键字的 declarative rule 被拒绝。
        let bundle = HookBundleConfig {
            rules: vec![HookRuleConfig {
                name: "redact-pii".into(),
                points: vec![HookPoint::BeforeInbound],
                priority: None,
                failure_mode: None,
                timeout_ms: None,
                when_regex: None,
                reject_reason: None,
                replacements: vec![],
                prepend: None,
                append: None,
            }],
            outbound_webhooks: vec![],
        };
        // contract 函数对未注册的 registry 应返回 true；将 bundle 注册后，
        // 名字含 "redact" 的 rule 会让 contract 拒绝（验证启动期 panic 路径）。
        let registry = std::sync::Arc::new(HookRegistry::new());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            register_bundle(&registry, "test-source", bundle).await;
            let violations = no_safety_rule_in_event_hooks(&registry).await;
            assert!(
                !violations.is_empty(),
                "contract 必须报告 safety/redact/secret 类规则违规，实际：{violations:?}"
            );
            assert!(
                violations.iter().any(|v| matches!(
                    v,
                    ContractViolation::NameSuggestsSafetyResponsibility { keyword, .. }
                        if keyword == "redact"
                )),
                "必须命中 redact 关键字，实际：{violations:?}"
            );
        });
    }

    /// `req_p03_pr2_agentic_loop_uses_reexport_bundle` — agent loop 使用
    /// `dasclaw_hooks::HookBundle`（reexport）时，trait seams 类型与
    /// `x_claw_agent::HookBundle` **完全相同**，可直接互传。
    #[test]
    fn req_p03_pr2_agentic_loop_uses_reexport_bundle() {
        // 构造一个全 noop 的 bundle（agent loop 默认形态），断言 reexport
        // 与原始类型 ABI 一致：HookBundle 字段类型必须是同一 trait object。
        let bundle: HookBundle = x_claw_agent::HookBundle::noop();
        // 字段层 type identity（编译通过即证）：
        let _: &std::sync::Arc<dyn SafetyHook> = &bundle.safety;
        let _: &std::sync::Arc<dyn SandboxExecutor> = &bundle.sandbox;
        let _: &std::sync::Arc<dyn SecretProvider> = &bundle.secrets;
        let _: &std::sync::Arc<dyn ApprovalGate> = &bundle.approval;
    }

    /// `req_p03_pr2_session_hooks_still_bridged` — `HookRegistry` 触发
    /// `OnSessionStart` 后，注册到该点的事件型 hook 被实际执行（不是空跑）。
    #[test]
    fn req_p03_pr2_session_hooks_still_bridged() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingHook {
            counter: std::sync::Arc<AtomicUsize>,
            points: Vec<HookPoint>,
        }
        #[async_trait::async_trait]
        impl Hook for CountingHook {
            fn name(&self) -> &str {
                "counting"
            }
            fn hook_points(&self) -> &[HookPoint] {
                &self.points
            }
            async fn execute(
                &self,
                _event: &HookEvent,
                _ctx: &HookContext,
            ) -> Result<HookOutcome, HookError> {
                self.counter.fetch_add(1, Ordering::SeqCst);
                Ok(HookOutcome::ok())
            }
        }

        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let registry = HookRegistry::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            registry
                .register(std::sync::Arc::new(CountingHook {
                    counter: counter.clone(),
                    points: vec![HookPoint::OnSessionStart],
                }))
                .await;
            // 走 SessionHooks bridge（PR #46 在 lib.rs 实现）。
            let bridge: &dyn x_claw_agent::SessionHooks = &registry;
            bridge.on_session_start("u1", "s1").await;
        });
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "OnSessionStart bridge 必须实际触发注册到该点的 event hook"
        );
    }

    /// `req_p03_pr2_declarative_bundle_audit_path_intact` — 用户声明式 bundle
    /// 注册的 audit 类规则（含 prepend/append/replacements）在 `BeforeInbound`
    /// dispatch 时仍正常执行。这是 ADR-113 §2.4 保留 dispatch 的核心理由。
    #[test]
    fn req_p03_pr2_declarative_bundle_audit_path_intact() {
        let bundle = HookBundleConfig {
            rules: vec![HookRuleConfig {
                name: "audit-prepend".into(),
                points: vec![HookPoint::BeforeInbound],
                priority: None,
                failure_mode: None,
                timeout_ms: None,
                when_regex: None,
                reject_reason: None,
                replacements: vec![],
                prepend: Some("[AUDIT] ".into()),
                append: None,
            }],
            outbound_webhooks: vec![],
        };
        let registry = std::sync::Arc::new(HookRegistry::new());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let outcome = rt.block_on(async {
            let summary = register_bundle(&registry, "user-bundle", bundle).await;
            assert_eq!(summary.hooks, 1, "audit-prepend rule 必须注册成功");
            registry
                .run(&HookEvent::Inbound {
                    user_id: "u1".into(),
                    channel: "test".into(),
                    content: "hello".into(),
                    thread_id: None,
                })
                .await
                .expect("dispatch must succeed")
        });
        match outcome {
            HookOutcome::Continue {
                modified: Some(text),
            } => assert_eq!(text, "[AUDIT] hello"),
            other => panic!("declarative audit rule 必须修改内容，实际：{other:?}"),
        }
    }

    /// `req_p03_pr2_outbound_webhook_intact` — 声明式 outbound webhook 配置
    /// 仍能注册到 registry（不发真实 HTTP，只验证注册路径完整）。
    #[test]
    fn req_p03_pr2_outbound_webhook_intact() {
        let bundle = HookBundleConfig {
            rules: vec![],
            outbound_webhooks: vec![OutboundWebhookConfig {
                name: "audit-sink".into(),
                url: "https://example.invalid/audit".into(),
                points: vec![HookPoint::BeforeOutbound],
                priority: None,
                timeout_ms: None,
                headers: Default::default(),
                max_in_flight: None,
            }],
        };
        let registry = std::sync::Arc::new(HookRegistry::new());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            let summary = register_bundle(&registry, "user-bundle", bundle).await;
            assert_eq!(
                summary.outbound_webhooks, 1,
                "outbound webhook 必须注册成功"
            );
            assert_eq!(summary.errors, 0, "URL/scheme 校验必须通过");
            let names = registry.list().await;
            assert!(
                names.iter().any(|n| n.contains("audit-sink")),
                "registry 必须包含已注册的 webhook hook，实际：{names:?}"
            );
        });
    }
}
