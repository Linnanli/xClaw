//! Tool Visibility Triple Gate — `ToolVisibilityPolicy` trait + closed-enum decisions.
//!
//! Implements ADR-149 v2.0 (Accepted 2026-05-13). Provides the seam through
//! which `dasclaw_agent` and `ironclaw` enforce a single source of truth for
//! tool exposure across:
//!
//! - **L2 — `LlmDefinitions`**: which tools are advertised to the model in
//!   the `tools: [...]` parameter of the LLM request.
//! - **L3 — `ExecutorPreflight`**: whether a specific tool invocation may
//!   actually execute (post-LLM, pre-handler dispatch).
//!
//! `PromptCatalog` is retained as a [`ToolGateLayer`] variant for forward
//! compatibility; today no upstream reference (codex / claude-code /
//! ironclaw-main) embeds the tool catalog into the system prompt — all three
//! pass tools through the LLM API's dedicated `tools` parameter. See
//! ADR-149 §1.1 addendum.
//!
//! # Fail-Closed by Construction
//!
//! - [`ToolGateDecision`] has **no `Default` impl** — callers must produce a
//!   decision explicitly. The only constructor that names "deny" is
//!   [`ToolGateDecision::deny_args`].
//! - There is no `Option<Arc<dyn ToolVisibilityPolicy>>` API surface here;
//!   host crates are required by ADR-149 §2.5 to demand a policy at
//!   construction time (Always-Has-Policy invariant).
//! - Source-aware default (ADR-149 §2.6): policies are expected to return
//!   [`ToolGateDecision::Hide`] for non-`BuiltIn` sources until #484 ships
//!   integrity verifiers for MCP / Skill / Extension / Wasm tools.
//!
//! # Identity
//!
//! [`ActorId`] / [`TenantId`] are local newtypes pending `dasclaw_identity`
//! W2 maturation. Once `dasclaw_identity` ships `ActorRef` / `TenantRef`
//! per ADR-115, replace these types here (single point of change). The
//! `TODO(identity-W2)` markers below trace the migration.

#[cfg(feature = "tool_visibility")]
use async_trait::async_trait;
use std::fmt;
#[cfg(feature = "tool_visibility")]
use std::sync::Arc;

// ---- Identity newtypes (TODO(identity-W2): replace with dasclaw_identity) ---

/// Stable identifier for the principal initiating a tool call.
///
/// Temporary newtype pending `dasclaw_identity::ActorRef` (W2). Carrying a
/// `String` (vs. `&str`) keeps the type owned & cheap to clone for audit
/// records.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ActorId(pub String);

impl ActorId {
    /// Construct from any `Into<String>`. Empty strings are accepted —
    /// callers are responsible for upstream validation.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrowed view for cheap matching / logging.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable identifier for the tenant under which the tool call is evaluated.
///
/// Temporary newtype pending `dasclaw_identity::TenantRef` (W2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---- Source / Layer / Env enums --------------------------------------------

/// Where a tool came from. Drives the source-aware default per ADR-149 §2.6:
/// policies should treat non-`BuiltIn` variants as opaque until #484 lands
/// integrity verifiers for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "tool_visibility", serde(rename_all = "snake_case"))]
pub enum ToolSource {
    /// First-party tool compiled into ironclaw / dasclaw_agent.
    BuiltIn,
    /// MCP-protocol tool from an external server.
    Mcp,
    /// Tool exposed via a Skill bundle.
    Skill,
    /// Extension-provided tool (host plugin).
    Extension,
    /// WASM sandbox-loaded tool.
    Wasm,
}

/// Which gate layer the decision is being requested from. Used for audit
/// metadata; policies may also branch on this if e.g. catalog-time hiding
/// differs from executor-time argument inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "tool_visibility", serde(rename_all = "snake_case"))]
pub enum ToolGateLayer {
    /// Reserved for future system-prompt embedding. See module-level
    /// addendum: no upstream today populates this layer.
    PromptCatalog,
    /// L2 — when computing the LLM `tools: [...]` parameter.
    LlmDefinitions,
    /// L3 — immediately before the tool handler executes.
    ExecutorPreflight,
}

/// Execution environment for the call. Mirrors the three contexts called
/// out in ADR-149 §2.4 (and ironclaw-main `ApprovalContext::Autonomous`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "tool_visibility", serde(rename_all = "snake_case"))]
pub enum Env {
    /// User is at the keyboard; interactive approvals are reachable.
    Interactive,
    /// Background job / routine; no interactive user to prompt.
    Autonomous,
    /// Sandboxed container worker; treat as Autonomous for approval reach
    /// but distinct for audit.
    Container,
}

// ---- Approval scope --------------------------------------------------------

/// How widely an approval, once granted, should be remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "tool_visibility", serde(rename_all = "snake_case"))]
pub enum ApprovalScope {
    /// Approval applies only to the single invocation being evaluated.
    OneTime,
    /// Approval is cached for the remainder of the current session.
    Session,
    /// Approval is persisted across sessions (subject to host storage).
    Persistent,
}

/// Details accompanying a [`ToolGateDecision::RequireApproval`] outcome.
///
/// `allow_always` is host-advisory: it indicates the policy considers it
/// safe to surface an "always allow for this scope" UI option. The host
/// MAY ignore this hint (e.g. an `ApprovalRequirement::Always`-equivalent
/// hard floor at the consumer side overrides it).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ApprovalSpec {
    /// Whether the host MAY offer the user an "always allow" option.
    pub allow_always: bool,
    /// Persistence scope hint for the approval cache.
    pub scope: ApprovalScope,
    /// Human-readable rationale, surfaced in the approval UI.
    pub reason: String,
}

impl ApprovalSpec {
    /// One-time approval with no "always" upgrade option.
    pub fn one_time(reason: impl Into<String>) -> Self {
        Self {
            allow_always: false,
            scope: ApprovalScope::OneTime,
            reason: reason.into(),
        }
    }

    /// Approval that may be cached for the session; `allow_always: true`
    /// signals the host MAY render a "remember for this session" button.
    pub fn session(reason: impl Into<String>) -> Self {
        Self {
            allow_always: true,
            scope: ApprovalScope::Session,
            reason: reason.into(),
        }
    }
}

// ---- Decision enum ---------------------------------------------------------

/// Closed-enum outcome of a [`ToolVisibilityPolicy::check`] call.
///
/// **No `Default` impl** — callers must construct a decision explicitly.
/// This is the fail-closed-by-construction guarantee called out in
/// ADR-149 §1.3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "tool_visibility",
    serde(tag = "kind", rename_all = "snake_case")
)]
pub enum ToolGateDecision {
    /// Tool is fully permitted.
    Allow,
    /// Tool exists but is omitted from the layer the check originated at.
    ///
    /// - At [`ToolGateLayer::LlmDefinitions`]: do not advertise to the model.
    /// - At [`ToolGateLayer::ExecutorPreflight`]: treat as if the tool were
    ///   not registered (caller is responsible for surfacing an explanatory
    ///   error to the model). Note ADR-149 §2.1 addendum: re-hiding a tool
    ///   mid-session that was previously advertised can invalidate the LLM
    ///   prompt cache; prefer [`ToolGateDecision::DenyArgs`] on subsequent
    ///   turns.
    Hide { reason: String },
    /// Tool is allowed in principle, but this specific invocation is
    /// rejected outright (cannot be overridden downstream).
    DenyArgs { reason: String },
    /// Tool requires an out-of-band approval before it may execute.
    RequireApproval(ApprovalSpec),
}

impl ToolGateDecision {
    /// Constructor for `Hide` with a descriptive reason.
    pub fn hide(reason: impl Into<String>) -> Self {
        Self::Hide {
            reason: reason.into(),
        }
    }

    /// Constructor for `DenyArgs` with a descriptive reason.
    pub fn deny_args(reason: impl Into<String>) -> Self {
        Self::DenyArgs {
            reason: reason.into(),
        }
    }

    /// Returns `true` iff the decision permits the tool to be visible at
    /// the requested layer (`Allow` and `RequireApproval` both qualify).
    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Allow | Self::RequireApproval(_))
    }

    /// Returns `true` iff the decision permits execution without further
    /// approval (only `Allow`).
    pub fn permits_execution(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

// ---- Audit metadata --------------------------------------------------------

/// Stable opaque identifier for a single decision event. Host-supplied
/// (e.g. ULID, UUIDv7); the trait surface only requires that it round-trip
/// through telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct DecisionId(pub String);

impl DecisionId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Audit envelope for one policy evaluation. Emitted alongside every
/// [`ToolGateDecision`] so that downstream telemetry can reconstruct the
/// full decision context without re-querying the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "tool_visibility",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct AuditMetadata {
    /// Identifier of the policy that produced the decision (matches
    /// [`ToolVisibilityPolicy::name`]).
    pub policy_name: String,
    /// Which gate layer asked.
    pub layer: ToolGateLayer,
    /// Stable id for this evaluation; surfaces in logs / traces.
    pub decision_id: DecisionId,
    /// Origin of the tool being evaluated.
    pub source: ToolSource,
    /// TODO(identity-W2): replace with `dasclaw_identity::ActorRef`.
    pub actor: ActorId,
    /// TODO(identity-W2): replace with `dasclaw_identity::TenantRef`.
    pub tenant: TenantId,
}

// ---- Context ---------------------------------------------------------------

/// Inputs to a single policy evaluation.
///
/// Borrowed for zero-clone hot-path use. Lifetime `'a` ties the context to
/// the caller's stack frame; the trait method is `async` but the future
/// returned by the implementation must not outlive `'a` (enforced by the
/// `async_trait` desugaring).
#[derive(Debug)]
pub struct ToolGateContext<'a> {
    /// Tool name as registered (no namespacing prefix stripping).
    pub tool_name: &'a str,
    /// Origin of the tool being evaluated.
    pub source: ToolSource,
    /// Which layer this evaluation is for.
    pub layer: ToolGateLayer,
    /// Principal initiating the call.
    pub actor: &'a ActorId,
    /// Tenant scope.
    pub tenant: &'a TenantId,
    /// Tool arguments — `Some` only when `layer == ExecutorPreflight`.
    /// `None` at L2 because arguments are not yet known.
    #[cfg(feature = "tool_visibility")]
    pub args: Option<&'a serde_json::Value>,
    /// Execution environment.
    pub env: Env,
}

// ---- Trait -----------------------------------------------------------------

/// The single seam through which all tool visibility decisions flow.
///
/// **Always-Has-Policy invariant** (ADR-149 §2.5): host constructors that
/// expose tools to a model MUST accept `Arc<dyn ToolVisibilityPolicy>`
/// by value — never `Option<Arc<…>>`. This is enforced at the consumer
/// side; this crate provides only the trait surface.
///
/// `Debug` is required so [`AuditMetadata`] can carry policy identity
/// without forcing `Box<dyn Any>` gymnastics in audit code.
#[cfg(feature = "tool_visibility")]
#[async_trait]
pub trait ToolVisibilityPolicy: Send + Sync + std::fmt::Debug {
    /// Stable identifier for this policy (surfaces in audit logs and
    /// [`AuditMetadata::policy_name`]).
    fn name(&self) -> &str;

    /// Evaluate the gate for one tool in one layer.
    ///
    /// Implementations MUST NOT panic; on internal error they should
    /// return a deny-shaped decision (e.g.
    /// `ToolGateDecision::DenyArgs { reason: "policy backend unavailable" }`).
    async fn check(&self, ctx: &ToolGateContext<'_>) -> ToolGateDecision;

    /// When `true`, the host SHOULD re-evaluate this tool at
    /// [`ToolGateLayer::ExecutorPreflight`] even if the same policy
    /// returned `Allow` at L2. Mirrors claude-code's `requireCanUseTool`:
    /// useful for tools whose argument shape must be re-inspected after
    /// the LLM picks them.
    fn always_check_at_executor(&self, _source: ToolSource) -> bool {
        false
    }
}

/// Convenience alias for the shared trait object pointer hosts pass around.
#[cfg(feature = "tool_visibility")]
pub type SharedToolVisibilityPolicy = Arc<dyn ToolVisibilityPolicy>;

// ---- AllowAllPolicy -------------------------------------------------------

/// The canonical "default permissive" policy.
///
/// Exists solely to satisfy the **Always-Has-Policy invariant** (ADR-149
/// §2.5): callers that have no real policy yet (early bootstrap, test
/// harnesses, scaffolded code paths still wired against legacy filters)
/// can pass `Arc::new(AllowAllPolicy)` rather than reaching for an
/// `Option`. This makes the absence of policy *explicit* in source —
/// `grep AllowAllPolicy` will surface every site that still needs to be
/// upgraded to a real policy.
///
/// Returns [`ToolGateDecision::Allow`] for every input. Carries no state.
#[cfg(feature = "tool_visibility")]
#[derive(Debug, Default, Clone, Copy)]
pub struct AllowAllPolicy;

#[cfg(feature = "tool_visibility")]
#[async_trait]
impl ToolVisibilityPolicy for AllowAllPolicy {
    fn name(&self) -> &str {
        "dasclaw_governance::AllowAllPolicy"
    }

    async fn check(&self, _ctx: &ToolGateContext<'_>) -> ToolGateDecision {
        ToolGateDecision::Allow
    }
}

// ---- ToolGateContextSeed --------------------------------------------------

/// Caller-supplied half of a [`ToolGateContext`].
///
/// The registry / executor fills in the per-tool fields (`tool_name`,
/// `source`, `layer`, and at L3 `args`); the seed carries the *call-site*
/// fields that only the host knows: who is calling, in which tenant, and
/// in what execution environment.
///
/// Owned (not borrowed) for ergonomics — the cost of two short
/// [`String`]s is negligible compared to one LLM round-trip, and an
/// owned struct can be stored on a worker / dispatcher long-term.
#[cfg(feature = "tool_visibility")]
#[derive(Debug, Clone)]
pub struct ToolGateContextSeed {
    /// Principal whose intent triggered this call.
    pub actor: ActorId,
    /// Tenant scope (multi-tenancy isolation point).
    pub tenant: TenantId,
    /// Execution environment for downstream policy reasoning.
    pub env: Env,
}

#[cfg(feature = "tool_visibility")]
impl ToolGateContextSeed {
    /// Construct a seed from owned components.
    pub fn new(actor: ActorId, tenant: TenantId, env: Env) -> Self {
        Self { actor, tenant, env }
    }

    /// Placeholder seed for code paths whose actor/tenant plumbing is not
    /// yet wired (W2 dependency). Uses `"system"` / `"default"` literals.
    ///
    /// **TODO(identity-W2):** replace every call site with a seed
    /// constructed from the real session/job/request context once
    /// `dasclaw_identity::ActorRef` ships. `grep "ToolGateContextSeed::system"`
    /// surfaces all such migration sites.
    pub fn system(env: Env) -> Self {
        Self {
            actor: ActorId::new("system"),
            tenant: TenantId::new("default"),
            env,
        }
    }
}
