use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use dasclaw_app_server_protocol::{
    CommandExecAvailability, CommandExecOutputDeltaNotification, CommandExecOutputStream,
    CommandExecParams, CommandExecResizeParams, CommandExecResizeResponse, CommandExecResponse,
    CommandExecTerminateParams, CommandExecTerminateResponse, CommandExecWriteParams,
    CommandExecWriteResponse, ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};
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
    completed: Mutex<HashMap<String, CompletedCommand>>,
    output_delta_events: Mutex<Vec<CommandExecOutputDeltaNotification>>,
}

#[derive(Debug, Clone)]
struct CompletedCommand {
    response: CommandExecResponse,
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
            completed: Mutex::new(HashMap::new()),
            output_delta_events: Mutex::new(Vec::new()),
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

    fn push_delta_events(
        &self,
        process_id: &str,
        response: &CommandExecResponse,
        cap_reached: bool,
    ) {
        let mut events = Vec::new();
        if !response.stdout.is_empty() {
            events.push(output_delta(
                process_id,
                CommandExecOutputStream::Stdout,
                &response.stdout,
                cap_reached,
            ));
        }
        if !response.stderr.is_empty() {
            events.push(output_delta(
                process_id,
                CommandExecOutputStream::Stderr,
                &response.stderr,
                cap_reached,
            ));
        }
        if !events.is_empty() {
            self.output_delta_events
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .extend(events);
        }
    }

    fn apply_output_cap(
        output_bytes_cap: Option<usize>,
        response: CommandExecResponse,
        executor_cap_reached: bool,
    ) -> Result<(CommandExecResponse, bool), AppServerError> {
        let Some(cap) = output_bytes_cap else {
            return Ok((response, executor_cap_reached));
        };

        let mut stdout = response.stdout;
        let mut stderr = response.stderr;
        let stdout_truncated = truncate_to_byte_cap(&mut stdout, cap);
        let stderr_truncated = truncate_to_byte_cap(&mut stderr, cap);
        Ok((
            CommandExecResponse {
                exit_code: response.exit_code,
                stdout,
                stderr,
            },
            executor_cap_reached || stdout_truncated || stderr_truncated,
        ))
    }
}

impl CommandExecService for AppServerCommandExecService {
    fn health(&self) -> ServiceHealth {
        match &self.workspace {
            Ok(_) => {
                let mut health = ServiceHealth::ready(ServiceName::CommandExec);
                health.message = Some(format!(
                    "root={} cwd_guard=root-contained sandbox=read-only/no-network read_scope=host-read-only not_workspace_read_limited non_interactive=true streaming=false",
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
        if params.tty.unwrap_or(false) || params.stream_stdin.unwrap_or(false) {
            return Err(command_unavailable(
                "interactive command execution requires PTY support",
            ));
        }

        if params.disable_output_cap.unwrap_or(false) {
            return Err(command_unavailable(
                "disableOutputCap is not supported by the buffered sandbox executor",
            ));
        }

        let cwd = self.resolve_cwd(params.cwd.as_deref())?;
        let process_id = Self::process_id(&params);
        let command = Self::command_line(&params.command)?;
        let output_bytes_cap = params.output_bytes_cap;
        let env = Self::env(params.env);
        let timeout = if params.disable_timeout.unwrap_or(false) {
            Duration::from_secs(24 * 60 * 60)
        } else {
            params
                .timeout_ms
                .map(Duration::from_millis)
                .unwrap_or(DEFAULT_TIMEOUT)
        };

        let (response, executor_cap_reached) =
            self.runtime()?.block_on("command/exec", async move {
                let executor = SandboxedShellExecutor::new(timeout, false, None);
                let output = executor
                    .execute(&command, &cwd, SandboxPolicy::new_read_only_policy(), env)
                    .await
                    .map_err(map_exec_error)?;
                Ok((
                    CommandExecResponse {
                        exit_code: output.exit_code as i32,
                        stdout: output.stdout,
                        stderr: output.stderr,
                    },
                    output.truncated,
                ))
            })?;
        let (response, cap_reached) =
            Self::apply_output_cap(output_bytes_cap, response, executor_cap_reached)?;

        self.completed
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                process_id.clone(),
                CompletedCommand {
                    response: response.clone(),
                },
            );
        self.push_delta_events(&process_id, &response, cap_reached);
        Ok(response)
    }

    fn write(
        &self,
        params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError> {
        Err(completed_or_unknown_process_error(
            &self.completed,
            &params.process_id,
            "command stdin streaming is unavailable for completed non-interactive executions",
        ))
    }

    fn terminate(
        &self,
        params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError> {
        Err(completed_or_unknown_process_error(
            &self.completed,
            &params.process_id,
            "command termination is unavailable for completed non-interactive executions",
        ))
    }

    fn resize(
        &self,
        params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError> {
        Err(completed_or_unknown_process_error(
            &self.completed,
            &params.process_id,
            "command resize is unavailable without PTY support",
        ))
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
            terminate: false,
            write: false,
            resize: false,
        }
    }
}

fn shell_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.' | ':' | '='))
    {
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
    !arg.is_empty()
        && arg
            .chars()
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

fn output_delta(
    process_id: &str,
    stream: CommandExecOutputStream,
    delta: &str,
    cap_reached: bool,
) -> CommandExecOutputDeltaNotification {
    CommandExecOutputDeltaNotification {
        process_id: process_id.to_string(),
        stream,
        delta_base64: base64::engine::general_purpose::STANDARD.encode(delta.as_bytes()),
        cap_reached,
    }
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

    use dasclaw_app_server_protocol::{
        CommandExecParams, CommandExecResizeParams, CommandExecTerminalSize,
        CommandExecTerminateParams, CommandExecWriteParams,
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
            stream_stdout_stderr: Some(true),
            tty: None,
        }
    }

    #[test]
    fn command_service_exec_captures_stdout_and_single_delta() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());

        let response = service
            .exec(exec_params(vec!["echo", "hello"]))
            .expect("exec succeeds");

        assert_eq!(response.exit_code, 0);
        assert!(response.stdout.contains("hello"));
        assert_eq!(response.stderr, "");

        let events = service.drain_output_delta_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].stream, CommandExecOutputStream::Stdout);
        assert!(!events[0].cap_reached);
        let delta = base64::engine::general_purpose::STANDARD
            .decode(&events[0].delta_base64)
            .expect("delta decodes");
        assert_eq!(delta, response.stdout.as_bytes());
        assert!(service.drain_output_delta_events().is_empty());
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
        assert!(message.contains("read_scope=host-read-only"));
        assert!(message.contains("not_workspace_read_limited"));
    }

    #[cfg(unix)]
    #[test]
    fn command_service_output_cap_truncates_response_and_delta() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerCommandExecService::new(temp.path().to_path_buf());
        let mut params = exec_params(vec!["printf", "abcdef"]);
        params.output_bytes_cap = Some(3);
        params.process_id = Some("capped".to_string());

        let response = service.exec(params).expect("exec succeeds");

        assert_eq!(response.stdout, "abc");
        let events = service.drain_output_delta_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].cap_reached);
        let delta = base64::engine::general_purpose::STANDARD
            .decode(&events[0].delta_base64)
            .expect("delta decodes");
        assert_eq!(delta, b"abc");
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
