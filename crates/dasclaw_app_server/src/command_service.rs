use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use dasclaw_app_server_protocol::{
    CommandExecAvailability, CommandExecOutputDeltaNotification, CommandExecOutputStream,
    CommandExecParams, CommandExecResizeParams, CommandExecResizeResponse, CommandExecResponse,
    CommandExecTerminalSize, CommandExecTerminateParams, CommandExecTerminateResponse,
    CommandExecWriteParams, CommandExecWriteResponse, ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};
use dasclaw_pty::{PtyExitStatus, PtySize, PtySpawnOptions, StreamingPty};
use dasclaw_shell_tools::{SandboxedShellExecutor, ShellExecError};
use dasclaw_workspace_cap::WorkspaceCapability;
use dasclaw_workspace_cap::policy::SandboxPolicy;
use uuid::Uuid;

use crate::AppServerError;
use crate::app_services::CommandExecService;
use crate::blocking_runtime::BlockingTokioRuntime;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const COMMAND_EXEC_CAPABILITY: &str = "command_exec";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    Posix,
    Cmd,
}

pub struct AppServerCommandExecService {
    root: PathBuf,
    lexical_root: PathBuf,
    workspace: Result<WorkspaceCapability, String>,
    runtime: Mutex<Option<Result<BlockingTokioRuntime, AppServerError>>>,
    completed: Arc<Mutex<HashMap<String, CompletedCommand>>>,
    running: Arc<Mutex<HashMap<String, RunningProcess>>>,
    output_delta_events: Arc<Mutex<Vec<CommandExecOutputDeltaNotification>>>,
}

#[derive(Debug, Clone)]
struct CompletedCommand {
    response: CommandExecResponse,
}

struct RunningCommand {
    control_tx: mpsc::Sender<CommandControl>,
    tty: bool,
}

enum RunningProcess {
    Reserved,
    Active(RunningCommand),
}

enum CommandControl {
    Write(Vec<u8>),
    CloseStdin,
    Terminate,
    Resize(CommandExecTerminalSize),
}

impl AppServerCommandExecService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let lexical_root = normalize_lexical(&root);
        let root = root.canonicalize().unwrap_or_else(|_| lexical_root.clone());
        let workspace = WorkspaceCapability::open(&root).map_err(|error| error.to_string());
        Self {
            root,
            lexical_root,
            workspace,
            runtime: Mutex::new(None),
            completed: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(HashMap::new())),
            output_delta_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn runtime(&self) -> Result<BlockingTokioRuntime, AppServerError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if runtime.is_none() {
            *runtime = Some(BlockingTokioRuntime::new(
                "dasclaw-app-server-command-exec",
                COMMAND_EXEC_CAPABILITY,
            ));
        }
        match runtime.as_ref() {
            Some(created) => created.as_ref().cloned().map_err(Clone::clone),
            None => Err(command_unavailable("command runtime failed to initialize")),
        }
    }

    fn resolve_cwd(&self, raw: Option<&str>) -> Result<PathBuf, AppServerError> {
        let Some(raw) = raw else {
            return Ok(self.root.clone());
        };
        self.reject_unsafe_cwd(raw)?;

        let candidate = validate_path(raw, Some(&self.root))
            .map_err(|error| command_unavailable(error.to_string()))?;
        let canonical = candidate
            .canonicalize()
            .map_err(|error| command_unavailable(error.to_string()))?;
        if !canonical.is_dir() {
            return Err(command_unavailable("command cwd is not a directory"));
        }
        if !canonical.starts_with(&self.root) {
            return Err(command_unavailable("command cwd is outside service root"));
        }

        if let Ok(relative) = canonical.strip_prefix(&self.root)
            && !relative.as_os_str().is_empty()
            && !self
                .workspace
                .as_ref()
                .map(|workspace| workspace.exists(relative))
                .unwrap_or(false)
        {
            return Err(command_unavailable(
                "command cwd is not reachable through workspace capability",
            ));
        }

        Ok(canonical)
    }

    fn reject_unsafe_cwd(&self, raw: &str) -> Result<(), AppServerError> {
        if raw.as_bytes().contains(&0) {
            return Err(command_unavailable("command cwd contains a null byte"));
        }

        let path = Path::new(raw);
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(command_unavailable("command cwd traversal is not allowed"));
        }

        if path.is_absolute() {
            let lexical = normalize_lexical(path);
            if !lexical.starts_with(&self.root) && !lexical.starts_with(&self.lexical_root) {
                return Err(command_unavailable(
                    "absolute command cwd is outside service root",
                ));
            }
        }

        Ok(())
    }

    fn process_id(params: &CommandExecParams) -> String {
        params
            .process_id
            .clone()
            .unwrap_or_else(|| format!("cmd_{}", Uuid::new_v4()))
    }

    fn command_line(command: &[String]) -> Result<String, AppServerError> {
        Self::command_line_for_shell(command, current_shell_kind())
    }

    fn command_line_for_shell(
        command: &[String],
        shell: ShellKind,
    ) -> Result<String, AppServerError> {
        if command.is_empty() {
            return Err(AppServerError::invalid_request(
                COMMAND_EXEC_CAPABILITY,
                "command must contain at least one argument",
            ));
        }
        if shell == ShellKind::Cmd
            && let Some(arg) = command.iter().find(|arg| !is_safe_cmd_arg(arg))
        {
            return Err(command_unavailable(format!(
                "Windows cmd metacharacter or quoting-sensitive character is not supported in command argument: {arg:?}"
            )));
        }
        Ok(command
            .iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" "))
    }

    fn env(env: BTreeMap<String, Option<String>>) -> HashMap<String, String> {
        env.into_iter()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect()
    }

    fn apply_output_cap(
        output_bytes_cap: Option<usize>,
        response: CommandExecResponse,
    ) -> CommandExecResponse {
        let Some(cap) = output_bytes_cap else {
            return response;
        };

        let mut stdout = response.stdout;
        let mut stderr = response.stderr;
        truncate_to_byte_cap(&mut stdout, cap);
        truncate_to_byte_cap(&mut stderr, cap);
        CommandExecResponse {
            exit_code: response.exit_code,
            stdout,
            stderr,
        }
    }

    fn streaming_requested(params: &CommandExecParams) -> bool {
        params.tty.unwrap_or(false)
            || params.stream_stdin.unwrap_or(false)
            || params.stream_stdout_stderr.unwrap_or(false)
    }

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

    fn validate_streaming_exec_options(
        params: &CommandExecParams,
    ) -> Result<String, AppServerError> {
        if params.command.is_empty() {
            return Err(AppServerError::invalid_request(
                COMMAND_EXEC_CAPABILITY,
                "command must contain at least one argument",
            ));
        }
        let process_id = params
            .process_id
            .as_ref()
            .map(|process_id| process_id.trim())
            .filter(|process_id| !process_id.is_empty())
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    COMMAND_EXEC_CAPABILITY,
                    "streaming command exec requires a non-empty client processId",
                )
            })?;
        if !params.tty.unwrap_or(false)
            && (params.stream_stdin.unwrap_or(false)
                || params.stream_stdout_stderr.unwrap_or(false))
        {
            return Err(command_unavailable(
                "non-tty command streaming requires a streaming pipe backend",
            ));
        }
        if params.sandbox_policy.is_some() {
            return Err(command_unavailable(
                "sandboxPolicy is not supported by PTY streaming command exec",
            ));
        }
        if params.disable_output_cap.unwrap_or(false) {
            return Err(command_unavailable(
                "disableOutputCap is not supported by PTY streaming command exec",
            ));
        }
        Ok(process_id.to_string())
    }

    fn exec_streaming(
        &self,
        params: CommandExecParams,
    ) -> Result<CommandExecResponse, AppServerError> {
        let process_id = Self::validate_streaming_exec_options(&params)?;
        let cwd = self.resolve_cwd(params.cwd.as_deref())?;
        let env = Self::env(params.env);
        let size = params
            .size
            .unwrap_or(CommandExecTerminalSize { cols: 80, rows: 24 });
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

        let mut running = self
            .running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match running.entry(process_id.clone()) {
            Entry::Occupied(_) => {
                return Err(command_unavailable(format!(
                    "command process is already running: {process_id}"
                )));
            }
            Entry::Vacant(entry) => {
                entry.insert(RunningProcess::Reserved);
            }
        }
        drop(running);

        let process = dasclaw_pty::default_backend()
            .spawn_process(PtySpawnOptions {
                program: params.command[0].clone(),
                args: params.command[1..].to_vec(),
                cwd: Some(cwd),
                env,
                size: PtySize {
                    rows: size.rows,
                    cols: size.cols,
                    pixel_width: 0,
                    pixel_height: 0,
                },
            })
            .map_err(|error| {
                self.running
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .remove(&process_id);
                command_unavailable(error.to_string())
            })?;

        let (control_tx, control_rx) = mpsc::channel();
        self.running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                process_id.clone(),
                RunningProcess::Active(RunningCommand {
                    control_tx,
                    tty: true,
                }),
            );

        let reader_thread = spawn_pty_reader(
            process_id.clone(),
            process.reader,
            Arc::clone(&self.output_delta_events),
            params.output_bytes_cap,
        );
        let exit_status = drive_pty_process(process.writer, process.control, control_rx, timeout);
        let _ = reader_thread.join();

        let response = CommandExecResponse {
            exit_code: exit_code_to_i32(exit_status),
            stdout: String::new(),
            stderr: String::new(),
        };
        self.running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&process_id);
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

    fn running_command(&self, process_id: &str) -> Option<(mpsc::Sender<CommandControl>, bool)> {
        self.running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(process_id)
            .and_then(|running| match running {
                RunningProcess::Reserved => None,
                RunningProcess::Active(running) => Some((running.control_tx.clone(), running.tty)),
            })
    }
}

impl CommandExecService for AppServerCommandExecService {
    fn health(&self) -> ServiceHealth {
        match &self.workspace {
            Ok(_) => {
                let mut health = ServiceHealth::ready(ServiceName::CommandExec);
                health.message = Some(format!(
                    "root={} cwd_guard=root-contained buffered_sandbox=read-only/no-network streaming_sandbox=none/unsandboxed read_scope=host-read-only not_workspace_read_limited non_interactive=true streaming=pty tty=true non_tty_streaming=false",
                    self.root.display()
                ));
                health
            }
            Err(message) => ServiceHealth::unavailable_fail_safe(
                ServiceName::CommandExec,
                format!("workspace capability unavailable: {message}"),
            ),
        }
    }

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

        let cwd = self.resolve_cwd(params.cwd.as_deref())?;
        let process_id = Self::process_id(&params);
        let command = Self::command_line(&params.command)?;
        let output_bytes_cap = params.output_bytes_cap;
        let env = Self::env(params.env);
        let timeout = params
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_TIMEOUT);

        let response = self.runtime()?.block_on("command/exec", async move {
            let executor = SandboxedShellExecutor::new(timeout, false, None);
            let output = executor
                .execute(&command, &cwd, SandboxPolicy::new_read_only_policy(), env)
                .await
                .map_err(map_exec_error)?;
            Ok(CommandExecResponse {
                exit_code: output.exit_code as i32,
                stdout: output.stdout,
                stderr: output.stderr,
            })
        })?;
        let response = Self::apply_output_cap(output_bytes_cap, response);

        self.completed
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                process_id.clone(),
                CompletedCommand {
                    response: response.clone(),
                },
            );
        Ok(response)
    }

    fn write(
        &self,
        params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError> {
        let Some((sender, _tty)) = self.running_command(&params.process_id) else {
            return Err(completed_or_unknown_process_error(
                &self.completed,
                &params.process_id,
                "command stdin streaming is unavailable for completed non-interactive executions",
            ));
        };

        if let Some(delta_base64) = params.delta_base64 {
            let decoded = BASE64_STANDARD.decode(delta_base64).map_err(|_| {
                AppServerError::invalid_request(
                    COMMAND_EXEC_CAPABILITY,
                    "deltaBase64 must be valid base64",
                )
            })?;
            sender
                .send(CommandControl::Write(decoded))
                .map_err(|_| command_unavailable("command process is no longer writable"))?;
        }
        if params.close_stdin.unwrap_or(false) {
            let _ = sender.send(CommandControl::CloseStdin);
        }
        Ok(CommandExecWriteResponse::default())
    }

    fn terminate(
        &self,
        params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError> {
        let Some((sender, _tty)) = self.running_command(&params.process_id) else {
            return Err(completed_or_unknown_process_error(
                &self.completed,
                &params.process_id,
                "command termination is unavailable for completed non-interactive executions",
            ));
        };
        sender
            .send(CommandControl::Terminate)
            .map_err(|_| command_unavailable("command process is no longer running"))?;
        Ok(CommandExecTerminateResponse::default())
    }

    fn resize(
        &self,
        params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError> {
        let Some((sender, tty)) = self.running_command(&params.process_id) else {
            return Err(completed_or_unknown_process_error(
                &self.completed,
                &params.process_id,
                "command resize is unavailable without an active PTY session",
            ));
        };
        if !tty {
            return Err(command_unavailable(
                "command resize is unavailable without an active PTY session",
            ));
        }
        sender
            .send(CommandControl::Resize(params.size))
            .map_err(|_| command_unavailable("command PTY session is no longer active"))?;
        Ok(CommandExecResizeResponse::default())
    }

    fn drain_output_delta_events(&self) -> Vec<CommandExecOutputDeltaNotification> {
        self.output_delta_events
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect()
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
}

fn spawn_pty_reader(
    process_id: String,
    mut reader: Box<dyn Read + Send>,
    output_delta_events: Arc<Mutex<Vec<CommandExecOutputDeltaNotification>>>,
    output_bytes_cap: Option<usize>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0_u8; 8192];
        let mut emitted = 0_usize;

        loop {
            let read = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(read) => read,
                Err(_) => break,
            };

            let chunk = &buf[..read];
            let (emit, cap_reached) = match output_bytes_cap {
                Some(cap) if emitted >= cap => (&[][..], false),
                Some(cap) => {
                    let remaining = cap - emitted;
                    if chunk.len() > remaining {
                        (&chunk[..remaining], true)
                    } else {
                        (chunk, false)
                    }
                }
                None => (chunk, false),
            };

            if emit.is_empty() {
                continue;
            }

            emitted += emit.len();
            let event = CommandExecOutputDeltaNotification {
                process_id: process_id.clone(),
                stream: CommandExecOutputStream::Stdout,
                delta_base64: BASE64_STANDARD.encode(emit),
                cap_reached,
            };
            output_delta_events
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .push(event);
        }
    })
}

fn drive_pty_process(
    writer: Box<dyn Write + Send>,
    mut control: Box<dyn dasclaw_pty::PtyProcessControl>,
    control_rx: mpsc::Receiver<CommandControl>,
    timeout: Option<Duration>,
) -> PtyExitStatus {
    let started = Instant::now();
    let mut writer = Some(writer);
    'driver: loop {
        while let Ok(command) = control_rx.try_recv() {
            match command {
                CommandControl::Write(bytes) => {
                    let Some(writer) = writer.as_mut() else {
                        continue;
                    };
                    if writer.write_all(&bytes).is_err() {
                        break 'driver control.try_wait().unwrap_or(PtyExitStatus::Signaled);
                    }
                }
                CommandControl::CloseStdin => {
                    if let Some(mut writer) = writer.take() {
                        let _ = writer.flush();
                    }
                }
                CommandControl::Terminate => {
                    let _ = control.kill();
                    break 'driver wait_after_kill(&mut *control);
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
            Ok(status) if status.is_finished() => {
                break status;
            }
            Ok(_) => {}
            Err(_) => {
                break PtyExitStatus::Signaled;
            }
        }

        if let Some(timeout) = timeout
            && started.elapsed() >= timeout
        {
            let _ = control.kill();
            break wait_after_kill(&mut *control);
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn wait_after_kill(control: &mut dyn dasclaw_pty::PtyProcessControl) -> PtyExitStatus {
    control
        .wait_for_exit(Some(Duration::from_secs(1)))
        .unwrap_or(PtyExitStatus::Signaled)
}

fn exit_code_to_i32(exit_status: PtyExitStatus) -> i32 {
    match exit_status {
        PtyExitStatus::Exited(code) => code,
        PtyExitStatus::Signaled | PtyExitStatus::Running => -1,
    }
}

fn shell_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if is_unquoted_shell_arg(arg) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}

fn current_shell_kind() -> ShellKind {
    if cfg!(target_os = "windows") {
        ShellKind::Cmd
    } else {
        ShellKind::Posix
    }
}

fn is_safe_cmd_arg(arg: &str) -> bool {
    !arg.is_empty() && is_unquoted_shell_arg(arg)
}

fn is_unquoted_shell_arg(arg: &str) -> bool {
    arg.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.' | ':' | '='))
}

fn truncate_to_byte_cap(value: &mut String, cap: usize) -> bool {
    if value.len() <= cap {
        return false;
    }
    let end = value.floor_char_boundary(cap);
    value.truncate(end);
    true
}

fn completed_or_unknown_process_error(
    completed: &Mutex<HashMap<String, CompletedCommand>>,
    process_id: &str,
    completed_message: &'static str,
) -> AppServerError {
    let completed = completed
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(command) = completed.get(process_id) {
        return command_unavailable(format!(
            "{completed_message}; process already exited with code {}",
            command.response.exit_code
        ));
    }
    command_unavailable(format!("unknown command process: {process_id}"))
}

fn map_exec_error(error: ShellExecError) -> AppServerError {
    match error {
        ShellExecError::Timeout(timeout) => command_unavailable(format!(
            "command timed out after {} ms",
            timeout.as_millis()
        )),
        ShellExecError::FullAccessNotPermitted | ShellExecError::ExecutionFailed(_) => {
            command_unavailable(error.to_string())
        }
    }
}

fn command_unavailable(message: impl Into<String>) -> AppServerError {
    AppServerError::capability_unavailable(COMMAND_EXEC_CAPABILITY, message)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    use dasclaw_app_server_protocol::{
        CommandExecOutputDeltaNotification, CommandExecOutputStream, CommandExecParams,
        CommandExecResizeParams, CommandExecTerminalSize, CommandExecTerminateParams,
        CommandExecWriteParams,
    };

    use super::*;

    fn exec_params(command: Vec<&str>) -> CommandExecParams {
        CommandExecParams {
            command: command.into_iter().map(str::to_string).collect(),
            cwd: None,
            timeout_ms: Some(5_000),
            disable_timeout: None,
            output_bytes_cap: None,
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

    #[cfg(unix)]
    fn wait_for_output_delta(
        service: &AppServerCommandExecService,
        process_id: &str,
        needle: &str,
    ) -> CommandExecOutputDeltaNotification {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            for event in service.drain_output_delta_events() {
                if event.process_id != process_id {
                    continue;
                }
                let decoded = BASE64_STANDARD
                    .decode(&event.delta_base64)
                    .expect("valid delta base64");
                let text = String::from_utf8_lossy(&decoded);
                if text.contains(needle) {
                    return event;
                }
            }

            assert!(
                Instant::now() < deadline,
                "timed out waiting for output delta containing {needle:?}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn command_service_exec_captures_stdout_without_streaming_delta() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let response = service
            .exec(exec_params(vec!["echo", "hello"]))
            .expect("exec succeeds");

        assert_eq!(response.exit_code, 0);
        assert!(response.stdout.contains("hello"));
        assert_eq!(response.stderr, "");
        assert!(service.drain_output_delta_events().is_empty());
    }

    #[test]
    fn command_service_rejects_stream_stdout_stderr_without_tty_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut params = exec_params(vec!["echo", "hello"]);
        params.process_id = Some("non_tty_stdout".to_string());
        params.stream_stdout_stderr = Some(true);

        let error = service
            .exec(params)
            .expect_err("non-tty streaming stdout/stderr unsupported");

        assert!(error.to_string().contains("streaming pipe backend"));
        assert!(service.drain_output_delta_events().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn command_service_streaming_pty_emits_output_delta_and_accepts_write() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
        let process_id = "pty_echo_session";
        let mut params = exec_params(vec![
            "sh",
            "-c",
            "printf ready; IFS= read -r line; printf 'got:%s' \"$line\"",
        ]);
        params.process_id = Some(process_id.to_string());
        params.tty = Some(true);
        params.stream_stdin = Some(true);
        params.stream_stdout_stderr = Some(true);
        params.disable_timeout = Some(true);

        let exec_service = Arc::clone(&service);
        let exec_thread = std::thread::spawn(move || exec_service.exec(params));

        let ready = wait_for_output_delta(&service, process_id, "ready");
        service
            .write(CommandExecWriteParams {
                process_id: process_id.to_string(),
                delta_base64: Some(BASE64_STANDARD.encode("hello from stdin\n")),
                close_stdin: None,
            })
            .expect("write succeeds");

        let got = wait_for_output_delta(&service, process_id, "got:hello from stdin");
        let response = exec_thread
            .join()
            .expect("exec thread should not panic")
            .expect("streaming exec succeeds");
        let availability = service.availability();

        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, "");
        assert_eq!(response.stderr, "");
        assert_eq!(ready.stream, CommandExecOutputStream::Stdout);
        assert_eq!(got.stream, CommandExecOutputStream::Stdout);
        assert!(availability.exec);
        assert!(availability.output_delta_events);
        assert!(availability.write);
        assert!(availability.terminate);
        assert!(availability.resize);
    }

    #[cfg(unix)]
    #[test]
    fn command_service_streaming_pty_terminate_stops_running_process() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
        let process_id = "pty_sleep_session";
        let mut params = exec_params(vec!["sh", "-c", "printf ready; sleep 30"]);
        params.process_id = Some(process_id.to_string());
        params.tty = Some(true);
        params.stream_stdout_stderr = Some(true);
        params.disable_timeout = Some(true);

        let exec_service = Arc::clone(&service);
        let exec_thread = std::thread::spawn(move || exec_service.exec(params));

        wait_for_output_delta(&service, process_id, "ready");

        service
            .terminate(CommandExecTerminateParams {
                process_id: process_id.to_string(),
            })
            .expect("terminate active process");
        let response = exec_thread
            .join()
            .expect("exec thread should not panic")
            .expect("streaming exec returns terminated result");

        assert_ne!(response.exit_code, 0);
        service
            .terminate(CommandExecTerminateParams {
                process_id: process_id.to_string(),
            })
            .expect_err("completed process cannot be terminated again");
    }

    #[cfg(unix)]
    #[test]
    fn command_service_streaming_pty_resize_accepts_active_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
        let process_id = "pty_resize_session";
        let mut params = exec_params(vec!["sh", "-c", "printf ready; IFS= read -r line"]);
        params.process_id = Some(process_id.to_string());
        params.tty = Some(true);
        params.stream_stdin = Some(true);
        params.stream_stdout_stderr = Some(true);
        params.size = Some(CommandExecTerminalSize { cols: 80, rows: 24 });
        params.disable_timeout = Some(true);

        let exec_service = Arc::clone(&service);
        let exec_thread = std::thread::spawn(move || exec_service.exec(params));

        wait_for_output_delta(&service, process_id, "ready");

        service
            .resize(CommandExecResizeParams {
                process_id: process_id.to_string(),
                size: CommandExecTerminalSize {
                    cols: 100,
                    rows: 30,
                },
            })
            .expect("resize active pty session");
        service
            .write(CommandExecWriteParams {
                process_id: process_id.to_string(),
                delta_base64: Some(BASE64_STANDARD.encode("\n")),
                close_stdin: None,
            })
            .expect("write newline to finish process");
        let response = exec_thread
            .join()
            .expect("exec thread should not panic")
            .expect("streaming exec succeeds");

        assert_eq!(response.exit_code, 0);
    }

    #[cfg(unix)]
    #[test]
    fn command_service_streaming_rejects_duplicate_active_process_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = Arc::new(AppServerCommandExecService::new(temp.path().to_path_buf()));
        let process_id = "pty_duplicate_session";
        let mut first = exec_params(vec![
            "sh",
            "-c",
            "printf first-ready; IFS= read -r line; printf 'first-got:%s' \"$line\"",
        ]);
        first.process_id = Some(process_id.to_string());
        first.tty = Some(true);
        first.stream_stdin = Some(true);
        first.stream_stdout_stderr = Some(true);
        first.disable_timeout = Some(true);

        let first_service = Arc::clone(&service);
        let first_thread = std::thread::spawn(move || first_service.exec(first));

        wait_for_output_delta(&service, process_id, "first-ready");

        let mut duplicate = exec_params(vec!["sh", "-c", "printf duplicate"]);
        duplicate.process_id = Some(process_id.to_string());
        duplicate.tty = Some(true);
        duplicate.stream_stdout_stderr = Some(true);
        duplicate.disable_timeout = Some(true);

        let error = service
            .exec(duplicate)
            .expect_err("duplicate active processId must fail safe");

        assert!(error.to_string().contains("already running"));
        service
            .write(CommandExecWriteParams {
                process_id: process_id.to_string(),
                delta_base64: Some(BASE64_STANDARD.encode("hello\n")),
                close_stdin: None,
            })
            .expect("write still reaches the original session");
        wait_for_output_delta(&service, process_id, "first-got:hello");
        let response = first_thread
            .join()
            .expect("first exec thread should not panic")
            .expect("first streaming exec succeeds");

        assert_eq!(response.exit_code, 0);
    }

    #[test]
    fn command_service_streaming_requires_client_process_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut missing = exec_params(vec!["printf", "hello"]);
        missing.tty = Some(true);

        let error = service.exec(missing).expect_err("processId required");

        assert!(error.to_string().contains("processId"));

        let mut blank = exec_params(vec!["printf", "hello"]);
        blank.tty = Some(true);
        blank.process_id = Some("  ".to_string());

        let error = service
            .exec(blank)
            .expect_err("non-empty processId required");

        assert!(error.to_string().contains("processId"));
    }

    #[test]
    fn command_service_non_tty_streaming_fails_safe_until_pipe_backend_exists() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut stdout = exec_params(vec!["printf", "hello"]);
        stdout.process_id = Some("pipe_stdout".to_string());
        stdout.stream_stdout_stderr = Some(true);

        let error = service
            .exec(stdout)
            .expect_err("non-tty output streaming needs pipe backend");

        assert!(error.to_string().contains("streaming pipe backend"));

        let mut stdin = exec_params(vec!["cat"]);
        stdin.process_id = Some("pipe_stdin".to_string());
        stdin.stream_stdin = Some(true);

        let error = service
            .exec(stdin)
            .expect_err("non-tty stdin streaming needs pipe backend");

        assert!(error.to_string().contains("streaming pipe backend"));
    }

    #[cfg(unix)]
    #[test]
    fn command_service_posix_metachar_argument_stays_literal() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let response = service
            .exec(exec_params(vec![
                "printf",
                "hello; printf injected && true",
            ]))
            .expect("exec succeeds");

        assert_eq!(response.exit_code, 0);
        assert_eq!(response.stdout, "hello; printf injected && true");
    }

    #[test]
    fn command_line_rejects_windows_cmd_metacharacters() {
        let command = vec!["echo".to_string(), "safe & whoami".to_string()];

        let error = AppServerCommandExecService::command_line_for_shell(&command, ShellKind::Cmd)
            .expect_err("cmd metacharacters must fail closed");

        assert!(error.to_string().contains("Windows cmd metacharacter"));
    }

    #[test]
    fn command_service_health_documents_process_read_limit_gap() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let message = service
            .health()
            .message
            .expect("ready service should describe limits");

        assert!(message.contains("cwd_guard=root-contained"));
        assert!(message.contains("buffered_sandbox=read-only/no-network"));
        assert!(message.contains("streaming_sandbox=none/unsandboxed"));
        assert!(message.contains("read_scope=host-read-only"));
        assert!(message.contains("not_workspace_read_limited"));
    }

    #[cfg(unix)]
    #[test]
    fn command_service_output_cap_truncates_buffered_response() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut params = exec_params(vec!["printf", "abcdef"]);
        params.output_bytes_cap = Some(3);
        params.process_id = Some("capped".to_string());

        let response = service.exec(params).expect("exec succeeds");

        assert_eq!(response.stdout, "abc");
        assert!(service.drain_output_delta_events().is_empty());
    }

    #[test]
    fn command_service_disable_output_cap_fails_safe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut params = exec_params(vec!["rustc", "--version"]);
        params.disable_output_cap = Some(true);

        let error = service.exec(params).expect_err("unsupported cap override");

        assert!(
            error
                .to_string()
                .contains("disableOutputCap is not supported")
        );
    }

    #[test]
    fn command_service_rejects_unsupported_exec_semantics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let mut sandbox_policy = exec_params(vec!["rustc", "--version"]);
        sandbox_policy.sandbox_policy = Some(serde_json::json!({"mode": "unrestricted"}));
        let error = service
            .exec(sandbox_policy)
            .expect_err("sandbox policy override unsupported");
        assert!(error.to_string().contains("sandboxPolicy is not supported"));

        let mut disable_timeout = exec_params(vec!["rustc", "--version"]);
        disable_timeout.disable_timeout = Some(true);
        let error = service
            .exec(disable_timeout)
            .expect_err("disable timeout unsupported");
        assert!(
            error
                .to_string()
                .contains("disableTimeout is not supported")
        );

        let mut size_without_tty = exec_params(vec!["rustc", "--version"]);
        size_without_tty.size = Some(CommandExecTerminalSize { cols: 80, rows: 24 });
        let error = service
            .exec(size_without_tty)
            .expect_err("terminal size requires tty");
        assert!(error.to_string().contains("size requires tty support"));
    }

    #[cfg(unix)]
    #[test]
    fn command_service_exec_captures_stderr_and_non_zero_exit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let response = service
            .exec(exec_params(vec!["ls", "__dasclaw_missing_path__"]))
            .expect("exec returns process result");

        assert_ne!(response.exit_code, 0);
        assert_eq!(response.stdout, "");
        assert!(response.stderr.contains("__dasclaw_missing_path__"));
    }

    #[test]
    fn command_service_rejects_cwd_outside_root_and_parent_traversal() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let mut outside_params = exec_params(vec!["printf", "nope"]);
        outside_params.cwd = Some(outside.path().display().to_string());
        assert!(service.exec(outside_params).is_err());

        let mut traversal_params = exec_params(vec!["printf", "nope"]);
        traversal_params.cwd = Some("../".to_string());
        assert!(service.exec(traversal_params).is_err());
    }

    #[test]
    fn command_service_rejects_symlink_escape_cwd() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let link = temp.path().join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &link).expect("symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(outside.path(), &link).expect("symlink");

        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut params = exec_params(vec!["printf", "nope"]);
        params.cwd = Some("escape".to_string());

        assert!(service.exec(params).is_err());
    }

    #[test]
    fn command_service_follow_up_methods_fail_safe_without_pty_process() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut completed = exec_params(vec!["rustc", "--version"]);
        completed.process_id = Some("completed".to_string());
        service.exec(completed).expect("exec completes");

        assert!(
            service
                .write(CommandExecWriteParams {
                    process_id: "missing".to_string(),
                    delta_base64: Some("aQ==".to_string()),
                    close_stdin: None,
                })
                .is_err()
        );
        assert!(
            service
                .terminate(CommandExecTerminateParams {
                    process_id: "missing".to_string(),
                })
                .is_err()
        );
        assert!(
            service
                .resize(CommandExecResizeParams {
                    process_id: "missing".to_string(),
                    size: CommandExecTerminalSize { cols: 80, rows: 24 },
                })
                .is_err()
        );
        assert!(
            service
                .terminate(CommandExecTerminateParams {
                    process_id: "completed".to_string(),
                })
                .is_err()
        );
    }
}
