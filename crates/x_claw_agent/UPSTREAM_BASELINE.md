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

## Porting log

| Date | Upstream commit | What | Notes |
|---|---|---|---|
| 2026-04-21 | `610b3470` | Initial baseline | Empty scaffold; no runtime code ported yet. |
