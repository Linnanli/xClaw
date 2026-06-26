# Dasclaw Agent SDK Priority Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to execute this plan.

**Goal:** Turn the three Dasclaw SDK whitepaper findings into an ordered, verifiable remediation roadmap focused on the agent SDK surface: tool argument safety, LLM egress safety, execution context, policy decisions, run lifecycle control, model/tool API unification, resumable state, tracing/audit, and SDK boundary hygiene.

**Architecture:** Preserve current compatibility APIs while adding safer internal seams first. P0 fixes hard security correctness defects and introduces mandatory SDK envelopes for execution context, policy decisions, and command-tool safety. P1 introduces event/run identity, extracts a `Runner`/`RunHandle` layer, and narrows model/tool extension APIs. P2 converts persistence, tool output, tracing, and app-server integration from product-owned convenience into SDK-owned contracts. P3 finishes public SDK packaging and engineering release hygiene once behavior is safe.

**Tech Stack:** Rust workspace crates `dasclaw_core`, `dasclaw_runtime`, `dasclaw_session`, `dasclaw_app_server`; `cargo nextest`; `cargo fmt`; `python3.12 scripts/check_no_panics.py`; existing code-review-graph semantic search and LSP checks before adding new modules.

---

## Startup 4 Questions

1. **是否新增模块 / crate / 文件？** 是。计划新增候选模块包括 `crates/dasclaw_core/src/execution_context.rs`、`crates/dasclaw_core/src/policy.rs`、`crates/dasclaw_core/src/model.rs`、`crates/dasclaw_core/src/telemetry.rs`、`crates/dasclaw_runtime/src/runner.rs`、`crates/dasclaw_runtime/src/run_context.rs` 与 `crates/dasclaw_runtime/src/tool_registry.rs`，已先用 code-review-graph 语义搜索核对现有等价计划/实现，文件级无已有计划命中，概念级命中分散实现。
2. **结论里是否含“X 没有 Y / X 缺 Y / X 是独家”等否定语？** 是。前置分析已用语义搜索与 `rg` 双证据核对，以下计划只把已核验问题转成修复顺序。
3. **是否做跨项目对账？** 否。本计划只面向当前 `x-claw` 工作区的 Dasclaw agent SDK 整改。
4. **是否写架构对账类文档？** 否。这是实施计划，不新增能力矩阵或跨项目架构对账。

## Evidence Summary

The whitepaper findings are materially real for the agent surface, with a few corrections:

- Tool argument safety is P0: `SequentialDispatcher` currently records and emits raw tool call arguments before approval/egress redaction, and the execution path still uses the original `call`.
- Default safety posture is mixed: sanitizer, egress gate, approval gate, and approval policy have compatibility defaults that are permissive or no-op, while `NoopSandboxExecutor` itself is fail-closed.
- Agent definition and runtime state are mixed: `Agent` owns configuration-like fields and per-run machinery such as signal receiver, cancellation token, approval inbox, and executor.
- Tool APIs are split: `Tool`, `ToolExecutor`, `AgentConfig.tools`, builder tools, and adapters do not share one typed registry with approval/risk metadata as the single execution path.
- DLP is not end-to-end: the core agentic loop scans only the latest user message before the LLM request path clones the full message context.
- SDK calls lack one mandatory `ExecutionContext` and one normalized `PolicyDecision` shape across model, tool, secret, connector, persistence, and audit boundaries.
- `JobContextCore` is still a product-shaped tool extension context; SDK tools need a generic `ToolContext<C>` or `RunContext<C>` so hosts can pass tenant/user/product state without baking those fields into core.
- `LlmProvider` and `AgentResponder` remain too broad for a stable SDK surface; model invocation, model catalog/routing/pricing, and runner lifecycle control need separate contracts.
- Observability exists as hooks and logging primitives, but the SDK lacks a small tracer/audit trait that records run/model/tool/policy metadata without collecting sensitive content by default.
- AppServer currently compensates for missing runtime-level run handles with its own `in_flight`, `active_agents`, pending approval, cancellation, and event bookkeeping.
- AppServer protocol and service construction still include product-host concerns such as RPC DTOs, `real_with_root` composition, FS/Command/Git/Search/Skills surfaces, and desktop protocol types.
- Session persistence exists through `dasclaw_session`, but snapshots are primarily message/model/workspace/compaction state rather than resumable run state.
- Streaming PTY override paths are fail-closed, but plain command streaming still spawns through the default PTY backend without the same sandbox policy surface.

## Agent SDK Coverage Boundary

This plan includes whitepaper items that define the reusable Agent SDK contract:

- runtime safety gates, DLP, policy decisions, and command-tool isolation
- execution context, run context, model/tool API contracts, and event IDs
- `Runner`, `RunHandle`, approval/cancel/steer control, session checkpoint, and tracing/audit metadata
- app-server as a Host and reference implementation that must consume public SDK APIs
- crate metadata, optional features, docs, examples, and CI gates needed to publish the SDK shape

SDK tracing/audit scope is intentionally narrow:

- SDK emits structured run/model/tool/policy/approval/checkpoint events.
- SDK events carry IDs, versions, classifications, decision IDs, and sanitized metadata.
- SDK provides `Tracer` / `AuditSink` traits so hosts can connect their own logging, audit, monitoring, or SIEM systems.
- SDK never stores raw prompts, raw tool arguments, or raw tool outputs by default.

This plan does not implement full政企 product features such as SSO/OIDC/SAML providers, LDAP/AD sync, ACL-aware enterprise RAG, Chinese document parsing, Office/OA/email/calendar connectors, audit-ledger storage, or long-running business workflow engines. It adds SDK extension points that those products and tool packs must use.

SDK tracing/audit does not include audit databases, immutable ledgers, query UIs, reports, dashboards, retention policies, legal hold workflows, tenant-specific compliance operations, or customer-specific governance packs. Those are host/product responsibilities built on top of SDK events.

## File Structure

```
crates/dasclaw_core/src/
  agentic_loop.rs              # AgentEvent, egress request envelope, responder boundary
  egress_apply.rs              # Shared text/message sanitization helpers if existing APIs fit
  execution_context.rs         # SDK-wide tenant/actor/request/purpose envelope
  hooks.rs                     # Hook defaults and fail-closed profile contracts
  model.rs                     # Narrow Model trait plus catalog/router extension traits
  policy.rs                    # PolicyDecision, PolicyEngine, and decision_id types
  telemetry.rs                 # Tracer/AuditSink traits and sanitized event metadata

crates/dasclaw_runtime/src/
  agent.rs                     # AgentBuilder, safety profile, run identity injection
  command_safety.rs            # Command-tool sandbox requirement checks
  tool_dispatch.rs             # SafeToolCall pipeline and dispatch ordering
  tool.rs                      # Legacy Tool trait compatibility boundary
  tool_to_executor_adapter.rs  # Adapter must preserve approval/risk metadata
  composite_executor.rs        # Registry-backed executor composition
  runner.rs                    # New RunSpec, Runner, RunHandle, RunControl
  run_context.rs               # RunContext<C> and ToolContext<'_, C>
  tool_registry.rs             # New typed ToolRegistry and ToolDefinition
  context/state.rs             # Tool output stash classification and redaction metadata
  llm_adapter.rs               # LLM request tests and request envelope verification
  model_adapter.rs             # Compatibility adapter from existing LlmProvider to Model
  lib.rs                       # Public exports for safe runtime API

crates/dasclaw_session/src/
  snapshot.rs                  # Run checkpoint metadata
  store.rs                     # Store trait compatibility
  jsonl.rs                     # JSONL persistence of checkpoint records
  lib.rs                       # Public checkpoint exports

crates/dasclaw_app_server/src/
  lib.rs                       # Consume Runner/RunHandle instead of owning runtime internals
  command_service.rs           # Keep PTY sandbox override behavior explicit
  main.rs                      # Host composition root after services are split out

crates/dasclaw_app_server_protocol/src/
  lib.rs                       # Host protocol only; not re-exported by the SDK facade

docs/superpowers/plans/
  2026-06-26-dasclaw-agent-sdk-priority-remediation.md
```

## P0. Security Correctness

### Task 1: Make Tool Argument Gates Authoritative

**Priority:** P0  
**Risk fixed:** Raw tool arguments can be recorded, emitted, approved, and executed before or despite egress transformation.

- [ ] Add a `SafeToolCall` internal struct in `crates/dasclaw_runtime/src/tool_dispatch.rs`:

```rust
struct SafeToolCall {
    name: String,
    sanitized_arguments: serde_json::Value,
    display_arguments: serde_json::Value,
    original_argument_hash: String,
    approval: Option<ApprovalRequest>,
}
```

- [ ] Change `SequentialDispatcher` so the ordering is:

```text
model tool call
  -> parse arguments
  -> apply argument egress policy
  -> build SafeToolCall
  -> emit ToolCallStart with display_arguments
  -> request approval with display_arguments plus original_argument_hash
  -> execute with sanitized_arguments only
  -> store sanitized tool call in ReasoningContext
```

- [ ] Prevent blocked or unapproved tool calls from entering `ReasoningContext` as executable assistant tool-call history.
- [ ] Update approval payloads so `ApprovalNeeded.tool_arguments` uses sanitized/display arguments and never raw arguments.
- [ ] Add a focused unit test that proves a redacted argument is what the executor receives:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(tool_dispatch_redacts_arguments_before_execution)'
```

Expected result:

```text
PASS tool_dispatch_redacts_arguments_before_execution
```

- [ ] Add a focused unit test that proves blocked arguments do not enter model-visible history:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(blocked_tool_arguments_are_not_recorded_in_reasoning_context)'
```

Expected result:

```text
PASS blocked_tool_arguments_are_not_recorded_in_reasoning_context
```

- [ ] Run the touched-crate compile gate:

```bash
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 2: Apply DLP to the Full LLM Request Envelope

**Priority:** P0  
**Risk fixed:** Only the latest user message is scanned before the LLM adapter sends the full context.

- [ ] Add a request-level egress helper in `crates/dasclaw_core/src/agentic_loop.rs` or reuse `egress_apply.rs` if it already supports structured message traversal.
- [ ] Scan every model-visible message content item before `llm_adapter` receives the request.
- [ ] Preserve role, tool-call, and response-link metadata while redacting only policy-matched text spans.
- [ ] Fail closed when the egress gate blocks any part of the request envelope.
- [ ] Add a regression where an older assistant/tool message contains a secret and the final user message is clean:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(llm_request_egress_scans_full_context)'
```

Expected result:

```text
PASS llm_request_egress_scans_full_context
```

- [ ] Add a regression where request-level DLP blocks the run before provider invocation:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(llm_request_egress_block_prevents_provider_call)'
```

Expected result:

```text
PASS llm_request_egress_block_prevents_provider_call
```

- [ ] Run:

```bash
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 3: Add an Enterprise-Safe Builder Profile

**Priority:** P0  
**Risk fixed:** Compatibility defaults allow SDK consumers to assemble an agent with no-op sanitizer, no-op egress, auto approval, or no approval policy without making that decision explicit.

- [ ] Add an explicit safety mode to `AgentBuilder` in `crates/dasclaw_runtime/src/agent.rs`:

```rust
pub enum AgentSafetyMode {
    Compatibility,
    EnterpriseFailClosed,
}
```

- [ ] Keep `AgentSafetyMode::Compatibility` as the default to avoid breaking existing tests and callers.
- [ ] Add `AgentBuilder::enterprise_fail_closed()` that requires:
  - non-noop sanitizer when tools or external provider calls are enabled
  - non-noop egress gate
  - non-auto approval gate when a tool can mutate state or touch the filesystem/network
  - explicit approval policy
  - observer or audit sink for tool execution events
- [ ] Return a structured build error instead of silently installing permissive defaults in enterprise mode.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(enterprise_mode_rejects_noop_safety_hooks) | test(compatibility_mode_keeps_legacy_defaults)'
```

Expected result:

```text
PASS enterprise_mode_rejects_noop_safety_hooks
PASS compatibility_mode_keeps_legacy_defaults
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 4: Add ExecutionContext and PolicyDecision Contracts

**Priority:** P0  
**Depends on:** Task 1, Task 2, Task 3  
**Risk fixed:** Model calls, tool calls, secrets, session writes, and host adapters do not share one mandatory SDK context or one auditable policy decision shape.

- [ ] Add `crates/dasclaw_core/src/execution_context.rs` with the SDK-owned context envelope:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionContext {
    pub tenant_id: TenantId,
    pub actor_id: SubjectId,
    pub delegated_user_id: Option<SubjectId>,
    pub request_id: RequestId,
    pub trace_id: TraceId,
    pub purpose: ProcessingPurpose,
    pub data_classification: DataClassification,
}
```

- [ ] Keep enterprise-specific identity providers outside `dasclaw_core`; the SDK only owns opaque IDs and purpose/classification enums.
- [ ] Add `crates/dasclaw_core/src/policy.rs` with normalized decisions:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyOutcome {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyDecision {
    pub decision_id: DecisionId,
    pub outcome: PolicyOutcome,
    pub reason: String,
    pub policy_version: Option<String>,
}

pub trait PolicyEngine<C>: Send + Sync {
    fn decide(&self, ctx: &ExecutionContext, input: &PolicyInput<C>) -> PolicyDecision;
}
```

- [ ] Wire `ExecutionContext` into `AgentRunOptions`, `RunSpec`, tool dispatch, LLM request egress, and session checkpoint metadata.
- [ ] Make Task 1 tool approval and Task 2 request egress carry `PolicyDecision.decision_id` in internal audit metadata.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_core -E 'test(execution_context_requires_tenant_actor_and_request) | test(policy_decision_has_stable_decision_id)'
cargo nextest run -p dasclaw_runtime -E 'test(tool_policy_decision_id_reaches_tool_event) | test(llm_egress_decision_id_reaches_run_event)'
```

Expected result:

```text
PASS execution_context_requires_tenant_actor_and_request
PASS policy_decision_has_stable_decision_id
PASS tool_policy_decision_id_reaches_tool_event
PASS llm_egress_decision_id_reaches_run_event
```

- [ ] Run:

```bash
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 5: Enforce a Safe Command-Tool Boundary for Streaming PTY

**Priority:** P0  
**Depends on:** Task 3, Task 4  
**Risk fixed:** Plain streaming PTY can remain available as a host command path without the SDK-level sandbox and policy checks expected for command tools.

- [ ] Add `crates/dasclaw_runtime/src/command_safety.rs` with a small SDK contract for command execution capabilities:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandExecutionMode {
    Disabled,
    BufferedSandboxed,
    StreamingSandboxed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSafetyDecision {
    pub mode: CommandExecutionMode,
    pub policy_decision: PolicyDecision,
}
```

- [ ] Make enterprise-fail-closed mode reject command tools unless the host supplies `StreamingSandboxed` or `BufferedSandboxed` with an allow policy decision.
- [ ] In `crates/dasclaw_app_server/src/command_service.rs`, map plain PTY streaming without sandbox support to `CommandExecutionMode::Disabled` for SDK-routed command tools.
- [ ] Keep existing app-server protocol compatibility for direct host command endpoints, but do not expose that endpoint through the SDK facade.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(enterprise_mode_rejects_unsandboxed_streaming_command_tool)'
cargo nextest run -p dasclaw_app_server -E 'test(streaming_command_without_sandbox_is_not_advertised_as_sdk_tool)'
```

Expected result:

```text
PASS enterprise_mode_rejects_unsandboxed_streaming_command_tool
PASS streaming_command_without_sandbox_is_not_advertised_as_sdk_tool
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_app_server --tests
```

Expected result:

```text
Finished `dev` profile
```

## P1. Runtime Control Plane

### Task 6: Introduce Run and Event Correlation Identity

**Priority:** P1  
**Depends on:** Task 1, Task 2, Task 4  
**Risk fixed:** Agent events cannot be reliably correlated across streaming, approvals, cancellation, tool calls, and persisted session records.

- [ ] Add stable IDs:
  - `RunId`
  - `TurnId`
  - `ToolCallId`
  - monotonically increasing event sequence number
- [ ] Prefer a wrapper that preserves current `AgentEvent` compatibility:

```rust
pub struct RunEvent {
    pub run_id: RunId,
    pub turn_id: TurnId,
    pub sequence: u64,
    pub event: AgentEvent,
}
```

- [ ] Emit `RunEvent` from runtime while app-server can still map to legacy SSE shapes.
- [ ] Add `tool_call_id` to tool start/result/error paths.
- [ ] Update app-server event plumbing to carry correlation IDs into pending approval and cancellation maps.
- [ ] Add ordering tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(run_events_have_monotonic_sequence) | test(tool_result_references_started_tool_call_id)'
```

Expected result:

```text
PASS run_events_have_monotonic_sequence
PASS tool_result_references_started_tool_call_id
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_app_server --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 7: Extract Runner and RunHandle

**Priority:** Deferred  
**Status:** Todo  
**Depends on:** Task 6  
**Risk fixed:** `Agent` mixes reusable definition/configuration with per-run cancellation, steer, approval, and event state; app-server duplicates runtime lifecycle management.

This task is intentionally moved out of the active PR stack. Downstream references to Task 7 remain roadmap placeholders and must be re-baselined before implementation starts.

- [ ] Before creating `runner.rs`, repeat the required semantic check:

```bash
/Users/nallylin/.local/bin/code-review-graph repos
```

Expected result:

```text
x-claw workspace repositories are listed
```

- [ ] Add `crates/dasclaw_runtime/src/runner.rs` with:

```rust
pub struct RunSpec {
    pub prompt: String,
    pub workspace: Option<String>,
    pub model: Option<String>,
}

pub struct Runner {
    agent: Arc<Agent>,
}

pub struct RunHandle {
    pub run_id: RunId,
    pub events: RunEventStream,
    pub control: RunControl,
}
```

- [ ] Move per-run cancellation token, steer signal channel, approval inbox binding, and sequence allocator behind `RunControl`.
- [ ] Keep `Agent::run` as a compatibility adapter that creates a `Runner` internally.
- [ ] Update app-server to store `RunHandle` for `in_flight` work instead of storing raw `Arc<Agent>` plus side maps.
- [ ] Add cancellation and approval tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(run_handle_cancel_stops_only_target_run) | test(run_handle_routes_approval_to_target_tool_call)'
```

Expected result:

```text
PASS run_handle_cancel_stops_only_target_run
PASS run_handle_routes_approval_to_target_tool_call
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_app_server --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 8: Replace Product-Shaped JobContextCore with Generic RunContext

**Priority:** P1  
**Depends on:** Task 4, Task 7  
**Risk fixed:** SDK tool authors currently depend on `JobContextCore`, which carries product/job assumptions instead of a host-defined application context.

- [ ] Add `crates/dasclaw_runtime/src/run_context.rs`:

```rust
pub struct RunContext<C> {
    pub execution: ExecutionContext,
    pub app: C,
    pub services: RuntimeServices,
}

pub struct ToolContext<'a, C> {
    pub run: &'a RunContext<C>,
    pub tool_call_id: ToolCallId,
    pub policy_decision: Option<PolicyDecision>,
}
```

- [ ] Add a typed tool trait for new SDK tools:

```rust
#[async_trait::async_trait]
pub trait SdkTool<C>: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;

    async fn call(
        &self,
        ctx: ToolContext<'_, C>,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError>;
}
```

- [ ] Keep `JobContextCore` as a compatibility adapter for existing tools under the legacy `Tool` path.
- [ ] Make `ToolRegistry` accept both `SdkTool<C>` and legacy `Tool` registrations, but convert both into one internal execution record.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(sdk_tool_receives_host_app_context) | test(legacy_job_context_tool_still_runs_through_adapter)'
```

Expected result:

```text
PASS sdk_tool_receives_host_app_context
PASS legacy_job_context_tool_still_runs_through_adapter
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 9: Unify Tool Registration Behind ToolRegistry

**Priority:** P1  
**Depends on:** Task 1, Task 7, Task 8  
**Risk fixed:** Tool schema, executor implementation, approval metadata, risk level, and adapter paths can diverge.

- [ ] Before creating `tool_registry.rs`, repeat the required semantic check with at least two concept queries:

```bash
/Users/nallylin/.local/bin/code-review-graph status --repo /Users/nallylin/Documents/code/x-claw
```

Expected result:

```text
Graph statistics are printed without database errors
```

- [ ] Add `ToolRegistry` as the single runtime-owned collection for executable tools:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub schema: serde_json::Value,
    pub risk: ToolRisk,
    pub approval: ApprovalRequirement,
    pub executor: Arc<dyn ToolExecutor>,
}

pub struct ToolRegistry {
    definitions: BTreeMap<String, ToolDefinition>,
}
```

- [ ] Make builder `.tool(...)`, `.tools(...)`, and `.tool_executor(...)` populate or wrap the registry.
- [ ] Make `ToolToExecutorAdapter` carry `requires_approval` and `risk_level_for` into the registry instead of dropping that metadata.
- [ ] Keep legacy `Tool` trait available, but execute it through the registry-backed path.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(tool_registry_preserves_approval_metadata_from_legacy_tool) | test(dispatch_uses_registry_schema_and_executor)'
```

Expected result:

```text
PASS tool_registry_preserves_approval_metadata_from_legacy_tool
PASS dispatch_uses_registry_schema_and_executor
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 10: Split Model API from AgentResponder Lifecycle Control

**Priority:** P1  
**Depends on:** Task 7  
**Risk fixed:** The existing `LlmProvider`/`AgentResponder` path blends model invocation, model catalog/routing/pricing, signal checks, loop lifecycle, and provider adaptation into one SDK-facing seam.

- [ ] Add `crates/dasclaw_core/src/model.rs`:

```rust
#[async_trait::async_trait]
pub trait Model: Send + Sync {
    fn id(&self) -> ModelId;
    fn capabilities(&self) -> ModelCapabilities;

    async fn invoke(&self, request: ModelRequest) -> Result<ModelResponse, ModelError>;
    async fn stream(&self, request: ModelRequest) -> Result<ModelStream, ModelError>;
}

pub trait ModelCatalog: Send + Sync {
    fn list(&self) -> Vec<ModelMetadata>;
}
```

- [ ] Move cancellation, steering, approval interruption, and loop iteration control into `Runner` and `RunControl`.
- [ ] Narrow `AgentResponder` usage to a compatibility adapter and stop recommending it as the primary SDK model extension point.
- [ ] Add `crates/dasclaw_runtime/src/model_adapter.rs` to adapt existing `LlmProvider` implementations into the new `Model` trait.
- [ ] Keep pricing, routing, retry, cache, and model switching as separate services or decorators instead of methods on the core model trait.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(llm_provider_adapter_invokes_model_trait) | test(model_trait_does_not_own_run_control_signals)'
```

Expected result:

```text
PASS llm_provider_adapter_invokes_model_trait
PASS model_trait_does_not_own_run_control_signals
```

- [ ] Run:

```bash
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

## P2. Persistence and Data Exposure

### Task 11: Upgrade Session Snapshots to Run Checkpoints

**Priority:** P2  
**Depends on:** Task 6, Task 7  
**Risk fixed:** Existing session snapshots can restore conversation-like state but do not capture enough run metadata for safe resumability or audit replay.

- [ ] Extend `crates/dasclaw_session/src/snapshot.rs` with checkpoint metadata:
  - `run_id`
  - `turn_id`
  - last event sequence
  - model/provider identity
  - agent configuration fingerprint
  - tool registry fingerprint
  - policy fingerprint
  - pending approval IDs
  - sanitized pending tool-call summaries
- [ ] Keep raw tool arguments and raw tool outputs out of durable checkpoint records by default.
- [ ] Add JSONL compatibility tests that old snapshots still load.
- [ ] Add checkpoint round-trip tests:

```bash
cargo nextest run -p dasclaw_session -E 'test(run_checkpoint_round_trips_without_raw_tool_arguments) | test(legacy_snapshot_loads_with_empty_run_checkpoint)'
```

Expected result:

```text
PASS run_checkpoint_round_trips_without_raw_tool_arguments
PASS legacy_snapshot_loads_with_empty_run_checkpoint
```

- [ ] Run:

```bash
cargo check -p dasclaw_session --tests
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 12: Secure Tool Output Stash

**Priority:** P2  
**Depends on:** Task 1, Task 2, Task 4  
**Risk fixed:** Tool output stash can retain full tool output and later expose it without classification, TTL, or sanitized references.

- [ ] Replace raw stash entries in `crates/dasclaw_runtime/src/context/state.rs` with a classified record:

```rust
pub struct ToolOutputRecord {
    pub id: String,
    pub classification: OutputClassification,
    pub sanitized_preview: String,
    pub raw_ref: Option<ProtectedOutputRef>,
    pub expires_at: Option<SystemTime>,
}
```

- [ ] Ensure model-visible context receives `sanitized_preview` or a protected reference, not raw output.
- [ ] Gate raw output retrieval through policy and audit logging.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(tool_output_stash_exposes_sanitized_preview_to_model) | test(raw_tool_output_requires_policy_grant)'
```

Expected result:

```text
PASS tool_output_stash_exposes_sanitized_preview_to_model
PASS raw_tool_output_requires_policy_grant
```

- [ ] Run:

```bash
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 13: Add SDK Tracer and AuditSink Interfaces

**Priority:** P2  
**Depends on:** Task 4, Task 6, Task 9, Task 12  
**Risk fixed:** SDK consumers cannot consistently observe model calls, tool calls, policy decisions, approvals, and sanitized outputs without relying on product logs or ad hoc hooks.

- [ ] Add `crates/dasclaw_core/src/telemetry.rs`:

```rust
#[derive(Clone, Debug)]
pub struct RunAuditEvent {
    pub run_id: RunId,
    pub turn_id: TurnId,
    pub sequence: u64,
    pub trace_id: TraceId,
    pub event_kind: AuditEventKind,
    pub policy_decision_id: Option<DecisionId>,
    pub metadata: BTreeMap<String, String>,
}

pub trait Tracer: Send + Sync {
    fn record(&self, event: RunAuditEvent);
}

pub trait AuditSink: Send + Sync {
    fn append(&self, event: RunAuditEvent) -> Result<(), AuditError>;
}
```

- [ ] Default metadata collection to IDs, versions, classifications, token/cost summaries, and sanitized previews.
- [ ] Keep full prompt, full tool arguments, and full tool output out of `RunAuditEvent` unless a host-specific policy explicitly adds a protected reference.
- [ ] Keep persistence and governance out of the SDK interface: `AuditSink` receives normalized events, while storage, query, retention, reporting, legal hold, and SIEM export are implemented by the host.
- [ ] Wire tracer calls into model request, model response, tool start, tool result, policy decision, approval decision, cancellation, and checkpoint write paths.
- [ ] Add tests:

```bash
cargo nextest run -p dasclaw_runtime -E 'test(tracer_records_policy_decision_ids_without_raw_arguments) | test(audit_event_sequence_matches_run_event_sequence)'
```

Expected result:

```text
PASS tracer_records_policy_decision_ids_without_raw_arguments
PASS audit_event_sequence_matches_run_event_sequence
```

- [ ] Run:

```bash
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
```

Expected result:

```text
Finished `dev` profile
```

### Task 14: Keep AppServer as Host, Not SDK Core

**Priority:** P2  
**Depends on:** Task 7, Task 9, Task 10, Task 13  
**Risk fixed:** SDK consumers inherit host-specific app-server lifecycle, auth, policy, and streaming decisions instead of depending on a small runtime API.

- [ ] Convert app-server agent endpoints to call public `Runner` and `ToolRegistry` APIs.
- [ ] Keep HTTP/SSE shape stable while removing direct ownership of runtime internals from app-server state.
- [ ] Move `AppServerServices::real_with_root()` composition into `crates/dasclaw_app_server/src/main.rs` or a host-specific builder module so the SDK-facing server library accepts injected services.
- [ ] Add domain request/response types for SDK-facing service boundaries and keep RPC DTO conversion inside app-server adapters.
- [ ] Keep `crates/dasclaw_app_server_protocol` out of the `dasclaw` SDK facade and out of `dasclaw_runtime` public exports.
- [ ] Move app-server-only policy adapters, FS, Command, Repo, Search, Config, Skills, and desktop compatibility services behind host composition code.
- [ ] Add a compile-time smoke test that `dasclaw_runtime` can be used without `dasclaw_app_server`.
- [ ] Add app-server integration tests for run start, stream event, approval, cancel, and completion through `RunHandle`, plus protocol DTO conversion tests.
- [ ] Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(agent_run_start_stream_approval_cancel_uses_run_handle)'
cargo nextest run -p dasclaw_app_server -E 'test(app_server_protocol_dto_maps_to_sdk_run_request)'
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_app_server --tests
```

Expected result:

```text
PASS agent_run_start_stream_approval_cancel_uses_run_handle
PASS app_server_protocol_dto_maps_to_sdk_run_request
Finished `dev` profile
```

## P3. SDK Packaging and Public API Hygiene

### Task 15: Stabilize SDK Public Surface and Release Gates

**Priority:** P3  
**Depends on:** Task 1 through Task 14  
**Risk fixed:** Public API shape and crate metadata remain unsuitable for external SDK consumption after the safety and lifecycle fixes land.

- [ ] Audit `crates/dasclaw_core/Cargo.toml`, `crates/dasclaw_runtime/Cargo.toml`, and related `publish = false` settings.
- [ ] Split host-only provider, keychain, database, and app-server dependencies behind optional features.
- [ ] Add `version` alongside `path` dependencies for crates intended to become publishable SDK crates.
- [ ] Ensure the root workspace, lockfile, and CI jobs can validate the SDK crate subset with one documented command.
- [ ] Mark extensible public enums `#[non_exhaustive]` where external callers will match on runtime events or errors.
- [ ] Add public docs for:
  - safe default builder
  - compatibility builder
  - execution context and policy decision
  - model trait and model adapter
  - typed tool context
  - tool registry
  - run handle lifecycle
  - checkpoint guarantees
  - tracer and audit metadata guarantees
- [ ] Add a small crate-level example that builds an enterprise-fail-closed agent with one read-only tool.
- [ ] Run:

```bash
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
cargo doc -p dasclaw_runtime --no-deps
cargo metadata --no-deps
```

Expected result:

```text
Finished `dev` profile
Documenting dasclaw_runtime
metadata
```

## Verification Sequence

Run these after each task slice that changes Rust code:

```bash
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected result:

```text
check_no_panics completed without new violations
```

Run these before opening the final SDK remediation PR stack:

```bash
cargo check -p dasclaw_core --tests
cargo check -p dasclaw_runtime --tests
cargo check -p dasclaw_session --tests
cargo check -p dasclaw_app_server --tests
cargo nextest run -p dasclaw_runtime -p dasclaw_session -p dasclaw_app_server \
  -E 'test(tool_dispatch_redacts_arguments_before_execution) | test(llm_request_egress_scans_full_context) | test(enterprise_mode_rejects_unsandboxed_streaming_command_tool) | test(run_handle_cancel_stops_only_target_run) | test(sdk_tool_receives_host_app_context) | test(llm_provider_adapter_invokes_model_trait) | test(run_checkpoint_round_trips_without_raw_tool_arguments) | test(tracer_records_policy_decision_ids_without_raw_arguments) | test(agent_run_start_stream_approval_cancel_uses_run_handle)'
```

Expected result:

```text
Finished `dev` profile
PASS tool_dispatch_redacts_arguments_before_execution
PASS llm_request_egress_scans_full_context
PASS enterprise_mode_rejects_unsandboxed_streaming_command_tool
PASS run_handle_cancel_stops_only_target_run
PASS sdk_tool_receives_host_app_context
PASS llm_provider_adapter_invokes_model_trait
PASS run_checkpoint_round_trips_without_raw_tool_arguments
PASS tracer_records_policy_decision_ids_without_raw_arguments
PASS agent_run_start_stream_approval_cancel_uses_run_handle
```

## PR Stack Recommendation

- [ ] PR 1, P0 dispatch and request security: Task 1 and Task 2. Keep the diff narrow and land before control-plane work.
- [ ] PR 2, P0 SDK context and fail-closed profile: Task 3 and Task 4. This creates the mandatory safety envelope used by later tasks.
- [ ] PR 3, P0 command-tool safety: Task 5. This closes the SDK-facing PTY/command boundary before exposing command tools.
- [ ] PR 4, P1 run event identity: Task 6. Keep `Runner` / `RunHandle` extraction out of the active stack for now.
- [ ] PR 5, P1 tool and model SDK APIs: Task 8, Task 9, and Task 10. This replaces product-shaped extension seams with SDK-owned traits.
- [ ] PR 6, P2 persistence and output exposure: Task 11 and Task 12. This depends on run IDs and policy decisions.
- [ ] PR 7, P2 tracing and app-server host boundary: Task 13 and Task 14. This makes app-server a public-API consumer and reference Host.
- [ ] PR 8, P3 release hygiene: Task 15. This should be last because it publishes and documents the safer architecture rather than defining it prematurely.
- [ ] Deferred todo after the active stack: Task 7. Re-baseline dependent task ordering before implementation.

## Stop Condition

The roadmap is complete when:

- P0 tool arguments are sanitized or blocked before model history, approval payloads, events, and execution.
- P0 LLM egress scans the entire provider-bound request envelope.
- Enterprise fail-closed builder mode rejects missing safety components.
- Every SDK run has an `ExecutionContext`, and every policy-controlled boundary can emit a `PolicyDecision.decision_id`.
- SDK-routed command tools cannot use unsandboxed streaming PTY in enterprise mode.
- Runtime emits correlated run events and exposes `RunHandle` for cancellation, approval routing, and stream ownership.
- New tools use `ToolContext<C>` / `RunContext<C>` while legacy `JobContextCore` tools remain compatibility adapters.
- Tool execution flows through one registry preserving schema, approval, and risk metadata.
- Model invocation uses a narrow `Model` trait, with `LlmProvider` and `AgentResponder` preserved as compatibility adapters rather than the primary SDK seam.
- Session checkpoints include run metadata without persisting raw sensitive tool data.
- Tracer and audit events record IDs, versions, classifications, and sanitized metadata without raw prompts, raw tool arguments, or raw tool outputs by default.
- AppServer consumes public runtime APIs instead of compensating for missing SDK control-plane primitives.
- AppServer protocol DTOs and product services remain Host concerns and are not re-exported by the SDK facade.
- Public crate metadata and docs describe the safe SDK surface accurately.
