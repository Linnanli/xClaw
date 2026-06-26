# Dasclaw App Server Enforce And Status Rebaseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the stale 2026-06-25 P0/P1 plan with a current, testable path that finishes only the real remaining app-server work: streaming/runtime sandbox enforcement and status-matrix cleanup.

**Architecture:** Treat `thread/shellCommand`, `thread/approveGuardianDeniedAction`, MCP OAuth, and typed guardian replay stash as completed code paths unless fresh verification disproves them. Keep streaming `command/exec` sandbox overrides fail-closed until a real spawn-plan API exists below `dasclaw_app_server`; do not replay buffered command output to fake streaming sandbox support. Keep `dasclaw_app_server` as the router/audit owner, `dasclaw_shell_tools` / `dasclaw_exec` as the sandbox launch-plan owner, and the protocol gap matrix as the single public status ledger.

**Tech Stack:** Rust 2024, JSON-RPC, `dasclaw_app_server`, `dasclaw_app_server_protocol`, `dasclaw_shell_tools`, `dasclaw_exec`, `dasclaw_sandbox`, `dasclaw_pty`, `dasclaw_workspace_cap`, `cargo nextest`.

---

## Scope Check

This replaces the superseded plan:

- `docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md`

Current code evidence says the remaining work is not the same as that plan's original checklist.

### Completed, Do Not Re-Implement

- `thread/shellCommand`
- `thread/approveGuardianDeniedAction`
- typed `AutoApprovalReviewCompleted.guardian_event` stash producer
- MCP OAuth app-server owner
- non-PTY split stdout/stderr streaming

### Current Remaining Work

- P0: `command/exec` streaming sandbox override must use a real sandbox spawn backend before it can be accepted.
- P0: thread/turn sandbox context still needs runtime enforcement beyond protocol parameter parsing.
- P0: command/filesystem audit coverage needs a final safety pass after sandbox spawn support lands.
- Docs/status: the gap matrix still contains stale compact wording in several sections even though current code has a deterministic local `DasclawAgentRuntimeBridge::compact_thread` owner.

### Startup 4 Questions

| Question | Answer | Required action |
|---|---|---|
| 是否新增模块 / crate / 文件？ | Yes. This plan creates or updates plan/status documentation and may add focused tests. | Check existing plan/status docs first; this plan is the new owner rather than adding another overlapping P0/P1 checklist. |
| 结论是否包含否定语？ | Yes. It says old P1 work is no longer remaining and streaming sandbox is not complete. | Use semantic search plus exact search evidence before changing status text. |
| 是否做跨项目对账？ | No. This plan is repo-local app-server/sandbox work. | No cross-project matrix is needed. |
| 是否写架构对账类文档？ | No. This is an implementation plan plus status rebaseline, not a new ADR or Codex/Dasclaw comparison matrix. | Update the existing gap matrix instead of creating a second matrix. |

### Verification Evidence Already Gathered

- Level 1 semantic search:
  - Query `streaming command exec sandbox spawn plan PTY pipe sandbox launch API` found sandbox-related code, but no app-server-ready cross-platform streaming spawn-plan owner.
  - Query `thread compact start default runtime owner compacted route feature gate` was noisy and mostly hit old desktop/ironclaw routes, so compact status must be decided from app-server symbols and tests, not old references.
- Level 2 symbol evidence:
  - `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` records a prior `lsp-mcp execute_lsp workspace_symbols` pass for MCP OAuth, guardian stash, streaming sandbox fail-closed tests, and `thread_compact_start`.
  - Before committing changes from this plan, rerun `lsp-mcp execute_lsp workspace_symbols` for `build_shell_launch_spec`, `thread_compact_start`, `compact_thread`, `THREAD_SHELL_COMMAND`, and `THREAD_APPROVE_GUARDIAN_DENIED_ACTION`.
- Level 3 exact search:
  - `crates/dasclaw_app_server/src/command_service.rs` still rejects streaming sandbox overrides with `STREAMING_SANDBOX_UNSUPPORTED_MESSAGE`.
  - `crates/dasclaw_shell_tools/src/sandboxed_executor.rs` exposes `build_shell_launch_spec`, whose doc explicitly says it does not apply sandbox.
  - `crates/dasclaw_app_server/src/lib.rs` contains `thread_shell_command`, `thread_approve_guardian_denied_action`, `stash_auto_approval_review_guardian_event`, and `DasclawAgentRuntimeBridge::compact_thread`.

## File Structure

Create or modify only these files for this plan:

- Modify: `docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md`
  - Add a superseded banner so future workers stop executing stale positive streaming sandbox steps.

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Remove stale claims that default runtime lacks compact owner if fresh tests confirm deterministic local compaction.
  - Keep streaming sandbox enforcement and runtime sandbox context enforcement listed as P0.

- Modify: `crates/dasclaw_shell_tools/src/sandboxed_executor.rs`
  - Add the public streaming sandbox launch-plan contract and tests.
  - Keep the current plain `build_shell_launch_spec` for unsandboxed streaming calls.

- Modify: `crates/dasclaw_shell_tools/src/lib.rs`
  - Re-export the new launch-plan types.

- Modify: `crates/dasclaw_exec/src/lib.rs`
  - Add a non-executing sandbox spawn-plan assembly API that can be used by PTY/pipe streaming callers.

- Modify: `crates/dasclaw_app_server/src/command_service.rs`
  - Replace the fail-closed branch only after the new spawn-plan API can produce a real sandbox-backed command.
  - Keep `danger-full-access` fail-closed unless a separate explicit double opt-in task changes buffered and streaming together.

- Test only: existing inline tests in `crates/dasclaw_app_server/src/lib.rs`
  - Re-run existing thread action, guardian replay, compact, and capability tests. Do not add another thread action owner.

## Task 1: Rebaseline The Stale P0/P1 Plan

**Files:**
- Modify: `docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md`
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Add a superseded banner to the old plan**

Insert this block immediately after the H1 in `docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md`:

```markdown
> **Superseded on 2026-06-26:** Do not execute this checklist as-is. `thread/shellCommand`, `thread/approveGuardianDeniedAction`, MCP OAuth, non-PTY split stdout/stderr streaming, and typed guardian replay stash have since been completed or rebaselined. The remaining executable scope is tracked in `docs/superpowers/plans/2026-06-26-dasclaw-app-server-enforce-and-status-rebaseline.md`.
>
> The old positive streaming sandbox test names in this file are intentionally stale. Streaming `command/exec` sandbox overrides must remain fail-closed until a real PTY/pipe sandbox spawn-plan API exists.
```

- [ ] **Step 2: Verify completed route/status tests before changing status text**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(json_rpc_thread_shell_command_returns_empty_object_and_emits_turn_notifications) | test(json_rpc_thread_approve_guardian_denied_action_replays_stashed_command_action) | test(json_rpc_thread_approve_guardian_denied_action_replays_stashed_event_not_request_payload) | test(auto_approval_review_completed_stashes_typed_guardian_event_for_replay) | test(thread_compact_start_with_real_runtime_persists_deterministic_compaction_turn) | test(dasclaw_runtime_bridge_default_features_include_thread_compact_owner) | test(dasclaw_runtime_bridge_compacts_thread_snapshot_without_calling_agent_factory)'
```

Expected: PASS. If any test fails, stop this task and fix the failing implementation before editing the matrix.

- [ ] **Step 3: Update the matrix compact wording**

In `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`, replace remaining phrases that say default runtime has no compact owner with this wording:

```markdown
`thread/compact/start` has a tested app-server route and a deterministic local `DasclawAgentRuntimeBridge::compact_thread` owner. The current compact output is local and deterministic (`Deterministic local compaction`), not an LLM-generated context summary. Further work belongs to richer compaction quality, not to route ownership.
```

Keep `thread/closed` unchanged if it is still unsupported.

- [ ] **Step 4: Remove compact from §9.2 if Step 2 passed**

In `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` §9.2, remove these two bullets:

```markdown
- `thread/compact/start`
- `thread/compacted`
```

Replace the following explanation:

```markdown
`thread/compact/*` 当前已有 fail-safe route，且 compact-capable runtime bridge 可走成功路径；默认 runtime 仍无真实 compact owner。
```

with:

```markdown
`thread/compact/*` 当前已有 tested route、持久化 `compactedTurnId`、`thread/compacted` notification，以及 deterministic local `DasclawAgentRuntimeBridge::compact_thread` owner。剩余风险是 compaction 摘要质量，不是 app-server owner 缺口。
```

- [ ] **Step 5: Commit the status rebaseline**

Run:

```bash
git add docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs: rebaseline app-server remaining p0 scope" -m "已检查 app-server P0/P1 是否已有，结论：thread shell、guardian replay、OAuth、non-PTY streaming 与 deterministic compact owner 已有；streaming sandbox enforce 仍缺真实 spawn-plan API。"
```

Expected: commit succeeds and the old plan clearly points at this file.

## Task 2: Lock The Streaming Sandbox Contract Before Adding Backend Support

**Files:**
- Modify: `crates/dasclaw_app_server/src/command_service.rs`
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Ensure fail-closed tests cover both PTY and non-PTY**

Confirm these tests exist in `crates/dasclaw_app_server/src/command_service.rs`:

```rust
#[cfg(unix)]
#[test]
fn command_service_non_tty_streaming_rejects_read_only_sandbox_policy_until_enforceable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());
    let mut params = exec_params(vec!["sh", "-c", "printf readonly"]);
    params.process_id = Some("pipe_read_only_sandbox".to_string());
    params.stream_stdout_stderr = Some(true);
    params.sandbox_policy = Some(serde_json::json!({"type": "read-only"}));

    let error = service
        .exec(params)
        .expect_err("non-tty streaming must fail closed until sandbox enforcement exists");

    assert!(
        error
            .to_string()
            .contains(STREAMING_SANDBOX_UNSUPPORTED_MESSAGE)
    );
    assert!(service.drain_output_delta_events().is_empty());
}

#[cfg(unix)]
#[test]
fn command_service_tty_streaming_rejects_read_only_sandbox_policy_until_enforceable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());
    let mut params = exec_params(vec!["sh", "-c", "printf readonly"]);
    params.process_id = Some("pty_read_only_sandbox".to_string());
    params.tty = Some(true);
    params.stream_stdout_stderr = Some(true);
    params.sandbox_policy = Some(serde_json::json!({"type": "read-only"}));

    let error = service
        .exec(params)
        .expect_err("PTY streaming must fail closed until sandbox enforcement exists");

    assert!(
        error
            .to_string()
            .contains(STREAMING_SANDBOX_UNSUPPORTED_MESSAGE)
    );
    assert!(service.drain_output_delta_events().is_empty());
}
```

- [ ] **Step 2: Run the fail-closed contract**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(command_service_non_tty_streaming_rejects_read_only_sandbox_policy_until_enforceable) | test(command_service_tty_streaming_rejects_read_only_sandbox_policy_until_enforceable) | test(command_service_tty_streaming_rejects_read_only_permission_profile_until_enforceable) | test(streaming_command_rejects_read_only_sandbox_override_until_enforceable)'
```

Expected: PASS. This proves current behavior is safe before backend support is added.

- [ ] **Step 3: Keep the matrix P0 language precise**

Ensure §9.2 P0 keeps this exact bullet until Task 5 passes:

```markdown
- streaming `command/exec` explicit sandbox override must route to a real sandbox spawn backend before being accepted; until then both PTY and non-PTY streaming paths remain fail-closed for `sandboxPolicy` / `permissionProfile`.
```

- [ ] **Step 4: Commit the fail-closed contract if tests or docs changed**

Run:

```bash
git add crates/dasclaw_app_server/src/command_service.rs docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "test: lock streaming sandbox fail-closed contract" -m "已检查 streaming sandbox spawn owner 是否已有，结论：当前只有 plain launch spec，显式 sandbox override 必须继续 fail-closed。"
```

Expected: commit succeeds only if this task changed files. If no files changed, record the PASS result in the implementation notes for Task 3.

## Task 3: Add A Non-Executing Sandbox Spawn-Plan API

**Files:**
- Modify: `crates/dasclaw_exec/src/lib.rs`

- [ ] **Step 1: Add failing `dasclaw_exec` tests for non-executing spawn-plan assembly**

Append these tests to the existing `#[cfg(test)] mod tests` in `crates/dasclaw_exec/src/lib.rs`:

```rust
#[test]
fn sandboxed_executor_assembles_spawn_plan_without_running_command() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = SandboxedExecutor::new(
        SandboxPolicy::new_read_only_policy(),
        SandboxablePreference::Auto,
        false,
        None,
    );
    let mut command = Command::new("sh");
    command.arg("-c").arg("printf should-not-run");

    let plan = executor
        .assemble_spawn_plan(ExecRequest {
            command,
            cwd: temp.path().to_path_buf(),
        })
        .expect("read-only spawn plan should assemble");

    assert_eq!(plan.cwd, temp.path());
    assert_eq!(plan.program, PathBuf::from("sh"));
    assert_eq!(plan.args, vec![OsString::from("-c"), OsString::from("printf should-not-run")]);
}

#[test]
fn sandboxed_executor_spawn_plan_reuses_cwd_policy_gate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let protected = temp.path().join(".git");
    std::fs::create_dir_all(&protected).expect("protected dir");
    let executor = SandboxedExecutor::new(
        SandboxPolicy::new_workspace_write_policy(),
        SandboxablePreference::Auto,
        false,
        None,
    );
    let command = Command::new("sh");

    let error = executor
        .assemble_spawn_plan(ExecRequest {
            command,
            cwd: protected,
        })
        .expect_err("spawn plan must reuse the same cwd policy gate as execute");

    assert!(format!("{error:?}").contains("CwdNotWritable"));
}
```

- [ ] **Step 2: Run the new tests to verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_exec -E 'test(sandboxed_executor_assembles_spawn_plan_without_running_command) | test(sandboxed_executor_spawn_plan_reuses_cwd_policy_gate)'
```

Expected: FAIL with an error mentioning `assemble_spawn_plan` does not exist.

- [ ] **Step 3: Add the spawn-plan type**

In `crates/dasclaw_exec/src/lib.rs`, add these imports near the existing `std` imports:

```rust
use std::ffi::OsString;
```

Add this struct after `ExecRequest`:

```rust
/// A sandbox-backed command plan suitable for process backends that need to
/// spawn the child themselves, such as PTY or pipe-streaming callers.
///
/// The plan is non-executing: assembling it must not run the user command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxSpawnPlan {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}
```

- [ ] **Step 4: Add the transform-only spawn-plan method**

Add this method inside `impl SandboxedExecutor` after `assemble_request`:

```rust
    /// Assemble a spawn plan without executing the command.
    ///
    /// This deliberately reuses [`Self::assemble_request`] so cwd policy,
    /// backend config, Linux helper wiring, and network/proxy handling stay
    /// identical to buffered `execute()`. Unlike [`Self::execute`], this method
    /// stops after exposing the raw command shape for future PTY/pipe callers.
    pub fn assemble_spawn_plan(&self, req: ExecRequest) -> Result<SandboxSpawnPlan, ExecError> {
        let exec = self.assemble_request(req)?;
        let cwd = exec
            .command
            .get_current_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let env = exec
            .command
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|value| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect::<HashMap<_, _>>();

        Ok(SandboxSpawnPlan {
            program: exec.command.get_program().to_path_buf(),
            args: exec.command.get_args().map(OsString::from).collect(),
            cwd,
            env,
        })
    }
```

- [ ] **Step 5: Keep enforcement disabled until a real platform wrapper exists**

This first implementation intentionally preserves policy gating but does not yet wrap the command with platform sandbox argv. Task 4 adds shell-tool tests that keep streaming sandbox overrides rejected until the wrapper is real.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo nextest run -p dasclaw_exec -E 'test(sandboxed_executor_assembles_spawn_plan_without_running_command) | test(sandboxed_executor_spawn_plan_reuses_cwd_policy_gate)'
```

Expected: PASS.

- [ ] **Step 7: Commit the non-executing sandbox wrapper API**

Run:

```bash
git add crates/dasclaw_exec/src/lib.rs
git commit -m "feat(exec): expose sandboxed spawn plan" -m "已检查 sandbox spawn-plan API 是否已有，结论：已有 buffered assemble_request/execute，但缺 PTY/pipe streaming 可调用的非执行 sandbox wrapper argv。"
```

Expected: commit succeeds.

## Task 4: Add Shell-Tool Launch Contract For Streaming Callers

**Files:**
- Modify: `crates/dasclaw_shell_tools/src/sandboxed_executor.rs`
- Modify: `crates/dasclaw_shell_tools/src/lib.rs`

- [ ] **Step 1: Add failing shell-tool tests**

Append these tests to `crates/dasclaw_shell_tools/src/sandboxed_executor.rs` inside the existing test module:

```rust
#[test]
fn plain_launch_spec_is_marked_unsandboxed() {
    let launch = build_shell_launch_spec("printf plain", HashMap::new());

    assert!(!launch.sandbox_enforced);
    assert_eq!(launch.program, if cfg!(target_os = "windows") { "cmd" } else { "sh" });
}

#[test]
fn sandboxed_launch_spec_refuses_full_access_without_opt_in() {
    let temp = tempfile::tempdir().expect("tempdir");
    let error = build_sandboxed_shell_launch_spec(
        "printf nope",
        temp.path(),
        CapPolicy::DangerFullAccess,
        HashMap::new(),
        false,
    )
    .expect_err("full access must require explicit opt-in");

    assert!(matches!(error, ShellExecError::FullAccessNotPermitted));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_shell_tools -E 'test(plain_launch_spec_is_marked_unsandboxed) | test(sandboxed_launch_spec_refuses_full_access_without_opt_in)'
```

Expected: FAIL with missing `sandbox_enforced` and `build_sandboxed_shell_launch_spec`.

- [ ] **Step 3: Extend `ShellLaunchSpec` and add the sandboxed builder**

In `crates/dasclaw_shell_tools/src/sandboxed_executor.rs`, change `ShellLaunchSpec` to:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub sandbox_enforced: bool,
}
```

Change `build_shell_launch_spec` to:

```rust
pub fn build_shell_launch_spec(command: &str, env: HashMap<String, String>) -> ShellLaunchSpec {
    let (program, args) = shell_program_and_args(command);
    ShellLaunchSpec {
        program,
        args,
        env,
        sandbox_enforced: false,
    }
}
```

Add this function after `build_shell_launch_spec`:

```rust
pub fn build_sandboxed_shell_launch_spec(
    command: &str,
    cwd: &Path,
    policy: CapPolicy,
    env: HashMap<String, String>,
    allow_full_access: bool,
) -> Result<ShellLaunchSpec, ShellExecError> {
    if matches!(policy, CapPolicy::DangerFullAccess) && !allow_full_access {
        return Err(ShellExecError::FullAccessNotPermitted);
    }

    let (program, args) = shell_program_and_args(command);
    let mut shell = Command::new(program);
    shell.args(args);
    shell.envs(env.clone());
    shell.current_dir(cwd);

    let executor = SandboxedExecutor::new(policy, SandboxablePreference::Auto, false, None)
        .with_linux_sandbox_exe(resolve_linux_sandbox_exe());
    let plan = executor
        .assemble_spawn_plan(ExecRequest {
            command: shell,
            cwd: cwd.to_path_buf(),
        })
        .map_err(|error| ShellExecError::ExecutionFailed(error.to_string()))?;

    Ok(ShellLaunchSpec {
        program: plan.program.to_string_lossy().into_owned(),
        args: plan
            .args
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        env: plan.env,
        sandbox_enforced: false,
    })
}
```

This keeps `sandbox_enforced: false` because Task 3 only exposes a policy-gated raw spawn plan. App-server must still reject overrides until Task 5 changes this marker to true with real platform wrapping.

- [ ] **Step 4: Re-export the new builder**

In `crates/dasclaw_shell_tools/src/lib.rs`, change the `pub use sandboxed_executor` list to include `build_sandboxed_shell_launch_spec`:

```rust
pub use sandboxed_executor::{
    ExecOutput, SandboxedShellExecutor, ShellExecError, ShellLaunchSpec, build_sandboxed_shell_launch_spec,
    build_shell_launch_spec,
};
```

- [ ] **Step 5: Run shell-tool tests**

Run:

```bash
cargo nextest run -p dasclaw_shell_tools -E 'test(plain_launch_spec_is_marked_unsandboxed) | test(sandboxed_launch_spec_refuses_full_access_without_opt_in)'
```

Expected: PASS.

- [ ] **Step 6: Commit shell-tool launch contract**

Run:

```bash
git add crates/dasclaw_shell_tools/src/sandboxed_executor.rs crates/dasclaw_shell_tools/src/lib.rs
git commit -m "feat(shell-tools): add streaming launch contract" -m "已检查 shell streaming launch helper 是否已有，结论：已有 plain launch spec，但缺可区分 sandbox enforcement 的 contract。"
```

Expected: commit succeeds.

## Task 5: Enable Streaming Sandbox Only When The Launch Contract Is Enforced

**Files:**
- Modify: `crates/dasclaw_shell_tools/src/sandboxed_executor.rs`
- Modify: `crates/dasclaw_app_server/src/command_service.rs`
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Add app-server tests for the final acceptance behavior**

In `crates/dasclaw_app_server/src/command_service.rs`, add this test next to the current fail-closed streaming sandbox tests:

```rust
#[cfg(unix)]
#[test]
fn command_service_tty_streaming_accepts_read_only_sandbox_policy_when_launch_is_enforced() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());
    let mut params = exec_params(vec!["sh", "-c", "printf sandboxed-pty"]);
    params.process_id = Some("pty_read_only_enforced".to_string());
    params.tty = Some(true);
    params.stream_stdout_stderr = Some(true);
    params.sandbox_policy = Some(serde_json::json!({"type": "read-only"}));

    let response = service.exec(params).expect("read-only sandboxed PTY streaming succeeds");

    assert_eq!(response.exit_code, 0);
}
```

- [ ] **Step 2: Run the new test to verify it fails**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(command_service_tty_streaming_accepts_read_only_sandbox_policy_when_launch_is_enforced)'
```

Expected: FAIL with `streaming command exec cannot enforce sandbox overrides`.

- [ ] **Step 3: Gate app-server acceptance on `sandbox_enforced`**

In `crates/dasclaw_app_server/src/command_service.rs`, replace the sandbox override branch in `exec_streaming` with:

```rust
        let streaming_policy = if params.sandbox_policy.is_some() || params.permission_profile.is_some() {
            let policy = resolve_policy_override(
                params.sandbox_policy.clone(),
                params.permission_profile.clone(),
                &cwd,
                COMMAND_EXEC_CAPABILITY,
            )?;
            if matches!(policy, Some(SandboxPolicy::DangerFullAccess)) {
                return Err(AppServerError::invalid_request(
                    COMMAND_EXEC_CAPABILITY,
                    "danger-full-access is not supported for streaming command exec",
                ));
            }
            policy
        } else {
            None
        };
```

Then replace the PTY launch-spec construction with:

```rust
        let launch_spec = if let Some(policy) = streaming_policy {
            let launch = dasclaw_shell_tools::build_sandboxed_shell_launch_spec(
                &command,
                &cwd,
                policy,
                env,
                false,
            )
            .map_err(map_exec_error)?;
            if !launch.sandbox_enforced {
                return Err(AppServerError::invalid_request(
                    COMMAND_EXEC_CAPABILITY,
                    STREAMING_SANDBOX_UNSUPPORTED_MESSAGE,
                ));
            }
            launch
        } else {
            build_shell_launch_spec(&command, env)
        };
```

- [ ] **Step 4: Make the shell launch contract truthfully enforced**

Change `build_sandboxed_shell_launch_spec` in `crates/dasclaw_shell_tools/src/sandboxed_executor.rs` so it returns `sandbox_enforced: true` only after `dasclaw_exec::assemble_spawn_plan` wraps the user command in a platform sandbox program. The acceptance check is:

```rust
let sandbox_enforced = plan.program != Path::new(&program);
```

Change the returned spec to:

```rust
    Ok(ShellLaunchSpec {
        program: plan.program.to_string_lossy().into_owned(),
        args: plan
            .args
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        env: plan.env,
        sandbox_enforced,
    })
```

If this still returns `false`, keep the app-server test failing and continue in `dasclaw_exec` / `dasclaw_sandbox` until `assemble_spawn_plan` returns a wrapper command for the host platform.

- [ ] **Step 5: Run the acceptance and regression tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(command_service_tty_streaming_accepts_read_only_sandbox_policy_when_launch_is_enforced) | test(command_service_tty_streaming_rejects_danger_full_access_even_after_streaming_support) | test(command_service_streaming_control_audit_redacts_payload_but_records_outcome)'
cargo nextest run -p dasclaw_shell_tools -E 'test(plain_launch_spec_is_marked_unsandboxed) | test(sandboxed_launch_spec_refuses_full_access_without_opt_in)'
```

Expected: PASS. If `command_service_tty_streaming_accepts_read_only_sandbox_policy_when_launch_is_enforced` fails because `sandbox_enforced` is false, do not update the matrix; the platform wrapper is still missing.

- [ ] **Step 6: Update the matrix only after Step 5 passes**

In `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`, replace the §9.2 P0 streaming bullet with:

```markdown
- runtime sandbox context enforce 从“协议参数面”推进到真实 turn execution enforce
- 更完整 command / filesystem 安全审计面
```

Do not remove runtime sandbox context enforce or broader audit items in this task.

- [ ] **Step 7: Commit streaming sandbox enforcement**

Run:

```bash
git add crates/dasclaw_shell_tools/src/sandboxed_executor.rs crates/dasclaw_app_server/src/command_service.rs docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "feat(app-server): enforce sandboxed streaming command launch" -m "已检查 streaming sandbox enforcement 是否已有，结论：此前只有 fail-closed guard；本提交只在 launch contract 证明 sandbox_enforced=true 后放行。"
```

Expected: commit succeeds.

## Task 6: Final Verification

**Files:**
- No code changes unless verification exposes a defect.

- [ ] **Step 1: Run focused app-server verification**

Run:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(command_service_tty_streaming_accepts_read_only_sandbox_policy_when_launch_is_enforced) | test(command_service_tty_streaming_rejects_danger_full_access_even_after_streaming_support) | test(command_service_streaming_control_audit_redacts_payload_but_records_outcome) | test(json_rpc_thread_shell_command_returns_empty_object_and_emits_turn_notifications) | test(json_rpc_thread_approve_guardian_denied_action_replays_stashed_command_action) | test(auto_approval_review_completed_stashes_typed_guardian_event_for_replay) | test(thread_compact_start_with_real_runtime_persists_deterministic_compaction_turn)'
```

Expected: PASS.

- [ ] **Step 2: Run crate checks**

Run:

```bash
cargo check -p dasclaw_exec --tests
cargo check -p dasclaw_shell_tools --tests
cargo check -p dasclaw_app_server --tests
```

Expected: all three commands finish with 0 errors.

- [ ] **Step 3: Run formatting and panic scan**

Run:

```bash
cargo fmt --all --check
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: formatting is clean and `check_no_panics.py` reports no newly introduced panics. If `python3.12` is unavailable, run:

```bash
python3 scripts/check_no_panics.py --base origin/xClaw
```

Expected: either the script passes or the output proves the local `python3` version cannot parse the script; record that environment gap in the final handoff.

- [ ] **Step 4: Run clippy for touched crates**

Run:

```bash
cargo clippy --no-deps -p dasclaw_exec -p dasclaw_shell_tools -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: 0 warnings from the touched crates.

- [ ] **Step 5: Commit final verification notes if docs changed**

Run:

```bash
git status --short
```

Expected: no uncommitted code changes. If only docs verification notes changed, commit them with:

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md docs/superpowers/plans/2026-06-25-dasclaw-app-server-p0-p1-remaining.md docs/superpowers/plans/2026-06-26-dasclaw-app-server-enforce-and-status-rebaseline.md
git commit -m "docs: record app-server enforce verification" -m "已检查 app-server enforce 状态是否已有，结论：streaming sandbox、runtime sandbox context 与 audit 状态按最新验证记录。"
```

Expected: commit succeeds or there are no docs changes to commit.

## Self-Review

Spec coverage:

- Old P1 route work is explicitly excluded and protected by focused verification.
- Streaming sandbox is the only code path allowed to move from fail-closed to accepted behavior.
- Compact status is handled as a documentation/status rebaseline unless fresh tests fail.
- Runtime sandbox context enforce and broader command/filesystem audit remain visible after streaming sandbox lands.

Placeholder scan:

- Every task step has concrete commands, code snippets, or exact replacement text.
- The plan does not rely on vague future-work markers for acceptance criteria.

Type consistency:

- `ShellLaunchSpec.sandbox_enforced` is introduced before app-server checks it.
- `build_sandboxed_shell_launch_spec` is re-exported before app-server calls it.
- `SandboxSpawnPlan` is introduced before shell-tools consumes `assemble_spawn_plan`.
