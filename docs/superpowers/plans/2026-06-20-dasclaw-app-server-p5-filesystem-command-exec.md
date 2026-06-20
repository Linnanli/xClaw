# Dasclaw App Server P5 Filesystem And Command Exec Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the P5 app-server filesystem and local command execution control-plane slice from `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`.

**Architecture:** Add explicit P5 protocol DTOs and service-owner traits, then wire real app-server services through `AppServerServices::real()` the same way P4 wired logs/jobs/skills/MCP. Filesystem operations reuse `dasclaw_fs_tools` path validation and std fs primitives; command execution starts with safe buffered execution plus honest capability gates for PTY-only follow-up methods until a sandboxed PTY owner is available.

**Tech Stack:** Rust 2024, JSON-RPC, `serde`, `base64`, `dasclaw_fs_tools`, `dasclaw_shell_tools`, `dasclaw_workspace_cap`, `cargo nextest`, existing app-server stdio tests. `dasclaw_pty` remains a follow-up dependency only if a sandboxed PTY owner is implemented.

---

## Evidence And Scope

4-question gate:

| Question | Answer | Required evidence |
|---|---|---|
| 是否新增模块 / crate / 文件？ | Yes. This plan creates `fs_service.rs`, `command_service.rs`, and this plan document. | `semantic_search_nodes_tool` first, then LSP/`rg`. |
| 结论是否含否定语？ | Yes. The plan says app-server does not yet own P5 services. | Semantic search + LSP + `rg` evidence below. |
| 是否做跨项目对账？ | No. Codex SDK is used only as local protocol reference. | No cross-project capability verdict. |
| 是否写架构对账类文档？ | No. This is an implementation plan, not an ADR/comparison matrix. | Gap matrix is only updated after implementation. |

Evidence captured before writing:

- Semantic search for `dasclaw app server P5 filesystem command exec fs changed service owner implementation plan` returned no existing P5 plan file.
- Semantic search for app-server filesystem/command service found no existing `Function` owner in `crates/dasclaw_app_server`; exact `rg` shows only P5 references in the gap matrix and previous P3/P4 plans.
- LSP `workspace_symbols` found `AppServerServices` only in `crates/dasclaw_app_server/src/app_services.rs` and `CommandExecutionOutputDeltaEvent` only as the P3 item notification in `crates/dasclaw_app_server_protocol/src/lib.rs`.
- Exact `rg` confirms `crates/dasclaw_fs_tools` owns `ReadFileTool` / `WriteFileTool` / `ListDirTool` and `crates/dasclaw_shell_tools` + `crates/dasclaw_exec` own shell/sandbox execution primitives.
- Codex SDK local reference confirms P5 method names and DTO fields: `fs/readFile`, `fs/writeFile`, `fs/createDirectory`, `fs/getMetadata`, `fs/readDirectory`, `fs/remove`, `fs/copy`, `fs/watch`, `fs/unwatch`, `fs/changed`, `command/exec`, `command/exec/write`, `command/exec/terminate`, `command/exec/resize`, `command/exec/outputDelta`.

Scope:

- Implement default app-server P5 filesystem methods and `fs/changed` using polling watches.
- Implement `command/exec` buffered execution through existing shell/sandbox primitives.
- Add honest command capability availability. `command/exec/write` and `command/exec/resize` must stay declared/unavailable unless the implementing worker also lands a sandboxed PTY service in the same PR and proves it with tests. `command/exec/terminate` is available only for streamed processes.
- Preserve P3 item-level command notifications: `item/commandExecution/outputDelta` remains for agent/tool item output; P5 standalone command streaming uses `command/exec/outputDelta`.
- Do not implement dynamic `item/tool/call`, file-change approval UI, account/plugin/marketplace/product domains, fuzzy file search, hooks, or Windows sandbox setup in this plan.

Commit message discipline for this task family:

```text
已检查 P5 filesystem/command app-server owner 是否已有，结论：未发现等价 owner；复用 dasclaw_fs_tools、dasclaw_shell_tools、dasclaw_workspace_cap 底座，PTY follow-up 暂不接入。
```

## File Structure

- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`
  - Add P5 method/event constants.
  - Add FS params/responses.
  - Add command params/responses and standalone `command/exec/outputDelta`.
  - Extend `CapabilityMatrix` and `AppServerServiceAvailability`.

- Modify: `crates/dasclaw_app_server/Cargo.toml`
  - Task 2 does not add P5 implementation dependencies. Tasks 3 and 4 add
    each dependency at the first real use site.

- Modify: `crates/dasclaw_app_server/src/app_services.rs`
  - Add `FsService` and `CommandExecService` traits.
  - Extend `AppServerServices`, default no-op services, real service construction, test fakes, and availability.

- Create: `crates/dasclaw_app_server/src/fs_service.rs`
  - Own app-server filesystem methods.
  - Validate all paths against a service root before touching disk.
  - Maintain polling watches and expose drained `FsChangedNotification` events.

- Create: `crates/dasclaw_app_server/src/command_service.rs`
  - Own standalone command execution methods.
  - Route buffered execution through `dasclaw_shell_tools::SandboxedShellExecutor` with sandbox enabled.
  - Keep streamed/PTY follow-up methods behind explicit availability until sandboxed PTY is proven.

- Modify: `crates/dasclaw_app_server/src/lib.rs`
  - Import new DTOs.
  - Route P5 JSON-RPC methods.
  - Drain `fs/changed` and `command/exec/outputDelta` events into the notification bus.
  - Add app-server unit and stdio-style tests.

- Inspect: `crates/dasclaw_app_server/src/main.rs`
  - Ensure default sidecar already uses `AppServerServices::real()` with P5 services.
  - No edit is required if the existing sidecar path already constructs the real service bundle.

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Strike through only the P5 methods/events covered by real tests.
  - Leave PTY-only methods unstruck unless their service is implemented and covered.

## Task 1: Protocol DTOs And Capability Surface

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [ ] **Step 1: Write failing protocol tests**

Append these tests inside the existing `#[cfg(test)] mod tests` in `crates/dasclaw_app_server_protocol/src/lib.rs`:

```rust
#[test]
fn p5_protocol_declares_fs_and_command_methods() {
    let matrix = CapabilityMatrix::phase_one().with_p5_filesystem_command(
        AppServerP5Availability {
            filesystem: true,
            command: CommandExecAvailability {
                exec: true,
                output_delta_events: true,
                terminate: false,
                write: false,
                resize: false,
            },
        },
    );

    assert_eq!(matrix.filesystem.status, CapabilityStatus::Implemented);
    assert!(matrix.filesystem.methods.contains(&method::FS_READ_FILE.to_string()));
    assert!(matrix.filesystem.methods.contains(&method::FS_WRITE_FILE.to_string()));
    assert!(matrix.filesystem.methods.contains(&method::FS_WATCH.to_string()));
    assert!(matrix.filesystem.events.contains(&event::FS_CHANGED.to_string()));

    assert_eq!(matrix.command_exec.status, CapabilityStatus::Implemented);
    assert!(matrix.command_exec.methods.contains(&method::COMMAND_EXEC.to_string()));
    assert!(!matrix.command_exec.methods.contains(&method::COMMAND_EXEC_WRITE.to_string()));
    assert!(matrix.command_exec.events.contains(&event::COMMAND_EXEC_OUTPUT_DELTA.to_string()));
}

#[test]
fn p5_protocol_serializes_fs_and_command_payloads() {
    let read = FsReadFileResponse {
        data_base64: "aGVsbG8=".to_string(),
    };
    assert_eq!(
        serde_json::to_value(read).expect("serialize read response"),
        serde_json::json!({"dataBase64": "aGVsbG8="})
    );

    let exec = CommandExecParams {
        command: vec!["sh".to_string(), "-c".to_string(), "printf hello".to_string()],
        cwd: Some("/tmp".to_string()),
        timeout_ms: Some(5_000),
        disable_timeout: None,
        output_bytes_cap: Some(1024),
        disable_output_cap: None,
        env: Default::default(),
        process_id: Some("proc_1".to_string()),
        sandbox_policy: None,
        size: None,
        stream_stdin: None,
        stream_stdout_stderr: Some(true),
        tty: None,
    };
    assert_eq!(
        serde_json::to_value(exec).expect("serialize exec params"),
        serde_json::json!({
            "command": ["sh", "-c", "printf hello"],
            "cwd": "/tmp",
            "timeoutMs": 5000,
            "outputBytesCap": 1024,
            "processId": "proc_1",
            "streamStdoutStderr": true
        })
    );

    let changed = ServerNotification::fs_changed(FsChangedNotification {
        watch_id: "watch_1".to_string(),
        path: "/tmp/example.txt".to_string(),
        kind: FsChangedKind::Modified,
    })
    .expect("fs changed notification should serialize");
    assert_eq!(changed.method, event::FS_CHANGED);
}
```

- [ ] **Step 2: Run protocol tests and verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol p5_protocol
```

Expected: FAIL with unresolved names such as `with_p5_filesystem_command`, `FsReadFileResponse`, and `FS_READ_FILE`.

- [ ] **Step 3: Add P5 method and event constants**

In `pub mod method`, add:

```rust
    pub const FS_READ_FILE: &str = "fs/readFile";
    pub const FS_WRITE_FILE: &str = "fs/writeFile";
    pub const FS_CREATE_DIRECTORY: &str = "fs/createDirectory";
    pub const FS_GET_METADATA: &str = "fs/getMetadata";
    pub const FS_READ_DIRECTORY: &str = "fs/readDirectory";
    pub const FS_REMOVE: &str = "fs/remove";
    pub const FS_COPY: &str = "fs/copy";
    pub const FS_WATCH: &str = "fs/watch";
    pub const FS_UNWATCH: &str = "fs/unwatch";
    pub const COMMAND_EXEC: &str = "command/exec";
    pub const COMMAND_EXEC_WRITE: &str = "command/exec/write";
    pub const COMMAND_EXEC_TERMINATE: &str = "command/exec/terminate";
    pub const COMMAND_EXEC_RESIZE: &str = "command/exec/resize";
```

In `pub mod event`, add:

```rust
    pub const FS_CHANGED: &str = "fs/changed";
    pub const COMMAND_EXEC_OUTPUT_DELTA: &str = "command/exec/outputDelta";
```

- [ ] **Step 4: Add P5 protocol types**

Add these structs/enums near the existing command execution structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadFileParams {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadFileResponse {
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteFileParams {
    pub path: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteFileResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCreateDirectoryParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FsCreateDirectoryResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGetMetadataParams {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGetMetadataResponse {
    pub is_file: bool,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub created_at_ms: u64,
    pub modified_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryParams {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryEntry {
    pub file_name: String,
    pub is_file: bool,
    pub is_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryResponse {
    pub entries: Vec<FsReadDirectoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRemoveParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FsRemoveResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCopyParams {
    pub source_path: String,
    pub destination_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FsCopyResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWatchParams {
    pub path: String,
    pub watch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWatchResponse {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsUnwatchParams {
    pub watch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FsUnwatchResponse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FsChangedKind {
    Created,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsChangedNotification {
    pub watch_id: String,
    pub path: String,
    pub kind: FsChangedKind,
}
```

Add command types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandExecOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecTerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecParams {
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_timeout: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_bytes_cap: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_output_cap: Option<bool>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<CommandExecTerminalSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_stdin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_stdout_stderr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecOutputDeltaNotification {
    pub process_id: String,
    pub stream: CommandExecOutputStream,
    pub delta_base64: String,
    pub cap_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecWriteParams {
    pub process_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_stdin: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecWriteResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecTerminateParams {
    pub process_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecTerminateResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResizeParams {
    pub process_id: String,
    pub size: CommandExecTerminalSize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResizeResponse {}
```

- [ ] **Step 5: Extend capabilities**

Extend `CapabilityMatrix`:

```rust
pub struct CapabilityMatrix {
    pub protocol: Capability,
    pub lifecycle: Capability,
    pub health: Capability,
    pub session: Capability,
    pub approval: Capability,
    pub dlp_policy: Capability,
    pub model_provider: Capability,
    pub tools: Capability,
    pub jobs: Capability,
    pub skills: Capability,
    pub mcp: Capability,
    pub sandbox: Capability,
    pub logs: Capability,
    pub filesystem: Capability,
    pub command_exec: Capability,
}
```

Initialize both new capabilities in `phase_one()`:

```rust
            filesystem: declared_future_capability("filesystem"),
            command_exec: declared_future_capability("command_exec"),
```

Add availability types:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerP5Availability {
    pub filesystem: bool,
    pub command: CommandExecAvailability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandExecAvailability {
    pub exec: bool,
    pub output_delta_events: bool,
    pub terminate: bool,
    pub write: bool,
    pub resize: bool,
}
```

Extend the existing `AppServerServiceAvailability`:

```rust
pub struct AppServerServiceAvailability {
    pub logs: bool,
    pub jobs: bool,
    pub skills: bool,
    pub mcp: McpServiceAvailability,
    pub p5: AppServerP5Availability,
}
```

Update every existing `AppServerServiceAvailability { ... }` literal in protocol tests with either:

```rust
    p5: AppServerP5Availability::default(),
```

or a `..AppServerServiceAvailability::default()` tail when that better matches the existing test.

Extend `ServiceName`:

```rust
    Filesystem,
    CommandExec,
```

Add the capability updater:

```rust
    #[must_use]
    pub fn with_p5_filesystem_command(mut self, availability: AppServerP5Availability) -> Self {
        if availability.filesystem {
            self.filesystem = Capability::implemented(
                "filesystem",
                &[
                    method::FS_READ_FILE,
                    method::FS_WRITE_FILE,
                    method::FS_CREATE_DIRECTORY,
                    method::FS_GET_METADATA,
                    method::FS_READ_DIRECTORY,
                    method::FS_REMOVE,
                    method::FS_COPY,
                    method::FS_WATCH,
                    method::FS_UNWATCH,
                ],
                &[event::FS_CHANGED],
            );
        }

        let mut command_methods = Vec::new();
        if availability.command.exec {
            command_methods.push(method::COMMAND_EXEC);
        }
        if availability.command.write {
            command_methods.push(method::COMMAND_EXEC_WRITE);
        }
        if availability.command.terminate {
            command_methods.push(method::COMMAND_EXEC_TERMINATE);
        }
        if availability.command.resize {
            command_methods.push(method::COMMAND_EXEC_RESIZE);
        }
        let command_events = if availability.command.output_delta_events {
            vec![event::COMMAND_EXEC_OUTPUT_DELTA]
        } else {
            Vec::new()
        };
        if !command_methods.is_empty() || !command_events.is_empty() {
            self.command_exec =
                Capability::implemented("command_exec", &command_methods, &command_events);
        }
        self
    }
```

- [ ] **Step 6: Add notification constructors and schema entries**

Add to `impl ServerNotification`:

```rust
    pub fn fs_changed(event: FsChangedNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::FS_CHANGED, event)
    }

    pub fn command_exec_output_delta(
        event: CommandExecOutputDeltaNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::COMMAND_EXEC_OUTPUT_DELTA, event)
    }
```

Add P5 methods to `phase_one_methods()` with the same `MethodSchema::new(...)` pattern used for existing methods:

```rust
MethodSchema::new(method::FS_READ_FILE, "filesystem", Some("FsReadFileParams"), "FsReadFileResponse", true)
MethodSchema::new(method::FS_WRITE_FILE, "filesystem", Some("FsWriteFileParams"), "FsWriteFileResponse", true)
MethodSchema::new(method::FS_CREATE_DIRECTORY, "filesystem", Some("FsCreateDirectoryParams"), "FsCreateDirectoryResponse", true)
MethodSchema::new(method::FS_GET_METADATA, "filesystem", Some("FsGetMetadataParams"), "FsGetMetadataResponse", true)
MethodSchema::new(method::FS_READ_DIRECTORY, "filesystem", Some("FsReadDirectoryParams"), "FsReadDirectoryResponse", true)
MethodSchema::new(method::FS_REMOVE, "filesystem", Some("FsRemoveParams"), "FsRemoveResponse", true)
MethodSchema::new(method::FS_COPY, "filesystem", Some("FsCopyParams"), "FsCopyResponse", true)
MethodSchema::new(method::FS_WATCH, "filesystem", Some("FsWatchParams"), "FsWatchResponse", true)
MethodSchema::new(method::FS_UNWATCH, "filesystem", Some("FsUnwatchParams"), "FsUnwatchResponse", true)
MethodSchema::new(method::COMMAND_EXEC, "command_exec", Some("CommandExecParams"), "CommandExecResponse", true)
MethodSchema::new(method::COMMAND_EXEC_WRITE, "command_exec", Some("CommandExecWriteParams"), "CommandExecWriteResponse", true)
MethodSchema::new(method::COMMAND_EXEC_TERMINATE, "command_exec", Some("CommandExecTerminateParams"), "CommandExecTerminateResponse", true)
MethodSchema::new(method::COMMAND_EXEC_RESIZE, "command_exec", Some("CommandExecResizeParams"), "CommandExecResizeResponse", true)
```

Add P5 events to `phase_one_events()`:

```rust
EventSchema::new(event::FS_CHANGED, "filesystem", "FsChangedNotification")
EventSchema::new(
    event::COMMAND_EXEC_OUTPUT_DELTA,
    "command_exec",
    "CommandExecOutputDeltaNotification",
)
```

- [ ] **Step 7: Run protocol tests and commit**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol p5_protocol
cargo check -p dasclaw_app_server_protocol --tests
```

Expected: PASS.

Commit:

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): add P5 protocol surface

已检查 P5 filesystem/command app-server owner 是否已有，结论：未发现等价 owner；复用 dasclaw_fs_tools、dasclaw_shell_tools、dasclaw_workspace_cap 底座，PTY follow-up 暂不接入。"
```

## Task 2: Service Traits, Availability, And No-Op Guards

**Files:**
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Keep implementation dependencies out of this abstraction task**

Do not add `base64`, `dasclaw_fs_tools`, `dasclaw_pty`,
`dasclaw_shell_tools`, `dasclaw_workspace_cap`, or `tempfile` in this task.
Task 3 adds filesystem dependencies at the first real filesystem service use
site; Task 4 adds command execution dependencies at the first real command
service use site.

- [ ] **Step 2: Write failing service availability tests**

In `crates/dasclaw_app_server/src/app_services.rs`, extend the existing `#[cfg(test)] mod tests` with:

```rust
#[test]
fn app_server_p5_availability_requires_ready_health() {
    let services = AppServerServices::for_tests(
        TestLogService::ready(),
        TestJobService::ready(vec![]),
        TestSkillsService::ready(vec![]),
        TestMcpService::ready(vec![]),
        TestFsService::disabled(),
        TestCommandExecService::disabled(),
    );

    let availability = services.availability();
    assert!(!availability.p5.filesystem);
    assert!(!availability.p5.command.exec);
}

#[test]
fn app_server_p5_availability_reports_ready_service_methods() {
    let services = AppServerServices::for_tests(
        TestLogService::ready(),
        TestJobService::ready(vec![]),
        TestSkillsService::ready(vec![]),
        TestMcpService::ready(vec![]),
        TestFsService::ready(),
        TestCommandExecService::ready_buffered(),
    );

    let availability = services.availability();
    assert!(availability.p5.filesystem);
    assert!(availability.p5.command.exec);
    assert!(!availability.p5.command.output_delta_events);
    assert!(!availability.p5.command.write);
    assert!(!availability.p5.command.resize);
}
```

- [ ] **Step 3: Run app-services tests and verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_app_server app_server_p5_availability
```

Expected: FAIL with missing `TestFsService`, `TestCommandExecService`, and `availability.p5.filesystem`.

- [ ] **Step 4: Add `FsService` and `CommandExecService` traits**

At the top import the P5 DTOs:

```rust
use dasclaw_app_server_protocol::{
    AppServerP5Availability, CommandExecAvailability, CommandExecOutputDeltaNotification,
    CommandExecParams, CommandExecResponse, CommandExecResizeParams, CommandExecResizeResponse,
    CommandExecTerminateParams, CommandExecTerminateResponse, CommandExecWriteParams,
    CommandExecWriteResponse, FsChangedNotification, FsCopyParams, FsCopyResponse,
    FsCreateDirectoryParams, FsCreateDirectoryResponse, FsGetMetadataParams,
    FsGetMetadataResponse, FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams,
    FsReadFileResponse, FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse,
    FsWatchParams, FsWatchResponse, FsWriteFileParams, FsWriteFileResponse,
};
```

Add traits after `McpService`:

```rust
pub trait FsService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn read_file(&self, params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError>;
    fn write_file(&self, params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError>;
    fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError>;
    fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError>;
    fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError>;
    fn remove(&self, params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError>;
    fn copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, AppServerError>;
    fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError>;
    fn unwatch(&self, params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError>;
    fn drain_changed_events(&self) -> Vec<FsChangedNotification> {
        Vec::new()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait CommandExecService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn exec(&self, params: CommandExecParams) -> Result<CommandExecResponse, AppServerError>;
    fn write(
        &self,
        params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError>;
    fn terminate(
        &self,
        params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError>;
    fn resize(
        &self,
        params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError>;
    fn drain_output_delta_events(&self) -> Vec<CommandExecOutputDeltaNotification> {
        Vec::new()
    }
    fn availability(&self) -> CommandExecAvailability {
        CommandExecAvailability::default()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}
```

- [ ] **Step 5: Extend `AppServerServices`**

Add fields:

```rust
    pub filesystem: Arc<dyn FsService>,
    pub command: Arc<dyn CommandExecService>,
```

Extend `Default`:

```rust
            filesystem: Arc::new(NoopFsService),
            command: Arc::new(NoopCommandExecService),
```

Extend `real()`:

```rust
            filesystem: Arc::new(NoopFsService),
            command: Arc::new(NoopCommandExecService),
```

Keep `real()` on no-op P5 owners in this task because `fs_service.rs` and
`command_service.rs` do not exist until Tasks 3 and 4. Task 3 replaces only the
filesystem owner with the real implementation; Task 4 replaces only the command
owner.

Extend `health()`:

```rust
            self.filesystem.health(),
            self.command.health(),
```

Add drain helpers:

```rust
    pub fn drain_fs_changed_events(&self) -> Vec<FsChangedNotification> {
        self.filesystem.drain_changed_events()
    }

    pub fn drain_command_exec_output_delta_events(&self) -> Vec<CommandExecOutputDeltaNotification> {
        self.command.drain_output_delta_events()
    }
```

Extend `availability()`:

```rust
        let command = if self.command.is_ready() {
            self.command.availability()
        } else {
            CommandExecAvailability::default()
        };
        AppServerServiceAvailability {
            logs: self.logs.is_ready(),
            jobs: self.jobs.is_ready(),
            skills: self.skills.is_ready(),
            mcp: if self.mcp.is_ready() {
                self.mcp.availability()
            } else {
                McpServiceAvailability::default()
            },
            p5: AppServerP5Availability {
                filesystem: self.filesystem.is_ready(),
                command,
            },
        }
```

Extend `for_tests(...)` to accept `filesystem` and `command` arguments.

Update every existing `AppServerServices::for_tests(...)` call site in `crates/dasclaw_app_server/src/lib.rs` and `crates/dasclaw_app_server/src/app_services.rs` to pass `TestFsService::disabled()` and `TestCommandExecService::disabled()` unless that test is explicitly asserting ready P5 behavior.

- [ ] **Step 6: Add no-op services**

Add `struct NoopFsService;` and `struct NoopCommandExecService;`, then implement both traits. Every method returns `AppServerError::capability_unavailable(...)`:

```rust
impl FsService for NoopFsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Filesystem, "filesystem service is not wired")
    }

    fn read_file(&self, _params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn write_file(&self, _params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn create_directory(
        &self,
        _params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn get_metadata(
        &self,
        _params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn read_directory(
        &self,
        _params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn remove(&self, _params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn copy(&self, _params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn watch(&self, _params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }

    fn unwatch(&self, _params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "filesystem service is not wired",
        ))
    }
}
```

Implement `NoopCommandExecService` with the same shape and capability `"command_exec"`.

- [ ] **Step 7: Add test fakes**

Inside `#[cfg(test)] pub mod test_fakes`, add:

```rust
pub struct TestFsService {
    ready: bool,
}

impl TestFsService {
    pub fn ready() -> Self {
        Self { ready: true }
    }

    pub fn disabled() -> Self {
        Self { ready: false }
    }
}

impl FsService for TestFsService {
    fn health(&self) -> ServiceHealth {
        if self.ready {
            ServiceHealth::ready(ServiceName::Filesystem)
        } else {
            ServiceHealth::disabled(ServiceName::Filesystem, "test filesystem disabled")
        }
    }

    fn read_file(&self, _params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError> {
        Ok(FsReadFileResponse {
            data_base64: "dGVzdA==".to_string(),
        })
    }

    fn write_file(&self, _params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError> {
        Ok(FsWriteFileResponse {})
    }

    fn create_directory(
        &self,
        _params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        Ok(FsCreateDirectoryResponse {})
    }

    fn get_metadata(
        &self,
        _params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        Ok(FsGetMetadataResponse {
            is_file: true,
            is_directory: false,
            is_symlink: false,
            created_at_ms: 0,
            modified_at_ms: 0,
        })
    }

    fn read_directory(
        &self,
        _params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        Ok(FsReadDirectoryResponse { entries: vec![] })
    }

    fn remove(&self, _params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        Ok(FsRemoveResponse {})
    }

    fn copy(&self, _params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        Ok(FsCopyResponse {})
    }

    fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        Ok(FsWatchResponse { path: params.path })
    }

    fn unwatch(&self, _params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        Ok(FsUnwatchResponse {})
    }
}
```

Add a `TestCommandExecService` fake:

```rust
pub struct TestCommandExecService {
    availability: CommandExecAvailability,
}

impl TestCommandExecService {
    pub fn ready_buffered() -> Self {
        Self {
            availability: CommandExecAvailability {
                exec: true,
                output_delta_events: false,
                terminate: false,
                write: false,
                resize: false,
            },
        }
    }

    pub fn disabled() -> Self {
        Self {
            availability: CommandExecAvailability::default(),
        }
    }
}

impl CommandExecService for TestCommandExecService {
    fn health(&self) -> ServiceHealth {
        if self.availability.exec {
            ServiceHealth::ready(ServiceName::CommandExec)
        } else {
            ServiceHealth::disabled(ServiceName::CommandExec, "test command exec disabled")
        }
    }

    fn exec(&self, _params: CommandExecParams) -> Result<CommandExecResponse, AppServerError> {
        Ok(CommandExecResponse {
            exit_code: 0,
            stdout: "test".to_string(),
            stderr: String::new(),
        })
    }

    fn write(
        &self,
        _params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "command stdin streaming is not wired",
        ))
    }

    fn terminate(
        &self,
        _params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "command termination is not wired",
        ))
    }

    fn resize(
        &self,
        _params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "command resize is not wired",
        ))
    }

    fn availability(&self) -> CommandExecAvailability {
        self.availability
    }
}
```

Extend the public test fake re-export near the existing `pub use test_fakes::{...}` line:

```rust
pub use test_fakes::{
    TestCommandExecService, TestFsService, TestJobService, TestLogService, TestMcpService,
    TestSkillsService,
};
```

Extend the app-services test-module import with `TestCommandExecService` and `TestFsService`.

- [ ] **Step 8: Run service tests and commit**

Run:

```bash
cargo nextest run -p dasclaw_app_server app_server_p5_availability
cargo check -p dasclaw_app_server --tests
```

Expected: PASS.

Commit:

```bash
git add crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "feat(app-server): add P5 service owners"
```

## Task 3: Filesystem Service Implementation

**Files:**
- Modify: `crates/dasclaw_app_server/Cargo.toml`
- Create: `crates/dasclaw_app_server/src/fs_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`

- [ ] **Step 1: Register the module**

Add first-use dependencies to `crates/dasclaw_app_server/Cargo.toml`:

```toml
base64 = "0.22"
dasclaw_fs_tools = { path = "../dasclaw_fs_tools" }

[dev-dependencies]
tempfile = "3"
```

At the top of `crates/dasclaw_app_server/src/lib.rs`, add:

```rust
pub mod fs_service;
```

- [ ] **Step 2: Write failing filesystem service tests**

Create `crates/dasclaw_app_server/src/fs_service.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread;
    use std::time::Duration;

    use base64::Engine;
    use dasclaw_app_server_protocol::{
        FsChangedKind, FsCopyParams, FsCreateDirectoryParams, FsGetMetadataParams,
        FsReadDirectoryParams, FsReadFileParams, FsRemoveParams, FsUnwatchParams,
        FsWatchParams, FsWriteFileParams,
    };

    use super::*;

    fn b64(input: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(input)
    }

    #[test]
    fn fs_service_reads_writes_lists_metadata_and_removes_inside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        service
            .create_directory(FsCreateDirectoryParams {
                path: temp.path().join("nested").display().to_string(),
                recursive: Some(true),
            })
            .expect("create directory");
        service
            .write_file(FsWriteFileParams {
                path: temp.path().join("nested/file.txt").display().to_string(),
                data_base64: b64(b"hello"),
            })
            .expect("write file");

        let read = service
            .read_file(FsReadFileParams {
                path: temp.path().join("nested/file.txt").display().to_string(),
            })
            .expect("read file");
        assert_eq!(read.data_base64, b64(b"hello"));

        let metadata = service
            .get_metadata(FsGetMetadataParams {
                path: temp.path().join("nested/file.txt").display().to_string(),
            })
            .expect("metadata");
        assert!(metadata.is_file);
        assert!(!metadata.is_directory);

        let listing = service
            .read_directory(FsReadDirectoryParams {
                path: temp.path().join("nested").display().to_string(),
            })
            .expect("read directory");
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].file_name, "file.txt");

        service
            .copy(FsCopyParams {
                source_path: temp.path().join("nested/file.txt").display().to_string(),
                destination_path: temp.path().join("nested/copy.txt").display().to_string(),
                recursive: None,
            })
            .expect("copy file");
        assert_eq!(
            fs::read_to_string(temp.path().join("nested/copy.txt")).expect("copied content"),
            "hello"
        );

        service
            .remove(FsRemoveParams {
                path: temp.path().join("nested/copy.txt").display().to_string(),
                recursive: None,
                force: None,
            })
            .expect("remove file");
        assert!(!temp.path().join("nested/copy.txt").exists());
    }

    #[test]
    fn fs_service_rejects_escape_outside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        let err = service
            .read_file(FsReadFileParams {
                path: outside.path().display().to_string(),
            })
            .expect_err("outside root must be rejected");
        assert!(
            err.to_string().contains("filesystem path is outside service root"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fs_service_watch_emits_changed_event() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = temp.path().join("watched.txt");
        fs::write(&target, "before").expect("write initial");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        let watched = service
            .watch(FsWatchParams {
                path: target.display().to_string(),
                watch_id: "watch_1".to_string(),
            })
            .expect("watch file");
        assert_eq!(watched.path, target.canonicalize().expect("canonical").display().to_string());

        fs::write(&target, "after").expect("modify watched");
        thread::sleep(Duration::from_millis(300));

        let events = service.drain_changed_events();
        assert!(events.iter().any(|event| {
            event.watch_id == "watch_1" && event.kind == FsChangedKind::Modified
        }));

        service
            .unwatch(FsUnwatchParams {
                watch_id: "watch_1".to_string(),
            })
            .expect("unwatch file");
    }
}
```

- [ ] **Step 3: Run filesystem service tests and verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_app_server fs_service
```

Expected: FAIL because `AppServerFsService` is not defined.

- [ ] **Step 4: Add minimal filesystem service implementation**

Add implementation above the test module in `crates/dasclaw_app_server/src/fs_service.rs`:

```rust
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use dasclaw_app_server_protocol::{
    FsChangedKind, FsChangedNotification, FsCopyParams, FsCopyResponse, FsCreateDirectoryParams,
    FsCreateDirectoryResponse, FsGetMetadataParams, FsGetMetadataResponse, FsReadDirectoryEntry,
    FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams, FsReadFileResponse,
    FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse, FsWatchParams,
    FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_utils::normalize_lexical;

use crate::app_services::FsService;
use crate::AppServerError;

#[derive(Debug)]
pub struct AppServerFsService {
    root: PathBuf,
    watches: Mutex<HashMap<String, FsWatch>>,
    events: Mutex<Vec<FsChangedNotification>>,
}

#[derive(Debug, Clone)]
struct FsWatch {
    path: PathBuf,
    last_state: WatchState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WatchState {
    Missing,
    Present { modified_at_ms: u64, is_directory: bool },
}

impl AppServerFsService {
    pub fn new(root: PathBuf) -> Self {
        let root = root
            .canonicalize()
            .unwrap_or_else(|_| normalize_lexical(&root));
        Self {
            root,
            watches: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
        }
    }

    fn resolve(&self, raw: &str) -> Result<PathBuf, AppServerError> {
        let path = PathBuf::from(raw);
        let resolved = if path.is_absolute() {
            if path.exists() {
                path.canonicalize().unwrap_or_else(|_| normalize_lexical(&path))
            } else {
                normalize_lexical(&path)
            }
        } else {
            normalize_lexical(&self.root.join(path))
        };
        if !resolved.starts_with(&self.root) {
            return Err(AppServerError::invalid_request(
                "filesystem",
                "filesystem path is outside service root",
            ));
        }
        Ok(resolved)
    }

    fn metadata_state(path: &Path) -> WatchState {
        match fs::symlink_metadata(path) {
            Ok(metadata) => WatchState::Present {
                modified_at_ms: system_time_ms(metadata.modified().unwrap_or(UNIX_EPOCH)),
                is_directory: metadata.is_dir(),
            },
            Err(_) => WatchState::Missing,
        }
    }

    fn poll_watches(&self) {
        let mut emitted = Vec::new();
        let mut watches = self.watches.lock().expect("watch lock");
        for (watch_id, watch) in watches.iter_mut() {
            let next = Self::metadata_state(&watch.path);
            if next != watch.last_state {
                let kind = match (&watch.last_state, &next) {
                    (WatchState::Missing, WatchState::Present { .. }) => FsChangedKind::Created,
                    (WatchState::Present { .. }, WatchState::Missing) => FsChangedKind::Removed,
                    (WatchState::Present { .. }, WatchState::Present { .. }) => {
                        FsChangedKind::Modified
                    }
                    (WatchState::Missing, WatchState::Missing) => continue,
                };
                watch.last_state = next;
                emitted.push(FsChangedNotification {
                    watch_id: watch_id.clone(),
                    path: watch.path.display().to_string(),
                    kind,
                });
            }
        }
        if !emitted.is_empty() {
            self.events.lock().expect("events lock").extend(emitted);
        }
    }
}

impl FsService for AppServerFsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Filesystem)
    }

    fn read_file(&self, params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        let bytes = fs::read(path)
            .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        Ok(FsReadFileResponse {
            data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
    }

    fn write_file(&self, params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(params.data_base64)
            .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppServerError::invalid_request("filesystem", error.to_string())
            })?;
        }
        fs::write(path, bytes)
            .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        self.poll_watches();
        Ok(FsWriteFileResponse {})
    }

    fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        if params.recursive.unwrap_or(true) {
            fs::create_dir_all(path)
        } else {
            fs::create_dir(path)
        }
        .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        self.poll_watches();
        Ok(FsCreateDirectoryResponse {})
    }

    fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        Ok(FsGetMetadataResponse {
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
            is_symlink: metadata.file_type().is_symlink(),
            created_at_ms: system_time_ms(metadata.created().unwrap_or(UNIX_EPOCH)),
            modified_at_ms: system_time_ms(metadata.modified().unwrap_or(UNIX_EPOCH)),
        })
    }

    fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)
            .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?
        {
            let entry = entry
                .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
            let metadata = entry
                .metadata()
                .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
            entries.push(FsReadDirectoryEntry {
                file_name: entry.file_name().to_string_lossy().to_string(),
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
            });
        }
        entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        Ok(FsReadDirectoryResponse { entries })
    }

    fn remove(&self, params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        if !path.exists() && params.force.unwrap_or(true) {
            return Ok(FsRemoveResponse {});
        }
        if path.is_dir() && params.recursive.unwrap_or(true) {
            fs::remove_dir_all(path)
        } else if path.is_dir() {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        }
        .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        self.poll_watches();
        Ok(FsRemoveResponse {})
    }

    fn copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        let source = self.resolve(&params.source_path)?;
        let destination = self.resolve(&params.destination_path)?;
        if source.is_dir() {
            if !params.recursive.unwrap_or(false) {
                return Err(AppServerError::invalid_request(
                    "filesystem",
                    "recursive=true is required for directory copies",
                ));
            }
            copy_dir_recursive(&source, &destination)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AppServerError::invalid_request("filesystem", error.to_string())
                })?;
            }
            fs::copy(source, destination)
                .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        }
        self.poll_watches();
        Ok(FsCopyResponse {})
    }

    fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        let path = self.resolve(&params.path)?;
        let canonical = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| normalize_lexical(&path))
        } else {
            normalize_lexical(&path)
        };
        let watch = FsWatch {
            path: canonical.clone(),
            last_state: Self::metadata_state(&canonical),
        };
        self.watches
            .lock()
            .expect("watch lock")
            .insert(params.watch_id, watch);
        Ok(FsWatchResponse {
            path: canonical.display().to_string(),
        })
    }

    fn unwatch(&self, params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        self.watches
            .lock()
            .expect("watch lock")
            .remove(&params.watch_id);
        Ok(FsUnwatchResponse {})
    }

    fn drain_changed_events(&self) -> Vec<FsChangedNotification> {
        self.poll_watches();
        self.events.lock().expect("events lock").drain(..).collect()
    }
}

fn system_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), AppServerError> {
    fs::create_dir_all(destination)
        .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?
    {
        let entry =
            entry.map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(from, to)
                .map_err(|error| AppServerError::invalid_request("filesystem", error.to_string()))?;
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run filesystem service tests and commit**

Before running checks, update `AppServerServices::real()` so only the filesystem
field uses the real service while command execution remains no-op:

```rust
            filesystem: Arc::new(crate::fs_service::AppServerFsService::new(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            )),
            command: Arc::new(NoopCommandExecService),
```

Run:

```bash
cargo nextest run -p dasclaw_app_server fs_service
cargo check -p dasclaw_app_server --tests
```

Expected: PASS.

Commit:

```bash
git add crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/fs_service.rs
git commit -m "feat(app-server): implement P5 filesystem service"
```

## Task 4: Command Exec Buffered Service

**Files:**
- Modify: `crates/dasclaw_app_server/Cargo.toml`
- Create: `crates/dasclaw_app_server/src/command_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`

- [ ] **Step 1: Register the module**

Add first-use command execution dependencies to
`crates/dasclaw_app_server/Cargo.toml`:

```toml
dasclaw_shell_tools = { path = "../dasclaw_shell_tools" }
dasclaw_workspace_cap = { path = "../dasclaw_workspace_cap" }
```

Do not add `dasclaw_pty` unless this task also implements sandboxed PTY
streaming; buffered-only command exec does not use it.

At the top of `crates/dasclaw_app_server/src/lib.rs`, add:

```rust
pub mod command_service;
```

- [ ] **Step 2: Write failing command service tests**

Create `crates/dasclaw_app_server/src/command_service.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use dasclaw_app_server_protocol::{
        CommandExecParams, CommandExecResizeParams, CommandExecTerminalSize,
        CommandExecWriteParams,
    };

    use super::*;

    fn exec_params(command: Vec<String>) -> CommandExecParams {
        CommandExecParams {
            command,
            cwd: None,
            timeout_ms: Some(5_000),
            disable_timeout: None,
            output_bytes_cap: Some(4096),
            disable_output_cap: None,
            env: BTreeMap::new(),
            process_id: None,
            sandbox_policy: None,
            size: None,
            stream_stdin: None,
            stream_stdout_stderr: None,
            tty: None,
        }
    }

    #[test]
    fn buffered_command_exec_captures_stdout() {
        let service = AppServerCommandExecService::buffered_only();
        let response = service
            .exec(exec_params(vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf dasclaw-p5".to_string(),
            ]))
            .expect("exec");

        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, "dasclaw-p5");
        assert_eq!(response.stderr, "");
    }

    #[test]
    fn buffered_command_rejects_empty_argv() {
        let service = AppServerCommandExecService::buffered_only();
        let err = service
            .exec(exec_params(Vec::new()))
            .expect_err("empty argv must be rejected");
        assert!(
            err.to_string().contains("command argv must not be empty"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn pty_followups_are_fail_safe_when_pty_is_not_enabled() {
        let service = AppServerCommandExecService::buffered_only();
        assert!(service
            .write(CommandExecWriteParams {
                process_id: "proc_1".to_string(),
                delta_base64: Some("Cg==".to_string()),
                close_stdin: None,
            })
            .is_err());
        assert!(service
            .resize(CommandExecResizeParams {
                process_id: "proc_1".to_string(),
                size: CommandExecTerminalSize { cols: 80, rows: 24 },
            })
            .is_err());
    }
}
```

- [ ] **Step 3: Run command service tests and verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_app_server command_service
```

Expected: FAIL because `AppServerCommandExecService` is not defined.

- [ ] **Step 4: Implement buffered command service**

Add implementation above the tests:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dasclaw_app_server_protocol::{
    CommandExecAvailability, CommandExecOutputDeltaNotification, CommandExecParams,
    CommandExecResizeParams, CommandExecResizeResponse, CommandExecResponse,
    CommandExecTerminateParams, CommandExecTerminateResponse, CommandExecWriteParams,
    CommandExecWriteResponse, ServiceHealth, ServiceName,
};
use dasclaw_shell_tools::SandboxedShellExecutor;
use dasclaw_workspace_cap::policy::SandboxPolicy;

use crate::app_services::CommandExecService;
use crate::blocking_runtime::BlockingTokioRuntime;
use crate::AppServerError;

pub struct AppServerCommandExecService {
    runtime: Arc<Mutex<Option<BlockingTokioRuntime>>>,
    default_timeout: Duration,
    output_bytes_cap: usize,
}

impl AppServerCommandExecService {
    pub fn buffered_only() -> Self {
        Self {
            runtime: Arc::new(Mutex::new(None)),
            default_timeout: Duration::from_secs(120),
            output_bytes_cap: 64 * 1024,
        }
    }

    fn shell_command(params: &CommandExecParams) -> Result<String, AppServerError> {
        if params.command.is_empty() {
            return Err(AppServerError::invalid_request(
                "command_exec",
                "command argv must not be empty",
            ));
        }
        shell_join(&params.command)
    }

    fn timeout_seconds(&self, params: &CommandExecParams) -> Result<u64, AppServerError> {
        if params.disable_timeout.unwrap_or(false) {
            return Err(AppServerError::invalid_request(
                "command_exec",
                "disableTimeout is not supported by buffered app-server command exec",
            ));
        }
        Ok(params
            .timeout_ms
            .map(|ms| ms.div_ceil(1000))
            .unwrap_or_else(|| self.default_timeout.as_secs()))
    }

    fn runtime(&self) -> Result<BlockingTokioRuntime, AppServerError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(runtime) = runtime.as_ref() {
            return Ok(runtime.clone());
        }
        let created = BlockingTokioRuntime::new(
            "dasclaw-app-server-command-exec",
            "command_exec",
        )?;
        *runtime = Some(created.clone());
        Ok(created)
    }
}

impl CommandExecService for AppServerCommandExecService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::CommandExec)
    }

    fn exec(&self, params: CommandExecParams) -> Result<CommandExecResponse, AppServerError> {
        if params.tty.unwrap_or(false)
            || params.stream_stdin.unwrap_or(false)
            || params.stream_stdout_stderr.unwrap_or(false)
        {
            return Err(AppServerError::capability_unavailable(
                "command_exec",
                "streaming command execution requires sandboxed PTY support",
            ));
        }
        if params.disable_output_cap.unwrap_or(false) {
            return Err(AppServerError::invalid_request(
                "command_exec",
                "disableOutputCap is not supported by buffered app-server command exec",
            ));
        }

        let command = Self::shell_command(&params)?;
        let timeout = self.timeout_seconds(&params)?;
        let output_cap = params.output_bytes_cap.unwrap_or(self.output_bytes_cap);
        let cwd = params
            .cwd
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let mut extra_env = HashMap::new();
        for (key, value) in params.env {
            if let Some(value) = value {
                extra_env.insert(key, value);
            }
        }

        let output = self.runtime()?.block_on("command_exec/exec", async move {
            let sandbox = SandboxedShellExecutor::new(Duration::from_secs(timeout), false, None);
            sandbox
                .execute(
                    &command,
                    &cwd,
                    SandboxPolicy::ReadOnly {
                        network_access: false,
                    },
                    extra_env,
                )
                .await
                .map_err(|error| {
                    AppServerError::invalid_request("command_exec", error.to_string())
                })
        })?;

        Ok(CommandExecResponse {
            exit_code: output.exit_code as i32,
            stdout: truncate_to_cap(output.stdout, output_cap),
            stderr: truncate_to_cap(output.stderr, output_cap),
        })
    }

    fn write(
        &self,
        _params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "command stdin streaming requires sandboxed PTY support",
        ))
    }

    fn terminate(
        &self,
        _params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "command termination requires a streamed process",
        ))
    }

    fn resize(
        &self,
        _params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "command resize requires sandboxed PTY support",
        ))
    }

    fn drain_output_delta_events(&self) -> Vec<CommandExecOutputDeltaNotification> {
        Vec::new()
    }

    fn availability(&self) -> CommandExecAvailability {
        CommandExecAvailability {
            exec: true,
            output_delta_events: false,
            terminate: false,
            write: false,
            resize: false,
        }
    }
}

fn shell_join(argv: &[String]) -> Result<String, AppServerError> {
    let mut out = String::new();
    for (index, arg) in argv.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(&shell_escape(arg));
    }
    Ok(out)
}

fn shell_escape(arg: &str) -> String {
    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_' | ':' | '='))
    {
        return arg.to_string();
    }
    let escaped = arg.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn truncate_to_cap(value: String, cap: usize) -> String {
    if value.len() <= cap {
        return value;
    }
    let mut end = cap;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}
```

This uses the existing `BlockingTokioRuntime` pattern from `job_service.rs` and `mcp_service.rs`; do not add a second ad-hoc runtime helper.

- [ ] **Step 5: Run command service tests and commit**

Before running checks, update `AppServerServices::real()` so the command field
uses the real buffered command service:

```rust
            command: Arc::new(crate::command_service::AppServerCommandExecService::buffered_only()),
```

Run:

```bash
cargo nextest run -p dasclaw_app_server command_service
cargo check -p dasclaw_app_server --tests
```

Expected: PASS.

Commit:

```bash
git add crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/command_service.rs
git commit -m "feat(app-server): implement buffered P5 command exec"
```

## Task 5: AppServer Routing, Notifications, And Capabilities

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/app_services.rs`

- [ ] **Step 1: Write failing JSON-RPC routing tests**

Append to `#[cfg(test)] mod tests` in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn app_server_p5_noop_routes_return_capability_unavailable() {
    let mut server = initialized_server();
    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"fs","method":"fs/readFile","params":{"path":"/tmp/missing"}}"#,
        )
        .expect("fs/readFile should return response");
    let value: Value = serde_json::from_str(&response).expect("fs response JSON");

    assert_eq!(value["id"], "fs");
    assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
    assert_eq!(value["error"]["data"]["capability"], "filesystem");
}

#[test]
fn app_server_ready_p5_routes_reach_service_owners() {
    let services = app_services::AppServerServices::for_tests(
        app_services::TestLogService::ready(),
        app_services::TestJobService::ready(vec![]),
        app_services::TestSkillsService::ready(vec![]),
        app_services::TestMcpService::ready(vec![]),
        app_services::TestFsService::ready(),
        app_services::TestCommandExecService::ready_buffered(),
    );
    let mut server = AppServer::new().with_app_services(services);
    server
        .handle_json_rpc(initialized_request_json())
        .expect("initialize should return a response");

    let fs = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"fs","method":"fs/readFile","params":{"path":"/workspace/file.txt"}}"#,
        )
        .expect("fs/readFile response");
    let command = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"cmd","method":"command/exec","params":{"command":["echo","hi"]}}"#,
        )
        .expect("command/exec response");

    let fs_value: Value = serde_json::from_str(&fs).expect("fs JSON");
    let command_value: Value = serde_json::from_str(&command).expect("command JSON");
    assert_eq!(fs_value["result"]["dataBase64"], "dGVzdA==");
    assert_eq!(command_value["result"]["stdout"], "test");
}
```

- [ ] **Step 2: Run routing tests and verify they fail**

Run:

```bash
cargo nextest run -p dasclaw_app_server app_server_p5
```

Expected: FAIL because P5 routes are not matched.

- [ ] **Step 3: Import P5 DTOs in `lib.rs`**

Extend the `use dasclaw_app_server_protocol::{ ... }` block with:

```rust
    CommandExecParams, CommandExecResizeParams, CommandExecTerminateParams,
    CommandExecWriteParams, FsCopyParams, FsCreateDirectoryParams, FsGetMetadataParams,
    FsReadDirectoryParams, FsReadFileParams, FsRemoveParams, FsUnwatchParams, FsWatchParams,
    FsWriteFileParams,
```

- [ ] **Step 4: Add AppServer service methods**

Add methods near existing `jobs_list`, `skills_list`, and `mcp_server_*` helpers:

```rust
    pub fn fs_read_file(
        &mut self,
        params: FsReadFileParams,
    ) -> Result<dasclaw_app_server_protocol::FsReadFileResponse, AppServerError> {
        self.app_services.filesystem.read_file(params)
    }

    pub fn fs_write_file(
        &mut self,
        params: FsWriteFileParams,
    ) -> Result<dasclaw_app_server_protocol::FsWriteFileResponse, AppServerError> {
        self.app_services.filesystem.write_file(params)
    }

    pub fn fs_create_directory(
        &mut self,
        params: FsCreateDirectoryParams,
    ) -> Result<dasclaw_app_server_protocol::FsCreateDirectoryResponse, AppServerError> {
        self.app_services.filesystem.create_directory(params)
    }

    pub fn fs_get_metadata(
        &mut self,
        params: FsGetMetadataParams,
    ) -> Result<dasclaw_app_server_protocol::FsGetMetadataResponse, AppServerError> {
        self.app_services.filesystem.get_metadata(params)
    }

    pub fn fs_read_directory(
        &mut self,
        params: FsReadDirectoryParams,
    ) -> Result<dasclaw_app_server_protocol::FsReadDirectoryResponse, AppServerError> {
        self.app_services.filesystem.read_directory(params)
    }

    pub fn fs_remove(
        &mut self,
        params: FsRemoveParams,
    ) -> Result<dasclaw_app_server_protocol::FsRemoveResponse, AppServerError> {
        self.app_services.filesystem.remove(params)
    }

    pub fn fs_copy(
        &mut self,
        params: FsCopyParams,
    ) -> Result<dasclaw_app_server_protocol::FsCopyResponse, AppServerError> {
        self.app_services.filesystem.copy(params)
    }

    pub fn fs_watch(
        &mut self,
        params: FsWatchParams,
    ) -> Result<dasclaw_app_server_protocol::FsWatchResponse, AppServerError> {
        self.app_services.filesystem.watch(params)
    }

    pub fn fs_unwatch(
        &mut self,
        params: FsUnwatchParams,
    ) -> Result<dasclaw_app_server_protocol::FsUnwatchResponse, AppServerError> {
        self.app_services.filesystem.unwatch(params)
    }

    pub fn command_exec(
        &mut self,
        params: CommandExecParams,
    ) -> Result<dasclaw_app_server_protocol::CommandExecResponse, AppServerError> {
        self.app_services.command.exec(params)
    }

    pub fn command_exec_write(
        &mut self,
        params: CommandExecWriteParams,
    ) -> Result<dasclaw_app_server_protocol::CommandExecWriteResponse, AppServerError> {
        self.app_services.command.write(params)
    }

    pub fn command_exec_terminate(
        &mut self,
        params: CommandExecTerminateParams,
    ) -> Result<dasclaw_app_server_protocol::CommandExecTerminateResponse, AppServerError> {
        self.app_services.command.terminate(params)
    }

    pub fn command_exec_resize(
        &mut self,
        params: CommandExecResizeParams,
    ) -> Result<dasclaw_app_server_protocol::CommandExecResizeResponse, AppServerError> {
        self.app_services.command.resize(params)
    }
```

- [ ] **Step 5: Route P5 JSON-RPC methods**

Add match arms in `route_json_rpc` before `method::APPROVAL_RESPOND`:

```rust
            method::FS_READ_FILE => route_with_params(
                request.id,
                request.params,
                |params: FsReadFileParams| self.fs_read_file(params),
            ),
            method::FS_WRITE_FILE => route_with_params(
                request.id,
                request.params,
                |params: FsWriteFileParams| self.fs_write_file(params),
            ),
            method::FS_CREATE_DIRECTORY => route_with_params(
                request.id,
                request.params,
                |params: FsCreateDirectoryParams| self.fs_create_directory(params),
            ),
            method::FS_GET_METADATA => route_with_params(
                request.id,
                request.params,
                |params: FsGetMetadataParams| self.fs_get_metadata(params),
            ),
            method::FS_READ_DIRECTORY => route_with_params(
                request.id,
                request.params,
                |params: FsReadDirectoryParams| self.fs_read_directory(params),
            ),
            method::FS_REMOVE => route_with_params(
                request.id,
                request.params,
                |params: FsRemoveParams| self.fs_remove(params),
            ),
            method::FS_COPY => route_with_params(
                request.id,
                request.params,
                |params: FsCopyParams| self.fs_copy(params),
            ),
            method::FS_WATCH => route_with_params(
                request.id,
                request.params,
                |params: FsWatchParams| self.fs_watch(params),
            ),
            method::FS_UNWATCH => route_with_params(
                request.id,
                request.params,
                |params: FsUnwatchParams| self.fs_unwatch(params),
            ),
            method::COMMAND_EXEC => route_with_params(
                request.id,
                request.params,
                |params: CommandExecParams| self.command_exec(params),
            ),
            method::COMMAND_EXEC_WRITE => route_with_params(
                request.id,
                request.params,
                |params: CommandExecWriteParams| self.command_exec_write(params),
            ),
            method::COMMAND_EXEC_TERMINATE => route_with_params(
                request.id,
                request.params,
                |params: CommandExecTerminateParams| self.command_exec_terminate(params),
            ),
            method::COMMAND_EXEC_RESIZE => route_with_params(
                request.id,
                request.params,
                |params: CommandExecResizeParams| self.command_exec_resize(params),
            ),
```

- [ ] **Step 6: Drain P5 notifications**

In `drain_service_updates()`, add loops matching the existing log/MCP drain pattern:

```rust
        for event in self.app_services.drain_fs_changed_events() {
            self.notifications.emit_fs_changed(event);
        }
        for event in self.app_services.drain_command_exec_output_delta_events() {
            self.notifications.emit_command_exec_output_delta(event);
        }
```

Add methods to `NotificationBus`:

```rust
    pub fn emit_fs_changed(&mut self, event: dasclaw_app_server_protocol::FsChangedNotification) {
        self.push(ServerNotification::fs_changed(event));
    }

    pub fn emit_command_exec_output_delta(
        &mut self,
        event: dasclaw_app_server_protocol::CommandExecOutputDeltaNotification,
    ) {
        self.push(ServerNotification::command_exec_output_delta(event));
    }
```

- [ ] **Step 7: Refresh capabilities with P5 availability**

Find `service_capability_snapshot()` and extend its baseline reset:

```rust
        capabilities.logs = service_baseline.logs;
        capabilities.jobs = service_baseline.jobs;
        capabilities.skills = service_baseline.skills;
        capabilities.mcp = service_baseline.mcp;
        capabilities.filesystem = service_baseline.filesystem;
        capabilities.command_exec = service_baseline.command_exec;
```

Then change the final service capability expression from:

```rust
capabilities.with_app_services(self.app_services.availability())
```

to:

```rust
let availability = self.app_services.availability();
capabilities
    .with_app_services(availability)
    .with_p5_filesystem_command(availability.p5)
```

- [ ] **Step 8: Run routing tests and commit**

Run:

```bash
cargo nextest run -p dasclaw_app_server app_server_p5
cargo check -p dasclaw_app_server --tests
```

Expected: PASS.

Commit:

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/app_services.rs
git commit -m "feat(app-server): route P5 filesystem and command methods"
```

## Task 6: Real Service JSON-RPC And Stdio Smoke

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Inspect: `crates/dasclaw_app_server/src/main.rs`

- [ ] **Step 1: Write real service JSON-RPC tests**

Append to `#[cfg(test)] mod tests` in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn app_server_real_fs_service_round_trips_file_content() {
    let temp = tempfile::tempdir().expect("tempdir");
    let services = app_services::AppServerServices {
        logs: Arc::new(app_services::TestLogService::ready()),
        jobs: Arc::new(app_services::TestJobService::ready(vec![])),
        skills: Arc::new(app_services::TestSkillsService::ready(vec![])),
        mcp: Arc::new(app_services::TestMcpService::ready(vec![])),
        filesystem: Arc::new(fs_service::AppServerFsService::new(temp.path().to_path_buf())),
        command: Arc::new(app_services::TestCommandExecService::ready_buffered()),
    };
    let mut server = AppServer::new().with_app_services(services);
    server
        .handle_json_rpc(initialized_request_json())
        .expect("initialize should return a response");

    let path = temp.path().join("note.txt").display().to_string();
    let write = format!(
        r#"{{"jsonrpc":"2.0","id":"write","method":"fs/writeFile","params":{{"path":{path:?},"dataBase64":"aGVsbG8="}}}}"#
    );
    let read = format!(
        r#"{{"jsonrpc":"2.0","id":"read","method":"fs/readFile","params":{{"path":{path:?}}}}}"#
    );

    let write_response = server.handle_json_rpc(&write).expect("write response");
    let read_response = server.handle_json_rpc(&read).expect("read response");
    let write_value: Value = serde_json::from_str(&write_response).expect("write JSON");
    let read_value: Value = serde_json::from_str(&read_response).expect("read JSON");
    assert!(write_value.get("error").is_none());
    assert_eq!(read_value["result"]["dataBase64"], "aGVsbG8=");
}

#[test]
fn app_server_real_command_exec_runs_buffered_command() {
    let services = app_services::AppServerServices {
        logs: Arc::new(app_services::TestLogService::ready()),
        jobs: Arc::new(app_services::TestJobService::ready(vec![])),
        skills: Arc::new(app_services::TestSkillsService::ready(vec![])),
        mcp: Arc::new(app_services::TestMcpService::ready(vec![])),
        filesystem: Arc::new(app_services::TestFsService::ready()),
        command: Arc::new(command_service::AppServerCommandExecService::buffered_only()),
    };
    let mut server = AppServer::new().with_app_services(services);
    server
        .handle_json_rpc(initialized_request_json())
        .expect("initialize should return a response");

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"cmd","method":"command/exec","params":{"command":["sh","-c","printf p5-command"]}}"#,
        )
        .expect("command response");
    let value: Value = serde_json::from_str(&response).expect("command JSON");
    assert_eq!(value["result"]["exitCode"], 0);
    assert_eq!(value["result"]["stdout"], "p5-command");
}
```

- [ ] **Step 2: Run real service tests and verify they fail or expose missing imports**

Run:

```bash
cargo nextest run -p dasclaw_app_server app_server_real_fs_service_round_trips_file_content app_server_real_command_exec_runs_buffered_command
```

Expected before wiring fixes: FAIL with missing imports or private module references.

- [ ] **Step 3: Fix imports and visibility**

Ensure the test module imports:

```rust
use std::sync::Arc;
```

Ensure `AppServerServices` fields remain public, as they already are for tests and manual construction.

- [ ] **Step 4: Confirm default sidecar service wiring**

Confirm `crates/dasclaw_app_server/src/main.rs` constructs the app server with `AppServerServices::real()`.
No `main.rs` edit is required if the existing sidecar path already uses the real service bundle.
If a self-check requested list exists and inspects service availability directly, include P5 capabilities there:

```rust
            "filesystem".to_string(),
            "command_exec".to_string(),
```

Extend `SelfCheckReport` only if it already serializes capability names; otherwise leave the response shape unchanged and rely on `capabilities` in initialize.

- [ ] **Step 5: Run app-server tests and command smoke**

Run:

```bash
cargo nextest run -p dasclaw_app_server app_server_real_fs_service_round_trips_file_content app_server_real_command_exec_runs_buffered_command
cargo check -p dasclaw_app_server --tests
```

Expected: PASS.

Run a manual stdio smoke:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"p5-smoke","version":"0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":["filesystem","command_exec"],"modelProvider":{"selectedModel":{"modelId":"test","provider":"test","name":"Test","supportsReasoning":false,"contextWindow":1},"models":[{"modelId":"test","provider":"test","name":"Test","supportsReasoning":false,"contextWindow":1}]}}}' \
  '{"jsonrpc":"2.0","id":"cmd","method":"command/exec","params":{"command":["sh","-c","printf app-server-p5"]}}' \
  | cargo run -p dasclaw_app_server --bin dasclaw-app-server
```

Expected output contains a JSON-RPC response with `"id":"cmd"`, `"exitCode":0`, and `"stdout":"app-server-p5"`.

- [ ] **Step 6: Commit**

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "test(app-server): cover real P5 stdio services"
```

## Task 7: Gap Matrix And Honest Capability Documentation

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Update the P5 row**

Change the P5 row from:

```markdown
| P5 | Filesystem / command exec | `fs/*`、`command/exec*`、`fs/changed`、command output | 必须在 P3 安全边界之后做，否则风险大 |
```

to:

```markdown
| P5 | Filesystem / command exec | ~~`fs/readFile`、`fs/writeFile`、`fs/createDirectory`、`fs/getMetadata`、`fs/readDirectory`、`fs/remove`、`fs/copy`、`fs/watch`、`fs/unwatch`、`fs/changed`、`command/exec`~~；`command/exec/write`、`command/exec/terminate`、`command/exec/resize` 仍需 sandboxed PTY service 才能进入 implemented capability | P5a 已接 app-server-owned filesystem 与 buffered command exec；PTY follow-up methods 保持 honest unavailable，避免绕过 P3/P5 sandbox 边界 |
```

- [ ] **Step 2: Update the current status summary**

In the “本轮已完成” summary near the top, add this sentence fragment after P4:

```markdown
；P5a `fs/*` filesystem owner、`fs/changed` polling watcher、buffered `command/exec` 已通过 app-server real service 与 stdio smoke 覆盖
```

- [ ] **Step 3: Keep unsupported methods visible**

Ensure the document still contains unstruck references to:

```markdown
`command/exec/write`
`command/exec/terminate`
`command/exec/resize`
```

These remain visible until a sandboxed PTY command service is implemented and tested.

- [ ] **Step 4: Commit**

Run:

```bash
git diff -- docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

Expected: only P5 status text changes.

Commit:

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs(app-server): mark tested P5a filesystem command surface"
```

## Task 8: Final Verification And Review Pipeline

**Files:**
- Verify all modified files from Tasks 1-7.

- [ ] **Step 1: Run targeted checks**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol p5_protocol
cargo nextest run -p dasclaw_app_server app_server_p5 fs_service command_service app_server_real_fs_service_round_trips_file_content app_server_real_command_exec_runs_buffered_command
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
```

Expected: PASS.

- [ ] **Step 2: Run formatting and panic check**

Run:

```bash
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: PASS. If the branch base is not `origin/xClaw`, replace only the base ref with the branch target.

- [ ] **Step 3: Run crate clippy**

Run:

```bash
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: PASS. If clippy reports warnings in untouched files, document the exact file/line and let CI be the final source for workspace-wide lint drift.

- [ ] **Step 4: Run required quality skills**

Run the repository’s required local review sequence:

```bash
# code-quality-audit skill
# code-simplifier skill
# code-review-expert skill
```

Expected: no P0/P1 findings. If findings are raised, fix them before push.

- [ ] **Step 5: Push branch and open PR**

Use the project’s normal GitHub flow. PR description must include:

```markdown
## Summary
- Adds app-server-owned P5 filesystem service and JSON-RPC routes.
- Adds buffered standalone `command/exec` through the existing shell/sandbox path.
- Keeps PTY follow-up command methods honest until sandboxed PTY service support exists.

## Verification
- `cargo nextest run -p dasclaw_app_server_protocol p5_protocol`
- `cargo nextest run -p dasclaw_app_server app_server_p5 fs_service command_service app_server_real_fs_service_round_trips_file_content app_server_real_command_exec_runs_buffered_command`
- `cargo check -p dasclaw_app_server_protocol --tests`
- `cargo check -p dasclaw_app_server --tests`
- `cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw`
- `cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings`
- `cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings`

## Process transparency
已检查 P5 filesystem/command app-server owner 是否已有，结论：未发现等价 owner；复用 dasclaw_fs_tools、dasclaw_shell_tools、dasclaw_workspace_cap 底座，PTY follow-up 暂不接入。
```

## Self-Review

Spec coverage:

- P5 `fs/*`: covered by Tasks 1, 3, 5, 6, and 7.
- P5 `fs/changed`: covered by Tasks 1, 3, 5, and 7.
- P5 `command/exec`: covered by Tasks 1, 4, 5, 6, and 7.
- P5 command output: covered by buffered `CommandExecResponse` in Tasks 4-6. `command/exec/outputDelta` is schema-declared but not advertised by buffered-only service availability.
- PTY-only `write`/`terminate`/`resize`: intentionally not marked implemented unless the worker extends Task 4 with a sandboxed PTY service and adds tests proving write, resize, and terminate. This is documented in Tasks 4 and 7.

Placeholder scan:

- No forbidden placeholder markers or vague edge-case steps.
- Every code-changing step includes concrete code snippets and exact commands.

Type consistency:

- `FsReadFileParams`, `FsReadFileResponse`, `CommandExecParams`, and `CommandExecResponse` names are used consistently across protocol, services, and routes.
- `command_exec` capability id is distinct from existing P3 `tools` and `item/commandExecution/*` event names.
