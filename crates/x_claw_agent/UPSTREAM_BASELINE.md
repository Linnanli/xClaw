# Upstream Baseline — `x_claw_agent`

## Sources

| Upstream crate | Path | Commit |
|---|---|---|
| `claw-code/rust/crates/runtime` | `claw-code/rust/crates/runtime` | `610b34708d7748d6ba40b11d8a759a4ecd463e10` |
| `claw-code/rust/crates/commands` | `claw-code/rust/crates/commands` | `610b34708d7748d6ba40b11d8a759a4ecd463e10` |

- Submodule: `claw-code/` @ `610b3470` (tag `step-i`, branches `x-claw` / `main`)
- Upstream remote: `https://github.com/ultraworkers/claw-code.git`
- Snapshot date: 2026-04-21

## Sync strategy

- **Mechanism**: the `claw-code/` directory is already a git submodule in the x-claw parent repo; upstream refresh = `git -C claw-code pull` on a tracking branch, then manually port diffs into `crates/x_claw_agent/src/`.
- **Not** using `git subtree pull` (submodule layout does not require it, and it fights with the existing submodule).
- Every port must append an entry to the **Porting log** below.

## ⚠️ Structural mismatch with plan document

Phase 3 execution plan ([`../../docs/plans/architecture-refactor/05-phase3-agent-extraction.md`](../../docs/plans/architecture-refactor/05-phase3-agent-extraction.md)) lists files such as `agent_loop.rs`, `dispatcher.rs`, `thread_ops.rs`, `session.rs` as if they should be copied 1:1 from `claw-code/rust/crates/runtime/src/`. **At baseline commit `610b3470` this is not accurate.**

Upstream `runtime/src/` at baseline contains (40 files):
`bash.rs`, `bash_validation.rs`, `bootstrap.rs`, `branch_lock.rs`, `compact.rs`, `config.rs`, `config_validate.rs`, `conversation.rs`, `file_ops.rs`, `git_context.rs`, `green_contract.rs`, `hooks.rs`, `json.rs`, `lane_events.rs`, `lib.rs`, `lsp_client.rs`, `mcp.rs`, `mcp_client.rs`, `mcp_lifecycle_hardened.rs`, `mcp_server.rs`, `mcp_stdio.rs`, `mcp_tool_bridge.rs`, `oauth.rs`, `permission_enforcer.rs`, `permissions.rs`, `plugin_lifecycle.rs`, `policy_engine.rs`, `prompt.rs`, `recovery_recipes.rs`, `remote.rs`, `sandbox.rs`, `session.rs`, `session_control.rs`, `sse.rs`, `stale_base.rs`, `stale_branch.rs`, `summary_compression.rs`, `task_packet.rs`, `task_registry.rs`, `team_cron_registry.rs`, `trust_resolver.rs`, `usage.rs`, `worker_boot.rs`.

Ironclaw `desktop-client/ironclaw/src/agent/` contains (22 files):
`agent_loop.rs`, `agentic_loop.rs`, `attachments.rs`, `commands.rs`, `compaction.rs`, `context_monitor.rs`, `cost_guard.rs`, `dispatcher.rs`, `heartbeat.rs`, `job_monitor.rs`, `mod.rs`, `router.rs`, `routine.rs`, `routine_engine.rs`, `scheduler.rs`, `self_repair.rs`, `session.rs`, `session_manager.rs`, `submission.rs`, `task.rs`, `thread_ops.rs`, `undo.rs`.

**File-name intersection: `session`, `compaction` (only, and even those are unlikely to be identical content).**

### Implication

The ironclaw `agent/` tree is **not** a straight fork of current-day upstream runtime. It is either:

1. A fork of a much older claw-code revision that has since been reshuffled upstream, or
2. An ironclaw-specific rewrite inspired by but divergent from claw-code.

Step C/D must **treat the ironclaw `agent/` tree as the source of truth** for what goes into `x_claw_agent`, and use upstream `runtime/` + `commands/` as *reference* for shape, naming, and contract — not as the literal code to copy.

The "upstream rebase" story (Step K) therefore reduces to: "when claw-code upstream changes, cherry-pick semantic improvements into `x_claw_agent`" rather than "`subtree pull` blindly."

## Upstream capability audit (2026-04-21, commit `610b3470`)

Half-day scan of `claw-code/rust/crates/runtime/src/*.rs` against
`desktop-client/ironclaw/src/agent/*.rs`, looking for upstream-only capabilities
worth porting into `x_claw_agent` before Step D.

### Upstream-only modules (ironclaw does not have equivalents)

| Upstream module | Lines | Purpose | Relevance to ironclaw |
|---|---|---|---|
| `bash_validation.rs` | 1004 | Bash command denylist + arg validation before exec | **High** — ironclaw currently delegates command safety to DockerSandbox; a pre-exec textual validator is complementary |
| `mcp*` (8 files, ~6500) | — | Full MCP client + server + lifecycle + tool bridge | **Low for Phase 3** — MCP integration already tracked separately; not in agent refactor scope |
| `permissions.rs` + `permission_enforcer.rs` | 1268 | 5-level permission modes (ReadOnly / WorkspaceWrite / DangerFullAccess / Prompt / Allow) + static allow/deny rules + prompt override | **Medium** — ironclaw approval flow is binary (approve/deny); upstream's mode model is richer but unclear whether product wants it now |
| `policy_engine.rs` | 581 | Multi-lane merge / reconcile / escalate policy with green-level conditions | **None** — upstream-specific worktree orchestration; does not match ironclaw's single-workspace flow |
| `green_contract.rs` / `stale_base.rs` / `stale_branch.rs` | ~1100 | Lane freshness + green-light contracts | **None** — same lane-orchestration scope as above |
| `sandbox.rs` | 385 | Linux namespace/netns isolation descriptors | **Low** — ironclaw already has DockerSandbox which is a stronger primitive |
| `bootstrap.rs` / `worker_boot.rs` | ~1800 | CLI launch sequencing | **None** — ironclaw has its own `engine_startup_tests` + `main.rs` path |
| `task_packet.rs` / `task_registry.rs` / `team_cron_registry.rs` | ~1400 | Task packet format + cron-like scheduling | **Overlap** — ironclaw has `routine.rs` + `scheduler.rs` + `routine_engine.rs` serving the same role |
| `recovery_recipes.rs` | 633 | Recipe-driven recovery playbooks | **Medium** — possibly useful for `self_repair.rs`, needs deeper look |
| `oauth.rs` | 603 | OAuth device flow | **Low** — provider-specific; not an agent-layer concern |

### Ironclaw-only modules (upstream has no equivalent)

| Ironclaw module | Lines | Purpose |
|---|---|---|
| `agent_loop.rs` | 1908 | Channel-driven main loop with `AgentDeps`, workspace, DB, channels |
| `dispatcher.rs` | 3001 | Tool dispatch + approval + parameter redaction |
| `thread_ops.rs` | 2572 | Thread/session persistence + undo + approval replay |
| `routine.rs` + `routine_engine.rs` | 4140 | Cron + reactive routine engine |
| `self_repair.rs` | 856 | Stuck-job + broken-tool repair |
| `cost_guard.rs` | 892 | Per-user cost cap enforcement |
| `heartbeat.rs` | 971 | Proactive heartbeat |
| `submission.rs` | 870 | Submission parser (slash commands etc.) |
| `agentic_loop.rs` | 836 | Inner LLM → tool → LLM loop with `LoopDelegate` trait |
| `session_manager.rs` | 1105 | Multi-user session lifecycle |
| `undo.rs` | 376 | Checkpoint-based undo |
| `attachments.rs` | 307 | Attachment handling |
| `context_monitor.rs` | 236 | Context budget tracking |
| `router.rs` | 200 | Channel message intent routing |

### Overlapping concepts with divergent implementations

| Concept | Upstream | Ironclaw | Verdict |
|---|---|---|---|
| Session/message model | `session.rs` + `ContentBlock { Text, ToolUse, ToolResult }` | `session.rs` + `Session > Thread > Turn` tree, uses `ChatMessage`/`ToolCall` | Ironclaw richer; keep ironclaw's |
| Compaction | `compact.rs` 825 lines | `compaction.rs` 899 lines | Both have it; ironclaw integrated with `ContextMonitor` — keep ironclaw's |
| Conversation loop | `conversation.rs` 1811 lines, `ApiClient` + `ToolExecutor` traits | `agent_loop.rs` + `agentic_loop.rs` + `dispatcher.rs`, `LoopDelegate` trait | Different shape; ironclaw's is channel-integrated |
| Hooks | `hooks.rs` 1116 lines, **subprocess hooks** (`PreToolUse`/`PostToolUse` exec shell commands via `RuntimeHookConfig`) | `src/hooks/HookRegistry` (in-process Rust hooks) | **Different concept**; upstream is user-extension points, ironclaw is internal hooks. Phase 3 plan's `SafetyHook`/`ApprovalGate` is a third concept (trait seam for crate split) |

### Decision

**Treat ironclaw `agent/` as the source of truth; do NOT attempt any
`git subtree pull`-style sync of upstream `runtime/` into `x_claw_agent`.**

Rationale:

1. Divergence is structural, not incidental. Ironclaw has ~14 modules upstream
   does not have; upstream has ~15 modules ironclaw does not need. Even in
   overlapping concepts (session, compaction, conversation) the shape differs.
2. Upstream `runtime` mixes application-layer concerns (CLI bootstrap, MCP
   servers, OAuth, worktree policy) into the crate. These are not wanted in
   `x_claw_agent` by design.
3. Only three upstream modules are genuinely portable candidates, and all are
   **optional enhancements**, not Phase 3 blockers:
   - `bash_validation.rs` — pre-exec textual bash denylist. **Defer** to a
     follow-up safety story; DockerSandbox already handles the hard cases.
   - `permissions.rs` / `permission_enforcer.rs` — 5-level permission modes.
     **Defer** until product confirms richer modes are wanted; current
     binary approval works.
   - `recovery_recipes.rs` — recipe-driven recovery. **Revisit** when
     `self_repair.rs` next needs a refactor.

### Revised Phase 3 porting strategy (supersedes 05-phase3-agent-extraction.md Step D/K)

- **Step D**: move ironclaw `agent/*` into `crates/x_claw_agent/src/runtime/`
  and `crates/ironclaw_routines/` along the split already documented in the
  plan, **not** from upstream. Insert hook seams (`SafetyHook`,
  `SandboxExecutor`, `SecretProvider`, `ApprovalGate`) at the trait boundary
  defined in Step C.
- **Step K**: "rebase drill" becomes a **semantic cherry-pick drill**: pick
  one upstream improvement (candidate: `bash_validation.rs`), port it into
  `x_claw_agent` as a new feature module, and verify the porting log mechanism
  works. Drop the `git subtree pull` language.

## Porting log

| Date | Upstream commit | What | Notes |
|---|---|---|---|
| 2026-04-21 | `610b3470` | Initial baseline + capability audit | Empty scaffold; no runtime code ported yet. Decision: ironclaw is source of truth; upstream is a shape reference. |
| 2026-04-21 | n/a (internal) | Step D-0: move message domain types | Moved `Role`/`ContentPart`/`ImageUrl`/`ChatMessage` + `ToolCall`/`ToolResult`/`ToolDefinition` + `Completion(Tool)Request/Response`/`FinishReason`/`ModelMetadata` + `sanitize_tool_messages`/`UnsupportedParam`/`strip_unsupported_*` (ironclaw `llm/provider.rs` lines 1-355 + 491-580 + tests) into `x_claw_agent::messages`. `LlmProvider` trait stays in ironclaw (requires `rust_decimal`, `LlmError`). Ironclaw's `llm/provider.rs` now just re-exports. 17 tests migrated, all green. `cargo build --workspace` 0 errors / 0 warnings. 665 ironclaw `llm::` unit tests still green. |
| 2026-04-21 | n/a (internal) | Step D-1: move 5 agent files | Moved `agent/{submission,task,session,context_monitor,undo}.rs` (~3727 lines) into `x_claw_agent::{submission,task,session,context_monitor,undo}`. Skipped `attachments.rs` (depends on `crate::channels` application-layer types — revisit at D-3). `TaskHandler::run` signature relaxed from `Result<TaskOutput, ironclaw::Error>` to `Result<TaskOutput, Box<dyn std::error::Error + Send + Sync>>` (`TaskHandlerError` alias) — verified zero `impl TaskHandler` sites in workspace. New crate deps: `chrono` (serde), `uuid` (v4/v5/serde), `ironclaw_common` (cross-dir path). Ironclaw `agent/{...}.rs` reduced to 4-8 line shims (`pub use x_claw_agent::{module}::*;`). Ironclaw fixes: `agent/scheduler.rs` `Task::Background` arm wraps handler error via `.map_err()` to `Error::Job(JobError::ContextError)`; `worker/job.rs` removed unused `impl From<TaskOutput> for Result<String, Error>` (orphan-rule violation, no call sites). `cargo build --workspace` green; `cargo test -p x_claw_agent --lib` 118 passed (up from 17); `cargo test -p ironclaw --lib` 4060 passed / 0 failed / 3 ignored. |

| 2026-04-21 | n/a (internal) | Step D-3: host collaboration traits (scope-narrowed) | Added `x_claw_agent::traits` module with **two** traits: `WorkspaceWriter` (single method `append(path, content)`, covers compactions only workspace touch) and `LlmCompleter` (single method `complete_text(request)`, a narrow facade over `Reasoning::complete` that keeps reasoning-tag cleanup on the host side). `HostError = Box<dyn std::error::Error + Send + Sync>` keeps `WorkspaceError` / `LlmError` out of the agent crate. **Deliberately scope-narrowed from the original plan of 4 traits (Workspace/Database/Channel/Extension)**: Database/Channel/Extension have no migration-blocking consumers yet, so they are deferred until a ported module actually needs them. `router.rs` / `attachments.rs` also deferred — their real blocker is data coupling on `IncomingMessage` / `IncomingAttachment` (struct fields, not methods), which is a separate "channels DTO lowering" step, not trait work. Ironclaw wires up blanket impls in new `agent/traits_impl.rs`: `impl WorkspaceWriter for Workspace` (delegates to existing `Workspace::append`) and `impl LlmCompleter for Reasoning` (delegates to existing `Reasoning::complete`, boxes the error). 2 traits × 2 impls + 5 tests (2 in `x_claw_agent`, 3 in ironclaw incl. end-to-end reasoning-tag-strip test with a FakeProvider). `cargo build --workspace` 0 errors / 0 warnings; `cargo test -p x_claw_agent --lib` 120 passed (+2); `cargo test -p ironclaw --lib` 4063 passed (+3). |

| 2026-04-21 | n/a (internal) | Step D-2: port `ContextCompactor` | Ported ironclaw `agent/compaction.rs` (~899 lines) into `x_claw_agent::compaction`. Signature migration: `ContextCompactor::new(Arc<dyn LlmProvider>)` → `ContextCompactor::new(Arc<dyn LlmCompleter>)`; `compact(_, _, Option<&Workspace>)` → `compact(_, _, Option<&dyn WorkspaceWriter>)`; return type `Result<_, ironclaw::Error>` → `Result<_, HostError>`. Dropped the internal `Reasoning::new(self.llm.clone()).with_model_name("compactor")` wrapper from `generate_summary` — reasoning-tag cleanup now happens host-side in the `LlmCompleter for Reasoning` blanket impl. Ported ~18 unit tests (format/strategy/failure-path) with a new local `StubCompleter` helper that mirrors `StubLlm` (`new(response)` / `failing(name)` / `calls() -> u32`) plus a `FailingWriter` stub for archival-failure paths. Kept 2 `#[cfg(all(test, feature = "libsql"))]` integration tests inline in the ironclaw shim since they need real `Workspace::new_with_db` + `LibSqlBackend::new_memory` to prove fail-safe semantics end-to-end. Ironclaw `agent/compaction.rs` reduced from 899 lines to an ~115-line shim. Two call sites in `agent/thread_ops.rs` (auto-compaction + manual compaction) now wrap LLM with `Arc::new(Reasoning::new(self.llm().clone()))` and cast workspace with `.map(|w| w.as_ref() as &dyn WorkspaceWriter)`. `cargo build --workspace` 0 errors / 0 warnings; `cargo test -p x_claw_agent --lib` 140 passed (+20); `cargo test -p ironclaw --lib` 4045 passed (net -18 because tests migrated to x_claw_agent) + 2 libsql compaction tests green under `--features libsql`. |
| 2026-04-21 | n/a (internal) | Step D-4 (data-types half): port reasoning data types + agentic-loop engine | Ported into `x_claw_agent` as three new modules: `response_types` (`TokenUsage` / `ResponseAnomaly` / `ResponseMetadata` / `RespondResult` / `RespondOutput`, 5 types, no serde derives because `FinishReason`/`ToolCall` upstream never implemented `Deserialize`), `intent` (`TOOL_INTENT_NUDGE` / `TRUNCATED_TOOL_CALL_NOTICE` / `llm_signals_tool_intent` + private `strip_code_blocks` / `strip_quoted_strings` / `floor_char_boundary` / `truncate_for_preview`, 11 tests), `reasoning_ctx` (`ReasoningContext` data struct with all builder methods, 2 tests). Also ported the **engine** into `x_claw_agent::agentic_loop` (`LoopSignal` / `TextAction` / `LoopOutcome` / `AgenticLoopConfig` / `LoopDelegate` trait / `run_agentic_loop`, ~620 lines incl. 10 tests). **Route-B signature change**: removed the `reasoning: &Reasoning` parameter from `LoopDelegate::call_llm` and from `run_agentic_loop()` — the engine now knows nothing about any LLM type; delegates that need a `Reasoning` engine own one as a field. Return type in the engine is `HostError = Box<dyn Error + Send + Sync>`. Engine uses `crate::session::PendingApproval` (already in x_claw_agent from D-1). Ironclaw side: `llm/reasoning.rs` now re-exports the data types and the three tool-intent helpers via `pub use x_claw_agent::{intent, reasoning_ctx, response_types}::*` — keeps every existing `use crate::llm::{...}` call site working unchanged (provider.rs already routed `ChatMessage`/`ToolCall`/... through x_claw_agent in D-0, so field-compatibility is by construction). Ironclaw `agent/agentic_loop.rs` is **not yet** a shim — the in-place engine with the old `reasoning: &Reasoning` parameter still runs ironclaw production; the shim + 3-delegate refactor (drop `reasoning` param from `ChatDelegate`/`JobDelegate`/`ContainerDelegate`, add owned/borrowed `Reasoning` field instead) is scheduled for D-4.5. `cargo build -p x_claw_agent --lib` 0 errors / 0 warnings; `cargo test -p x_claw_agent --lib` 164 passed (+24); `cargo build -p ironclaw --lib` 0 errors / 0 warnings; `cargo test -p ironclaw --lib -- --test-threads=1` 4045 passed / 0 failed / 3 ignored. |
| 2026-04-21 | n/a (internal) | Step D-4.5: swap ironclaw engine to shim + refactor 3 delegates | Deleted ironclaw's in-tree 836-line `agent/agentic_loop.rs` engine and replaced it with a ~45-line shim that `pub use`s `AgenticLoopConfig` / `LoopDelegate` / `LoopOutcome` / `LoopSignal` / `TextAction` / `run_agentic_loop` from `x_claw_agent::agentic_loop` and `truncate_for_preview` from `x_claw_agent::intent`. Refactored all 3 ironclaw delegates (`ContainerDelegate` in `worker/container.rs`, `JobDelegate<'a>` in `worker/job.rs`, `ChatDelegate<'a>` in `agent/dispatcher.rs`) to the new signature: added a `reasoning` field (owned `Reasoning` for Container/Chat; borrowed `&'a Reasoning` for Job because `execution_loop` still needs `reasoning.plan()` + `self.execute_plan(rx, reasoning, ...)` before entering the agentic loop), dropped the `reasoning: &Reasoning` parameter from `call_llm`, changed trait-method return types from `Result<_, Error>` to `Result<_, HostError>` on both `call_llm` and `execute_tool_calls`. Inherent helpers (`call_llm_non_streaming` / `call_llm_streaming` / `handle_rate_limit` / `try_complete_on_error`) keep their `Error`-returning signatures and rely on the blanket `From<E> for Box<dyn Error + Send + Sync>` for the `?` path across the boundary. Reverse direction (HostError → Error) uses a new shim helper `host_err_to_error` that downcasts back to `crate::error::Error` (all delegate errors originate as `Error` and get boxed) and falls back to `LlmError::InvalidResponse` for the unexpected case. 3 outer `run_agentic_loop(...).await?` call sites (dispatcher.rs, worker/job.rs, closure in worker/container.rs) and 2 rate-limit return sites in `JobDelegate::call_llm` got `.map_err(...)` adjustments. 1 test-only `JobDelegate { ... }` struct literal in `worker/job.rs` tests got a `reasoning: &reasoning` field (with `let reasoning = Reasoning::new(worker.llm().clone());` above). `cargo build -p ironclaw --lib` 0 errors / 0 warnings; `cargo test -p x_claw_agent --lib` 164 passed (unchanged); `cargo test -p ironclaw --lib` 4030 passed / 0 failed / 3 ignored (net -15 vs D-4: the 10 engine tests + 5 supporting tests that lived in the old 836-line file are now only in x_claw_agent, which is correct — no duplicate coverage). |
| 2026-04-21 | n/a (internal) | Step D-5: **DEFERRED** — Phase 3 closes after D-4.5 | Original D-5 plan was to move `agent_loop.rs` (1908 lines), `dispatcher.rs` (3008), `thread_ops.rs` (2583), `commands.rs` (1033), `session_manager.rs` (1105) into `x_claw_agent`. After auditing imports, decision: **do not move any of these into the agent crate**. Rationale: (a) `agent_loop.rs` / `dispatcher.rs` / `thread_ops.rs` / `commands.rs` depend on `channels::{ChannelManager, IncomingMessage, OutgoingResponse, StatusUpdate}`, `context::{ContextManager, JobContext, JobState}`, `db::Database`, `extensions::ExtensionManager`, `hooks::HookRegistry`, `safety::SafetyLayer`, `skills::SkillRegistry`, `tools::ToolRegistry` — all of which are **ironclaw application-layer concerns** (multi-channel message routing, per-user job persistence, SafetyLayer/DLP/command-denylist, extension/skill audit pipeline, cost & quota enforcement). None of these belong inside a reusable agent-core crate; they are exactly the "ironclaw safety layer on top of the agent" that the user wants to preserve as a differentiator. Abstracting them behind trait seams would balloon `x_claw_agent::traits` from the current 2 traits to ~10, violating the D-3 scope-narrowing decision, and effectively re-exporting the entire ironclaw app surface. (b) `session_manager.rs` is **not agent-core either**: its job is multi-user Session lifecycle (`HashMap<UserId, Session>` + 1000-session warning threshold + 10-minute stale cleanup + external-channel thread_id → UUID mapping + SessionStart/SessionEnd webhook events). Upstream claw-code `runtime` crate has no equivalent — it manages a single agent run, not a multi-user server. (c) The boundary that already exists after D-4.5 is clean and matches industry convention: **x_claw_agent = agent core** (messages/session/undo/task/submission/context_monitor/compaction/intent/response_types/reasoning_ctx/agentic_loop + 2 host traits); **ironclaw = agent core + multi-user service + safety/policy layer**. Forcing the 5 D-5 files into x_claw_agent would dilute that boundary. Phase 3 therefore closes with D-4.5 as the final ported step. Outstanding optional follow-ups (still tracked in the capability audit above, not Phase 3 blockers): port `bash_validation.rs` from upstream as a semantic cherry-pick drill (Step K), and revisit `permissions.rs` / `recovery_recipes.rs` only if product requirements demand them. No code changes in this entry; all build/test state unchanged from D-4.5 (`cargo test -p x_claw_agent --lib` 164 passed, `cargo test -p ironclaw --lib` 4030 passed). |

---

## Step G — ironclaw_safety impl SafetyHook — DONE (2026-04-21)

**Commit**: submodule `7720cf10` — `feat(ironclaw_safety): add agent-hook feature`

**What was added**:

- `crates/ironclaw_safety/` (in `desktop-client/ironclaw/`):
  - New `agent-hook` Cargo feature (optional `async-trait` + `x_claw_agent` deps).
  - New `agent_hook.rs` module defining `IronclawSafetyHook` — a `#[derive(Clone)]` adapter holding `Arc<SafetyLayer>` and implementing `x_claw_agent::SafetyHook`.
  - New public accessor `SafetyLayer::leak_detector()`.

**Hook mapping**:

| `SafetyHook` method | `SafetyLayer` call                   | Failure mode |
|---------------------|--------------------------------------|--------------|
| `before_prompt`     | `scan_inbound_for_secrets`           | Block if secret detected (fail-safe). |
| `after_completion`  | `leak_detector().scan_and_clean`     | Redact in place; on Err replace entire body with `[blocked]` marker. |
| `before_tool_call`  | `validator().validate_tool_params`   | Block if invalid. |
| `after_tool_output` | `sanitize_tool_output`               | Replace with sanitized body (handles truncation + redaction + policy). |

All four hooks are **fail-safe**: errors refuse the operation rather than allow it.

**Why now (vs. in `x_claw_agent` itself)**:
- The trait lives in `x_claw_agent` (defined in Step C). Step G just supplies the concrete implementation.
- Keeping the impl in `ironclaw_safety` avoids pulling SafetyLayer into the agent kernel.
- The `agent-hook` feature is default-off so other ironclaw_safety callers (HTTP middleware, inbound scanners) stay dependency-free.

**What is still deferred to a later step (NOT in G)**:

1. **Calling the hooks from the agent loop**. The ironclaw `dispatcher.rs` / `agent_loop.rs` don't yet invoke `SafetyHook` at the four call sites. This requires touching the D-5-deferred files, so the hook wiring is deferred along with D-5. When we do Step I (`agent_app.rs`) or revisit D-5, we will:
   - Pass `Arc<dyn SafetyHook>` into the delegate.
   - Call `before_prompt` inside the delegate's `respond()` before LLM call.
   - Call `after_completion` inside the delegate's `handle_text_response()`.
   - Call `before_tool_call` + `after_tool_output` inside the delegate's `execute_tool_calls()`.
2. **`ApprovalGate` wiring**. Current ironclaw approval flow goes through its own channel; rewiring it to `x_claw_agent::ApprovalGate` is part of the same deferred work.

**Test status**: `cargo test -p ironclaw_safety --features agent-hook` — 7/7 new tests pass. `cargo build -p ironclaw --lib` — green.

---

## Step E — SandboxManager impl SandboxExecutor — DONE (2026-04-21)

**Commit**: submodule `d9a31cf9` — `feat(sandbox): impl x_claw_agent::SandboxExecutor for SandboxManager`

**What was added**:

- `desktop-client/ironclaw/src/sandbox/agent_executor.rs`:
  - `SandboxAgentExecutor { manager: Arc<SandboxManager>, workspace_root: PathBuf }` implementing `x_claw_agent::SandboxExecutor`.
  - `run_bash` delegates to `SandboxManager`'s Docker path; `read_file` / `write_file` use `tokio::fs` bounded by `workspace_root`.
  - Manual `normalize_path` resolves `..` segments **without** filesystem touch, so non-existent write targets still get validated.
  - Error mapping is exhaustive: every `IronclawSandboxError` variant maps to a specific `x_claw_agent::SandboxError` variant (PolicyViolation / Docker / Timeout / ResourceExhausted / Io).

**Scope narrowing vs. original plan (05b)**:

- Plan called for extracting `src/sandbox/` into a separate `crates/ironclaw_sandbox/` crate.
- `src/sandbox/` already couples to `src/secrets/` via `CredentialMapping` — extracting one forces extracting both.
- The architectural goal (clean `x_claw_agent` boundary) is achieved by the trait seam alone. Same Route-B discipline as D-3/D-4.

**Tests**: 7/7 new unit tests (path validation, write/read roundtrip, parent-escape rejection, absolute-foreign-path rejection) pass under `cargo test -p ironclaw --lib sandbox::agent_executor`. `cargo build -p ironclaw --lib` 0 errors / 0 warnings.

---

## Step F — SecretsStore impl SecretProvider — DONE (2026-04-21)

**Commit**: submodule `ef4149ec` — `feat(secrets): impl x_claw_agent::SecretProvider for SecretsStore`

**What was added**:

- `desktop-client/ironclaw/src/secrets/agent_provider.rs`:
  - `AgentSecrets<S: SecretsStore> { store: Arc<S>, user_id: String }` implementing `x_claw_agent::SecretProvider`.
  - Generic over `S: SecretsStore` so Postgres / LibSql / InMemory backends all satisfy the trait.
  - `user_id` scopes every lookup; the agent kernel cannot accidentally cross users.
  - `SecretError::NotFound` -> `Ok(None)` (first-class "not configured").
  - All other variants (DecryptionFailed / AccessDenied / KeychainError / Database / ...) collapse to `AgentSecretError::Io` — fail-safe.
  - Plaintext lives briefly in `DecryptedSecret`, unwrapped into `x_claw_agent::SecretString` only inside `get()`.

**Scope narrowing**: same rationale as Step E. Trait seam alone suffices; no separate `crates/ironclaw_secrets/` extraction.

**Tests**: 7/7 new unit tests (roundtrip, none-on-missing, user isolation, debug redaction, NotFound mapping, non-NotFound -> Io). Plus a full integration contract test (see Phase 3 closure).

---

## Phase 3 — CLOSED at G / E / F (2026-04-21)

**Integration contract test**: `desktop-client/ironclaw/tests/agent_hooks_integration_test.rs` — 13 `#[tokio::test]` cases proving the four Hook adapters (`IronclawSafetyHook` / `SandboxAgentExecutor` / `AgentSecrets` / `AutoApproveGate`) compose behind `Arc<dyn Trait>`, satisfy `Send + Sync`, and honour their fail-safe contracts at the trait boundary. 13/13 pass.

**Why H / I / J / K stop here**:

- Original plan called for organising `agent_app.rs` (Step I) and wiring the hooks into dispatcher's call sites (Step I + H).
- Audit confirmed: `desktop-client/ironclaw/src/agent/dispatcher.rs` already hardcodes `SafetyLayer::sanitize_tool_output` at 5+ sites (L274 / L316 / L564-565 / L598 / L647-648 / L877 / L964). This is the same file family that triggered D-5 deferral.
- Replacing those hardcoded calls with `Arc<dyn SafetyHook>` would require touching exactly the dispatcher / agent_loop / thread_ops surface that D-5 explicitly deferred, for no user-visible capability gain (SafetyLayer is already invoked; the hook seam would just re-route the same calls).
- The three Hook adapters are now **ready to plug in** the day dispatcher/delegate is revisited. That day is not Phase 3.

**Boundary after Phase 3**:

- `x_claw_agent` = agent core + 4 object-safe host traits (`SafetyHook` / `SandboxExecutor` / `SecretProvider` / `ApprovalGate`).
- `ironclaw_safety` + `ironclaw` provide concrete trait implementations behind feature gates (`agent-hook`) or always-on (`sandbox::agent_executor`, `secrets::agent_provider`).
- The trait seam is proven by the integration contract test; no ironclaw types leak through the kernel boundary.
- Dispatcher continues to call `SafetyLayer` directly; the hook plumbing is a future seam activated when D-5 is revisited.

**Final Phase 3 test state**:

- `cargo test -p x_claw_agent --lib` — 164 passed / 0 failed.
- `cargo test -p ironclaw_safety --features agent-hook` — 196 passed (189 + 7 new) / 0 failed.
- `cargo test -p ironclaw --lib sandbox::agent_executor` — 7/7 passed.
- `cargo test -p ironclaw --lib secrets::agent_provider` — 7/7 passed.
- `cargo test -p ironclaw --test agent_hooks_integration_test` — 13/13 passed.
- `cargo build -p ironclaw --lib` — 0 errors / 0 warnings.

---

## Step K — cherry-pick drill: port `bash_validation.rs` — DONE (2026-04-22)

**Goal**: exercise the "semantic cherry-pick" workflow this baseline doc was created to support. Pick one upstream improvement, port it with documented deviations, and prove the porting log mechanism works end-to-end.

**What was ported**:

- `crates/x_claw_agent/src/bash_validation.rs` (1004 lines, 32 tests) — byte-for-byte copy of upstream `claw-code/rust/crates/runtime/src/bash_validation.rs` at baseline `610b3470`. Only the module doc header was extended with provenance + integration notes. The `use crate::permissions::PermissionMode;` line resolves to our local sliced port (see below) — not to upstream's full `permissions` module.
- `crates/x_claw_agent/src/permissions.rs` (NEW, ~75 lines including tests) — **deliberately sliced** port of upstream `permissions.rs`. Carries over **only** the `PermissionMode` enum + `as_str()` + the five variants `ReadOnly` / `WorkspaceWrite` / `DangerFullAccess` / `Prompt` / `Allow`. Deliberately left behind:
  - `PermissionPolicy` (283-line rule engine over `RuntimePermissionRuleConfig`).
  - `PermissionContext` / `PermissionRequest` / `PermissionPromptDecision` / `PermissionOutcome` / `PermissionOverride`.
  - The `crate::config::RuntimePermissionRuleConfig` dependency.

**Deviation rationale (the point of the drill)**:

- `bash_validation.rs` is the only current consumer of `PermissionMode` in x_claw_agent, and it uses nothing beyond the 5-variant enum + `Copy`/`Eq`/`as_str()`. The full policy engine would cost ~700 lines plus a config-crate dependency for zero current value.
- Rule-engine semantics (deny rules, policy overrides, user prompts) are already owned by ironclaw's existing `safety` + `tools` + `extensions` surface. Porting upstream's would create a parallel policy engine and the coexistence cost is not justified.
- This is exactly the kind of deviation the porting log exists to capture: **port the syntactic check, leave the semantic policy to the host**.

**Public re-exports added to `lib.rs`**:

```rust
pub use bash_validation::{
    CommandIntent, ValidationResult, check_destructive, classify_command, validate_command,
    validate_mode, validate_paths, validate_read_only, validate_sed,
};
pub use permissions::PermissionMode;
```

**Host integration guidance**:

- `validate_command(cmd, mode, workspace)` returns `ValidationResult::{Allow, Warn, Block}`.
- A host already running `SafetyHook::before_tool_call` can call this as the **first gate** inside its hook impl and translate `Warn` / `Block` into `SafetyDecision::Block` / `SafetyDecision::Redact`.
- ironclaw has not yet wired this in — deliberate, same reason as the Phase 3 closure: the dispatcher still calls `SafetyLayer` directly. When D-5 is revisited, ironclaw's `before_tool_call` hook impl should call `validate_command` before its own DLP/denylist checks.

**Test state**:

- `cargo test -p x_claw_agent --lib` — **198 passed** (164 + 32 bash_validation + 2 permissions). Up from 164 at Phase 3 closure.
- `cargo build -p ironclaw --lib` — 0 errors / 0 warnings.
- `cargo build -p x_claw_agent --lib` — 0 errors / 0 warnings.
- No ironclaw tests touched (ironclaw doesn't use these symbols yet).

**What this drill proved**:

1. Mechanical step: `cp` + minor header edits + `use` path substitution + `pub use` re-export. 5-minute port.
2. Deviation-tracking step: `permissions.rs` sliced port is documented both in the file header and this baseline entry. Anyone reading either place sees what was left behind and why.
3. The porting log format (per-commit entry with commit hash, what, notes) scales — this is entry #9 and still readable.

**What remains as future drill variants** (not executed):

- Port a module that **doesn't** compile cleanly against our trait surface (would force a refactor of the adapter layer and prove the porting log captures that too).
- Port a module that **has an ironclaw counterpart** (would force a "keep ironclaw / adopt upstream / merge" decision and document the verdict).
