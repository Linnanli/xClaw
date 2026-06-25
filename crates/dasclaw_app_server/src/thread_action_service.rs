use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use dasclaw_app_server_protocol::{CommandExecParams, CommandExecResponse};

use crate::AppServerError;
use crate::app_services::CommandExecService;

#[derive(Clone)]
pub struct ThreadActionService {
    command_exec: Arc<dyn CommandExecService>,
    pending_guardian: Arc<Mutex<HashMap<String, Vec<serde_json::Value>>>>,
}

impl ThreadActionService {
    #[must_use]
    pub fn new(command_exec: Arc<dyn CommandExecService>) -> Self {
        Self {
            command_exec,
            pending_guardian: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn run_shell_command(&self, cwd: &str, command: &str) -> Result<String, AppServerError> {
        self.run_execve_command(cwd, shell_program(), shell_argv(command))
    }

    pub fn run_execve_command(
        &self,
        cwd: &str,
        program: String,
        argv: Vec<String>,
    ) -> Result<String, AppServerError> {
        let mut command = Vec::with_capacity(argv.len() + 1);
        command.push(program);
        command.extend(argv);
        self.run_command_exec(CommandExecParams {
            command,
            cwd: Some(cwd.to_string()),
            timeout_ms: None,
            disable_timeout: None,
            output_bytes_cap: None,
            disable_output_cap: None,
            env: Default::default(),
            process_id: None,
            sandbox_policy: None,
            permission_profile: None,
            size: None,
            stream_stdin: None,
            stream_stdout_stderr: None,
            tty: None,
        })
    }

    pub fn run_command_exec(&self, params: CommandExecParams) -> Result<String, AppServerError> {
        let response = self.command_exec.exec(params)?;
        Ok(command_output(response))
    }

    pub fn stash_pending_guardian(
        &self,
        thread_id: impl Into<String>,
        event: serde_json::Value,
    ) -> Result<(), AppServerError> {
        let mut pending = self.pending_guardian.lock().map_err(|_| {
            AppServerError::service_degraded(
                "thread_actions",
                "pending guardian store lock is poisoned",
            )
        })?;
        pending.entry(thread_id.into()).or_default().push(event);
        Ok(())
    }

    pub fn take_pending_guardian(
        &self,
        thread_id: &str,
    ) -> Result<Vec<serde_json::Value>, AppServerError> {
        let mut pending = self.pending_guardian.lock().map_err(|_| {
            AppServerError::service_degraded(
                "thread_actions",
                "pending guardian store lock is poisoned",
            )
        })?;
        Ok(pending.remove(thread_id).unwrap_or_default())
    }
}

impl fmt::Debug for ThreadActionService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadActionService")
            .finish_non_exhaustive()
    }
}

fn command_output(response: CommandExecResponse) -> String {
    if response.stderr.is_empty() {
        response.stdout
    } else if response.stdout.is_empty() {
        response.stderr
    } else {
        format!("{}{}", response.stdout, response.stderr)
    }
}

#[cfg(windows)]
fn shell_program() -> String {
    "cmd".to_string()
}

#[cfg(windows)]
fn shell_argv(command: &str) -> Vec<String> {
    vec!["/C".to_string(), command.to_string()]
}

#[cfg(not(windows))]
fn shell_program() -> String {
    "/bin/sh".to_string()
}

#[cfg(not(windows))]
fn shell_argv(command: &str) -> Vec<String> {
    vec!["-lc".to_string(), command.to_string()]
}
