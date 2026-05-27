//! Tool registry for managing available tools.

use std::collections::HashMap;
use std::sync::Arc;

use dasclaw_governance::tool_visibility::{
    AllowAllPolicy, SharedToolVisibilityPolicy, ToolGateContext, ToolGateContextSeed,
    ToolGateDecision, ToolGateLayer, ToolSource as PolicyToolSource,
};
use tokio::sync::RwLock;

use crate::db::Database;
use crate::extensions::ExtensionManager;
use crate::llm::{LlmProvider, ToolDefinition};
use crate::orchestrator::job_manager::ContainerJobManager;
use crate::skills::catalog::SkillCatalog;
use crate::skills::registry::SkillRegistry;
use crate::tools::builder::{
    BuildSoftwareTool, BuilderConfig, LlmSoftwareBuilder, SoftwareBuilder,
};
use crate::tools::builtin::{
    ApplyPatchTool, CancelJobTool, CodeEditTool, CreateJobTool, EchoTool, ExtensionInfoTool,
    GitBranchTool, GitCommitTool, GitDiffTool, GitLogTool, GitPushTool, GitStaleCheckTool,
    GitStatusTool, GlobSearchTool, GrepSearchTool, HttpTool, JobEventsTool, JobPromptTool,
    JobStatusTool, JsonTool, ListDirTool, ListJobsTool, LspQueryTool, MemoryReadTool,
    MemorySearchTool, MemoryTreeTool, MemoryWriteTool, PlanModeTool, PromptQueue, ReadFileTool,
    SessionForkTool, ShellTool, SkillInstallTool, SkillListTool, SkillRemoveTool, SkillSearchTool,
    SubAgentTool, TimeTool, ToolActivateTool, ToolAuthTool, ToolInstallTool, ToolListTool,
    ToolRemoveTool, ToolSearchTool, ToolUpgradeTool, WebFetchTool, WebSearchTool, WriteFileTool,
};
use crate::tools::canonical_name::{CanonicalKind, CanonicalToolName};
use crate::tools::rate_limiter::RateLimiter;
use crate::tools::registration_report::{
    RegistrationOutcome, RejectionReason, ToolRegistrationEntry, ToolRegistrationReport, ToolSource,
};
use crate::tools::tool::{ApprovalRequirement, Tool, ToolDomain};
use crate::tools::wasm::{
    Capabilities, OAuthRefreshConfig, ResourceLimits, SharedCredentialRegistry, WasmError,
    WasmStorageError, WasmToolRuntime, WasmToolStore, WasmToolWrapper,
};
use crate::workspace::Workspace;
use dasclaw_runtime::context::ContextManager;
use dasclaw_runtime::secrets::SecretsStore;

/// Names of built-in tools that cannot be shadowed by dynamic registrations.
/// This prevents a dynamically built or installed tool from replacing a
/// security-critical built-in like "shell" or "memory_write".
const PROTECTED_TOOL_NAMES: &[&str] = &[
    "echo",
    "time",
    "json",
    "http",
    "shell",
    "read_file",
    "write_file",
    "list_dir",
    "apply_patch",
    "code_edit",
    "grep_search",
    "glob_search",
    "memory_search",
    "memory_write",
    "memory_read",
    "memory_tree",
    "create_job",
    "list_jobs",
    "job_status",
    "cancel_job",
    "build_software",
    "tool_search",
    "tool_install",
    "tool_auth",
    "tool_activate",
    "tool_list",
    "tool_remove",
    "routine_create",
    "routine_list",
    "routine_update",
    "routine_delete",
    "routine_fire",
    "routine_history",
    "event_emit",
    "skill_list",
    "skill_search",
    "skill_install",
    "skill_remove",
    "message",
    "web_fetch",
    "restart",
    "image_generate",
    "image_edit",
    "image_analyze",
    "tool_info",
    "git_status",
    "git_diff",
    "git_log",
    "git_commit",
    "git_branch",
    "git_push",
    "lsp_query",
];

/// Map a parsed canonical name kind to the report-facing source bucket.
/// Centralised so the `register()` flow and any future caller agree on the
/// classification.
fn canonical_kind_to_source(kind: CanonicalKind) -> ToolSource {
    match kind {
        // `register_sync` is the only path that produces `Builtin` entries;
        // if a dynamic registration somehow hints `Builtin` (shouldn't
        // happen because `parse(_, false)` never returns Builtin), fall
        // back to the safe-by-default `Dynamic` bucket.
        CanonicalKind::Builtin | CanonicalKind::LegacyDynamic => ToolSource::Dynamic,
        CanonicalKind::Mcp => ToolSource::Mcp,
        CanonicalKind::Wasm => ToolSource::Wasm,
        CanonicalKind::Extension => ToolSource::Extension,
    }
}

/// Registry of available tools.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    /// Tracks which names were registered via the built-in startup path.
    builtin_names: RwLock<std::collections::HashSet<String>>,
    /// Shared credential registry populated by WASM tools, consumed by HTTP tool.
    credential_registry: Option<Arc<SharedCredentialRegistry>>,
    /// Secrets store for credential injection (shared with HTTP tool).
    secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
    /// Shared rate limiter for built-in tool invocations.
    rate_limiter: RateLimiter,
    /// Reference to the message tool for setting context per-turn.
    message_tool: RwLock<Option<Arc<crate::tools::builtin::MessageTool>>>,
    /// Append-only ledger of registration attempts (issue #88).
    /// Records both accepted and rejected attempts, classified by
    /// [`ToolSource`]. Used by [`Self::registration_report`] to render the
    /// startup tool collision/visibility view without observing insertion
    /// order.
    registration_log: RwLock<Vec<ToolRegistrationEntry>>,
    /// ADR-149 / issue #485 — L2 tool visibility gate. Always present (no
    /// `Option`) to enforce the **Always-Has-Policy invariant**: callers
    /// can never accidentally bypass the gate by passing `None`. Defaults
    /// to [`AllowAllPolicy`] (behavioral no-op) so the 20+ existing
    /// `ToolRegistry::new()` call sites keep working; production wires a
    /// real policy via [`Self::with_policy`].
    policy: SharedToolVisibilityPolicy,
}

impl ToolRegistry {
    fn tool_definition(tool: &Arc<dyn Tool>) -> ToolDefinition {
        let schema = tool.schema();
        ToolDefinition {
            name: schema.name,
            description: schema.description,
            parameters: schema.parameters,
        }
    }

    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            builtin_names: RwLock::new(std::collections::HashSet::new()),
            credential_registry: None,
            secrets_store: None,
            rate_limiter: RateLimiter::new(),
            message_tool: RwLock::new(None),
            registration_log: RwLock::new(Vec::new()),
            // ADR-149 §2.5: default to the explicit no-op policy rather
            // than `Option<…>`. Real wiring lands in `app.rs` via
            // [`Self::with_policy`]. `grep AllowAllPolicy` surfaces every
            // call site still using the placeholder.
            policy: Arc::new(AllowAllPolicy),
        }
    }

    /// Create a registry with credential injection support.
    pub fn with_credentials(
        mut self,
        credential_registry: Arc<SharedCredentialRegistry>,
        secrets_store: Arc<dyn SecretsStore + Send + Sync>,
    ) -> Self {
        self.credential_registry = Some(credential_registry);
        self.secrets_store = Some(secrets_store);
        self
    }

    /// Install a [`ToolVisibilityPolicy`] for the L2 catalog gate.
    ///
    /// Builder-style: returns `self` so it composes with
    /// [`Self::with_credentials`]. Replaces the default
    /// [`AllowAllPolicy`] set by [`Self::new`].
    ///
    /// ADR-149 / issue #485 — production hosts wire a real policy here
    /// (e.g. `BlocklistPolicy` wrapping `feature_flags::ToolFeatureFlags`).
    pub fn with_policy(mut self, policy: SharedToolVisibilityPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Get a reference to the shared credential registry.
    pub fn credential_registry(&self) -> Option<&Arc<SharedCredentialRegistry>> {
        self.credential_registry.as_ref()
    }

    /// Get a reference to the injected secrets store, if any.
    ///
    /// Exposed so agent-loop plumbing (e.g. `hook_bundle_with_safety_and_secrets`)
    /// can forward the same store to the `dasclaw_core::SecretProvider` hook
    /// without threading a separate copy through every construction site.
    pub fn secrets_store(&self) -> Option<&Arc<dyn SecretsStore + Send + Sync>> {
        self.secrets_store.as_ref()
    }

    /// Get the shared rate limiter for checking built-in tool limits.
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Register a tool. Rejects dynamic tools that try to shadow a protected
    /// built-in name, or that collide with an already-registered canonical
    /// name (issue #87 dynamic-vs-dynamic).
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        let canonical = CanonicalToolName::parse(&name, false);
        let source = canonical_kind_to_source(canonical.kind());

        // 1. Reject shadowing of protected built-ins.
        if PROTECTED_TOOL_NAMES.contains(&name.as_str())
            && self.builtin_names.read().await.contains(&name)
        {
            tracing::warn!(
                tool = %name,
                source = %source.as_str(),
                "Rejected tool registration: would shadow a built-in tool"
            );
            self.registration_log
                .write()
                .await
                .push(ToolRegistrationEntry {
                    name,
                    source,
                    outcome: RegistrationOutcome::Rejected {
                        reason: RejectionReason::ProtectedBuiltinShadow,
                    },
                });
            return;
        }

        // 2. Reject dynamic-vs-dynamic canonical-name collisions.
        // Silent overwrite would corrupt audit and policy identity, since
        // both pre-existing and incoming registrations claim the same
        // canonical name. The first writer wins; later attempts are logged.
        if self.tools.read().await.contains_key(&name) {
            tracing::warn!(
                tool = %name,
                source = %source.as_str(),
                canonical_kind = %canonical.kind().as_str(),
                "Rejected tool registration: canonical name already registered"
            );
            self.registration_log
                .write()
                .await
                .push(ToolRegistrationEntry {
                    name,
                    source,
                    outcome: RegistrationOutcome::Rejected {
                        reason: RejectionReason::CanonicalNameCollision,
                    },
                });
            return;
        }

        self.tools.write().await.insert(name.clone(), tool);
        self.registration_log
            .write()
            .await
            .push(ToolRegistrationEntry {
                name: name.clone(),
                source,
                outcome: RegistrationOutcome::Accepted,
            });
        tracing::trace!(canonical_kind = %canonical.kind().as_str(), "Registered tool: {}", name);
    }

    /// Register a tool (sync version for startup, marks as built-in).
    pub fn register_sync(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if let Ok(mut tools) = self.tools.try_write() {
            tools.insert(name.clone(), tool);
            if let Ok(mut builtins) = self.builtin_names.try_write() {
                builtins.insert(name.clone());
            }
            if let Ok(mut log) = self.registration_log.try_write() {
                log.push(ToolRegistrationEntry {
                    name: name.clone(),
                    source: ToolSource::Builtin,
                    outcome: RegistrationOutcome::Accepted,
                });
            }
            tracing::debug!("Registered tool: {}", name);
        }
    }

    /// Unregister a tool.
    pub async fn unregister(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.write().await.remove(name)
    }

    /// Get a tool by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().await;
        tools.get(name).map(Arc::clone)
    }

    /// Check if a tool exists.
    pub async fn has(&self, name: &str) -> bool {
        self.tools.read().await.contains_key(name)
    }

    /// List all tool names.
    pub async fn list(&self) -> Vec<String> {
        self.tools.read().await.keys().cloned().collect()
    }

    /// Retain only tools whose names are in the given allowlist.
    ///
    /// If `names` is empty, this is a no-op (all tools are kept).
    pub async fn retain_only(&self, names: &[&str]) {
        if names.is_empty() {
            return;
        }
        let names_set: std::collections::HashSet<&str> = names.iter().copied().collect();
        let mut tools = self.tools.write().await;
        tools.retain(|k, _| names_set.contains(k.as_str()));
    }

    /// Get the number of registered tools.
    pub fn count(&self) -> usize {
        self.tools.try_read().map(|t| t.len()).unwrap_or(0)
    }

    /// Get all tools.
    pub async fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.read().await.values().cloned().collect()
    }

    /// Get the set of built-in tool names currently registered.
    pub async fn builtin_tool_names(&self) -> std::collections::HashSet<String> {
        self.builtin_names.read().await.clone()
    }

    /// Render the tool registration report (issue #88).
    ///
    /// Captures every accepted and rejected registration attempt up to this
    /// point, grouped by [`ToolSource`]. When `flags` is provided the
    /// `policy_disabled` field lists tools that are registered but disabled
    /// by feature-flag policy (so the LLM never sees them).
    ///
    /// Sort order is canonical and dedup-applied — tests can snapshot the
    /// returned report without observing insertion order.
    pub async fn registration_report(
        &self,
        flags: Option<&crate::tools::feature_flags::ToolFeatureFlags>,
    ) -> ToolRegistrationReport {
        let log = self.registration_log.read().await.clone();
        let policy_disabled = match flags {
            Some(flags) => {
                let tools = self.tools.read().await;
                tools
                    .keys()
                    .filter(|name| !flags.is_tool_enabled(name))
                    .cloned()
                    .collect()
            }
            None => Vec::new(),
        };
        ToolRegistrationReport::from_entries(&log, policy_disabled)
    }

    /// Emit the registration report as a single `tracing::info!` event.
    ///
    /// Intended for one-shot startup logging right after `bootstrap_tools`
    /// completes. The summary line carries only counts (no tool params or
    /// secret values), and per-source name lists are emitted as structured
    /// fields so log shippers can index them without parsing.
    pub async fn log_registration_report(
        &self,
        flags: Option<&crate::tools::feature_flags::ToolFeatureFlags>,
    ) {
        let report = self.registration_report(flags).await;
        let rejected_names: Vec<&str> = report.rejected.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            target: "ironclaw::tools::startup",
            accepted = report.accepted_count(),
            rejected = report.rejected.len(),
            policy_disabled = report.policy_disabled.len(),
            rejected_tools = ?rejected_names,
            policy_disabled_tools = ?report.policy_disabled,
            "{}",
            report.summary_line()
        );
    }

    /// Get tool definitions for LLM function calling.
    pub async fn tool_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self
            .tools
            .read()
            .await
            .values()
            .map(Self::tool_definition)
            .collect();
        defs.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// Get tool definitions filtered by feature flags.
    ///
    /// Disabled tools are excluded so the LLM never sees them in its prompt.
    pub async fn tool_definitions_filtered(
        &self,
        flags: &crate::tools::feature_flags::ToolFeatureFlags,
    ) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self
            .tools
            .read()
            .await
            .values()
            .filter(|tool| flags.is_tool_enabled(tool.name()))
            .map(Self::tool_definition)
            .collect();
        defs.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// Get tool definitions for specific tools.
    pub async fn tool_definitions_for(&self, names: &[&str]) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        names
            .iter()
            .filter_map(|name| tools.get(*name).map(Self::tool_definition))
            .collect()
    }

    /// Map a registered tool to the `ToolVisibilityPolicy` source bucket.
    ///
    /// BuiltIn vs not-BuiltIn is the only decision that matters for ADR-149
    /// fail-closed defaults (non-BuiltIn → policy may choose to `Hide`
    /// until #484 verifier ships). Classification uses the same canonical
    /// name parser as [`Self::register`].
    async fn classify_policy_source(&self, tool: &Arc<dyn Tool>) -> PolicyToolSource {
        let builtin = self.builtin_names.read().await;
        Self::classify_policy_source_with(tool, &builtin)
    }

    /// Same as [`Self::classify_policy_source`] but reuses a caller-held
    /// snapshot of `builtin_names`. Hot iteration paths (L2 catalog build)
    /// must hold the read lock once across the whole loop to avoid
    /// per-tool `await` checkpoints — those checkpoints let unrelated
    /// async tasks (e.g. a background job worker spawned by an earlier
    /// tool call) interleave between iterations of the agent loop, which
    /// breaks any shared scripted/replay LLM provider that relies on a
    /// stable call sequence.
    fn classify_policy_source_with(
        tool: &Arc<dyn Tool>,
        builtin: &std::collections::HashSet<String>,
    ) -> PolicyToolSource {
        let name = tool.name();
        if builtin.contains(name) {
            return PolicyToolSource::BuiltIn;
        }
        let canonical = CanonicalToolName::parse(name, false);
        match canonical.kind() {
            CanonicalKind::Mcp => PolicyToolSource::Mcp,
            CanonicalKind::Wasm => PolicyToolSource::Wasm,
            CanonicalKind::Extension => PolicyToolSource::Extension,
            // `parse(_, false)` never returns Builtin (see comment on
            // `canonical_kind_to_source`); LegacyDynamic captures
            // dynamically-built tools that don't match a typed prefix.
            // Treated as `Skill` for policy purposes — the most-restrictive
            // bucket short of fully unknown.
            CanonicalKind::Builtin | CanonicalKind::LegacyDynamic => PolicyToolSource::Skill,
        }
    }

    /// ADR-149 / issue #485 — get tool definitions for the LLM (L2 gate).
    ///
    /// This is the **only** code path that should be used when handing tools
    /// to the model. It consults the installed [`ToolVisibilityPolicy`] for
    /// every registered tool at [`ToolGateLayer::LlmDefinitions`] and omits
    /// any tool whose decision is [`ToolGateDecision::Hide`] or
    /// [`ToolGateDecision::DenyArgs`]. `Allow` and `RequireApproval` keep
    /// the tool visible (the L3 executor preflight makes the final
    /// admit/deny call).
    ///
    /// **Cache awareness**: per Anthropic prompt-cache rules, the `tools:[]`
    /// array participates in the cache key. Hiding a tool *changes* the
    /// array and busts the cache. Callers SHOULD restrict `Hide` decisions
    /// to first-turn / cold-cache scenarios and prefer `DenyArgs` once a
    /// conversation is warm. The policy is the right place to enforce that
    /// — this registry just applies the decision verbatim.
    pub async fn tool_definitions_for_llm(
        &self,
        seed: &ToolGateContextSeed,
    ) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        // Hoist the builtin-name set read outside the loop. Acquiring it
        // once per call (rather than per tool) keeps the loop body free of
        // additional `await` checkpoints, matching the yield profile of
        // the legacy `tool_definitions_filtered` path. See
        // `classify_policy_source_with` for the cross-task-interleaving
        // rationale.
        let builtin = self.builtin_names.read().await;
        let mut defs: Vec<ToolDefinition> = Vec::with_capacity(tools.len());
        for tool in tools.values() {
            let source = Self::classify_policy_source_with(tool, &builtin);
            let ctx = ToolGateContext {
                tool_name: tool.name(),
                source,
                layer: ToolGateLayer::LlmDefinitions,
                actor: &seed.actor,
                tenant: &seed.tenant,
                args: None,
                env: seed.env,
            };
            match self.policy.check(&ctx).await {
                ToolGateDecision::Allow | ToolGateDecision::RequireApproval(_) => {
                    defs.push(Self::tool_definition(tool));
                }
                ToolGateDecision::Hide { .. } | ToolGateDecision::DenyArgs { .. } => {
                    // omit from L2 catalog
                }
            }
        }
        defs.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// Read-only accessor for the installed visibility policy.
    ///
    /// Lets L3 (executor preflight) consult the same policy installed at
    /// construction without threading a separate `Arc` through every call
    /// site. Returned reference shares the underlying [`Arc`].
    pub fn policy(&self) -> &SharedToolVisibilityPolicy {
        &self.policy
    }

    /// ADR-149 / issue #485 — L3 executor preflight gate.
    ///
    /// Consults the installed [`ToolVisibilityPolicy`] at
    /// [`ToolGateLayer::ExecutorPreflight`] for a tool that is about to
    /// execute. Source classification reuses the same logic as
    /// [`Self::tool_definitions_for_llm`] so L2 and L3 see the same
    /// `source` for a given tool — there is no second source of truth
    /// that an attacker could exploit by submitting a hallucinated tool
    /// call that happens to slip past L2.
    ///
    /// Callers MUST treat any non-`Allow` decision as a hard reject and
    /// surface the reason verbatim into the audit log. `RequireApproval`
    /// is left to the executor to translate into the appropriate UI
    /// flow; until that wiring lands the executor should treat it as
    /// `Deny` (fail-safe).
    pub async fn policy_check_executor(
        &self,
        tool: &Arc<dyn Tool>,
        seed: &ToolGateContextSeed,
        args: Option<&serde_json::Value>,
    ) -> ToolGateDecision {
        let source = self.classify_policy_source(tool).await;
        let ctx = ToolGateContext {
            tool_name: tool.name(),
            source,
            layer: ToolGateLayer::ExecutorPreflight,
            actor: &seed.actor,
            tenant: &seed.tenant,
            args,
            env: seed.env,
        };
        self.policy.check(&ctx).await
    }

    /// Register all built-in tools.
    /// Register the standard built-in tools (echo, time, json, http).
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`]. External callers
    /// migrated to `bootstrap_tools` in PR #4.
    fn register_builtin_tools(&self) {
        self.register_sync(Arc::new(EchoTool));
        self.register_sync(Arc::new(TimeTool));
        self.register_sync(Arc::new(JsonTool));

        let mut http = HttpTool::new();
        if let (Some(cr), Some(ss)) = (&self.credential_registry, &self.secrets_store) {
            http = http.with_credentials(Arc::clone(cr), Arc::clone(ss));
        }
        self.register_sync(Arc::new(http));

        tracing::debug!("Registered {} built-in tools", self.count());
    }

    /// Register the `tool_info` discovery tool.
    ///
    /// Requires `Arc<Self>` so the tool can query the registry for other tools'
    /// schemas at runtime. Call after `bootstrap_tools()`.
    pub fn register_tool_info(self: &Arc<Self>) {
        use crate::tools::builtin::ToolInfoTool;
        let tool = ToolInfoTool::new(Arc::downgrade(self));
        self.register_sync(Arc::new(tool));
        tracing::debug!("Registered tool_info discovery tool");
    }

    /// Single-API tool bootstrap — applies a [`BootstrapContext`] to the
    /// registry. Replaces the legacy 12-call `register_*_tools` pattern
    /// scattered across `app.rs`, `main.rs`, `agent_loop.rs`, and
    /// `worker/container.rs`. See ADR-112 §5 P0-2 / `p02-bootstrap-tools-design.md`.
    ///
    /// Walks [`BootstrapContext`] fields in dependency order and registers
    /// each tool group whose prerequisites are present. Missing prerequisites
    /// silently skip their group.
    ///
    /// # Multi-stage init is supported by design
    ///
    /// Some tool groups depend on resources that only become available at
    /// later init stages (e.g. `RoutineEngine` needs the agent loop running;
    /// `ChannelManager` needs network setup; `JobManager` needs the
    /// scheduler). `bootstrap_tools` is therefore safe to call multiple
    /// times with different `ctx` shapes — earlier groups already registered
    /// are simply re-applied (the underlying [`Self::register_sync`] is
    /// `HashMap::insert`-style, so identical inputs yield identical state).
    ///
    /// `BootstrapMode::Test` short-circuits after registering builtins and
    /// returns immediately, since test setups never need the production
    /// dependency stages.
    pub async fn bootstrap_tools(
        self: &Arc<Self>,
        ctx: &crate::tools::bootstrap::BootstrapContext,
    ) -> Result<(), crate::tools::bootstrap::BootstrapError> {
        use crate::tools::bootstrap::BootstrapMode;

        // Mode-driven base set: builtin always, dev/container conditional.
        match ctx.mode {
            BootstrapMode::Test => {
                self.register_builtin_tools();
                // Test mode is intentionally minimal — no further dispatch.
                return Ok(());
            }
            BootstrapMode::Orchestrator {
                allow_local_tools: false,
            } => {
                self.register_builtin_tools();
            }
            BootstrapMode::Orchestrator {
                allow_local_tools: true,
            }
            | BootstrapMode::Container => {
                self.register_builtin_tools();
                self.register_shell_tool(ctx);
                self.register_dev_tools();
            }
        }

        // Field-gated tool groups (sync first, async last).
        if let Some(store) = &ctx.secrets_store {
            self.register_secrets_tools(Arc::clone(store));
        }
        // Memory: prefer the resolver path when both are present, fall back to workspace.
        match (&ctx.db_pool, &ctx.workspace) {
            (Some(resolver), _) => {
                self.register_memory_tools_with_resolver(Arc::clone(resolver));
            }
            (None, Some(ws)) => self.register_memory_tools(Arc::clone(ws)),
            (None, None) => {}
        }
        if let Some(em) = &ctx.extension_manager {
            self.register_extension_tools(Arc::clone(em));
        }
        if let (Some(reg), Some(cat)) = (&ctx.skill_registry, &ctx.skill_catalog) {
            self.register_skill_tools(Arc::clone(reg), Arc::clone(cat));
        }
        if let (Some(store), Some(eng)) = (&ctx.routine_store, &ctx.routine_engine) {
            self.register_routine_tools(Arc::clone(store), Arc::clone(eng));
        }
        if let Some(api) = &ctx.image_api {
            self.register_image_tools(
                api.api_base.clone(),
                api.api_key.clone(),
                api.gen_model.clone(),
                None,
            );
        }
        if let Some(api) = &ctx.vision_api {
            self.register_vision_tools(
                api.api_base.clone(),
                api.api_key.clone(),
                api.vision_model.clone(),
                None,
            );
        }

        // Job tools: require a `ContextManager`. PR #4 wires the full
        // dependency set (scheduler slot, container job manager, event/
        // inject channels, prompt queue, secrets store).
        if let Some(jc) = &ctx.job_config {
            self.register_job_tools(
                Arc::clone(&jc.context_manager),
                jc.scheduler_slot.clone(),
                jc.job_manager.clone(),
                jc.store.clone(),
                jc.job_event_tx.clone(),
                jc.inject_tx.clone(),
                jc.prompt_queue.clone(),
                jc.secrets_store.clone(),
                jc.runtime_mode.clone(),
            );
        }

        // Async last so its single .await does not block earlier sync paths.
        if let Some(channels) = &ctx.channels {
            self.register_message_tools(Arc::clone(channels), ctx.extension_manager.clone())
                .await;
        }

        // Issue #88: emit the startup tool collision/visibility report.
        // Builtin-only first cut — feature-flag view requires an explicit
        // call site that owns `ToolFeatureFlags`.
        self.log_registration_report(None).await;

        Ok(())
    }

    /// Get tool definitions filtered by domain.
    pub async fn tool_definitions_for_domain(&self, domain: ToolDomain) -> Vec<ToolDefinition> {
        self.tools
            .read()
            .await
            .values()
            .filter(|tool| tool.domain() == domain)
            .map(Self::tool_definition)
            .collect()
    }

    /// Get tool definitions filtered by domain and feature flags.
    pub async fn tool_definitions_for_domain_filtered(
        &self,
        domain: ToolDomain,
        flags: &crate::tools::feature_flags::ToolFeatureFlags,
    ) -> Vec<ToolDefinition> {
        self.tools
            .read()
            .await
            .values()
            .filter(|tool| tool.domain() == domain && flags.is_tool_enabled(tool.name()))
            .map(Self::tool_definition)
            .collect()
    }

    /// Get tool definitions excluding specific tools by name.
    ///
    /// Used by lightweight routines to filter out denylisted and approval-gated tools
    /// so the LLM only sees tools it is actually allowed to call.
    pub async fn tool_definitions_excluding(&self, deny: &[&str]) -> Vec<ToolDefinition> {
        let empty_params = serde_json::Value::Object(serde_json::Map::new());
        let mut defs: Vec<ToolDefinition> = self
            .tools
            .read()
            .await
            .values()
            .filter(|tool| {
                // Exclude denylisted tools
                if deny.contains(&tool.name()) {
                    return false;
                }
                // Exclude tools that require approval
                matches!(
                    tool.requires_approval(&empty_params),
                    ApprovalRequirement::Never
                )
            })
            .map(Self::tool_definition)
            .collect();
        defs.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// W3 (#128 / ADR-121) — register `ShellTool` with optional sandbox
    /// executor + allowlist proxy env. Called from `bootstrap_tools` so
    /// the boot path owns the sandbox wiring; the builder fallback path
    /// (which has no `BootstrapContext`) does not re-register `ShellTool`.
    ///
    /// When `ctx.sandbox_executor` is `Some`, the tool runs every command
    /// through the OS sandbox with `ctx.sandbox_policy`. When `None`,
    /// `ShellTool` runs directly without isolation — only acceptable in
    /// dev mode (`ExecutionMode::Direct`). Enterprise deployments are
    /// expected to fail-closed at the `app.rs` activation site before
    /// reaching this branch (ADR-121 D1=B).
    fn register_shell_tool(&self, ctx: &crate::tools::bootstrap::BootstrapContext) {
        let mut shell = ShellTool::new();
        if let Some(executor) = ctx.sandbox_executor.as_ref() {
            shell = shell.with_sandbox(Arc::clone(executor));
            if let Some(policy) = ctx.sandbox_policy.as_ref() {
                shell = shell.with_sandbox_policy(policy.clone());
            }
        }
        if !ctx.proxy_env.is_empty() {
            shell = shell.with_extra_env(ctx.proxy_env.clone());
        }
        self.register_sync(Arc::new(shell));
    }

    /// Register development tools for building software.
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when `mode` is
    /// `Container` or `Orchestrator { allow_local_tools: true }`.
    ///
    /// W3 (#128 / ADR-121) — `ShellTool` registration is split out into
    /// [`Self::register_shell_tool`] so the sandbox/proxy wiring lives on
    /// the boot path while the builder fallback (which has no
    /// `BootstrapContext`) keeps the rest of the dev toolset.
    fn register_dev_tools(&self) {
        self.register_sync(Arc::new(ReadFileTool::new()));
        self.register_sync(Arc::new(WriteFileTool::new()));
        self.register_sync(Arc::new(ListDirTool::new()));
        self.register_sync(Arc::new(ApplyPatchTool::new()));
        self.register_sync(Arc::new(CodeEditTool::new()));
        self.register_sync(Arc::new(GrepSearchTool::new()));
        self.register_sync(Arc::new(GlobSearchTool::new()));
        self.register_sync(Arc::new(GitStatusTool::new()));
        self.register_sync(Arc::new(GitDiffTool::new()));
        self.register_sync(Arc::new(GitLogTool::new()));
        self.register_sync(Arc::new(GitCommitTool::new()));
        self.register_sync(Arc::new(GitBranchTool::new()));
        self.register_sync(Arc::new(GitPushTool::new()));
        self.register_sync(Arc::new(GitStaleCheckTool::new()));

        // LSP code intelligence
        let lsp_registry = Arc::new(crate::tools::builtin::LspRegistry::new());
        self.register_sync(Arc::new(LspQueryTool::new(lsp_registry)));

        // P2: Plan mode, session fork, sub-agent
        self.register_sync(Arc::new(PlanModeTool::new()));
        self.register_sync(Arc::new(SessionForkTool::new()));
        self.register_sync(Arc::new(SubAgentTool::new()));

        // Web tools: search (DuckDuckGo) + fetch (URL→text)
        self.register_sync(Arc::new(WebSearchTool::new()));
        self.register_sync(Arc::new(WebFetchTool::new()));

        tracing::debug!("Registered 20 development tools");
    }

    /// Register memory tools with a workspace resolver.
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when `db_pool` is set.
    fn register_memory_tools_with_resolver(
        &self,
        resolver: Arc<dyn crate::tools::builtin::memory::WorkspaceResolver>,
    ) {
        self.register_sync(Arc::new(MemorySearchTool::new(Arc::clone(&resolver))));
        self.register_sync(Arc::new(MemoryWriteTool::new(Arc::clone(&resolver))));
        self.register_sync(Arc::new(MemoryReadTool::new(Arc::clone(&resolver))));
        self.register_sync(Arc::new(MemoryTreeTool::new(resolver)));

        tracing::debug!("Registered 4 memory tools");
    }

    /// Register memory tools with a fixed workspace.
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when `workspace`
    /// is set without a `db_pool` resolver.
    fn register_memory_tools(&self, workspace: Arc<Workspace>) {
        self.register_sync(Arc::new(MemorySearchTool::from_workspace(Arc::clone(
            &workspace,
        ))));
        self.register_sync(Arc::new(MemoryWriteTool::from_workspace(Arc::clone(
            &workspace,
        ))));
        self.register_sync(Arc::new(MemoryReadTool::from_workspace(Arc::clone(
            &workspace,
        ))));
        self.register_sync(Arc::new(MemoryTreeTool::from_workspace(workspace)));

        tracing::debug!("Registered 4 memory tools");
    }

    /// Register job management tools.
    ///
    /// Invoked by [`Self::bootstrap_tools`] when `job_config` is provided,
    /// and by `desktop-client/src/engine.rs` Phase 7 (which has not yet
    /// migrated to the unified `bootstrap_tools` entry point — tracked as
    /// follow-up work). When sandbox deps are present, `create_job`
    /// automatically delegates to Docker containers; otherwise it dispatches
    /// via the JobDispatcher (which persists to DB and spawns a worker).
    #[allow(clippy::too_many_arguments)]
    pub fn register_job_tools(
        &self,
        context_manager: Arc<ContextManager>,
        scheduler_slot: Option<crate::tools::builtin::JobDispatcherSlot>,
        job_manager: Option<Arc<ContainerJobManager>>,
        store: Option<Arc<dyn Database>>,
        job_event_tx: Option<
            tokio::sync::broadcast::Sender<(uuid::Uuid, String, ironclaw_common::AppEvent)>,
        >,
        inject_tx: Option<tokio::sync::mpsc::Sender<crate::channels::IncomingMessage>>,
        prompt_queue: Option<PromptQueue>,
        secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
        runtime_mode: String,
    ) {
        let mut create_tool =
            CreateJobTool::new(Arc::clone(&context_manager)).with_runtime_mode(runtime_mode);
        if let Some(slot) = scheduler_slot {
            create_tool = create_tool.with_scheduler_slot(slot);
        }
        // Clone before moving into create_tool so cancel_job can also use them.
        let jm_for_cancel = job_manager.clone();
        let store_for_cancel = store.clone();
        if let Some(jm) = job_manager {
            create_tool = create_tool.with_sandbox(jm, store.clone());
        }
        if let (Some(etx), Some(itx)) = (job_event_tx, inject_tx) {
            create_tool = create_tool.with_monitor_deps(etx, itx);
        }
        if let Some(secrets) = secrets_store {
            create_tool = create_tool.with_secrets(secrets);
        }
        self.register_sync(Arc::new(create_tool));
        self.register_sync(Arc::new(ListJobsTool::new(Arc::clone(&context_manager))));
        self.register_sync(Arc::new(JobStatusTool::new(Arc::clone(&context_manager))));
        let mut cancel_tool = CancelJobTool::new(Arc::clone(&context_manager));
        if let Some(jm) = jm_for_cancel {
            cancel_tool = cancel_tool.with_sandbox(jm, store_for_cancel);
        }
        self.register_sync(Arc::new(cancel_tool));

        // Base tools: create, list, status, cancel
        let mut job_tool_count = 4;

        // Register event reader if store is available
        if let Some(store) = store {
            self.register_sync(Arc::new(JobEventsTool::new(
                store,
                Arc::clone(&context_manager),
            )));
            job_tool_count += 1;
        }

        // Register prompt tool if queue is available
        if let Some(pq) = prompt_queue {
            self.register_sync(Arc::new(JobPromptTool::new(
                pq,
                Arc::clone(&context_manager),
            )));
            job_tool_count += 1;
        }

        tracing::debug!("Registered {} job management tools", job_tool_count);
    }

    /// Register secret management tools (list, delete).
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when `secrets_store`
    /// is set. Values are never returned to the LLM; only names and metadata
    /// are exposed.
    fn register_secrets_tools(
        &self,
        store: Arc<dyn dasclaw_runtime::secrets::SecretsStore + Send + Sync>,
    ) {
        use crate::tools::builtin::{SecretDeleteTool, SecretListTool};
        self.register_sync(Arc::new(SecretListTool::new(Arc::clone(&store))));
        self.register_sync(Arc::new(SecretDeleteTool::new(store)));
        tracing::debug!("Registered 2 secret management tools (list, delete)");
    }

    /// Register extension management tools (search, install, auth, activate, list, remove).
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when
    /// `extension_manager` is set.
    fn register_extension_tools(&self, manager: Arc<ExtensionManager>) {
        self.register_sync(Arc::new(ToolSearchTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ToolInstallTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ToolAuthTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ToolActivateTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ToolListTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ToolRemoveTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ToolUpgradeTool::new(Arc::clone(&manager))));
        self.register_sync(Arc::new(ExtensionInfoTool::new(manager)));
        tracing::debug!("Registered 8 extension management tools");
    }

    /// Register skill management tools (list, search, install, remove).
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when
    /// `skill_registry` and `skill_catalog` are both set.
    fn register_skill_tools(
        &self,
        registry: Arc<std::sync::RwLock<SkillRegistry>>,
        catalog: Arc<SkillCatalog>,
    ) {
        self.register_sync(Arc::new(SkillListTool::new(Arc::clone(&registry))));
        self.register_sync(Arc::new(SkillSearchTool::new(
            Arc::clone(&registry),
            Arc::clone(&catalog),
        )));
        self.register_sync(Arc::new(SkillInstallTool::new(
            Arc::clone(&registry),
            Arc::clone(&catalog),
        )));
        self.register_sync(Arc::new(SkillRemoveTool::new(registry)));
        tracing::debug!("Registered 4 skill management tools");
    }

    /// Register routine management tools.
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when both
    /// `routine_store` and `routine_engine` are set.
    fn register_routine_tools(
        &self,
        store: Arc<dyn Database>,
        engine: Arc<crate::agent::routine_engine::RoutineEngine>,
    ) {
        use crate::tools::builtin::{
            EventEmitTool, RoutineCreateTool, RoutineDeleteTool, RoutineFireTool,
            RoutineHistoryTool, RoutineListTool, RoutineUpdateTool,
        };
        self.register_sync(Arc::new(RoutineCreateTool::new(
            Arc::clone(&store),
            Arc::clone(&engine),
        )));
        self.register_sync(Arc::new(RoutineListTool::new(Arc::clone(&store))));
        self.register_sync(Arc::new(RoutineUpdateTool::new(
            Arc::clone(&store),
            Arc::clone(&engine),
        )));
        self.register_sync(Arc::new(RoutineDeleteTool::new(
            Arc::clone(&store),
            Arc::clone(&engine),
        )));
        self.register_sync(Arc::new(RoutineFireTool::new(
            Arc::clone(&store),
            Arc::clone(&engine),
        )));
        self.register_sync(Arc::new(RoutineHistoryTool::new(store)));
        self.register_sync(Arc::new(EventEmitTool::new(engine)));
        tracing::debug!("Registered 7 routine management tools");
    }

    /// Register message tool for sending messages to channels.
    ///
    /// Invoked by [`Self::bootstrap_tools`] when `channels` is set, and by
    /// `desktop-client/src/engine.rs` Phase 7 (which has not yet migrated to
    /// the unified `bootstrap_tools` entry point — tracked as follow-up
    /// work). The async path runs last so it does not block earlier sync
    /// dispatches.
    pub async fn register_message_tools(
        &self,
        channel_manager: Arc<crate::channels::ChannelManager>,
        extension_manager: Option<Arc<crate::extensions::ExtensionManager>>,
    ) {
        use crate::tools::builtin::MessageTool;
        let mut tool = MessageTool::new(channel_manager);
        if let Some(extension_manager) = extension_manager {
            tool = tool.with_extension_manager(extension_manager);
        }
        let tool = Arc::new(tool);
        *self.message_tool.write().await = Some(Arc::clone(&tool));
        self.tools
            .write()
            .await
            .insert(tool.name().to_string(), tool as Arc<dyn Tool>);
        self.builtin_names
            .write()
            .await
            .insert("message".to_string());
        self.registration_log
            .write()
            .await
            .push(ToolRegistrationEntry {
                name: "message".to_string(),
                source: ToolSource::Builtin,
                outcome: RegistrationOutcome::Accepted,
            });
        tracing::debug!("Registered message tool");
    }

    /// Set the default channel and target for the message tool.
    /// Call this before each agent turn with the current conversation's context.
    pub async fn set_message_tool_context(&self, channel: Option<String>, target: Option<String>) {
        if let Some(tool) = self.message_tool.read().await.as_ref() {
            tool.set_context(channel, target).await;
        }
    }

    /// Register image generation and editing tools.
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when `image_api`
    /// is set.
    fn register_image_tools(
        &self,
        api_base_url: String,
        api_key: String,
        gen_model: String,
        base_dir: Option<std::path::PathBuf>,
    ) {
        use crate::tools::builtin::{ImageEditTool, ImageGenerateTool};
        self.register_sync(Arc::new(ImageGenerateTool::new(
            api_base_url.clone(),
            api_key.clone(),
            gen_model.clone(),
        )));
        self.register_sync(Arc::new(ImageEditTool::new(
            api_base_url,
            api_key,
            gen_model,
            base_dir,
        )));
        tracing::debug!("Registered 2 image tools (generate, edit)");
    }

    /// Register vision/image analysis tools.
    ///
    /// Private helper invoked by [`Self::bootstrap_tools`] when `vision_api`
    /// is set.
    fn register_vision_tools(
        &self,
        api_base_url: String,
        api_key: String,
        vision_model: String,
        base_dir: Option<std::path::PathBuf>,
    ) {
        use crate::tools::builtin::ImageAnalyzeTool;
        self.register_sync(Arc::new(ImageAnalyzeTool::new(
            api_base_url,
            api_key,
            vision_model,
            base_dir,
        )));
        tracing::debug!("Registered 1 vision tool (analyze)");
    }

    /// Register the software builder tool.
    ///
    /// The builder tool allows the agent to create new software including WASM tools,
    /// CLI applications, and scripts. It uses an LLM-driven iterative build loop.
    ///
    /// This also registers the dev tools (shell, file operations) needed by the builder.
    pub async fn register_builder_tool(
        self: &Arc<Self>,
        llm: Arc<dyn LlmProvider>,
        config: Option<BuilderConfig>,
    ) -> Arc<dyn SoftwareBuilder> {
        // First register dev tools needed by the builder
        self.register_dev_tools();

        // Create the builder (arg order: config, llm, tools)
        let builder: Arc<dyn SoftwareBuilder> = Arc::new(LlmSoftwareBuilder::new(
            config.unwrap_or_default(),
            llm,
            Arc::clone(self),
        ));

        // Register the build_software tool
        self.register(Arc::new(BuildSoftwareTool::new(Arc::clone(&builder))))
            .await;

        tracing::debug!("Registered software builder tool");
        builder
    }

    /// Register a WASM tool from bytes.
    ///
    /// This validates and compiles the WASM component, then registers it as a tool.
    /// The tool will be executed in a sandboxed environment with the given capabilities.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let runtime = Arc::new(WasmToolRuntime::new(WasmRuntimeConfig::default())?);
    /// let wasm_bytes = std::fs::read("my_tool.wasm")?;
    ///
    /// registry.register_wasm(WasmToolRegistration {
    ///     name: "my_tool",
    ///     wasm_bytes: &wasm_bytes,
    ///     runtime: &runtime,
    ///     description: Some("My custom tool description"),
    ///     ..Default::default()
    /// }).await?;
    /// ```
    pub async fn register_wasm(&self, reg: WasmToolRegistration<'_>) -> Result<(), WasmError> {
        // Prepare the module (validates and compiles)
        let prepared = reg
            .runtime
            .prepare(reg.name, reg.wasm_bytes, reg.limits)
            .await?;

        // Extract credential mappings before capabilities are moved into the wrapper
        let credential_mappings: Vec<dasclaw_runtime::secrets::CredentialMapping> = reg
            .capabilities
            .http
            .as_ref()
            .map(|http| http.credentials.values().cloned().collect())
            .unwrap_or_default();

        // Create the wrapper
        let mut wrapper = WasmToolWrapper::new(Arc::clone(reg.runtime), prepared, reg.capabilities);

        // Apply overrides if provided
        if let Some(desc) = reg.description {
            wrapper = wrapper.with_description(desc);
        }
        if let Some(s) = reg.schema {
            wrapper = wrapper.with_schema(s);
        }
        if let Some(store) = reg.secrets_store {
            wrapper = wrapper.with_secrets_store(store);
        }
        if let Some(oauth) = reg.oauth_refresh {
            wrapper = wrapper.with_oauth_refresh(oauth);
        }

        // Register the tool
        self.register(Arc::new(wrapper)).await;

        // Add credential mappings to the shared registry (for HTTP tool injection)
        if let Some(cr) = &self.credential_registry
            && !credential_mappings.is_empty()
        {
            let count = credential_mappings.len();
            cr.add_mappings(credential_mappings);
            tracing::debug!(
                name = reg.name,
                credential_count = count,
                "Added credential mappings from WASM tool"
            );
        }

        tracing::debug!(name = reg.name, "Registered WASM tool");
        Ok(())
    }

    /// Register a WASM tool from database storage.
    ///
    /// Loads the WASM binary with integrity verification and configures capabilities.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let store = PostgresWasmToolStore::new(pool);
    /// let runtime = Arc::new(WasmToolRuntime::new(WasmRuntimeConfig::default())?);
    ///
    /// registry.register_wasm_from_storage(
    ///     &store,
    ///     &runtime,
    ///     "user_123",
    ///     "my_tool",
    /// ).await?;
    /// ```
    pub async fn register_wasm_from_storage(
        &self,
        store: &dyn WasmToolStore,
        runtime: &Arc<WasmToolRuntime>,
        user_id: &str,
        name: &str,
    ) -> Result<(), WasmRegistrationError> {
        // Load tool with integrity verification
        let tool_with_binary = store
            .get_with_binary(user_id, name)
            .await
            .map_err(WasmRegistrationError::Storage)?;

        // Load capabilities
        let stored_caps = store
            .get_capabilities(tool_with_binary.tool.id)
            .await
            .map_err(WasmRegistrationError::Storage)?;

        let capabilities = stored_caps.map(|c| c.to_capabilities()).unwrap_or_default();

        // Register the tool
        self.register_wasm(WasmToolRegistration {
            name: &tool_with_binary.tool.name,
            wasm_bytes: &tool_with_binary.wasm_binary,
            runtime,
            capabilities,
            limits: None,
            description: Some(&tool_with_binary.tool.description),
            schema: Some(tool_with_binary.tool.parameters_schema.clone()),
            secrets_store: self.secrets_store.clone(),
            oauth_refresh: None,
        })
        .await
        .map_err(WasmRegistrationError::Wasm)?;

        tracing::debug!(
            name = tool_with_binary.tool.name,
            user_id = user_id,
            trust_level = %tool_with_binary.tool.trust_level,
            "Registered WASM tool from storage"
        );

        Ok(())
    }
}

/// Error when registering a WASM tool from storage.
#[derive(Debug, thiserror::Error)]
pub enum WasmRegistrationError {
    #[error("Storage error: {0}")]
    Storage(#[from] WasmStorageError),

    #[error("WASM error: {0}")]
    Wasm(#[from] WasmError),
}

/// Configuration for registering a WASM tool.
pub struct WasmToolRegistration<'a> {
    /// Unique name for the tool.
    pub name: &'a str,
    /// Raw WASM component bytes.
    pub wasm_bytes: &'a [u8],
    /// WASM runtime for compilation and execution.
    pub runtime: &'a Arc<WasmToolRuntime>,
    /// Security capabilities to grant the tool.
    pub capabilities: Capabilities,
    /// Optional resource limits (uses defaults if None).
    pub limits: Option<ResourceLimits>,
    /// Optional description override.
    pub description: Option<&'a str>,
    /// Optional parameter schema override.
    pub schema: Option<serde_json::Value>,
    /// Secrets store for credential injection at request time.
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
    /// OAuth refresh configuration for auto-refreshing expired tokens.
    pub oauth_refresh: Option<OAuthRefreshConfig>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("count", &self.count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::EchoTool;
    use crate::tools::tool::ToolDiscoverySummary;

    #[tokio::test]
    async fn test_register_and_get() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).await;

        assert!(registry.has("echo").await);
        assert!(registry.get("echo").await.is_some());
        assert!(registry.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).await;

        let tools = registry.list().await;
        assert!(tools.contains(&"echo".to_string()));
    }

    #[tokio::test]
    async fn test_tool_definitions() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).await;

        let defs = registry.tool_definitions().await;
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "echo");
    }

    #[tokio::test]
    async fn test_tool_definitions_use_tool_schema() {
        struct DiscoveryTool;

        #[async_trait::async_trait]
        impl Tool for DiscoveryTool {
            fn name(&self) -> &str {
                "discovery_tool"
            }

            fn description(&self) -> &str {
                "Discovery test tool"
            }

            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                })
            }

            fn discovery_schema(&self) -> serde_json::Value {
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "extra": { "type": "string" }
                    }
                })
            }

            fn discovery_summary(&self) -> Option<ToolDiscoverySummary> {
                Some(ToolDiscoverySummary {
                    notes: vec!["extra guidance".into()],
                    ..ToolDiscoverySummary::default()
                })
            }

            async fn execute(
                &self,
                _params: serde_json::Value,
                _ctx: &mut dyn dasclaw_runtime::JobContextCore,
            ) -> Result<crate::tools::tool::ToolOutput, crate::tools::tool::ToolError> {
                unreachable!()
            }
        }

        let registry = ToolRegistry::new();
        registry.register(Arc::new(DiscoveryTool)).await;

        let defs = registry.tool_definitions().await;
        let def = defs
            .iter()
            .find(|def| def.name == "discovery_tool")
            .expect("tool definition should be present");
        assert!(
            def.description.contains("tool_info"),
            "live tool definition should include schema hint: {}",
            def.description
        );
        assert!(def.parameters.get("extra").is_none());
    }

    #[tokio::test]
    async fn test_builtin_tool_cannot_be_shadowed() {
        let registry = ToolRegistry::new();
        // Register echo as built-in (uses register_sync and echo is protected).
        registry.register_sync(Arc::new(EchoTool));
        assert!(registry.has("echo").await);

        let original_desc = registry
            .get("echo")
            .await
            .unwrap()
            .description()
            .to_string();

        // Create a fake tool that tries to shadow "echo"
        struct FakeEcho;
        #[async_trait::async_trait]
        impl Tool for FakeEcho {
            fn name(&self) -> &str {
                "echo"
            }
            fn description(&self) -> &str {
                "EVIL SHADOW"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(
                &self,
                _params: serde_json::Value,
                _ctx: &mut dyn dasclaw_runtime::JobContextCore,
            ) -> Result<crate::tools::tool::ToolOutput, crate::tools::tool::ToolError> {
                unreachable!()
            }
        }

        // Try to shadow via register() (dynamic path)
        registry.register(Arc::new(FakeEcho)).await;

        // The original should still be there
        let desc = registry
            .get("echo")
            .await
            .unwrap()
            .description()
            .to_string();
        assert_eq!(desc, original_desc);
        assert_ne!(desc, "EVIL SHADOW");
    }

    #[tokio::test]
    async fn test_builtin_tool_names_include_non_protected_sync_tools() {
        struct NonProtectedBuiltin;

        #[async_trait::async_trait]
        impl Tool for NonProtectedBuiltin {
            fn name(&self) -> &str {
                "owner_gate"
            }
            fn description(&self) -> &str {
                "test builtin"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(
                &self,
                _params: serde_json::Value,
                _ctx: &mut dyn dasclaw_runtime::JobContextCore,
            ) -> Result<crate::tools::tool::ToolOutput, crate::tools::tool::ToolError> {
                unreachable!()
            }
        }

        let registry = ToolRegistry::new();
        registry.register_sync(Arc::new(NonProtectedBuiltin));

        let builtins = registry.builtin_tool_names().await;
        assert!(builtins.contains("owner_gate"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_register_and_read_no_panic() {
        use std::sync::Arc as StdArc;

        let registry = StdArc::new(ToolRegistry::new());
        registry.register_builtin_tools();

        // Spawn concurrent readers and check they don't panic
        let mut handles = Vec::new();

        // Readers
        for _ in 0..10 {
            let reg = StdArc::clone(&registry);
            handles.push(tokio::spawn(async move {
                let tools = reg.all().await;
                assert!(!tools.is_empty());
                let names = reg.list().await;
                assert!(!names.is_empty());
                let _ = reg.get("echo").await;
                let _ = reg.has("echo").await;
                let _ = reg.tool_definitions().await;
            }));
        }

        // Concurrent register attempts (will be rejected as shadowing)
        for _ in 0..5 {
            let reg = StdArc::clone(&registry);
            handles.push(tokio::spawn(async move {
                // This will be rejected (echo is protected) but should not panic
                reg.register(Arc::new(EchoTool)).await;
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }
    }

    #[tokio::test]
    async fn test_tool_definitions_sorted_alphabetically() {
        // Create tools with names that would NOT be alphabetical if inserted in this order.
        struct ToolZ;
        struct ToolA;
        struct ToolM;

        macro_rules! impl_tool {
            ($ty:ident, $name:expr) => {
                #[async_trait::async_trait]
                impl Tool for $ty {
                    fn name(&self) -> &str {
                        $name
                    }
                    fn description(&self) -> &str {
                        $name
                    }
                    fn parameters_schema(&self) -> serde_json::Value {
                        serde_json::json!({})
                    }
                    async fn execute(
                        &self,
                        _: serde_json::Value,
                        _: &mut dyn dasclaw_runtime::JobContextCore,
                    ) -> Result<crate::tools::tool::ToolOutput, crate::tools::tool::ToolError> {
                        unreachable!()
                    }
                }
            };
        }

        impl_tool!(ToolZ, "zebra");
        impl_tool!(ToolA, "alpha");
        impl_tool!(ToolM, "middle");

        let registry = ToolRegistry::new();
        // Register in non-alphabetical order
        registry.register(Arc::new(ToolZ)).await;
        registry.register(Arc::new(ToolA)).await;
        registry.register(Arc::new(ToolM)).await;

        let defs = registry.tool_definitions().await;
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    #[tokio::test]
    async fn test_retain_only_filters_tools() {
        let registry = ToolRegistry::new();
        registry.register_builtin_tools();
        let all = registry.list().await;
        assert!(all.len() > 2, "expected multiple built-in tools");
        registry.retain_only(&["echo", "time"]).await;
        let remaining = registry.list().await;
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&"echo".to_string()));
        assert!(remaining.contains(&"time".to_string()));
    }

    #[tokio::test]
    async fn test_retain_only_empty_is_noop() {
        let registry = ToolRegistry::new();
        registry.register_builtin_tools();
        let before = registry.list().await.len();
        registry.retain_only(&[]).await;
        let after = registry.list().await.len();
        assert_eq!(before, after);
    }

    // ─── PR #3: bootstrap_tools() behaviour tests (ADR-112 §5.4.1 B1) ────
    //
    // Each test exercises one observable contract of `bootstrap_tools`. The
    // shared invariant is that the legacy 12 register_*_tools paths do not
    // re-fire — bootstrap_tools is the single entry point under test.

    use crate::tools::bootstrap::{BootstrapContext, BootstrapMode};

    #[tokio::test]
    async fn req_p02_pr3_a_orchestrator_with_local_tools_includes_dev() {
        // Orchestrator { allow_local_tools: true } must register dev tools
        // (shell / read_file / write_file). This is the local-development
        // path equivalent to the legacy `register_dev_tools` call site.
        let registry = Arc::new(ToolRegistry::new());
        let ctx = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: true,
            },
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx).await.unwrap();

        assert!(registry.has("echo").await, "builtin tools must register");
        assert!(
            registry.has("shell").await,
            "dev tools (shell) must register when allow_local_tools=true"
        );
        assert!(
            registry.has("read_file").await,
            "dev tools (read_file) must register when allow_local_tools=true"
        );
    }

    #[tokio::test]
    async fn req_p02_pr3_b_orchestrator_default_skips_dev_tools() {
        // Orchestrator { allow_local_tools: false } — the production main
        // process path — must NOT register dev tools. Container-domain tools
        // belong inside the sandboxed worker, not the orchestrator.
        let registry = Arc::new(ToolRegistry::new());
        let ctx = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx).await.unwrap();

        assert!(registry.has("echo").await, "builtin tools must register");
        assert!(
            !registry.has("shell").await,
            "shell must NOT register on default orchestrator"
        );
        assert!(
            !registry.has("read_file").await,
            "read_file must NOT register on default orchestrator"
        );
    }

    #[tokio::test]
    async fn req_p02_pr3_c_field_gates_secrets_tool_group() {
        // Field-to-group mapping: secrets_store=None → no secret tools;
        // Some → secret_list / secret_delete present. This shape repeats
        // for every other field-gated group (extension / skill / routine /
        // image / vision); secrets is the simplest to wire because
        // InMemorySecretsStore needs no async setup.
        use crate::testing::credentials::test_secrets_store;
        use dasclaw_runtime::secrets::SecretsStore;

        // Without secrets_store: secret tools must be absent.
        let registry_a = Arc::new(ToolRegistry::new());
        let ctx_a = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            ..Default::default()
        };
        registry_a.bootstrap_tools(&ctx_a).await.unwrap();
        assert!(!registry_a.has("secret_list").await);

        // With secrets_store: secret_list and secret_delete must register.
        let store: Arc<dyn SecretsStore + Send + Sync> = Arc::new(test_secrets_store());
        let registry_b = Arc::new(ToolRegistry::new());
        let ctx_b = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            secrets_store: Some(store),
            ..Default::default()
        };
        registry_b.bootstrap_tools(&ctx_b).await.unwrap();
        assert!(
            registry_b.has("secret_list").await,
            "secret_list must register when secrets_store is provided"
        );
        assert!(
            registry_b.has("secret_delete").await,
            "secret_delete must register when secrets_store is provided"
        );
    }

    #[tokio::test]
    async fn req_p02_pr3_d_incremental_calls_accumulate_groups() {
        // Multi-stage init contract: production callers (app.rs / main.rs /
        // agent_loop.rs / worker/container.rs) reach `bootstrap_tools` at
        // different points as their dependencies materialize. Subsequent
        // calls must accumulate groups rather than fail — the underlying
        // `register_sync` is HashMap::insert-style, so identical inputs
        // are naturally idempotent and new fields just add new tools.
        use crate::testing::credentials::test_secrets_store;
        use dasclaw_runtime::secrets::SecretsStore;

        let registry = Arc::new(ToolRegistry::new());

        // Stage 1: orchestrator boot — builtins only, no production deps.
        let ctx_stage_1 = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx_stage_1).await.unwrap();
        assert!(registry.has("echo").await);
        assert!(!registry.has("secret_list").await);

        // Stage 2: secrets store materialized later (e.g. after keychain unlock).
        let store: Arc<dyn SecretsStore + Send + Sync> = Arc::new(test_secrets_store());
        let ctx_stage_2 = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            secrets_store: Some(store),
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx_stage_2).await.unwrap();
        assert!(
            registry.has("echo").await,
            "stage-1 builtins must survive stage-2 application"
        );
        assert!(
            registry.has("secret_list").await,
            "stage-2 must add secrets group on top of stage-1"
        );
    }

    #[tokio::test]
    async fn req_p02_pr3_e_test_mode_registers_only_builtins() {
        // Test mode is the minimum-viable setup used by `BootstrapContext::for_test()`.
        // It must register the four core built-ins (echo / time / json / http)
        // and must NOT register any dev or production-gated tool group, even
        // if other fields are accidentally set on the context.
        let registry = Arc::new(ToolRegistry::new());
        let ctx = BootstrapContext::for_test();
        registry.bootstrap_tools(&ctx).await.unwrap();

        for builtin in ["echo", "time", "json", "http"] {
            assert!(
                registry.has(builtin).await,
                "Test mode must register builtin '{builtin}'"
            );
        }
        for excluded in ["shell", "read_file", "secret_list", "memory_read"] {
            assert!(
                !registry.has(excluded).await,
                "Test mode must NOT register '{excluded}'"
            );
        }
    }

    // ─── PR #4: caller migration / JobToolsConfig dispatch ────────────────
    //
    // PR #4 deletes the public `register_*_tools` wrappers and routes every
    // call site through `bootstrap_tools(&BootstrapContext{..})`. These tests
    // pin the *new* invariants introduced by PR #4 — primarily the
    // `JobToolsConfig` field-gate dispatch — so future regressions surface as
    // a focused test failure rather than a generic build break.

    use crate::tools::bootstrap::JobToolsConfig;
    use dasclaw_runtime::context::ContextManager;

    #[tokio::test]
    async fn req_p02_pr4_a_job_config_minimum_registers_core_job_tools() {
        // With only the required `context_manager`, the four core job
        // management tools (create_job / list_jobs / job_status / cancel_job)
        // must register. Optional dependencies (sandbox, secrets, prompt
        // queue) are absent → optional tools (job_events / job_prompt) stay
        // unregistered.
        let registry = Arc::new(ToolRegistry::new());
        let ctx_mgr = Arc::new(ContextManager::new(8));
        let ctx = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            job_config: Some(JobToolsConfig::new(Arc::clone(&ctx_mgr))),
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx).await.unwrap();

        for core in ["create_job", "list_jobs", "job_status", "cancel_job"] {
            assert!(
                registry.has(core).await,
                "core job tool '{core}' must register when JobToolsConfig present"
            );
        }
        // job_events requires `store`; job_prompt requires `prompt_queue`.
        assert!(
            !registry.has("job_events").await,
            "job_events must not register without `store`"
        );
        assert!(
            !registry.has("job_prompt").await,
            "job_prompt must not register without `prompt_queue`"
        );
    }

    #[tokio::test]
    async fn req_p02_pr4_b_job_config_absent_skips_job_tools() {
        // Field-gate parity: `job_config = None` must skip the entire job
        // group, even when other production-shaped fields are filled in.
        let registry = Arc::new(ToolRegistry::new());
        let ctx = BootstrapContext {
            mode: BootstrapMode::Orchestrator {
                allow_local_tools: false,
            },
            job_config: None,
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx).await.unwrap();

        for absent in ["create_job", "list_jobs", "job_status", "cancel_job"] {
            assert!(
                !registry.has(absent).await,
                "'{absent}' must NOT register without JobToolsConfig"
            );
        }
    }

    #[tokio::test]
    async fn req_p02_pr4_c_test_mode_short_circuits_before_job_dispatch() {
        // Regression guard: `BootstrapMode::Test` must short-circuit *before*
        // any field-gated dispatch — even if a fully-formed JobToolsConfig is
        // attached, no job tools may register. This pins the invariant that
        // `req_p02_pr3_e` already guarantees, but specifically against the
        // PR #4 job-dispatch path that did not exist when `_e` was authored.
        let registry = Arc::new(ToolRegistry::new());
        let ctx_mgr = Arc::new(ContextManager::new(8));
        let ctx = BootstrapContext {
            mode: BootstrapMode::Test,
            job_config: Some(JobToolsConfig::new(ctx_mgr)),
            ..Default::default()
        };
        registry.bootstrap_tools(&ctx).await.unwrap();

        assert!(registry.has("echo").await, "Test mode keeps builtins");
        assert!(
            !registry.has("create_job").await,
            "Test mode must skip job dispatch even with a populated JobToolsConfig"
        );
    }

    #[tokio::test]
    async fn req_p02_pr4_d_multi_stage_accumulates_job_then_secrets() {
        // Production multi-stage init pattern (mirrors `app.rs` reaching
        // `bootstrap_tools` twice — once after `init_tools`, once after a
        // later phase wires more deps). Stage-1 wires the job group;
        // stage-2 attaches a secrets store on top. Both groups must coexist
        // after the second call without re-running the legacy register paths.
        use crate::testing::credentials::test_secrets_store;
        use dasclaw_runtime::secrets::SecretsStore;

        let registry = Arc::new(ToolRegistry::new());
        let ctx_mgr = Arc::new(ContextManager::new(8));

        // Stage 1: job tools only.
        registry
            .bootstrap_tools(&BootstrapContext {
                mode: BootstrapMode::Orchestrator {
                    allow_local_tools: false,
                },
                job_config: Some(JobToolsConfig::new(Arc::clone(&ctx_mgr))),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(registry.has("create_job").await);
        assert!(!registry.has("secret_list").await);

        // Stage 2: secrets store materializes; job group must survive.
        let store: Arc<dyn SecretsStore + Send + Sync> = Arc::new(test_secrets_store());
        registry
            .bootstrap_tools(&BootstrapContext {
                mode: BootstrapMode::Orchestrator {
                    allow_local_tools: false,
                },
                secrets_store: Some(store),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(
            registry.has("create_job").await,
            "stage-1 job tools must survive stage-2 application"
        );
        assert!(
            registry.has("secret_list").await,
            "stage-2 secrets tools must register"
        );
    }

    #[tokio::test]
    async fn req_p02_pr4_e_job_config_new_carries_only_required_field() {
        // Constructor contract: `JobToolsConfig::new(ctx)` produces a config
        // whose required field is set and every optional field defaults to
        // `None`. Asserts the invariant separately from the bootstrap path
        // so a future field addition that breaks the default cannot pass
        // unnoticed.
        let ctx_mgr = Arc::new(ContextManager::new(8));
        let cfg = JobToolsConfig::new(Arc::clone(&ctx_mgr));

        assert!(Arc::ptr_eq(&cfg.context_manager, &ctx_mgr));
        assert!(cfg.scheduler_slot.is_none());
        assert!(cfg.job_manager.is_none());
        assert!(cfg.store.is_none());
        assert!(cfg.job_event_tx.is_none());
        assert!(cfg.inject_tx.is_none());
        assert!(cfg.prompt_queue.is_none());
        assert!(cfg.secrets_store.is_none());
    }

    // ── issue #88: tool collision and visibility startup report ─────────

    #[tokio::test]
    async fn req_88_registration_report_captures_builtin_via_register_sync() {
        let registry = Arc::new(ToolRegistry::new());
        registry
            .bootstrap_tools(&BootstrapContext::for_test())
            .await
            .unwrap();

        let report = registry.registration_report(None).await;

        // Test mode registers exactly the four base built-ins.
        let builtins = report.accepted.get("builtin").expect("builtin bucket");
        for name in ["echo", "time", "json", "http"] {
            assert!(
                builtins.contains(&name.to_string()),
                "missing built-in {name} in {builtins:?}"
            );
        }
        assert!(
            report.rejected.is_empty(),
            "no rejections expected in test mode"
        );
        assert!(report.policy_disabled.is_empty());

        // Builtin bucket is sorted (deterministic ordering for fixtures).
        let mut sorted = builtins.clone();
        sorted.sort();
        assert_eq!(builtins, &sorted);
    }

    #[tokio::test]
    async fn req_88_registration_report_records_protected_shadow_rejection() {
        // A dynamic tool that tries to register under a protected built-in
        // name must be rejected and recorded with the deterministic reason
        // `protected_builtin_shadow` — never silently dropped.
        struct FakeShell;
        #[async_trait::async_trait]
        impl Tool for FakeShell {
            fn name(&self) -> &str {
                "shell"
            }
            fn description(&self) -> &str {
                "fake"
            }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(
                &self,
                _params: serde_json::Value,
                _ctx: &mut dyn dasclaw_runtime::JobContextCore,
            ) -> Result<crate::tools::tool::ToolOutput, crate::tools::tool::ToolError> {
                unreachable!("rejected before registration")
            }
        }

        let registry = Arc::new(ToolRegistry::new());
        registry
            .bootstrap_tools(&BootstrapContext {
                mode: BootstrapMode::Container,
                ..Default::default()
            })
            .await
            .unwrap();

        registry.register(Arc::new(FakeShell)).await;

        let report = registry.registration_report(None).await;
        assert_eq!(report.rejected.len(), 1, "one rejection expected");
        let r = &report.rejected[0];
        assert_eq!(r.name, "shell");
        assert_eq!(r.source, super::ToolSource::Dynamic);
        assert_eq!(r.reason.as_str(), "protected_builtin_shadow");

        // The legitimate built-in `shell` is still registered under the
        // builtin bucket — the dynamic shadow attempt did not displace it.
        let builtin_shell_present = report
            .accepted
            .get("builtin")
            .is_some_and(|v| v.contains(&"shell".to_string()));
        assert!(builtin_shell_present, "built-in shell must remain accepted");
    }

    #[tokio::test]
    async fn req_88_registration_report_policy_disabled_view() {
        use crate::tools::feature_flags::ToolFeatureFlags;

        let registry = Arc::new(ToolRegistry::new());
        registry
            .bootstrap_tools(&BootstrapContext::for_test())
            .await
            .unwrap();

        let flags = ToolFeatureFlags::with_disabled(vec!["http".into(), "echo".into()]);
        let report = registry.registration_report(Some(&flags)).await;

        assert_eq!(
            report.policy_disabled,
            vec!["echo".to_string(), "http".to_string()],
            "policy_disabled must be sorted and dedup'd"
        );
    }

    #[tokio::test]
    async fn req_88_registration_report_does_not_leak_tool_state() {
        // Defence-in-depth: the report only carries names + source labels +
        // rejection reason kinds. Snapshot it as JSON and assert the JSON
        // body never contains parameter/schema/secret terminology.
        let registry = Arc::new(ToolRegistry::new());
        registry
            .bootstrap_tools(&BootstrapContext::for_test())
            .await
            .unwrap();
        let report = registry.registration_report(None).await;
        let json = serde_json::to_string(&report).unwrap();

        for forbidden in [
            "parameters_schema",
            "api_key",
            "secret",
            "password",
            "token",
        ] {
            assert!(
                !json.contains(forbidden),
                "registration report leaked '{forbidden}' in JSON: {json}"
            );
        }
    }

    // ── issue #87: dynamic tool namespace + canonical-name collisions ───

    /// Minimal test double to drive `register()` without standing up a full
    /// MCP/WASM stack. Only `name()` is meaningful for namespace tests.
    struct StubTool {
        name: String,
    }

    #[async_trait::async_trait]
    impl Tool for StubTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &mut dyn dasclaw_runtime::JobContextCore,
        ) -> Result<crate::tools::tool::ToolOutput, crate::tools::tool::ToolError> {
            unreachable!("not invoked in namespace tests")
        }
    }

    fn stub(name: &str) -> Arc<dyn Tool> {
        Arc::new(StubTool {
            name: name.to_string(),
        })
    }

    #[tokio::test]
    async fn req_87_canonical_source_buckets_classified_in_report() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(stub("mcp.github.create_issue")).await;
        registry.register(stub("wasm.acme.lint")).await;
        registry.register(stub("ext.acme-suite.formatter")).await;
        registry.register(stub("legacy_no_prefix")).await;

        let report = registry.registration_report(None).await;

        assert_eq!(
            report.accepted.get("mcp").map(|v| v.as_slice()),
            Some(["mcp.github.create_issue".to_string()].as_slice()),
            "mcp.* must classify into the `mcp` bucket"
        );
        assert_eq!(
            report.accepted.get("wasm").map(|v| v.as_slice()),
            Some(["wasm.acme.lint".to_string()].as_slice()),
            "wasm.* must classify into the `wasm` bucket"
        );
        assert_eq!(
            report.accepted.get("extension").map(|v| v.as_slice()),
            Some(["ext.acme-suite.formatter".to_string()].as_slice()),
            "ext.* must classify into the `extension` bucket"
        );
        assert_eq!(
            report.accepted.get("dynamic").map(|v| v.as_slice()),
            Some(["legacy_no_prefix".to_string()].as_slice()),
            "names without a reserved prefix stay in the legacy `dynamic` bucket"
        );
    }

    #[tokio::test]
    async fn req_87_dynamic_mcp_collision_rejected_first_writer_wins() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(stub("mcp.github.create_issue")).await;
        // Second registration under the same canonical name — must be
        // dropped, not silently overwrite the first.
        registry.register(stub("mcp.github.create_issue")).await;

        let report = registry.registration_report(None).await;

        assert_eq!(
            report.accepted.get("mcp").map(|v| v.len()).unwrap_or(0),
            1,
            "only the first MCP registration may live in the registry"
        );
        assert_eq!(report.rejected.len(), 1, "second attempt must be rejected");
        let r = &report.rejected[0];
        assert_eq!(r.name, "mcp.github.create_issue");
        assert_eq!(r.source, super::ToolSource::Mcp);
        assert_eq!(r.reason, super::RejectionReason::CanonicalNameCollision);
        assert_eq!(r.reason.as_str(), "canonical_name_collision");
    }

    #[tokio::test]
    async fn req_87_dynamic_wasm_vs_wasm_collision_rejected() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(stub("wasm.acme.lint")).await;
        registry.register(stub("wasm.acme.lint")).await;

        let report = registry.registration_report(None).await;

        assert_eq!(report.rejected.len(), 1);
        assert_eq!(report.rejected[0].source, super::ToolSource::Wasm);
        assert_eq!(
            report.rejected[0].reason,
            super::RejectionReason::CanonicalNameCollision
        );
    }

    #[tokio::test]
    async fn req_87_dynamic_legacy_collision_also_rejected() {
        // Legacy (un-prefixed) dynamic registrations must follow the same
        // first-writer-wins rule; otherwise canonical-name uniqueness is
        // not actually a registry invariant.
        let registry = Arc::new(ToolRegistry::new());
        registry.register(stub("legacy_tool")).await;
        registry.register(stub("legacy_tool")).await;

        let report = registry.registration_report(None).await;

        assert_eq!(report.rejected.len(), 1);
        assert_eq!(report.rejected[0].source, super::ToolSource::Dynamic);
        assert_eq!(
            report.rejected[0].reason,
            super::RejectionReason::CanonicalNameCollision
        );
    }

    #[tokio::test]
    async fn req_87_cross_namespace_names_do_not_collide() {
        // `mcp.foo.bar` and `wasm.foo.bar` are distinct canonical names; both
        // must register successfully and surface in their own buckets.
        let registry = Arc::new(ToolRegistry::new());
        registry.register(stub("mcp.foo.bar")).await;
        registry.register(stub("wasm.foo.bar")).await;

        let report = registry.registration_report(None).await;

        assert!(report.rejected.is_empty(), "no collision across namespaces");
        assert_eq!(report.accepted_count(), 2);
        assert!(
            report
                .accepted
                .get("mcp")
                .is_some_and(|v| v.contains(&"mcp.foo.bar".to_string()))
        );
        assert!(
            report
                .accepted
                .get("wasm")
                .is_some_and(|v| v.contains(&"wasm.foo.bar".to_string()))
        );
    }

    #[tokio::test]
    async fn req_87_protected_shadow_takes_precedence_over_collision_check() {
        // If a dynamic tool tries to register under a built-in's bare name,
        // the rejection reason must be `protected_builtin_shadow`, not the
        // generic `canonical_name_collision`. Audit/policy needs the more
        // specific signal to flag intentional shadow attempts.
        let registry = Arc::new(ToolRegistry::new());
        registry
            .bootstrap_tools(&BootstrapContext::for_test())
            .await
            .unwrap();
        registry.register(stub("echo")).await;

        let report = registry.registration_report(None).await;
        assert_eq!(report.rejected.len(), 1);
        assert_eq!(
            report.rejected[0].reason,
            super::RejectionReason::ProtectedBuiltinShadow,
            "protected-shadow must win over generic collision"
        );
    }
}
