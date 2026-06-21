# Dasclaw App Server Streaming Command Exec Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **Status 2026-06-21:** P5 PTY-backed streaming `command/exec` capability is implemented and verified by targeted tests; completed implementation steps are checked and struck through below. Non-PTY split stdout/stderr streaming remains explicitly out of scope.

**Goal:** Build the remaining Codex-compatible standalone command execution capabilities in Dasclaw app-server: `command/exec/outputDelta`, `command/exec/write`, `command/exec/terminate`, and `command/exec/resize`.

**Architecture:** Keep the existing P5 buffered `command/exec` path intact for one-shot commands, and add a PTY-backed streaming path for interactive commands. Streaming commands are tracked by `processId`, emit `command/exec/outputDelta` notifications while running, and accept follow-up control requests through the existing app-server command service trait; the stdio loop must dispatch streaming `command/exec` in the background so `write`, `terminate`, and `resize` can be processed before the final `command/exec` response is ready.

**Tech Stack:** Rust 2024, JSON-RPC, `serde`, `base64`, `portable-pty` through `dasclaw_pty`, `std::sync::mpsc`, existing `AppServerServices`, `cargo nextest`, `cargo fmt`, `scripts/check_no_panics.py`.

---

## Evidence And Scope

4-question gate:

| Question | Answer | Evidence |
|---|---|---|
| 是否新增模块 / crate / 文件？ | Yes. This plan creates one plan document and implementation will modify app-server plus PTY support. | Ran semantic search before writing: `semantic_search_nodes_tool` for app-server command streaming found lower-level PTY functions, not an app-server streaming owner. |
| 结论是否含否定语？ | Yes. This plan says the current app-server path does not yet expose the complex command controls. | `rg` evidence: `crates/dasclaw_app_server/src/command_service.rs:197-229` rejects `tty`, `streamStdin`, `streamStdoutStderr`, and terminal size; `:295-326` returns unavailable for `write`, `terminate`, `resize`; `:336-346` advertises only buffered `exec`. |
| 是否做跨项目对账？ | Yes. Codex CLI is used as reference for the missing command execution behavior. | Subagent analysis found Codex implementation in `codex-cli-main/codex-rs/app-server/src/command_exec.rs` and protocol mappings in `codex-cli-main/codex-rs/app-server-protocol/src/protocol/common.rs`. |
| 是否写架构对账类文档？ | Yes. This is an implementation plan with architecture choices. | Semantic search plus exact `rg` evidence are recorded here; execution should rerun LSP symbol/reference checks if `execute_lsp` is available in that session. |

Reference facts:

- Current Dasclaw protocol already defines the methods/events and DTOs:
  - `crates/dasclaw_app_server_protocol/src/lib.rs:51-54` has `command/exec`, `command/exec/write`, `command/exec/terminate`, `command/exec/resize`.
  - `crates/dasclaw_app_server_protocol/src/lib.rs:96` has `command/exec/outputDelta`.
  - `crates/dasclaw_app_server_protocol/src/lib.rs:2055-2149` has `CommandExecOutputStream`, terminal size, params, response, output delta, write, terminate, and resize DTOs.
- Current Dasclaw app-server already routes these requests:
  - `crates/dasclaw_app_server/src/lib.rs:886-916` exposes service calls.
  - `crates/dasclaw_app_server/src/lib.rs:1442-1460` routes JSON-RPC methods.
  - `crates/dasclaw_app_server/src/lib.rs:1778-1790` drains command output delta service events.
- Current command service is buffered only:
  - `crates/dasclaw_app_server/src/command_service.rs:251-292` executes a command and returns one final response.
  - `crates/dasclaw_app_server/src/command_service.rs:295-326` fails follow-up methods.
- Dasclaw has lower-level PTY capability:
  - `crates/dasclaw_pty/src/lib.rs:187-218` exposes `spawn`, `write`, `read`, `resize`, `kill`, and wait.
  - `crates/dasclaw_pty/src/portable.rs:54-63` already splits the underlying `portable-pty` master into a reader and writer internally.
- Codex CLI reference behavior:
  - `codex-cli-main/codex-rs/app-server/src/command_exec.rs:48-69` has a session map keyed by connection/process.
  - `codex-cli-main/codex-rs/app-server/src/command_exec.rs:311-372` handles `write`, `terminate`, and `resize`.
  - `codex-cli-main/codex-rs/app-server/src/command_exec.rs:566-613` emits `CommandExecOutputDelta`.

Scope:

- Implement PTY-backed streaming `command/exec` when any of `tty`, `streamStdin`, or `streamStdoutStderr` is true.
- Require client-supplied `processId` for streaming/interactive execution, matching the Codex reference.
- `tty: true` implies stdin streaming and output streaming.
- Emit `command/exec/outputDelta` with `stream: "stdout"` for PTY output, because PTY output is a merged terminal stream.
- Support `command/exec/write` for active streaming sessions, including `deltaBase64` and `closeStdin`.
- Support `command/exec/terminate` for active streaming sessions.
- Support `command/exec/resize` only for PTY-backed active sessions.
- Keep non-streaming buffered execution behavior unchanged.
- Keep non-PTY split stdout/stderr streaming fail-safe in this plan. That requires a streaming sandbox/process backend separate from PTY and should not be claimed as done by this work.
- Update the gap matrix only for the PTY-backed streaming capability that is proven by tests, and add the note that non-PTY split streaming remains out of this slice.

Commit message discipline for this task family:

```text
已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。
```

## File Structure

- Modify: `crates/dasclaw_pty/src/lib.rs`
  - Add a split PTY process API so one thread can read while another sends write/resize/kill controls.

- Modify: `crates/dasclaw_pty/src/portable.rs`
  - Reuse existing `portable-pty` reader/writer split to implement the new split process API.

- Modify: `crates/dasclaw_app_server/Cargo.toml`
  - Add `dasclaw_pty = { path = "../dasclaw_pty" }`.

- Modify: `crates/dasclaw_app_server/src/command_service.rs`
  - Add active session table keyed by `processId`.
  - Keep existing buffered `command/exec` path for non-streaming commands.
  - Add PTY streaming path and follow-up controls.

- Modify: `crates/dasclaw_app_server/src/app_services.rs`
  - Add test helper for a fully streaming command service availability surface if needed by route tests.

- Modify: `crates/dasclaw_app_server/src/lib.rs`
  - Add detached JSON-RPC handling for streaming `command/exec`.
  - Continue draining command output delta notifications during the stdio loop.

- Modify: `crates/dasclaw_app_server/src/main.rs`
  - Add stdio integration tests for background streaming command execution.

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Strike only the rows covered by verified PTY-backed streaming behavior.

## Task 1: Split PTY Process API

**Files:**
- Modify: `crates/dasclaw_pty/src/lib.rs`
- Modify: `crates/dasclaw_pty/src/portable.rs`

- [x] ~~**Step 1: Write failing split PTY test**~~

Append this test to `crates/dasclaw_pty/src/tests.rs`:

```rust
#[cfg(unix)]
#[test]
fn split_process_allows_read_write_and_wait_without_shared_child_lock() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    let backend = crate::PortablePtyBackend::new();
    let options = crate::PtySpawnOptions::new("sh")
        .with_args(["-c", "printf ready; read line; printf \"got:$line\""]);
    let mut process = backend.spawn_process(options).expect("spawn split pty process");

    let mut reader = process.reader;
    let mut writer = process.writer;
    let mut control = process.control;

    let reader_thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        let mut buf = [0_u8; 128];
        while Instant::now() < deadline {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    output.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&output);
                    if text.contains("got:hello") {
                        return text.into_owned();
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => panic!("pty read failed: {error}"),
            }
        }
        String::from_utf8_lossy(&output).into_owned()
    });

    writer.write_all(b"hello\n").expect("write stdin");
    let status = control
        .wait_for_exit(Some(Duration::from_secs(5)))
        .expect("wait for shell exit");
    let output = reader_thread.join().expect("reader thread");

    assert!(status.is_success(), "status: {status:?}, output: {output:?}");
    assert!(output.contains("ready"), "output: {output:?}");
    assert!(output.contains("got:hello"), "output: {output:?}");
}
```

- [x] ~~**Step 2: Run the split PTY test and verify it fails**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_pty split_process_allows_read_write_and_wait_without_shared_child_lock
```

Expected: FAIL with an error that `spawn_process` does not exist on `PortablePtyBackend`.

- [x] ~~**Step 3: Add split process types**~~

In `crates/dasclaw_pty/src/lib.rs`, add `Read` and `Write` to the imports:

```rust
use std::io::{Read, Write};
```

Then add these public types after `PtyChild`:

```rust
/// Split PTY process handle for concurrent read, write, resize, kill, and wait.
///
/// `PtyChild` is intentionally simple and blocking. App-server streaming needs
/// independent reader/writer/control ownership, otherwise a blocking read can
/// hold the only mutable child handle and starve `write` or `resize`.
pub struct PtyProcess {
    /// Read side of the PTY master. PTY output is a merged terminal stream.
    pub reader: Box<dyn Read + Send>,
    /// Write side of the PTY master.
    pub writer: Box<dyn Write + Send>,
    /// Process and terminal control side.
    pub control: Box<dyn PtyProcessControl>,
}

/// Control half of a split PTY process.
pub trait PtyProcessControl: Send {
    /// Adjust terminal size.
    fn resize(&mut self, size: PtySize) -> Result<(), PtyError>;

    /// Kill the child process. Already-exited children return `Ok(())`.
    fn kill(&mut self) -> Result<(), PtyError>;

    /// Non-blocking process status check.
    fn try_wait(&mut self) -> Result<PtyExitStatus, PtyError>;

    /// Wait for process exit, optionally bounded by `timeout`.
    fn wait_for_exit(&mut self, timeout: Option<Duration>) -> Result<PtyExitStatus, PtyError>;
}

/// PTY backends that can return split reader/writer/control handles.
pub trait StreamingPty: Pty {
    /// Start a process with independent IO/control handles.
    fn spawn_process(&self, options: PtySpawnOptions) -> Result<PtyProcess, PtyError>;
}
```

- [x] ~~**Step 4: Implement the split API for `PortablePtyBackend`**~~

In `crates/dasclaw_pty/src/portable.rs`, update the import:

```rust
use crate::{
    Pty, PtyChild, PtyError, PtyExitStatus, PtyProcess, PtyProcessControl, PtySize,
    PtySpawnOptions, StreamingPty,
};
```

Replace the body of `impl Pty for PortablePtyBackend` with:

```rust
impl Pty for PortablePtyBackend {
    fn spawn(&self, options: PtySpawnOptions) -> Result<Box<dyn PtyChild>, PtyError> {
        let process = self.spawn_process(options)?;
        Ok(Box::new(PortablePtyChild {
            reader: process.reader,
            writer: process.writer,
            control: process.control,
        }))
    }
}
```

Add this implementation below it:

```rust
impl StreamingPty for PortablePtyBackend {
    fn spawn_process(&self, options: PtySpawnOptions) -> Result<PtyProcess, PtyError> {
        let pty_system: Box<dyn PtySystem> = native_pty_system();
        let pair: PtyPair = pty_system
            .openpty(to_portable_size(options.size))
            .map_err(|e| PtyError::Open(e.to_string()))?;

        let mut cmd = CommandBuilder::new(&options.program);
        cmd.args(&options.args);
        if let Some(cwd) = options.cwd.as_ref() {
            cmd.cwd(cwd);
        }
        for (k, v) in &options.env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Open(format!("clone reader: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Open(format!("take writer: {e}")))?;

        drop(pair.slave);

        Ok(PtyProcess {
            reader,
            writer,
            control: Box::new(PortablePtyControl {
                master: pair.master,
                child,
            }),
        })
    }
}
```

Replace `PortablePtyChild` with:

```rust
struct PortablePtyChild {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    control: Box<dyn PtyProcessControl>,
}
```

Add the control type:

```rust
struct PortablePtyControl {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtyProcessControl for PortablePtyControl {
    fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        self.master
            .resize(to_portable_size(size))
            .map_err(|e| PtyError::Resize(e.to_string()))
    }

    fn kill(&mut self) -> Result<(), PtyError> {
        if let Ok(Some(_)) = self.child.try_wait() {
            return Ok(());
        }
        match self.child.kill() {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) if e.raw_os_error() == Some(3) => Ok(()),
            Err(e) => Err(PtyError::Kill(e)),
        }
    }

    fn try_wait(&mut self) -> Result<PtyExitStatus, PtyError> {
        match self.child.try_wait() {
            Ok(None) => Ok(PtyExitStatus::Running),
            Ok(Some(status)) => Ok(map_exit_status(status)),
            Err(e) => Err(PtyError::Wait(e.to_string())),
        }
    }

    fn wait_for_exit(&mut self, timeout: Option<Duration>) -> Result<PtyExitStatus, PtyError> {
        wait_for_exit_with(timeout, |control| control.try_wait(), self)
    }
}
```

Replace the `PtyChild` control methods with delegation:

```rust
impl PtyChild for PortablePtyChild {
    fn write(&mut self, buf: &[u8]) -> Result<usize, PtyError> {
        self.writer.write(buf).map_err(PtyError::Write)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        self.reader.read(buf).map_err(PtyError::Read)
    }

    fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        self.control.resize(size)
    }

    fn kill(&mut self) -> Result<(), PtyError> {
        self.control.kill()
    }

    fn try_wait(&mut self) -> Result<PtyExitStatus, PtyError> {
        self.control.try_wait()
    }

    fn wait_for_exit(&mut self, timeout: Option<Duration>) -> Result<PtyExitStatus, PtyError> {
        self.control.wait_for_exit(timeout)
    }
}
```

Add this helper above `to_portable_size`:

```rust
fn wait_for_exit_with<T>(
    timeout: Option<Duration>,
    mut try_wait: impl FnMut(&mut T) -> Result<PtyExitStatus, PtyError>,
    target: &mut T,
) -> Result<PtyExitStatus, PtyError> {
    const POLL_INTERVAL: Duration = Duration::from_millis(25);
    let deadline = timeout.map(|t| (Instant::now() + t, t));

    loop {
        match try_wait(target)? {
            PtyExitStatus::Running => {}
            done => return Ok(done),
        }

        if let Some((deadline, total)) = deadline
            && Instant::now() >= deadline
        {
            return Err(PtyError::WaitTimeout(total));
        }

        thread::sleep(POLL_INTERVAL);
    }
}
```

- [x] ~~**Step 5: Run the PTY crate tests**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_pty
```

Expected: PASS.

- [x] ~~**Step 6: Commit**~~

```bash
git add crates/dasclaw_pty/src/lib.rs crates/dasclaw_pty/src/portable.rs crates/dasclaw_pty/src/tests.rs
git commit -m "feat: expose split PTY process handles" -m "已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。"
```

## Task 2: Command Service Streaming Session Table

**Files:**
- Modify: `crates/dasclaw_app_server/Cargo.toml`
- Modify: `crates/dasclaw_app_server/src/command_service.rs`

- [x] ~~**Step 1: Add focused failing service tests**~~

Append these tests inside `#[cfg(test)] mod tests` in `crates/dasclaw_app_server/src/command_service.rs`:

```rust
#[cfg(unix)]
fn wait_for_output_delta(
    service: &AppServerCommandExecService,
    process_id: &str,
    needle: &str,
) -> Vec<CommandExecOutputDeltaNotification> {
    use base64::Engine as _;
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut collected = Vec::new();
    while Instant::now() < deadline {
        collected.extend(service.drain_output_delta_events());
        let decoded = collected
            .iter()
            .filter(|event| event.process_id == process_id)
            .filter_map(|event| {
                base64::engine::general_purpose::STANDARD
                    .decode(&event.delta_base64)
                    .ok()
            })
            .flat_map(|chunk| String::from_utf8_lossy(&chunk).into_owned().into_bytes())
            .collect::<Vec<_>>();
        let text = String::from_utf8_lossy(&decoded);
        if text.contains(needle) {
            return collected;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    collected
}

#[cfg(unix)]
#[test]
fn command_service_streaming_pty_emits_output_delta_and_accepts_write() {
    use base64::Engine as _;
    use std::sync::Arc;
    use std::time::Duration;

    let temp = tempfile::tempdir().expect("tempdir");
    let service = Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
    let mut params = exec_params(vec![
        "sh",
        "-c",
        "printf ready; read line; printf \"got:$line\"",
    ]);
    params.process_id = Some("stream_write".to_string());
    params.tty = Some(true);
    params.stream_stdin = Some(true);
    params.stream_stdout_stderr = Some(true);

    let exec_service = Arc::clone(&service);
    let exec_thread = std::thread::spawn(move || exec_service.exec(params).expect("exec response"));

    let ready_events = wait_for_output_delta(&service, "stream_write", "ready");
    assert!(!ready_events.is_empty(), "expected ready output delta");

    service
        .write(CommandExecWriteParams {
            process_id: "stream_write".to_string(),
            delta_base64: Some(
                base64::engine::general_purpose::STANDARD.encode("hello\n"),
            ),
            close_stdin: None,
        })
        .expect("write succeeds");

    let output_events = wait_for_output_delta(&service, "stream_write", "got:hello");
    let response = exec_thread.join().expect("exec thread");

    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");
    assert!(output_events.iter().any(|event| !event.cap_reached));
    assert_eq!(service.availability().output_delta_events, true);
    assert_eq!(service.availability().write, true);
    assert_eq!(service.availability().terminate, true);
    assert_eq!(service.availability().resize, true);

    std::thread::sleep(Duration::from_millis(25));
    assert!(service.drain_output_delta_events().is_empty());
}

#[cfg(unix)]
#[test]
fn command_service_streaming_pty_terminate_stops_running_process() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = std::sync::Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
    let mut params = exec_params(vec!["sh", "-c", "while true; do printf tick; sleep 1; done"]);
    params.process_id = Some("stream_terminate".to_string());
    params.tty = Some(true);
    params.stream_stdout_stderr = Some(true);

    let exec_service = std::sync::Arc::clone(&service);
    let exec_thread = std::thread::spawn(move || exec_service.exec(params).expect("exec response"));

    let events = wait_for_output_delta(&service, "stream_terminate", "tick");
    assert!(!events.is_empty(), "expected tick output before terminate");

    service
        .terminate(CommandExecTerminateParams {
            process_id: "stream_terminate".to_string(),
        })
        .expect("terminate succeeds");
    let response = exec_thread.join().expect("exec thread");

    assert_ne!(response.exit_code, 0);
    assert!(
        service
            .terminate(CommandExecTerminateParams {
                process_id: "stream_terminate".to_string(),
            })
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn command_service_streaming_pty_resize_accepts_active_session() {
    use base64::Engine as _;

    let temp = tempfile::tempdir().expect("tempdir");
    let service = std::sync::Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
    let mut params = exec_params(vec!["sh", "-c", "printf ready; read line"]);
    params.process_id = Some("stream_resize".to_string());
    params.tty = Some(true);
    params.stream_stdin = Some(true);
    params.stream_stdout_stderr = Some(true);
    params.size = Some(CommandExecTerminalSize { cols: 100, rows: 30 });

    let exec_service = std::sync::Arc::clone(&service);
    let exec_thread = std::thread::spawn(move || exec_service.exec(params).expect("exec response"));

    let events = wait_for_output_delta(&service, "stream_resize", "ready");
    assert!(!events.is_empty(), "expected ready output before resize");

    service
        .resize(CommandExecResizeParams {
            process_id: "stream_resize".to_string(),
            size: CommandExecTerminalSize { cols: 120, rows: 40 },
        })
        .expect("resize succeeds");
    service
        .write(CommandExecWriteParams {
            process_id: "stream_resize".to_string(),
            delta_base64: Some(base64::engine::general_purpose::STANDARD.encode("\n")),
            close_stdin: None,
        })
        .expect("write exit newline");
    let response = exec_thread.join().expect("exec thread");

    assert_eq!(response.exit_code, 0);
}

#[test]
fn command_service_streaming_requires_client_process_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());
    let mut params = exec_params(vec!["rustc", "--version"]);
    params.tty = Some(true);

    let error = service.exec(params).expect_err("missing processId should fail");

    assert!(error.to_string().contains("processId"));
}

#[test]
fn command_service_non_tty_streaming_fails_safe_until_pipe_backend_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = AppServerCommandExecService::new(temp.path().to_path_buf());
    let mut params = exec_params(vec!["rustc", "--version"]);
    params.process_id = Some("pipe_stream".to_string());
    params.stream_stdout_stderr = Some(true);
    params.tty = Some(false);

    let error = service.exec(params).expect_err("non-tty streaming unsupported");

    assert!(
        error
            .to_string()
            .contains("non-tty streamStdoutStderr requires a streaming pipe backend")
    );
}
```

- [x] ~~**Step 2: Run the service tests and verify they fail**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(command_service_streaming_pty) | test(command_service_streaming_requires_client_process_id) | test(command_service_non_tty_streaming_fails_safe_until_pipe_backend_exists)'
```

Expected: FAIL because `tty` and streaming options are still rejected by `reject_unsupported_exec_options`.

- [x] ~~**Step 3: Add the PTY dependency**~~

In `crates/dasclaw_app_server/Cargo.toml`, add this dependency beside the other Dasclaw crates:

```toml
dasclaw_pty = { path = "../dasclaw_pty" }
```

- [x] ~~**Step 4: Add session/control structures**~~

In `crates/dasclaw_app_server/src/command_service.rs`, update imports:

```rust
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use dasclaw_pty::{PtyExitStatus, PtyProcessControl, PtySize, PtySpawnOptions, StreamingPty};
```

Change `AppServerCommandExecService` fields to:

```rust
pub struct AppServerCommandExecService {
    root: PathBuf,
    lexical_root: PathBuf,
    workspace: Result<WorkspaceCapability, String>,
    runtime: Mutex<Option<Result<BlockingTokioRuntime, AppServerError>>>,
    running: Mutex<HashMap<String, RunningCommand>>,
    completed: Mutex<HashMap<String, CompletedCommand>>,
    output_delta_events: std::sync::Arc<Mutex<Vec<CommandExecOutputDeltaNotification>>>,
}
```

Update `new` accordingly:

```rust
Self {
    root,
    lexical_root,
    workspace,
    runtime: Mutex::new(None),
    running: Mutex::new(HashMap::new()),
    completed: Mutex::new(HashMap::new()),
    output_delta_events: std::sync::Arc::new(Mutex::new(Vec::new())),
}
```

Add these structs/enums after `CompletedCommand`:

```rust
#[derive(Debug)]
struct RunningCommand {
    control_tx: mpsc::Sender<CommandControl>,
    tty: bool,
}

#[derive(Debug)]
enum CommandControl {
    Write(Vec<u8>),
    CloseStdin,
    Terminate,
    Resize(CommandExecTerminalSize),
}

#[derive(Debug)]
struct StreamingCommandOptions {
    process_id: String,
    cwd: PathBuf,
    command: Vec<String>,
    env: HashMap<String, String>,
    timeout: Option<Duration>,
    output_bytes_cap: Option<usize>,
    size: CommandExecTerminalSize,
    stream_stdin: bool,
    stream_stdout_stderr: bool,
    tty: bool,
}
```

- [x] ~~**Step 5: Split buffered and streaming validation**~~

Replace `reject_unsupported_exec_options` with:

```rust
fn reject_unsupported_buffered_exec_options(
    params: &CommandExecParams,
) -> Result<(), AppServerError> {
    if params.tty.unwrap_or(false) {
        return Err(command_unavailable("tty requires streaming command support"));
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
    if params.sandbox_policy.is_some() {
        return Err(command_unavailable(
            "sandboxPolicy is not supported by the buffered sandbox executor",
        ));
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

fn streaming_requested(params: &CommandExecParams) -> bool {
    params.tty.unwrap_or(false)
        || params.stream_stdin.unwrap_or(false)
        || params.stream_stdout_stderr.unwrap_or(false)
}
```

Add this builder:

```rust
fn streaming_options(&self, params: CommandExecParams) -> Result<StreamingCommandOptions, AppServerError> {
    if params.sandbox_policy.is_some() {
        return Err(command_unavailable(
            "sandboxPolicy is not supported by the PTY streaming executor",
        ));
    }
    if params.disable_output_cap.unwrap_or(false) {
        return Err(command_unavailable(
            "disableOutputCap is not supported by the PTY streaming executor",
        ));
    }

    let tty = params.tty.unwrap_or(false);
    let stream_stdin = tty || params.stream_stdin.unwrap_or(false);
    let stream_stdout_stderr = tty || params.stream_stdout_stderr.unwrap_or(false);
    if !tty && stream_stdout_stderr {
        return Err(command_unavailable(
            "non-tty streamStdoutStderr requires a streaming pipe backend",
        ));
    }
    if !tty && stream_stdin {
        return Err(command_unavailable(
            "non-tty streamStdin requires a streaming pipe backend",
        ));
    }

    let Some(process_id) = params.process_id.clone().filter(|id| !id.trim().is_empty()) else {
        return Err(AppServerError::invalid_request(
            COMMAND_EXEC_CAPABILITY,
            "streaming command/exec requires a client-supplied processId",
        ));
    };

    let timeout = if params.disable_timeout.unwrap_or(false) {
        None
    } else {
        Some(
            params
                .timeout_ms
                .map(Duration::from_millis)
                .unwrap_or(DEFAULT_TIMEOUT),
        )
    };
    let cwd = self.resolve_cwd(params.cwd.as_deref())?;
    let size = params
        .size
        .unwrap_or(CommandExecTerminalSize { cols: 80, rows: 24 });

    Ok(StreamingCommandOptions {
        process_id,
        cwd,
        command: params.command,
        env: Self::env(params.env),
        timeout,
        output_bytes_cap: params.output_bytes_cap,
        size,
        stream_stdin,
        stream_stdout_stderr,
        tty,
    })
}
```

- [x] ~~**Step 6: Add streaming execution helpers**~~

Add these helper functions inside `impl AppServerCommandExecService`:

```rust
fn exec_streaming(&self, params: CommandExecParams) -> Result<CommandExecResponse, AppServerError> {
    let options = self.streaming_options(params)?;
    if options.command.is_empty() {
        return Err(AppServerError::invalid_request(
            COMMAND_EXEC_CAPABILITY,
            "command must contain at least one argument",
        ));
    }
    self.ensure_process_id_available(&options.process_id)?;

    let backend = dasclaw_pty::default_backend();
    let pty_size = PtySize {
        rows: options.size.rows,
        cols: options.size.cols,
        pixel_width: 0,
        pixel_height: 0,
    };
    let spawn_options = PtySpawnOptions {
        program: options.command[0].clone(),
        args: options.command[1..].to_vec(),
        cwd: Some(options.cwd.clone()),
        env: options.env.clone(),
        size: pty_size,
    };
    let process = backend
        .spawn_process(spawn_options)
        .map_err(|error| command_unavailable(error.to_string()))?;

    self.run_streaming_process(options, process)
}

fn ensure_process_id_available(&self, process_id: &str) -> Result<(), AppServerError> {
    if self
        .running
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .contains_key(process_id)
    {
        return Err(AppServerError::invalid_request(
            COMMAND_EXEC_CAPABILITY,
            format!("command process already running: {process_id}"),
        ));
    }
    Ok(())
}

fn run_streaming_process(
    &self,
    options: StreamingCommandOptions,
    process: dasclaw_pty::PtyProcess,
) -> Result<CommandExecResponse, AppServerError> {
    let process_id = options.process_id.clone();
    let (control_tx, control_rx) = mpsc::channel::<CommandControl>();
    self.running
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(
            process_id.clone(),
            RunningCommand {
                control_tx,
                tty: options.tty,
            },
        );

    let event_sink = std::sync::Arc::clone(&self.output_delta_events);
    let reader_process_id = process_id.clone();
    let reader_cap = options.output_bytes_cap;
    let reader = process.reader;
    let reader_thread = thread::spawn(move || {
        read_pty_output_to_events(reader, reader_process_id, event_sink, reader_cap);
    });

    let started_at = Instant::now();
    let mut writer = process.writer;
    let mut control = process.control;
    let exit_code = drive_streaming_process(
        &mut writer,
        &mut *control,
        control_rx,
        options.timeout,
        started_at,
    );

    let _ = reader_thread.join();
    self.running
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .remove(&process_id);

    let response = CommandExecResponse {
        exit_code,
        stdout: String::new(),
        stderr: String::new(),
    };
    self.completed
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(
            process_id,
            CompletedCommand {
                response: response.clone(),
            },
        );
    Ok(response)
}
```

Add these free functions near the existing helper functions:

```rust
fn read_pty_output_to_events(
    mut reader: Box<dyn Read + Send>,
    process_id: String,
    event_sink: std::sync::Arc<Mutex<Vec<CommandExecOutputDeltaNotification>>>,
    output_bytes_cap: Option<usize>,
) {
    let mut emitted = 0_usize;
    let mut cap_reached = false;
    let mut buf = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };

        let mut chunk = buf[..read].to_vec();
        if let Some(cap) = output_bytes_cap {
            if emitted >= cap {
                cap_reached = true;
                continue;
            }
            let remaining = cap - emitted;
            if chunk.len() > remaining {
                chunk.truncate(remaining);
                cap_reached = true;
            }
        }
        emitted += chunk.len();
        if chunk.is_empty() {
            continue;
        }
        let event = CommandExecOutputDeltaNotification {
            process_id: process_id.clone(),
            stream: dasclaw_app_server_protocol::CommandExecOutputStream::Stdout,
            delta_base64: base64::engine::general_purpose::STANDARD.encode(chunk),
            cap_reached,
        };
        event_sink
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(event);
    }
}

fn drive_streaming_process(
    writer: &mut Box<dyn Write + Send>,
    control: &mut dyn PtyProcessControl,
    control_rx: mpsc::Receiver<CommandControl>,
    timeout: Option<Duration>,
    started_at: Instant,
) -> i32 {
    loop {
        while let Ok(command) = control_rx.try_recv() {
            match command {
                CommandControl::Write(bytes) => {
                    let _ = writer.write_all(&bytes);
                    let _ = writer.flush();
                }
                CommandControl::CloseStdin => {
                    let _ = writer.flush();
                }
                CommandControl::Terminate => {
                    let _ = control.kill();
                }
                CommandControl::Resize(size) => {
                    let _ = control.resize(PtySize {
                        rows: size.rows,
                        cols: size.cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        }

        match control.try_wait() {
            Ok(PtyExitStatus::Exited(code)) => return code,
            Ok(PtyExitStatus::Signaled) => return -1,
            Ok(PtyExitStatus::Running) => {}
            Err(_) => return -1,
        }

        if let Some(timeout) = timeout
            && started_at.elapsed() >= timeout
        {
            let _ = control.kill();
            return -1;
        }

        thread::sleep(Duration::from_millis(10));
    }
}
```

- [x] ~~**Step 7: Route `exec` to buffered or streaming path**~~

In `impl CommandExecService for AppServerCommandExecService`, replace the beginning of `exec` with:

```rust
fn exec(&self, params: CommandExecParams) -> Result<CommandExecResponse, AppServerError> {
    if let Err(message) = &self.workspace {
        return Err(command_unavailable(format!(
            "workspace capability unavailable: {message}"
        )));
    }
    if Self::streaming_requested(&params) {
        return self.exec_streaming(params);
    }
    Self::reject_unsupported_buffered_exec_options(&params)?;
```

Keep the existing buffered body after this validation.

- [x] ~~**Step 8: Implement follow-up controls and availability**~~

Replace `write`, `terminate`, `resize`, and `availability` with:

```rust
fn write(
    &self,
    params: CommandExecWriteParams,
) -> Result<CommandExecWriteResponse, AppServerError> {
    let sender = self.running_sender(&params.process_id)?;
    if let Some(delta_base64) = params.delta_base64 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(delta_base64)
            .map_err(|error| {
                AppServerError::invalid_request(
                    COMMAND_EXEC_CAPABILITY,
                    format!("invalid command stdin deltaBase64: {error}"),
                )
            })?;
        sender
            .send(CommandControl::Write(bytes))
            .map_err(|_| command_unavailable("command process is no longer running"))?;
    }
    if params.close_stdin.unwrap_or(false) {
        sender
            .send(CommandControl::CloseStdin)
            .map_err(|_| command_unavailable("command process is no longer running"))?;
    }
    Ok(CommandExecWriteResponse {})
}

fn terminate(
    &self,
    params: CommandExecTerminateParams,
) -> Result<CommandExecTerminateResponse, AppServerError> {
    let sender = self.running_sender(&params.process_id)?;
    sender
        .send(CommandControl::Terminate)
        .map_err(|_| command_unavailable("command process is no longer running"))?;
    Ok(CommandExecTerminateResponse {})
}

fn resize(
    &self,
    params: CommandExecResizeParams,
) -> Result<CommandExecResizeResponse, AppServerError> {
    let running = self
        .running
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(command) = running.get(&params.process_id) else {
        drop(running);
        return Err(completed_or_unknown_process_error(
            &self.completed,
            &params.process_id,
            "command resize is unavailable after process exit",
        ));
    };
    if !command.tty {
        return Err(command_unavailable(
            "command resize is available only for PTY-backed executions",
        ));
    }
    command
        .control_tx
        .send(CommandControl::Resize(params.size))
        .map_err(|_| command_unavailable("command process is no longer running"))?;
    Ok(CommandExecResizeResponse {})
}

fn availability(&self) -> CommandExecAvailability {
    if self.workspace.is_err() {
        return CommandExecAvailability::default();
    }
    CommandExecAvailability {
        exec: true,
        output_delta_events: true,
        terminate: true,
        write: true,
        resize: true,
    }
}
```

Add `running_sender` inside the impl:

```rust
fn running_sender(&self, process_id: &str) -> Result<mpsc::Sender<CommandControl>, AppServerError> {
    let running = self
        .running
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(command) = running.get(process_id) {
        return Ok(command.control_tx.clone());
    }
    drop(running);
    Err(completed_or_unknown_process_error(
        &self.completed,
        process_id,
        "command control is unavailable after process exit",
    ))
}
```

- [x] ~~**Step 9: Update health message**~~

In `health`, replace the ready message with:

```rust
health.message = Some(format!(
    "root={} cwd_guard=root-contained sandbox=read-only/no-network read_scope=host-read-only not_workspace_read_limited non_interactive=true streaming=pty tty=true non_tty_streaming=false",
    self.root.display()
));
```

- [x] ~~**Step 10: Run command service tests**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(command_service_streaming_pty) | test(command_service_streaming_requires_client_process_id) | test(command_service_non_tty_streaming_fails_safe_until_pipe_backend_exists) | test(command_service_exec_captures_stdout_without_streaming_delta) | test(command_service_follow_up_methods_fail_safe_without_pty_process)'
```

Expected: PASS.

- [x] ~~**Step 11: Commit**~~

```bash
git add crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/command_service.rs
git commit -m "feat: add PTY-backed app-server command sessions" -m "已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。"
```

## Task 3: Capability Tests And Test Fakes

**Files:**
- Modify: `crates/dasclaw_app_server/src/app_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] ~~**Step 1: Add a streaming-ready test fake**~~

In `crates/dasclaw_app_server/src/app_services.rs`, add this constructor to `impl TestCommandExecService`:

```rust
pub fn ready_streaming() -> Self {
    Self {
        availability: CommandExecAvailability {
            exec: true,
            output_delta_events: true,
            terminate: true,
            write: true,
            resize: true,
        },
        output_delta_events: Arc::new(Mutex::new(Vec::new())),
    }
}
```

- [x] ~~**Step 2: Add capability test**~~

Append this test near `app_server_ready_p5_routes_reach_service_owners` in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn app_server_ready_p5_streaming_command_capabilities_are_advertised() {
    let services = app_services::AppServerServices::for_tests(
        app_services::TestLogService::ready(),
        app_services::TestJobService::ready(vec![]),
        app_services::TestSkillsService::ready(vec![]),
        app_services::TestMcpService::ready(vec![]),
        app_services::TestFsService::disabled(),
        app_services::TestCommandExecService::ready_streaming(),
    );
    let server = AppServer::new().with_app_services(services);

    let command = server.capabilities().capabilities.command_exec;

    assert_eq!(command.status, CapabilityStatus::Implemented);
    assert!(command.methods.contains(&method::COMMAND_EXEC.to_string()));
    assert!(
        command
            .methods
            .contains(&method::COMMAND_EXEC_WRITE.to_string())
    );
    assert!(
        command
            .methods
            .contains(&method::COMMAND_EXEC_TERMINATE.to_string())
    );
    assert!(
        command
            .methods
            .contains(&method::COMMAND_EXEC_RESIZE.to_string())
    );
    assert!(
        command
            .events
            .contains(&event::COMMAND_EXEC_OUTPUT_DELTA.to_string())
    );
}
```

- [x] ~~**Step 3: Run capability tests**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(app_server_ready_p5_streaming_command_capabilities_are_advertised) | test(app_server_p5_service_events_drain_to_notifications)'
```

Expected: PASS.

- [x] ~~**Step 4: Commit**~~

```bash
git add crates/dasclaw_app_server/src/app_services.rs crates/dasclaw_app_server/src/lib.rs
git commit -m "test: cover streaming command capability advertisement" -m "已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。"
```

## Task 4: Detached Stdio Streaming Command Dispatch

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] ~~**Step 1: Add failing stdio test**~~

Append this test in the `#[cfg(test)] mod tests` of `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[cfg(unix)]
#[test]
fn stdio_streaming_command_exec_accepts_write_before_final_exec_response() {
    use base64::Engine as _;
    use std::io::Cursor;

    let temp = TestDir::new("stdio_streaming_command_exec_accepts_write");
    let services = app_services::AppServerServices::for_tests(
        app_services::TestLogService::ready(),
        app_services::TestJobService::ready(vec![]),
        app_services::TestSkillsService::ready(vec![]),
        app_services::TestMcpService::ready(vec![]),
        app_services::TestFsService::disabled(),
        command_service::AppServerCommandExecService::new(temp.path().to_path_buf()),
    );
    let server = AppServer::new().with_app_services(services);
    let initialize = initialized_request_json();
    let exec = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "cmd",
        "method": "command/exec",
        "params": {
            "command": ["sh", "-c", "printf ready; read line; printf \"got:$line\""],
            "processId": "stdio_proc",
            "tty": true,
            "streamStdin": true,
            "streamStdoutStderr": true
        }
    });
    let write = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "write",
        "method": "command/exec/write",
        "params": {
            "processId": "stdio_proc",
            "deltaBase64": base64::engine::general_purpose::STANDARD.encode("hello\n")
        }
    });

    let input = format!("{initialize}\n{exec}\n{write}\n");
    let mut output = Vec::new();
    run_stdio_server_with_app_server(server, Cursor::new(input), &mut output)
        .expect("stdio server should complete");

    let text = String::from_utf8(output).expect("stdio output utf8");
    let values = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json line"))
        .collect::<Vec<_>>();

    assert!(
        values
            .iter()
            .any(|value| value["method"] == event::COMMAND_EXEC_OUTPUT_DELTA
                && value["params"]["processId"] == "stdio_proc")
    );
    assert!(
        values
            .iter()
            .any(|value| value["id"] == "write" && value["result"].is_object())
    );
    assert!(
        values
            .iter()
            .any(|value| value["id"] == "cmd"
                && value["result"]["exitCode"] == 0
                && value["result"]["stdout"] == "")
    );
}
```

- [x] ~~**Step 2: Run the stdio test and verify it fails**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server stdio_streaming_command_exec_accepts_write_before_final_exec_response
```

Expected: FAIL or hang until test timeout because the current stdio loop handles `command/exec` synchronously and does not process `command/exec/write` until the exec call returns.

- [x] ~~**Step 3: Add detached response plumbing**~~

In `crates/dasclaw_app_server/src/lib.rs`, add these types near `NotificationWrite`:

```rust
struct DetachedResponse {
    json: String,
}

enum DetachedCommandExecDispatch {
    NotDetached,
    Spawned,
    ImmediateResponse(String),
}
```

Add this method inside `impl AppServer`:

```rust
fn try_spawn_detached_command_exec(
    &self,
    input: &str,
    response_sender: mpsc::Sender<DetachedResponse>,
) -> DetachedCommandExecDispatch {
    let incoming = match serde_json::from_str::<JsonRpcIncoming>(input) {
        Ok(JsonRpcIncoming::Request(request)) => request,
        Ok(JsonRpcIncoming::ClientResponse(_)) => return DetachedCommandExecDispatch::NotDetached,
        Err(_) => return DetachedCommandExecDispatch::NotDetached,
    };
    if incoming.method != method::COMMAND_EXEC {
        return DetachedCommandExecDispatch::NotDetached;
    }
    let Some(params_value) = incoming.params.clone() else {
        return DetachedCommandExecDispatch::NotDetached;
    };
    let params = match serde_json::from_value::<CommandExecParams>(params_value) {
        Ok(params) => params,
        Err(_) => return DetachedCommandExecDispatch::NotDetached,
    };
    let streaming = params.tty.unwrap_or(false)
        || params.stream_stdin.unwrap_or(false)
        || params.stream_stdout_stderr.unwrap_or(false);
    if !streaming {
        return DetachedCommandExecDispatch::NotDetached;
    }
    if let Err(error) = self.require_initialized("command_exec") {
        return DetachedCommandExecDispatch::ImmediateResponse(serialize_response(
            &app_error_response(incoming.id.clone(), error),
        ));
    }

    let id = incoming.id.clone();
    let command = std::sync::Arc::clone(&self.app_services.command);
    thread::spawn(move || {
        let response = match command.exec(params) {
            Ok(result) => json_rpc_ok(id, result),
            Err(error) => app_error_response(id, error),
        };
        let _ = response_sender.send(DetachedResponse {
            json: serialize_response(&response),
        });
    });
    DetachedCommandExecDispatch::Spawned
}
```

- [x] ~~**Step 4: Drain detached responses in stdio loop**~~

In `run_stdio_server_with_app_server`, add a response channel before the loop:

```rust
let (detached_response_sender, detached_response_receiver) = mpsc::channel::<DetachedResponse>();
```

In the request branch, replace:

```rust
let response = server.handle_json_rpc(&line);
```

with:

```rust
let response = match server.try_spawn_detached_command_exec(
    &line,
    detached_response_sender.clone(),
) {
    DetachedCommandExecDispatch::Spawned => None,
    DetachedCommandExecDispatch::NotDetached => server.handle_json_rpc(&line),
    DetachedCommandExecDispatch::ImmediateResponse(response) => Some(response),
};
```

After each `write_pending_notifications` call, drain detached responses:

```rust
while let Ok(response) = detached_response_receiver.try_recv() {
    writeln!(writer, "{}", response.json)?;
}
```

Place the same drain before the final flush at the end of the function.

- [x] ~~**Step 5: Run the stdio test**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server stdio_streaming_command_exec_accepts_write_before_final_exec_response
```

Expected: PASS.

- [x] ~~**Step 6: Commit**~~

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "feat: dispatch streaming command exec without blocking stdio controls" -m "已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。"
```

## Task 5: JSON-RPC Real Route Coverage For Controls

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] ~~**Step 1: Add route-level tests for terminate and resize**~~

Append these tests near the existing real P5 command exec route tests:

```rust
#[cfg(unix)]
#[test]
fn app_server_real_p5_streaming_command_exec_route_terminate_completes_process() {
    let temp = TestDir::new("app_server_real_p5_streaming_terminate");
    let services = app_services::AppServerServices::for_tests(
        app_services::TestLogService::ready(),
        app_services::TestJobService::ready(vec![]),
        app_services::TestSkillsService::ready(vec![]),
        app_services::TestMcpService::ready(vec![]),
        app_services::TestFsService::disabled(),
        command_service::AppServerCommandExecService::new(temp.path().to_path_buf()),
    );
    let service = std::sync::Arc::clone(&services.command);
    let mut params = CommandExecParams {
        command: vec![
            "sh".to_string(),
            "-c".to_string(),
            "while true; do printf tick; sleep 1; done".to_string(),
        ],
        cwd: None,
        timeout_ms: Some(10_000),
        disable_timeout: None,
        output_bytes_cap: None,
        disable_output_cap: None,
        env: Default::default(),
        process_id: Some("route_stream_terminate".to_string()),
        sandbox_policy: None,
        size: Some(CommandExecTerminalSize { cols: 80, rows: 24 }),
        stream_stdin: Some(false),
        stream_stdout_stderr: Some(true),
        tty: Some(true),
    };
    let exec_thread = std::thread::spawn(move || service.exec(params).expect("exec response"));
    let mut server = AppServer::new().with_app_services(services);
    server
        .handle_json_rpc(initialized_request_json())
        .expect("initialize should return a response");
    let _ = server.drain_notifications();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let terminate = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"terminate","method":"command/exec/terminate","params":{"processId":"route_stream_terminate"}}"#,
        )
        .expect("terminate should return response");
    let response = exec_thread.join().expect("exec thread");
    let value: Value = serde_json::from_str(&terminate).expect("terminate JSON");

    assert!(value["result"].is_object());
    assert_ne!(response.exit_code, 0);
}

#[cfg(unix)]
#[test]
fn app_server_real_p5_streaming_command_exec_route_resize_succeeds_for_pty() {
    use base64::Engine as _;

    let temp = TestDir::new("app_server_real_p5_streaming_resize");
    let services = app_services::AppServerServices::for_tests(
        app_services::TestLogService::ready(),
        app_services::TestJobService::ready(vec![]),
        app_services::TestSkillsService::ready(vec![]),
        app_services::TestMcpService::ready(vec![]),
        app_services::TestFsService::disabled(),
        command_service::AppServerCommandExecService::new(temp.path().to_path_buf()),
    );
    let service = std::sync::Arc::clone(&services.command);
    let params = CommandExecParams {
        command: vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf ready; read line".to_string(),
        ],
        cwd: None,
        timeout_ms: Some(10_000),
        disable_timeout: None,
        output_bytes_cap: None,
        disable_output_cap: None,
        env: Default::default(),
        process_id: Some("route_stream_resize".to_string()),
        sandbox_policy: None,
        size: Some(CommandExecTerminalSize { cols: 80, rows: 24 }),
        stream_stdin: Some(true),
        stream_stdout_stderr: Some(true),
        tty: Some(true),
    };
    let exec_thread = std::thread::spawn(move || service.exec(params).expect("exec response"));
    let mut server = AppServer::new().with_app_services(services);
    server
        .handle_json_rpc(initialized_request_json())
        .expect("initialize should return a response");
    let _ = server.drain_notifications();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let resize = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"resize","method":"command/exec/resize","params":{"processId":"route_stream_resize","size":{"cols":120,"rows":40}}}"#,
        )
        .expect("resize should return response");
    let write = server
        .handle_json_rpc(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": "write",
                "method": "command/exec/write",
                "params": {
                    "processId": "route_stream_resize",
                    "deltaBase64": base64::engine::general_purpose::STANDARD.encode("\n")
                }
            })
            .to_string(),
        )
        .expect("write should return response");
    let response = exec_thread.join().expect("exec thread");
    let resize_value: Value = serde_json::from_str(&resize).expect("resize JSON");
    let write_value: Value = serde_json::from_str(&write).expect("write JSON");

    assert!(resize_value["result"].is_object());
    assert!(write_value["result"].is_object());
    assert_eq!(response.exit_code, 0);
}
```

- [x] ~~**Step 2: Run route-level tests**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(app_server_real_p5_streaming_command_exec_route)'
```

Expected: PASS.

- [x] ~~**Step 3: Commit**~~

```bash
git add crates/dasclaw_app_server/src/lib.rs
git commit -m "test: cover streaming command JSON-RPC controls" -m "已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。"
```

## Task 6: Gap Matrix Update

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [x] ~~**Step 1: Find command exec rows**~~

Run:

```bash
rg -n "command/exec/outputDelta|command/exec/write|command/exec/terminate|command/exec/resize|streamStdoutStderr|PTY" docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

Expected: output includes the unstruck command rows that were intentionally left open after the 2026-06-20 P5 filesystem/buffered command slice.

- [x] ~~**Step 2: Update only verified rows**~~

Edit the matrix so these entries are struck through only if the tests from Tasks 2, 4, and 5 passed:

```markdown
~~`command/exec/outputDelta`~~
~~`command/exec/write`~~
~~`command/exec/terminate`~~
~~`command/exec/resize`~~
```

Add this note to the existing command exec notes/details column:

```markdown
PTY-backed streaming path implemented and tested; non-PTY split stdout/stderr streaming remains outside this slice.
```

Do not strike any wording that claims full non-PTY pipe streaming support.

- [x] ~~**Step 3: Check markdown diff**~~

Run:

```bash
git diff -- docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

Expected: diff only changes the corresponding command exec cells/notes.

- [x] ~~**Step 4: Commit**~~

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs: mark PTY streaming command exec coverage" -m "已检查 streaming command exec 是否已有，结论：app-server 已有协议/路由和底层 PTY 积木，但没有运行中 processId 会话表、outputDelta 生成或 write/terminate/resize 接线。"
```

## Task 7: Verification And Review Gate

**Files:**
- Inspect: all files modified above.

- [x] ~~**Step 1: Run targeted command tests**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_pty
```

Expected: PASS.

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(command_service_streaming_pty) | test(command_service_streaming_requires_client_process_id) | test(command_service_non_tty_streaming_fails_safe_until_pipe_backend_exists) | test(stdio_streaming_command_exec_accepts_write_before_final_exec_response) | test(app_server_real_p5_streaming_command_exec_route) | test(app_server_p5_service_events_drain_to_notifications)'
```

Expected: PASS.

- [x] ~~**Step 2: Run protocol tests**~~

Run:

```bash
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server_protocol -E 'test(p5_protocol)'
```

Expected: PASS.

- [x] ~~**Step 3: Run crate checks**~~

Run:

```bash
RUSTC_WRAPPER= cargo check -p dasclaw_pty --tests
```

Expected: PASS with zero errors.

Run:

```bash
RUSTC_WRAPPER= cargo check -p dasclaw_app_server --tests
```

Expected: PASS with zero errors.

- [ ] **Step 4: Format and panic scan**

Current re-check note: skipped `cargo fmt --all` because this turn only edits docs; `python3.12` is unavailable in the current shell, and `python3` is too old for `scripts/check_no_panics.py`.

Run:

```bash
cargo fmt --all
```

Expected: no output or only formatted files.

Run:

```bash
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: PASS. If this script reports pre-existing unrelated findings, record the exact output in the implementation summary.

- [ ] **Step 5: Run clippy for touched crates**

Current re-check note: `dasclaw_pty` clippy passes; `dasclaw_app_server` clippy currently fails on pre-existing unrelated `crates/dasclaw_app_server/src/mcp_service.rs:232` `clippy::needless_update`.

Run:

```bash
RUSTC_WRAPPER= cargo clippy --no-deps -p dasclaw_pty --all-targets -- -D warnings
```

Expected: PASS.

Run:

```bash
RUSTC_WRAPPER= cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Expected: PASS or only pre-existing untouched warnings. Do not claim clean clippy if warnings remain.

- [x] ~~**Step 6: Run diff hygiene**~~

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

Run:

```bash
git status --short
```

Expected: only intentional files are modified.

- [x] ~~**Step 7: Self-review**~~

Check these before asking for code review:

```text
1. Buffered command/exec still returns stdout/stderr and emits no outputDelta.
2. Streaming command/exec requires processId.
3. PTY streaming emits command/exec/outputDelta while process is active.
4. write accepts valid base64 and rejects invalid base64.
5. terminate affects only the matching active processId.
6. resize succeeds only for PTY-backed active sessions.
7. Completed processId returns a clear unavailable error for follow-up controls.
8. stdio can process write/terminate/resize before the streaming exec final response.
9. The gap matrix does not claim non-PTY split stdout/stderr streaming.
```

## Self-Review

Spec coverage:

- `command/exec/outputDelta`: covered by Task 2 service tests, Task 4 stdio test, Task 5 route tests, and Task 6 docs update.
- `command/exec/write`: covered by Task 2 service test and Task 4 stdio test.
- `command/exec/terminate`: covered by Task 2 service test and Task 5 route test.
- `command/exec/resize`: covered by Task 2 service test and Task 5 route test.
- Codex reference processId/session model: covered by Task 2 session table and Task 4 detached dispatch.
- Existing buffered behavior preservation: covered by Task 2 targeted regression command.

Placeholder scan:

- This plan avoids deferred implementation markers. Non-PTY split stdout/stderr streaming is explicitly out of scope and must remain unclaimed in the gap matrix.

Type consistency:

- `CommandExecParams`, `CommandExecWriteParams`, `CommandExecTerminateParams`, `CommandExecResizeParams`, `CommandExecOutputDeltaNotification`, and `CommandExecTerminalSize` match the existing protocol crate names.
- `processId`, `deltaBase64`, `streamStdin`, and `streamStdoutStderr` match the existing camelCase JSON shape.
