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
