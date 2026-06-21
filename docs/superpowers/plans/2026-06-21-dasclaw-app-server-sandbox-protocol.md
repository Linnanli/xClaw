# Dasclaw App Server Sandbox Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐 Dasclaw app-server 里 Codex 风格沙箱协议的缺口：`command/exec.sandboxPolicy` 不再被拒绝、`command/exec.permissionProfile` 进入参数面、`thread/start` 与 `turn/start` 接受沙箱 override、`configRequirements/read.allowedSandboxModes` 暴露当前策略约束。

**Architecture:** 保持 `dasclaw_app_server_protocol` 为轻量 wire crate，只建模 app-server JSON-RPC 边界；在 `dasclaw_app_server` 内新增一个薄桥接层，把 wire JSON 解析到已有 `dasclaw_protocol::{protocol::SandboxPolicy, models::PermissionProfile}`，再转换到 `dasclaw_workspace_cap::policy::SandboxPolicy` 供命令执行使用。默认仍 fail-closed：未指定 override 时沿用只读禁网，`DangerFullAccess` 即使被成功解析也继续被 `SandboxedShellExecutor::new(timeout, false, None)` 的双 opt-in 拒绝。

**Tech Stack:** Rust 2024, JSON-RPC, `serde`, `serde_json`, `dasclaw_app_server_protocol`, `dasclaw_protocol`, `dasclaw_workspace_cap`, `dasclaw_shell_tools::SandboxedShellExecutor`, `cargo nextest`, `cargo fmt`, `scripts/check_no_panics.py`.

---

## Evidence And Scope

4-question gate:

| Question | Answer | Evidence |
|---|---|---|
| 是否新增模块 / crate / 文件？ | Yes. Implementation creates one app-server helper module and this plan document. | Semantic search before planning found reusable `dasclaw_protocol::models::PermissionProfile` conversions and `dasclaw_workspace_cap::policy::SandboxPolicy`; no app-server owner for `configRequirements/read` or permission profile command params was found. |
| 结论是否含否定语？ | Yes. This plan says specific protocol fields/routes are currently missing or rejected. | Exact `rg`: `crates/dasclaw_app_server_protocol/src/lib.rs:1549` `ThreadStartParams` only has `cwd`; `:1599` `TurnStartParams` lacks sandbox fields; `:2071` `CommandExecParams` has only `sandbox_policy: Option<Value>`; `crates/dasclaw_app_server/src/command_service.rs:287` and `:340` reject `sandboxPolicy`; `configRequirements/read` is absent from method constants and routes. |
| 是否做跨项目对账？ | Yes. The target shape follows Codex app-server protocol fields already recorded in the gap matrix. | `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md:186`, `:209`, `:212`, `:214`, `:284`, `:286` now list the precise sandbox protocol sub-gaps. |
| 是否写架构对账类文档？ | Yes. This is an implementation plan for a protocol gap. | Level 1 semantic search ran twice in hybrid mode; Level 3 exact `rg` evidence is recorded above. Level 2 LSP was not exposed by the current tool list; when implementing in a session with `execute_lsp`, run references on `ThreadStartParams`, `TurnStartParams`, `CommandExecParams`, and `RuntimeTurnStartRequest` before editing. |

Reference facts:

- Lower-level policy already exists:
  - `crates/dasclaw_workspace_cap/src/policy.rs:67` defines the Codex-compatible four-tier `SandboxPolicy`.
  - `crates/dasclaw_protocol/src/protocol.rs:1033` defines the vendored Codex `SandboxPolicy`.
  - `crates/dasclaw_protocol/src/models.rs:360` defines `PermissionProfile`, with `to_legacy_sandbox_policy(cwd)` at `:446`.
  - `crates/dasclaw_protocol/src/config_types.rs:66` defines `SandboxMode`.
- Command execution already has an enforcement point:
  - `crates/dasclaw_shell_tools/src/sandboxed_executor.rs:93` accepts a `dasclaw_workspace_cap::policy::SandboxPolicy`.
  - `crates/dasclaw_app_server/src/command_service.rs:517` always passes `SandboxPolicy::new_read_only_policy()`.
- The app-server protocol surface is the missing layer:
  - `command/exec.sandboxPolicy` exists as raw JSON but is rejected by buffered and PTY paths.
  - `command/exec.permissionProfile`, `thread/start.sandbox`, `thread/start.permissionProfile`, `turn/start.sandboxPolicy`, `turn/start.permissionProfile`, and `configRequirements/read.allowedSandboxModes` are not modeled.

Scope:

- Accept and validate the Codex wire fields listed above.
- Enforce per-command override for buffered `command/exec`.
- Preserve PTY streaming fail-safe for sandbox override in this slice, because PTY currently advertises `streaming_sandbox=none/unsandboxed`; return a clear capability-unavailable error if a streaming command supplies `sandboxPolicy` or `permissionProfile`.
- Store and pass thread/turn sandbox context to the runtime bridge request so runtime implementations can consume the policy context without re-reading JSON-RPC params.
- Add `configRequirements/read` with `allowedSandboxModes` reflecting app-server policy constraints.
- Update the gap matrix only for fields/routes proven by tests.

Out of scope:

- `windowsSandbox/setupStart` / `windowsSandbox/setupCompleted`.
- A three-platform public sandbox manager API or policy negotiation API beyond `configRequirements/read.allowedSandboxModes`.
- Sandboxed PTY execution and non-PTY split stdout/stderr streaming.
- Product config write APIs such as `config/read`, `config/value/write`, or `config/batchWrite`.

Commit message discipline for this task family:

```text
已检查 sandbox protocol 是否已有，结论：底层 SandboxPolicy/PermissionProfile 与 shell sandbox executor 已有，app-server 仍缺 thread/turn/command 参数面、configRequirements/read.allowedSandboxModes 与 per-command override 接线。
```

## File Structure

- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Add `configRequirements/read` method constant.
  - Add app-server wire DTOs for `SandboxMode`, `ConfigRequirementsReadResponse`, and sandbox fields on thread/turn/command params.
  - Update protocol schema method list and schema tests.

- Modify: `crates/dasclaw_app_server/Cargo.toml`
  - Add `dasclaw_protocol = { path = "../dasclaw_protocol" }`.

- Add: `crates/dasclaw_app_server/src/sandbox_protocol.rs`
  - Parse and validate `sandboxPolicy` / `permissionProfile`.
  - Convert Codex wire policy into `dasclaw_workspace_cap::policy::SandboxPolicy`.
  - Build runtime-visible sandbox context for thread and turn start.

- Modify: `crates/dasclaw_app_server/src/lib.rs`
  - Route `configRequirements/read`.
  - Thread/turn start validate and preserve sandbox context.
  - Extend `RuntimeTurnStartRequest` with resolved sandbox context.
  - Update `supported_methods()` and route/schema parity tests.

- Modify: `crates/dasclaw_app_server/src/command_service.rs`
  - Remove buffered `sandboxPolicy` rejection.
  - Add `permissionProfile` rejection/acceptance rules.
  - Resolve buffered command policy through the new helper.
  - Keep streaming sandbox override fail-closed until sandboxed PTY exists.

- Modify: `crates/dasclaw_app_server/src/main.rs`
  - Update test bridge struct initializers and add stdio route coverage for `configRequirements/read`.

- Modify: `crates/dasclaw_app_server_client/src/lib.rs`
  - Update dev-test bridge request assertions if `RuntimeTurnStartRequest` field additions affect tests.

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Strike only the app-server sandbox protocol sub-items implemented and verified by this work.

## Task 1: Protocol DTOs And Schema

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Add failing protocol shape tests**

Add tests near the existing command/fs protocol tests:

```rust
#[test]
fn sandbox_protocol_params_match_codex_wire_shape() {
    let thread: ThreadStartParams = serde_json::from_value(serde_json::json!({
        "cwd": "/repo",
        "sandbox": "workspace-write",
        "permissionProfile": {
            "type": "managed",
            "file_system": {
                "type": "restricted",
                "entries": []
            },
            "network": "restricted"
        }
    }))
    .expect("thread/start should accept sandbox fields");

    assert_eq!(thread.sandbox, Some(SandboxMode::WorkspaceWrite));
    assert!(thread.permission_profile.is_some());

    let turn: TurnStartParams = serde_json::from_value(serde_json::json!({
        "threadId": "thread_1",
        "input": [{ "type": "text", "text": "hi" }],
        "sandboxPolicy": { "type": "read-only", "network_access": false },
        "permissionProfile": { "type": "disabled" }
    }))
    .expect("turn/start should accept sandbox fields");

    assert_eq!(turn.sandbox_policy.as_ref().unwrap()["type"], "read-only");
    assert_eq!(turn.permission_profile.as_ref().unwrap()["type"], "disabled");

    let command: CommandExecParams = serde_json::from_value(serde_json::json!({
        "command": ["sh", "-c", "printf hi"],
        "sandboxPolicy": { "type": "workspace-write" },
        "permissionProfile": { "type": "disabled" }
    }))
    .expect("command/exec should accept permissionProfile");

    assert!(command.sandbox_policy.is_some());
    assert!(command.permission_profile.is_some());
}

#[test]
fn config_requirements_read_is_advertised_in_schema() {
    let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
    assert!(
        schema
            .methods
            .iter()
            .any(|method| method.method == method::CONFIG_REQUIREMENTS_READ),
        "protocol/schema must include configRequirements/read"
    );
}
```

Run and confirm the tests fail for the expected missing fields/method:

```bash
cargo nextest run -p dasclaw_app_server_protocol sandbox_protocol_params_match_codex_wire_shape config_requirements_read_is_advertised_in_schema
```

- [ ] **Step 2: Add protocol constants and DTO fields**

Add the method constant:

```rust
pub mod method {
    pub const CONFIG_REQUIREMENTS_READ: &str = "configRequirements/read";
}
```

Add a lightweight wire enum in the protocol crate instead of depending on the heavier vendored protocol crate:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequirementsReadResponse {
    pub allowed_sandbox_modes: Vec<SandboxMode>,
}
```

Extend params without changing existing field names:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadStartParams {
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecParams {
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
}
```

The `CommandExecParams` snippet shows only the new field location; preserve all existing fields.

- [ ] **Step 3: Advertise the method in schema and capabilities**

Add `configRequirements/read` to `phase_one_methods()`:

```rust
MethodSchema::new(
    method::CONFIG_REQUIREMENTS_READ,
    "config",
    None,
    "ConfigRequirementsReadResponse",
    true,
),
```

Keep the existing `CapabilityMatrix` fields unchanged for this slice. The schema method entry is enough for routability; a larger config capability can be introduced with the product config APIs.

- [ ] **Step 4: Update protocol struct literals**

Update existing `CommandExecParams`, `ThreadStartParams`, and `TurnStartParams` literals in protocol tests by adding:

```rust
sandbox: None,
permission_profile: None,
```

or:

```rust
sandbox_policy: None,
permission_profile: None,
```

as appropriate for the struct.

- [ ] **Step 5: Verify protocol crate**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol sandbox_protocol_params_match_codex_wire_shape config_requirements_read_is_advertised_in_schema
cargo check -p dasclaw_app_server_protocol --tests
```

## Task 2: Sandbox Protocol Bridge

**Files:**
- Modify: `crates/dasclaw_app_server/Cargo.toml`
- Add: `crates/dasclaw_app_server/src/sandbox_protocol.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Add dependency and module**

Add to `crates/dasclaw_app_server/Cargo.toml`:

```toml
dasclaw_protocol = { path = "../dasclaw_protocol" }
```

Register the module in `crates/dasclaw_app_server/src/lib.rs`:

```rust
mod sandbox_protocol;
```

- [ ] **Step 2: Add bridge types and conversion tests**

Create `crates/dasclaw_app_server/src/sandbox_protocol.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use dasclaw_workspace_cap::policy::SandboxPolicy;
    use serde_json::json;

    use super::resolve_policy_override;

    #[test]
    fn resolves_command_sandbox_policy_json_to_workspace_policy() {
        let policy = resolve_policy_override(
            Some(json!({
                "type": "workspace-write",
                "writable_roots": ["/tmp/extra"],
                "network_access": true,
                "exclude_tmpdir_env_var": true,
                "exclude_slash_tmp": true
            })),
            None,
            Path::new("/repo"),
            "command/exec",
        )
        .expect("resolve")
        .expect("policy");

        assert_eq!(
            policy,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![std::path::PathBuf::from("/tmp/extra")],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }

    #[test]
    fn rejects_ambiguous_policy_and_permission_profile() {
        let error = resolve_policy_override(
            Some(json!({ "type": "read-only" })),
            Some(json!({ "type": "disabled" })),
            Path::new("/repo"),
            "command/exec",
        )
        .expect_err("ambiguous override");

        assert!(
            error.public_message().contains("choose either sandboxPolicy or permissionProfile"),
            "{error:?}"
        );
    }
}
```

`sandbox_protocol.rs` is a child module of `lib.rs`, so it can call the crate-root private `AppServerError::public_message()` helper in its unit tests.

- [ ] **Step 3: Implement bridge functions**

Add the bridge implementation:

```rust
use std::path::Path;

use dasclaw_app_server_protocol::SandboxMode;
use dasclaw_protocol::models::PermissionProfile;
use dasclaw_protocol::protocol as codex_protocol;
use dasclaw_workspace_cap::policy::{
    NetworkAccess as WorkspaceNetworkAccess, SandboxPolicy as WorkspaceSandboxPolicy,
};
use serde_json::Value;

use crate::AppServerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSandboxContext {
    pub sandbox: Option<SandboxMode>,
    pub policy: Option<WorkspaceSandboxPolicy>,
}

impl RuntimeSandboxContext {
    pub fn empty() -> Self {
        Self {
            sandbox: None,
            policy: None,
        }
    }
}

pub fn resolve_thread_context(
    sandbox: Option<SandboxMode>,
    permission_profile: Option<Value>,
    cwd: &Path,
    capability: &'static str,
) -> Result<RuntimeSandboxContext, AppServerError> {
    if sandbox.is_some() && permission_profile.is_some() {
        return Err(AppServerError::invalid_request(
            capability,
            "choose either sandbox or permissionProfile, not both",
        ));
    }

    let policy = match (sandbox, permission_profile) {
        (Some(mode), None) => Some(policy_from_mode(mode)),
        (None, Some(value)) => Some(policy_from_permission_profile(value, cwd, capability)?),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("checked above"),
    };

    Ok(RuntimeSandboxContext { sandbox, policy })
}

pub fn resolve_policy_override(
    sandbox_policy: Option<Value>,
    permission_profile: Option<Value>,
    cwd: &Path,
    capability: &'static str,
) -> Result<Option<WorkspaceSandboxPolicy>, AppServerError> {
    if sandbox_policy.is_some() && permission_profile.is_some() {
        return Err(AppServerError::invalid_request(
            capability,
            "choose either sandboxPolicy or permissionProfile, not both",
        ));
    }

    if let Some(value) = sandbox_policy {
        let policy = serde_json::from_value::<codex_protocol::SandboxPolicy>(value).map_err(
            |error| {
                AppServerError::invalid_request(
                    capability,
                    format!("invalid sandboxPolicy: {error}"),
                )
            },
        )?;
        return Ok(Some(workspace_policy_from_codex(policy)));
    }

    if let Some(value) = permission_profile {
        return Ok(Some(policy_from_permission_profile(value, cwd, capability)?));
    }

    Ok(None)
}

fn policy_from_mode(mode: SandboxMode) -> WorkspaceSandboxPolicy {
    match mode {
        SandboxMode::ReadOnly => WorkspaceSandboxPolicy::new_read_only_policy(),
        SandboxMode::WorkspaceWrite => WorkspaceSandboxPolicy::new_workspace_write_policy(),
        SandboxMode::DangerFullAccess => WorkspaceSandboxPolicy::DangerFullAccess,
    }
}

fn policy_from_permission_profile(
    value: Value,
    cwd: &Path,
    capability: &'static str,
) -> Result<WorkspaceSandboxPolicy, AppServerError> {
    let profile = serde_json::from_value::<PermissionProfile>(value).map_err(|error| {
        AppServerError::invalid_request(capability, format!("invalid permissionProfile: {error}"))
    })?;
    let legacy = profile.to_legacy_sandbox_policy(cwd).map_err(|error| {
        AppServerError::invalid_request(
            capability,
            format!("invalid permissionProfile path policy: {error}"),
        )
    })?;
    Ok(workspace_policy_from_codex(legacy))
}

fn workspace_policy_from_codex(
    policy: codex_protocol::SandboxPolicy,
) -> WorkspaceSandboxPolicy {
    match policy {
        codex_protocol::SandboxPolicy::DangerFullAccess => {
            WorkspaceSandboxPolicy::DangerFullAccess
        }
        codex_protocol::SandboxPolicy::ReadOnly { network_access } => {
            WorkspaceSandboxPolicy::ReadOnly { network_access }
        }
        codex_protocol::SandboxPolicy::ExternalSandbox { network_access } => {
            WorkspaceSandboxPolicy::ExternalSandbox {
                network_access: match network_access {
                    codex_protocol::NetworkAccess::Restricted => {
                        WorkspaceNetworkAccess::Restricted
                    }
                    codex_protocol::NetworkAccess::Enabled => WorkspaceNetworkAccess::Enabled,
                },
            }
        }
        codex_protocol::SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => WorkspaceSandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots
                .into_iter()
                .map(Into::into)
                .collect(),
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        },
    }
}
```

- [ ] **Step 4: Verify bridge tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server sandbox_protocol
cargo check -p dasclaw_app_server --tests
```

## Task 3: `configRequirements/read`

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/main.rs`

- [ ] **Step 1: Add route tests**

Add a JSON-RPC route test near other method route tests:

```rust
#[test]
fn json_rpc_config_requirements_read_returns_allowed_sandbox_modes() {
    let mut server = initialized_server();
    let response = server
        .handle_json_rpc_line(&format!(
            r#"{{"jsonrpc":"2.0","id":"cfg","method":"{}"}}"#,
            method::CONFIG_REQUIREMENTS_READ
        ))
        .expect("route config requirements");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json response");

    assert_eq!(value["result"]["allowedSandboxModes"][0], "read-only");
    assert_eq!(value["result"]["allowedSandboxModes"][1], "workspace-write");
    assert_eq!(
        value["result"]["allowedSandboxModes"]
            .as_array()
            .expect("modes")
            .len(),
        2
    );
}
```

Add the method to the existing routability tests that compare `protocol/schema` and `supported_methods()`.

- [ ] **Step 2: Implement route and response**

Import `ConfigRequirementsReadResponse` and `SandboxMode`, then add:

```rust
pub fn config_requirements_read(&self) -> Result<ConfigRequirementsReadResponse, AppServerError> {
    self.require_initialized("config")?;
    Ok(ConfigRequirementsReadResponse {
        allowed_sandbox_modes: vec![SandboxMode::ReadOnly, SandboxMode::WorkspaceWrite],
    })
}
```

Add route:

```rust
method::CONFIG_REQUIREMENTS_READ => {
    route_with_no_params(request.id, request.params, || self.config_requirements_read())
}
```

Add method to `supported_methods()`:

```rust
method::CONFIG_REQUIREMENTS_READ,
```

The response intentionally excludes `danger-full-access`, because command execution currently constructs `SandboxedShellExecutor::new(timeout, false, None)` and refuses host full access without a second local opt-in.

- [ ] **Step 3: Verify route**

Run:

```bash
cargo nextest run -p dasclaw_app_server json_rpc_config_requirements_read_returns_allowed_sandbox_modes protocol_schema_methods_are_all_routable
```

## Task 4: Buffered `command/exec` Sandbox Override

**Files:**
- Modify: `crates/dasclaw_app_server/src/command_service.rs`

- [ ] **Step 1: Add failing command service tests**

Replace the existing `command_service_rejects_unsupported_exec_semantics` assertion for buffered `sandboxPolicy` with tests that prove acceptance and fail-closed behavior:

```rust
#[test]
fn command_service_accepts_buffered_read_only_sandbox_policy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());

    let mut params = exec_params(vec!["rustc", "--version"]);
    params.sandbox_policy = Some(serde_json::json!({
        "type": "read-only",
        "network_access": false
    }));

    let response = service.exec(params).expect("read-only sandbox command");
    assert_eq!(response.exit_code, 0);
}

#[test]
fn command_service_rejects_ambiguous_sandbox_policy_and_permission_profile() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());

    let mut params = exec_params(vec!["rustc", "--version"]);
    params.sandbox_policy = Some(serde_json::json!({ "type": "read-only" }));
    params.permission_profile = Some(serde_json::json!({ "type": "disabled" }));

    let error = service.exec(params).expect_err("ambiguous override");
    assert!(
        format!("{error:?}").contains("choose either sandboxPolicy or permissionProfile"),
        "{error:?}"
    );
}

#[test]
fn command_service_keeps_danger_full_access_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());

    let mut params = exec_params(vec!["rustc", "--version"]);
    params.sandbox_policy = Some(serde_json::json!({
        "type": "danger-full-access"
    }));

    let error = service.exec(params).expect_err("danger full access must be refused");
    assert!(
        format!("{error:?}").contains("FullAccess") ||
            format!("{error:?}").contains("full access"),
        "{error:?}"
    );
}
```

Add a streaming rejection test for the still-unsandboxed PTY path:

```rust
#[test]
fn streaming_command_rejects_sandbox_override_until_pty_is_sandboxed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());

    let mut params = exec_params(vec!["sh", "-c", "printf hi"]);
    params.process_id = Some("proc_sandbox".to_string());
    params.tty = Some(true);
    params.sandbox_policy = Some(serde_json::json!({ "type": "read-only" }));

    let error = service.exec(params).expect_err("streaming sandbox unavailable");
    assert!(
        format!("{error:?}").contains("sandboxPolicy is not supported by PTY streaming"),
        "{error:?}"
    );
}
```

- [ ] **Step 2: Wire buffered policy resolution**

Import the helper:

```rust
use crate::sandbox_protocol::resolve_policy_override;
```

Update unsupported-option validation:

```rust
fn reject_unsupported_buffered_exec_options(
    params: &CommandExecParams,
) -> Result<(), AppServerError> {
    if params.tty.unwrap_or(false) {
        return Err(command_unavailable("tty requires sandboxed PTY support"));
    }
    if params.stream_stdin.unwrap_or(false) {
        return Err(command_unavailable(
            "streamStdin requires streaming command support",
        ));
    }
    if params.stream_stdout_stderr.unwrap_or(false) {
        return Err(command_unavailable(
            "streamStdoutStderr requires streaming command support",
        ));
    }
    if params.size.is_some() {
        return Err(command_unavailable("size requires tty support"));
    }
    if params.disable_timeout.unwrap_or(false) {
        return Err(command_unavailable(
            "disableTimeout is not supported by the buffered sandbox executor",
        ));
    }
    if params.disable_output_cap.unwrap_or(false) {
        return Err(command_unavailable(
            "disableOutputCap is not supported by the buffered sandbox executor",
        ));
    }
    Ok(())
}
```

Resolve policy before entering the async block:

```rust
let policy = resolve_policy_override(
    params.sandbox_policy,
    params.permission_profile,
    &cwd,
    COMMAND_EXEC_CAPABILITY,
)?
.unwrap_or_else(SandboxPolicy::new_read_only_policy);
```

Pass `policy` to the executor:

```rust
let output = executor
    .execute(&command, &cwd, policy, env)
    .await
    .map_err(map_exec_error)?;
```

- [ ] **Step 3: Keep PTY override fail-closed**

Extend streaming validation:

```rust
if params.sandbox_policy.is_some() || params.permission_profile.is_some() {
    return Err(command_unavailable(
        "sandboxPolicy is not supported by PTY streaming command exec until sandboxed PTY is implemented",
    ));
}
```

Update command service health text from:

```text
buffered_sandbox=read-only/no-network streaming_sandbox=none/unsandboxed
```

to:

```text
buffered_sandbox=policy-override/default-read-only streaming_sandbox=none/unsandboxed
```

- [ ] **Step 4: Verify command service**

Run:

```bash
cargo nextest run -p dasclaw_app_server command_service_accepts_buffered_read_only_sandbox_policy command_service_rejects_ambiguous_sandbox_policy_and_permission_profile command_service_keeps_danger_full_access_fail_closed streaming_command_rejects_sandbox_override_until_pty_is_sandboxed
cargo check -p dasclaw_app_server --tests
```

## Task 5: Thread And Turn Sandbox Context

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/main.rs`
- Modify: `crates/dasclaw_app_server_client/src/lib.rs`

- [ ] **Step 1: Add runtime bridge observation tests**

Add tests that prove `thread/start` default context is inherited by `turn/start`, and that `turn/start` overrides it:

```rust
#[test]
fn turn_start_passes_thread_sandbox_context_to_runtime_bridge() {
    let runtime = Arc::new(RecordingRuntimeBridge::default());
    let mut server = initialized_server_with_runtime(runtime.clone());

    let thread = server
        .thread_start(ThreadStartParams {
            cwd: Some("/tmp".to_string()),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            permission_profile: None,
        })
        .expect("thread start");

    server
        .turn_start(TurnStartParams {
            thread_id: thread.thread.id,
            input: vec![UserInput::Text {
                text: "hi".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: None,
            model: None,
            summary: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .expect("turn start");

    let calls = runtime.calls.lock().expect("calls");
    assert_eq!(
        calls[0].sandbox_context.sandbox,
        Some(SandboxMode::WorkspaceWrite)
    );
}

#[test]
fn turn_start_policy_overrides_thread_sandbox_context() {
    let runtime = Arc::new(RecordingRuntimeBridge::default());
    let mut server = initialized_server_with_runtime(runtime.clone());

    let thread = server
        .thread_start(ThreadStartParams {
            cwd: Some("/tmp".to_string()),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            permission_profile: None,
        })
        .expect("thread start");

    server
        .turn_start(TurnStartParams {
            thread_id: thread.thread.id,
            input: vec![UserInput::Text {
                text: "hi".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: None,
            model: None,
            summary: None,
            sandbox_policy: Some(serde_json::json!({ "type": "read-only" })),
            permission_profile: None,
        })
        .expect("turn start");

    let calls = runtime.calls.lock().expect("calls");
    assert_eq!(
        calls[0].sandbox_context.policy,
        Some(dasclaw_workspace_cap::policy::SandboxPolicy::new_read_only_policy())
    );
}
```

Use the repository's existing test helper names if they differ; the important assertion target is `RuntimeTurnStartRequest`.

- [ ] **Step 2: Store thread sandbox context**

Extend `ThreadRecord` and `ThreadSummary`:

```rust
pub struct ThreadRecord {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
    pub sandbox_context: RuntimeSandboxContext,
}

pub struct ThreadSummary {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
    pub sandbox_context: RuntimeSandboxContext,
}
```

Change `SessionThreadHost::create` from `fn create(&mut self, params: ThreadStartParams) -> String` to `fn create(&mut self, params: ThreadStartParams) -> Result<String, AppServerError>`, then resolve context before pushing:

```rust
let cwd = params
    .cwd
    .as_deref()
    .map(std::path::Path::new)
    .unwrap_or_else(|| std::path::Path::new("."));
let sandbox_context = crate::sandbox_protocol::resolve_thread_context(
    params.sandbox,
    params.permission_profile,
    cwd,
    "thread/start",
)?;
```

Update `create_thread_record` to call `let thread_id = self.threads.create(params)?;` before emitting `thread/started`.

- [ ] **Step 3: Add context to runtime turn request**

Extend `RuntimeTurnStartRequest`:

```rust
#[derive(Debug, Clone)]
pub struct RuntimeTurnStartRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub prompt: String,
    pub model_provider: RuntimeModelProviderSnapshot,
    pub reasoning_summary: ReasoningSummary,
    pub sandbox_context: RuntimeSandboxContext,
    pub updates: RuntimeTurnUpdateSink,
}
```

In `turn_start`, compute the effective context:

```rust
let thread_summary = self
    .threads
    .summary(&params.thread_id)
    .ok_or_else(|| AppServerError::invalid_request("session", "thread not found"))?;
let turn_cwd = params
    .cwd
    .as_deref()
    .or(thread_summary.workspace_root.as_deref())
    .map(std::path::Path::new)
    .unwrap_or_else(|| std::path::Path::new("."));
let turn_policy = crate::sandbox_protocol::resolve_policy_override(
    params.sandbox_policy,
    params.permission_profile,
    turn_cwd,
    "turn/start",
)?;
let sandbox_context = match turn_policy {
    Some(policy) => RuntimeSandboxContext {
        sandbox: None,
        policy: Some(policy),
    },
    None => thread_summary.sandbox_context,
};
```

Pass `sandbox_context` into `RuntimeTurnStartRequest`.

- [ ] **Step 4: Update all direct struct initializers**

Use `rg` to find direct initializers:

```bash
rg -n "RuntimeTurnStartRequest \\{" crates/dasclaw_app_server crates/dasclaw_app_server_client
rg -n "ThreadStartParams \\{" crates/dasclaw_app_server crates/dasclaw_app_server_protocol crates/dasclaw_app_server_client
rg -n "TurnStartParams \\{" crates/dasclaw_app_server crates/dasclaw_app_server_protocol crates/dasclaw_app_server_client
rg -n "CommandExecParams \\{" crates/dasclaw_app_server crates/dasclaw_app_server_protocol crates/dasclaw_app_server_client
```

For test-only runtime request construction, add:

```rust
sandbox_context: RuntimeSandboxContext::empty(),
```

For params construction, add `sandbox: None`, `sandbox_policy: None`, and `permission_profile: None` according to the struct.

- [ ] **Step 5: Verify thread/turn route behavior**

Run:

```bash
cargo nextest run -p dasclaw_app_server turn_start_passes_thread_sandbox_context_to_runtime_bridge turn_start_policy_overrides_thread_sandbox_context
cargo nextest run -p dasclaw_app_server_client
cargo check -p dasclaw_app_server --tests
cargo check -p dasclaw_app_server_client --tests
```

## Task 6: Documentation And Final Verification

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Update the gap matrix honestly**

After tests pass, update the sandbox rows to say:

- `command/exec.sandboxPolicy` is supported for buffered `command/exec`.
- `command/exec.permissionProfile` is modeled and supported for buffered `command/exec`.
- PTY streaming still rejects sandbox override until sandboxed PTY exists.
- `thread/start.sandbox`, `thread/start.permissionProfile`, `turn/start.sandboxPolicy`, and `turn/start.permissionProfile` are accepted and passed as runtime sandbox context.
- `configRequirements/read.allowedSandboxModes` is implemented with `read-only` and `workspace-write`; `danger-full-access` is intentionally excluded by policy.
- Windows setup remains unimplemented and separate.

Do not mark sandboxed PTY, Windows setup, non-PTY pipe streaming, or broad config APIs as complete.

- [ ] **Step 2: Run targeted verification**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol sandbox_protocol_params_match_codex_wire_shape config_requirements_read_is_advertised_in_schema
cargo nextest run -p dasclaw_app_server sandbox_protocol
cargo nextest run -p dasclaw_app_server command_service_accepts_buffered_read_only_sandbox_policy command_service_rejects_ambiguous_sandbox_policy_and_permission_profile command_service_keeps_danger_full_access_fail_closed streaming_command_rejects_sandbox_override_until_pty_is_sandboxed
cargo nextest run -p dasclaw_app_server json_rpc_config_requirements_read_returns_allowed_sandbox_modes turn_start_passes_thread_sandbox_context_to_runtime_bridge turn_start_policy_overrides_thread_sandbox_context
cargo nextest run -p dasclaw_app_server_client
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo check -p dasclaw_app_server_client --tests
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
```

- [ ] **Step 3: Run mandatory local review passes**

Because this touches `crates/dasclaw_*`, run the repo-required checks before pushing:

```bash
cargo clippy --no-deps -p dasclaw_app_server_protocol -p dasclaw_app_server -p dasclaw_app_server_client --all-targets -- -D warnings
```

Then run the required skills/checks for a non-trivial Rust change in this repository:

- `code-quality-audit`
- `code-simplifier`
- `adr-compliance-check`
- `code-review-expert`

For `adr-compliance-check`, cite that the implementation imports `dasclaw_protocol` as an existing vendored protocol source and does not edit vendored `crates/dasclaw_protocol/src/*.rs`.

- [ ] **Step 4: Final manual inspection**

Run:

```bash
rg -n "sandboxPolicy is not supported by the buffered|permissionProfile|configRequirements/read|allowedSandboxModes|RuntimeSandboxContext" crates/dasclaw_app_server crates/dasclaw_app_server_protocol docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

Expected final state:

- No buffered `sandboxPolicy is not supported` rejection remains.
- PTY streaming still has an explicit sandbox override rejection.
- `permissionProfile` appears in protocol DTOs, command service, and thread/turn handling.
- `configRequirements/read` appears in method constants, schema, supported methods, and route handling.
- Gap matrix distinguishes implemented buffered command policy support from remaining sandboxed PTY and Windows setup gaps.
