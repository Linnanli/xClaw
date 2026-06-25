use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};

use dasclaw_app_server_protocol::{CommandExecParams, CommandExecResponse};
use dasclaw_protocol::approvals::{GuardianAssessmentAction, GuardianAssessmentEvent};

use crate::AppServerError;
use crate::app_services::CommandExecService;

#[derive(Debug, Clone)]
pub struct PendingGuardianAction {
    pub thread_id: String,
    pub event: GuardianAssessmentEvent,
}

#[derive(Clone)]
pub struct ThreadActionService {
    command_exec: Arc<dyn CommandExecService>,
    pending_guardian: Arc<Mutex<HashMap<String, HashMap<String, PendingGuardianAction>>>>,
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
            timeout_ms: Some(30_000),
            disable_timeout: None,
            output_bytes_cap: None,
            disable_output_cap: None,
            env: BTreeMap::new(),
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

    pub fn stash_guardian_action(
        &self,
        thread_id: &str,
        event: GuardianAssessmentEvent,
    ) -> Result<(), AppServerError> {
        let mut pending = self.pending_guardian.lock().map_err(|_| {
            AppServerError::service_degraded(
                "thread_actions",
                "pending guardian store lock is poisoned",
            )
        })?;
        pending.entry(thread_id.to_string()).or_default().insert(
            event.id.clone(),
            PendingGuardianAction {
                thread_id: thread_id.to_string(),
                event,
            },
        );
        Ok(())
    }

    pub fn take_guardian_action(
        &self,
        thread_id: &str,
        event_id: &str,
    ) -> Result<Option<PendingGuardianAction>, AppServerError> {
        let mut pending = self.pending_guardian.lock().map_err(|_| {
            AppServerError::service_degraded(
                "thread_actions",
                "pending guardian store lock is poisoned",
            )
        })?;
        let Some(thread_pending) = pending.get_mut(thread_id) else {
            return Ok(None);
        };
        let Some(action) = thread_pending.remove(event_id) else {
            return Ok(None);
        };
        if thread_pending.is_empty() {
            pending.remove(thread_id);
        }
        if action.thread_id != thread_id {
            return Err(AppServerError::invalid_request(
                "thread_actions",
                "guardian action belongs to a different thread",
            ));
        }
        Ok(Some(action))
    }

    pub fn replay_guardian_action(
        &self,
        pending: PendingGuardianAction,
    ) -> Result<String, AppServerError> {
        match pending.event.action {
            GuardianAssessmentAction::Command { command, cwd, .. } => {
                let cwd = cwd.as_path().to_string_lossy().into_owned();
                self.run_shell_command(&cwd, &command)
            }
            GuardianAssessmentAction::Execve {
                program, argv, cwd, ..
            } => {
                let cwd = cwd.as_path().to_string_lossy().into_owned();
                self.run_execve_command(&cwd, program, argv)
            }
            _ => Err(AppServerError::capability_unavailable(
                "thread_actions",
                "guardian replay currently supports command-like actions only",
            )),
        }
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
