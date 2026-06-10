use std::ffi::OsString;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::info;

/// Port for the external IronClaw server
pub const EMBEDDED_SERVER_PORT: u16 = 38080;
pub const APP_SERVER_BINARY_ENV: &str = "DASCLAW_APP_SERVER_BIN";
pub const APP_SERVER_STARTUP_SMOKE_ENV: &str = "DASCLAW_APP_SERVER_STARTUP_SMOKE";
pub const APP_SERVER_SUPERVISOR_ENV: &str = "DASCLAW_APP_SERVER_SUPERVISOR";
const APP_SERVER_STDIO_SMOKE_TIMEOUT: Duration = Duration::from_secs(10);
const APP_SERVER_SUPERVISOR_HEALTH_INTERVAL: Duration = Duration::from_secs(5);
const APP_SERVER_SUPERVISOR_RESTART_BACKOFF: Duration = Duration::from_millis(250);
const APP_SERVER_SUPERVISOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const APP_SERVER_SUPERVISOR_MAX_RESTARTS: usize = 3;
const SUPERVISOR_INIT_ID: &str = "supervisor-init";
const SUPERVISOR_HEALTH_ID: &str = "supervisor-health";
const SUPERVISOR_STOP_ID: &str = "supervisor-stop";

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type AppServerSupervisorSlot = Arc<Mutex<Option<AppServerSupervisorHandle>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServerSidecarSmokeReport {
    pub notification_count: usize,
    pub session_status: String,
    pub runtime_health_status: String,
    pub shutdown_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppServerSupervisorState {
    Stopped,
    Starting,
    Ready,
    Restarting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServerSupervisorStatus {
    pub state: AppServerSupervisorState,
    pub restart_count: usize,
    pub connection_generation: u64,
    pub notification_count: usize,
    pub runtime_health_status: Option<String>,
    pub last_error: Option<String>,
}

impl Default for AppServerSupervisorStatus {
    fn default() -> Self {
        Self {
            state: AppServerSupervisorState::Stopped,
            restart_count: 0,
            connection_generation: 0,
            notification_count: 0,
            runtime_health_status: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppServerSupervisorConfig {
    pub binary: OsString,
    pub args: Vec<OsString>,
    pub health_interval: Duration,
    pub restart_backoff: Duration,
    pub request_timeout: Duration,
    pub max_restarts: usize,
}

impl Default for AppServerSupervisorConfig {
    fn default() -> Self {
        Self {
            binary: app_server_sidecar_binary(),
            args: Vec::new(),
            health_interval: APP_SERVER_SUPERVISOR_HEALTH_INTERVAL,
            restart_backoff: APP_SERVER_SUPERVISOR_RESTART_BACKOFF,
            request_timeout: APP_SERVER_SUPERVISOR_REQUEST_TIMEOUT,
            max_restarts: APP_SERVER_SUPERVISOR_MAX_RESTARTS,
        }
    }
}

#[derive(Debug)]
pub struct AppServerSupervisorHandle {
    status: Arc<Mutex<AppServerSupervisorStatus>>,
    shutdown: watch::Sender<bool>,
    join: JoinHandle<()>,
}

impl AppServerSupervisorHandle {
    pub fn status(&self) -> AppServerSupervisorStatus {
        supervisor_status_snapshot(&self.status)
    }

    pub async fn shutdown(self) -> AppServerSupervisorStatus {
        let _ = self.shutdown.send(true);
        let _ = self.join.await;
        supervisor_status_snapshot(&self.status)
    }
}

#[derive(Debug)]
struct AppServerSupervisorFailure {
    error: String,
}

#[derive(Debug)]
enum SupervisorRequestError {
    Transport(String),
    Timeout(String),
    JsonRpc(String),
    Protocol(String),
}

impl std::fmt::Display for SupervisorRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => {
                write!(formatter, "app-server supervisor transport error: {error}")
            }
            Self::Timeout(error) => write!(formatter, "app-server supervisor timeout: {error}"),
            Self::JsonRpc(error) => write!(formatter, "app-server JSON-RPC error: {error}"),
            Self::Protocol(error) => {
                write!(formatter, "app-server supervisor protocol error: {error}")
            }
        }
    }
}

impl std::error::Error for SupervisorRequestError {}

/// Check if the external IronClaw server is running
pub async fn check_server_health() -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("http://127.0.0.1:{}/api/health", EMBEDDED_SERVER_PORT);

    info!("Checking external IronClaw server at {}", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        info!("External IronClaw server is healthy");
        Ok(())
    } else {
        Err(format!("Health check failed with status: {}", response.status()).into())
    }
}

pub async fn check_app_server_sidecar_stdio() -> Result<AppServerSidecarSmokeReport, BoxError> {
    check_app_server_sidecar_stdio_at(app_server_sidecar_binary()).await
}

pub async fn check_app_server_sidecar_stdio_at(
    binary: impl Into<OsString>,
) -> Result<AppServerSidecarSmokeReport, BoxError> {
    let mut command = Command::new(binary.into());
    check_app_server_sidecar_stdio_with_command(&mut command, APP_SERVER_STDIO_SMOKE_TIMEOUT).await
}

async fn check_app_server_sidecar_stdio_with_command(
    command: &mut Command,
    timeout: Duration,
) -> Result<AppServerSidecarSmokeReport, BoxError> {
    let mut child = command
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| invalid_data("app-server sidecar stdin is unavailable"))?;
    stdin
        .write_all(app_server_stdio_smoke_requests().as_bytes())
        .await?;
    drop(stdin);

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            invalid_data(format!(
                "app-server sidecar stdio smoke timed out after {}ms",
                timeout.as_millis()
            ))
        })??;
    if !output.status.success() {
        return Err(invalid_data(format!(
            "app-server sidecar exited with {}; stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8(output.stdout)?;
    parse_app_server_stdio_smoke_stdout(&stdout)
}

pub fn spawn_app_server_sidecar_supervisor(
    config: AppServerSupervisorConfig,
) -> AppServerSupervisorHandle {
    let status = Arc::new(Mutex::new(AppServerSupervisorStatus::default()));
    let (shutdown, shutdown_rx) = watch::channel(false);
    let supervisor_status = Arc::clone(&status);
    let join = tokio::spawn(async move {
        run_app_server_sidecar_supervisor(config, supervisor_status, shutdown_rx).await;
    });

    AppServerSupervisorHandle {
        status,
        shutdown,
        join,
    }
}

pub fn start_app_server_sidecar_supervisor_if_enabled() -> bool {
    if !app_server_supervisor_enabled() {
        tracing::debug!(
            env = APP_SERVER_SUPERVISOR_ENV,
            "Skipping app-server sidecar supervisor"
        );
        return false;
    }

    start_app_server_sidecar_supervisor_once(AppServerSupervisorConfig::default())
}

pub fn start_app_server_sidecar_supervisor_once(config: AppServerSupervisorConfig) -> bool {
    let slot = app_server_supervisor_slot();
    let Ok(mut guard) = slot.lock() else {
        tracing::warn!("app-server supervisor slot lock poisoned; supervisor not started");
        return false;
    };
    if guard.is_some() {
        return false;
    }

    *guard = Some(spawn_app_server_sidecar_supervisor(config));
    true
}

pub fn app_server_sidecar_supervisor_status() -> AppServerSupervisorStatus {
    let slot = app_server_supervisor_slot();
    slot.lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(AppServerSupervisorHandle::status))
        .unwrap_or_default()
}

pub async fn shutdown_app_server_sidecar_supervisor_if_running() -> AppServerSupervisorStatus {
    let handle = app_server_supervisor_slot()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    match handle {
        Some(handle) => handle.shutdown().await,
        None => AppServerSupervisorStatus::default(),
    }
}

pub fn app_server_sidecar_binary() -> OsString {
    std::env::var_os(APP_SERVER_BINARY_ENV).unwrap_or_else(|| OsString::from("dasclaw-app-server"))
}

pub fn app_server_startup_smoke_enabled() -> bool {
    app_server_startup_smoke_enabled_value(std::env::var_os(APP_SERVER_STARTUP_SMOKE_ENV))
}

pub fn app_server_startup_smoke_enabled_value(value: Option<OsString>) -> bool {
    app_server_bool_env_value(value)
}

pub fn app_server_supervisor_enabled() -> bool {
    app_server_supervisor_enabled_value(std::env::var_os(APP_SERVER_SUPERVISOR_ENV))
}

pub fn app_server_supervisor_enabled_value(value: Option<OsString>) -> bool {
    app_server_bool_env_value(value)
}

fn app_server_bool_env_value(value: Option<OsString>) -> bool {
    value
        .and_then(|value| value.into_string().ok())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn app_server_supervisor_slot() -> &'static AppServerSupervisorSlot {
    static SLOT: OnceLock<AppServerSupervisorSlot> = OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None)))
}

async fn run_app_server_sidecar_supervisor(
    config: AppServerSupervisorConfig,
    status: Arc<Mutex<AppServerSupervisorStatus>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut restart_count = 0;
    let mut connection_generation = 0;

    loop {
        if *shutdown.borrow() {
            set_supervisor_status(&status, |current| {
                current.state = AppServerSupervisorState::Stopped;
            });
            return;
        }

        set_supervisor_status(&status, |current| {
            current.state = AppServerSupervisorState::Starting;
            current.restart_count = restart_count;
            current.connection_generation = connection_generation;
            current.last_error = None;
        });

        match start_supervised_app_server_sidecar(&config).await {
            Ok(mut sidecar) => {
                connection_generation += 1;
                sidecar.connection_generation = connection_generation;
                set_supervisor_status(&status, |current| {
                    current.state = AppServerSupervisorState::Ready;
                    current.restart_count = restart_count;
                    current.connection_generation = connection_generation;
                    current.notification_count = sidecar.notification_count;
                    current.runtime_health_status = sidecar.runtime_health_status.clone();
                    current.last_error = None;
                });

                let failure = supervise_running_app_server_sidecar(
                    &config,
                    &status,
                    &mut shutdown,
                    &mut sidecar,
                )
                .await;
                let Some(error) = failure else {
                    return;
                };

                shutdown_supervised_child(&mut sidecar.child).await;
                if restart_count >= config.max_restarts {
                    set_supervisor_status(&status, |current| {
                        current.state = AppServerSupervisorState::Failed;
                        current.restart_count = restart_count;
                        current.connection_generation = connection_generation;
                        current.last_error = Some(error.error);
                    });
                    return;
                }

                restart_count += 1;
                set_supervisor_status(&status, |current| {
                    current.state = AppServerSupervisorState::Restarting;
                    current.restart_count = restart_count;
                    current.connection_generation = connection_generation;
                    current.last_error = Some(error.error);
                });
            }
            Err(error) => {
                if restart_count >= config.max_restarts {
                    set_supervisor_status(&status, |current| {
                        current.state = AppServerSupervisorState::Failed;
                        current.restart_count = restart_count;
                        current.connection_generation = connection_generation;
                        current.last_error = Some(error.to_string());
                    });
                    return;
                }

                restart_count += 1;
                set_supervisor_status(&status, |current| {
                    current.state = AppServerSupervisorState::Restarting;
                    current.restart_count = restart_count;
                    current.connection_generation = connection_generation;
                    current.last_error = Some(error.to_string());
                });
            }
        }

        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    set_supervisor_status(&status, |current| {
                        current.state = AppServerSupervisorState::Stopped;
                    });
                    return;
                }
            }
            _ = tokio::time::sleep(config.restart_backoff) => {}
        }
    }
}

struct SupervisedSidecar {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    connection_generation: u64,
    notification_count: usize,
    runtime_health_status: Option<String>,
}

async fn start_supervised_app_server_sidecar(
    config: &AppServerSupervisorConfig,
) -> Result<SupervisedSidecar, BoxError> {
    let mut command = Command::new(&config.binary);
    command
        .args(&config.args)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| invalid_data("app-server supervisor stdin is unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid_data("app-server supervisor stdout is unavailable"))?;
    let mut stdout = BufReader::new(stdout);
    let mut notification_count = 0;

    let _initialize = send_supervisor_request(
        &mut stdin,
        &mut stdout,
        SUPERVISOR_INIT_ID,
        "initialize",
        Some(serde_json::json!({
            "client": {
                "name": "desktop-client-sidecar-supervisor",
                "version": "0.1.0",
                "transport": "stdio"
            },
            "protocolVersion": { "major": 0, "minor": 1, "patch": 0 },
            "requestedCapabilities": ["protocol", "lifecycle", "health", "session", "codex_app_server_v2"]
        })),
        config.request_timeout,
        &mut notification_count,
    )
    .await?;
    let health = send_supervisor_request(
        &mut stdin,
        &mut stdout,
        SUPERVISOR_HEALTH_ID,
        "health/check",
        Some(serde_json::json!({ "includeDetails": true })),
        config.request_timeout,
        &mut notification_count,
    )
    .await?;
    let runtime_health_status = service_health_status(&health, "runtime").ok();

    Ok(SupervisedSidecar {
        child,
        stdin,
        stdout,
        connection_generation: 0,
        notification_count,
        runtime_health_status,
    })
}

async fn supervise_running_app_server_sidecar(
    config: &AppServerSupervisorConfig,
    status: &Arc<Mutex<AppServerSupervisorStatus>>,
    shutdown: &mut watch::Receiver<bool>,
    sidecar: &mut SupervisedSidecar,
) -> Option<AppServerSupervisorFailure> {
    let mut interval = tokio::time::interval(config.health_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    shutdown_app_server_sidecar_session(config, sidecar).await;
                    set_supervisor_status(status, |current| {
                        current.state = AppServerSupervisorState::Stopped;
                    });
                    return None;
                }
            }
            _ = interval.tick() => {
                if let Err(error) = ensure_supervised_child_running(&mut sidecar.child) {
                    return Some(AppServerSupervisorFailure {
                        error: error.to_string(),
                    });
                }

                match send_supervisor_request(
                    &mut sidecar.stdin,
                    &mut sidecar.stdout,
                    SUPERVISOR_HEALTH_ID,
                    "health/check",
                    Some(serde_json::json!({ "includeDetails": true })),
                    config.request_timeout,
                    &mut sidecar.notification_count,
                )
                .await {
                    Ok(health) => {
                        sidecar.runtime_health_status = service_health_status(&health, "runtime").ok();
                        set_supervisor_status(status, |current| {
                            current.state = AppServerSupervisorState::Ready;
                            current.notification_count = sidecar.notification_count;
                            current.runtime_health_status = sidecar.runtime_health_status.clone();
                            current.last_error = None;
                        });
                    }
                    Err(error) => {
                        return Some(AppServerSupervisorFailure {
                            error: error.to_string(),
                        });
                    }
                }
            }
        }
    }
}

async fn shutdown_app_server_sidecar_session(
    config: &AppServerSupervisorConfig,
    sidecar: &mut SupervisedSidecar,
) {
    let _ = send_supervisor_request(
        &mut sidecar.stdin,
        &mut sidecar.stdout,
        SUPERVISOR_STOP_ID,
        "shutdown",
        Some(serde_json::json!({ "reason": "client_exit" })),
        config.request_timeout,
        &mut sidecar.notification_count,
    )
    .await;
    let _ = tokio::time::timeout(config.request_timeout, sidecar.child.wait()).await;
    shutdown_supervised_child(&mut sidecar.child).await;
}

async fn send_supervisor_request<W, R>(
    stdin: &mut W,
    stdout: &mut R,
    id: &str,
    method: &str,
    params: Option<serde_json::Value>,
    timeout: Duration,
    notification_count: &mut usize,
) -> Result<serde_json::Value, SupervisorRequestError>
where
    W: AsyncWrite + Unpin,
    R: AsyncBufRead + Unpin,
{
    Ok(send_supervisor_request_round_trip(
        stdin,
        stdout,
        id,
        method,
        params,
        timeout,
        notification_count,
    )
    .await?
    .response)
}

struct SupervisorResponseRoundTrip {
    response: serde_json::Value,
}

async fn send_supervisor_request_round_trip<W, R>(
    stdin: &mut W,
    stdout: &mut R,
    id: &str,
    method: &str,
    params: Option<serde_json::Value>,
    timeout: Duration,
    notification_count: &mut usize,
) -> Result<SupervisorResponseRoundTrip, SupervisorRequestError>
where
    W: AsyncWrite + Unpin,
    R: AsyncBufRead + Unpin,
{
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .map_err(|error| SupervisorRequestError::Transport(error.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|error| SupervisorRequestError::Transport(error.to_string()))?;

    tokio::time::timeout(
        timeout,
        read_supervisor_response_round_trip(stdout, id, notification_count),
    )
    .await
    .map_err(|_| {
        SupervisorRequestError::Timeout(format!(
            "app-server supervisor request {method} timed out after {}ms",
            timeout.as_millis()
        ))
    })?
}

#[cfg(test)]
async fn read_supervisor_response<R>(
    stdout: &mut R,
    expected_id: &str,
    notification_count: &mut usize,
) -> Result<serde_json::Value, BoxError>
where
    R: AsyncBufRead + Unpin,
{
    Ok(
        read_supervisor_response_round_trip(stdout, expected_id, notification_count)
            .await?
            .response,
    )
}

async fn read_supervisor_response_round_trip<R>(
    stdout: &mut R,
    expected_id: &str,
    notification_count: &mut usize,
) -> Result<SupervisorResponseRoundTrip, SupervisorRequestError>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = stdout
            .read_line(&mut line)
            .await
            .map_err(|error| SupervisorRequestError::Transport(error.to_string()))?;
        if bytes == 0 {
            return Err(SupervisorRequestError::Transport(
                "app-server supervisor stdout closed".to_string(),
            ));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|error| SupervisorRequestError::Protocol(error.to_string()))?;
        if value.get("id").is_none() && value.get("method").is_some() {
            validate_app_server_notification(&value)
                .map_err(|error| SupervisorRequestError::Protocol(error.to_string()))?;
            reject_supervisor_queue_overflow_notification(&value)?;
            *notification_count += 1;
            continue;
        }
        reject_supervisor_json_rpc_error(&value)?;
        if value.get("id").and_then(serde_json::Value::as_str) == Some(expected_id) {
            return Ok(SupervisorResponseRoundTrip { response: value });
        }
        if let Some(actual_id) = value.get("id") {
            return Err(SupervisorRequestError::Protocol(format!(
                "unexpected app-server supervisor response id: expected {expected_id}, got {actual_id}"
            )));
        }
        return Err(SupervisorRequestError::Protocol(
            "app-server supervisor line is neither response nor notification".to_string(),
        ));
    }
}

fn ensure_supervised_child_running(child: &mut Child) -> Result<(), BoxError> {
    match child.try_wait()? {
        Some(status) => Err(invalid_data(format!(
            "app-server sidecar exited with {status}"
        ))),
        None => Ok(()),
    }
}

async fn shutdown_supervised_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;
}

fn supervisor_status_snapshot(
    status: &Arc<Mutex<AppServerSupervisorStatus>>,
) -> AppServerSupervisorStatus {
    status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| AppServerSupervisorStatus {
            state: AppServerSupervisorState::Failed,
            last_error: Some("app-server supervisor status lock poisoned".to_string()),
            ..AppServerSupervisorStatus::default()
        })
}

fn set_supervisor_status(
    status: &Arc<Mutex<AppServerSupervisorStatus>>,
    update: impl FnOnce(&mut AppServerSupervisorStatus),
) {
    if let Ok(mut status) = status.lock() {
        update(&mut status);
    }
}

pub fn app_server_stdio_smoke_requests() -> String {
    [
        r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"desktop-client-sidecar-smoke","version":"0.1.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":["protocol","lifecycle","health","session"]}}"#,
        r#"{"jsonrpc":"2.0","id":"health","method":"health/check","params":{"includeDetails":true}}"#,
        r#"{"jsonrpc":"2.0","id":"capabilities","method":"capabilities/list"}"#,
        r#"{"jsonrpc":"2.0","id":"stop","method":"shutdown","params":{"reason":"test"}}"#,
    ]
    .join("\n")
        + "\n"
}

pub fn parse_app_server_stdio_smoke_stdout(
    stdout: &str,
) -> Result<AppServerSidecarSmokeReport, BoxError> {
    let mut notification_count = 0;
    let mut initialize_response = None;
    let mut health_response = None;
    let mut capabilities_response = None;
    let mut shutdown_response = None;
    let mut response_order = Vec::new();

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let value = serde_json::from_str::<serde_json::Value>(line)?;
        if value.get("method").is_some() && value.get("id").is_none() {
            validate_app_server_notification(&value)?;
            reject_queue_overflow_notification(&value)?;
            notification_count += 1;
            continue;
        }

        reject_json_rpc_error(&value)?;
        match value.get("id").and_then(serde_json::Value::as_str) {
            Some("init") => {
                response_order.push("init");
                initialize_response = Some(value);
            }
            Some("health") => {
                response_order.push("health");
                health_response = Some(value);
            }
            Some("capabilities") => {
                response_order.push("capabilities");
                capabilities_response = Some(value);
            }
            Some("stop") => {
                response_order.push("stop");
                shutdown_response = Some(value);
            }
            Some(id) => {
                return Err(invalid_data(format!("unexpected smoke response id: {id}")));
            }
            None => {
                return Err(invalid_data(
                    "smoke line is neither response nor notification",
                ))
            }
        }
    }

    let initialize =
        initialize_response.ok_or_else(|| invalid_data("missing initialize response"))?;
    let health = health_response.ok_or_else(|| invalid_data("missing health response"))?;
    let capabilities =
        capabilities_response.ok_or_else(|| invalid_data("missing capabilities response"))?;
    let shutdown = shutdown_response.ok_or_else(|| invalid_data("missing shutdown response"))?;
    if response_order != ["init", "health", "capabilities", "stop"] {
        return Err(invalid_data(format!(
            "unexpected smoke response order: {}",
            response_order.join(",")
        )));
    }
    let initialize_state = initialize
        .pointer("/result/lifecycle/state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_data("missing initialize lifecycle state"))?;
    if initialize_state != "ready" {
        return Err(invalid_data(format!(
            "initialize lifecycle state must be ready, got {initialize_state}"
        )));
    }
    let health_ok = health
        .pointer("/result/ok")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| invalid_data("missing health ok flag"))?;
    if !health_ok {
        return Err(invalid_data("health response ok must be true"));
    }
    let session = capabilities
        .pointer("/result/capabilities/session")
        .ok_or_else(|| invalid_data("missing session capability"))?;
    let session_status = string_field(session, "status")?;
    let methods = session
        .get("methods")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_data("session capability methods must be an array"))?;
    if !methods
        .iter()
        .any(|method| method.as_str() == Some("turn/start"))
    {
        return Err(invalid_data(
            "session capability did not advertise turn/start",
        ));
    }

    let runtime_health_status = service_health_status(&health, "runtime")?;
    let shutdown_state = shutdown
        .pointer("/result/lifecycle/state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_data("missing shutdown lifecycle state"))?
        .to_string();

    Ok(AppServerSidecarSmokeReport {
        notification_count,
        session_status,
        runtime_health_status,
        shutdown_state,
    })
}

/// Get the external server URL
pub fn get_server_url() -> String {
    format!("http://127.0.0.1:{}", EMBEDDED_SERVER_PORT)
}

/// Print instructions for starting the external IronClaw server
pub fn print_server_instructions() {
    eprintln!(
        "\n⚠️  External IronClaw server is not running on port {}",
        EMBEDDED_SERVER_PORT
    );
    eprintln!("\n📋 To start the IronClaw server:");
    eprintln!("   1. Open a new terminal");
    eprintln!("   2. Run: cargo run -- run --no-onboard");
    eprintln!("   3. Wait for the server to start");
    eprintln!("   4. Restart the Desktop Client\n");
    eprintln!("💡 The Desktop Client will continue, but chat features will not work.\n");
}

fn reject_json_rpc_error(value: &serde_json::Value) -> Result<(), BoxError> {
    if let Some(error) = value.get("error") {
        return Err(invalid_data(format!("app-server JSON-RPC error: {error}")));
    }
    Ok(())
}

fn reject_supervisor_json_rpc_error(
    value: &serde_json::Value,
) -> Result<(), SupervisorRequestError> {
    if let Some(error) = value.get("error") {
        return Err(SupervisorRequestError::JsonRpc(error.to_string()));
    }
    Ok(())
}

fn reject_supervisor_queue_overflow_notification(
    value: &serde_json::Value,
) -> Result<(), SupervisorRequestError> {
    if !is_queue_overflow_notification(value) {
        return Ok(());
    }

    let message = value
        .pointer("/params/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("app-server notification queue overflowed; reconnect required");
    Err(SupervisorRequestError::Protocol(format!(
        "app-server supervisor notification queue overflow: {message}"
    )))
}

fn reject_queue_overflow_notification(value: &serde_json::Value) -> Result<(), BoxError> {
    if !is_queue_overflow_notification(value) {
        return Ok(());
    }

    let message = value
        .pointer("/params/message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("app-server notification queue overflowed; reconnect required");
    Err(invalid_data(format!(
        "app-server notification queue overflow: {message}"
    )))
}

fn is_queue_overflow_notification(value: &serde_json::Value) -> bool {
    value.get("method").and_then(serde_json::Value::as_str) == Some("error")
        && value
            .pointer("/params/code")
            .and_then(serde_json::Value::as_str)
            == Some("NOTIFICATION_QUEUE_OVERFLOW")
}

fn validate_app_server_notification(value: &serde_json::Value) -> Result<(), BoxError> {
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_data("app-server notification method must be a string"))?;
    if !matches!(
        method,
        "notifications/initialized"
            | "lifecycle/changed"
            | "health/changed"
            | "capabilities/changed"
            | "log/entry"
            | "thread/created"
            | "thread/started"
            | "turn/started"
            | "turn/delta"
            | "turn/completed"
            | "turn/failed"
            | "turn/cancelled"
            | "item/started"
            | "item/agentMessage/delta"
            | "item/completed"
            | "error"
    ) {
        return Err(invalid_data(format!(
            "unexpected app-server notification method: {method}"
        )));
    }
    if !value
        .get("params")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(invalid_data(
            "app-server notification params must be an object",
        ));
    }
    Ok(())
}

fn string_field(value: &serde_json::Value, field: &str) -> Result<String, BoxError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| invalid_data(format!("missing string field: {field}")))
}

fn service_health_status(
    value: &serde_json::Value,
    service_name: &str,
) -> Result<String, BoxError> {
    let services = value
        .pointer("/result/services")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_data("health response services must be an array"))?;
    services
        .iter()
        .find(|service| {
            service.get("service").and_then(serde_json::Value::as_str) == Some(service_name)
        })
        .and_then(|service| service.get("status").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .ok_or_else(|| invalid_data(format!("missing health service status: {service_name}")))
}

fn invalid_data(message: impl Into<String>) -> BoxError {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn app_server_stdio_smoke_requests_cover_initialize_health_capabilities_and_shutdown() {
        let requests = app_server_stdio_smoke_requests();

        assert!(requests.contains(r#""method":"initialize""#));
        assert!(requests.contains(r#""method":"health/check""#));
        assert!(requests.contains(r#""method":"capabilities/list""#));
        assert!(requests.contains(r#""method":"shutdown""#));
        assert!(requests.ends_with('\n'));
    }

    #[test]
    fn app_server_startup_smoke_env_is_opt_in() {
        assert!(!app_server_startup_smoke_enabled_value(None));
        assert!(!app_server_startup_smoke_enabled_value(Some(
            OsString::from("0")
        )));
        assert!(app_server_startup_smoke_enabled_value(Some(
            OsString::from("1")
        )));
        assert!(app_server_startup_smoke_enabled_value(Some(
            OsString::from("true")
        )));
        assert!(app_server_startup_smoke_enabled_value(Some(
            OsString::from("YES")
        )));
    }

    #[test]
    fn app_server_supervisor_env_is_opt_in() {
        assert!(!app_server_supervisor_enabled_value(None));
        assert!(!app_server_supervisor_enabled_value(Some(OsString::from(
            "0"
        ))));
        assert!(app_server_supervisor_enabled_value(Some(OsString::from(
            "1"
        ))));
        assert!(app_server_supervisor_enabled_value(Some(OsString::from(
            "true"
        ))));
        assert!(app_server_supervisor_enabled_value(Some(OsString::from(
            "YES"
        ))));
    }

    #[test]
    fn parses_app_server_stdio_smoke_transcript() {
        let stdout = [
            r#"{"jsonrpc":"2.0","method":"lifecycle/changed","params":{"lifecycle":{"state":"initializing","reason":"initialize_requested","since":"0"},"previousState":"starting"}}"#,
            r#"{"jsonrpc":"2.0","method":"thread/started","params":{"threadId":"thread_1"}}"#,
            r#"{"jsonrpc":"2.0","id":"init","result":{"lifecycle":{"state":"ready"}}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["thread/create","turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","method":"lifecycle/changed","params":{"lifecycle":{"state":"stopped","reason":"shutdown_requested","since":"1"},"previousState":"ready"}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let report = parse_app_server_stdio_smoke_stdout(&stdout).expect("transcript should parse");

        assert_eq!(report.notification_count, 3);
        assert_eq!(report.session_status, "implemented");
        assert_eq!(report.runtime_health_status, "degraded");
        assert_eq!(report.shutdown_state, "stopped");
    }

    #[test]
    fn rejects_app_server_stdio_smoke_error_response() {
        let stdout = [
            r#"{"jsonrpc":"2.0","id":"capabilities","error":{"code":-32002,"message":"bad"}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("JSON-RPC error responses should fail the smoke");

        assert!(error.to_string().contains("JSON-RPC error"));
    }

    #[test]
    fn rejects_app_server_stdio_smoke_without_initialize_response() {
        let stdout = [
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("initialize response is part of the smoke contract");

        assert!(error.to_string().contains("missing initialize response"));
    }

    #[test]
    fn rejects_app_server_stdio_smoke_when_health_is_not_ok() {
        let stdout = [
            r#"{"jsonrpc":"2.0","id":"init","result":{"lifecycle":{"state":"ready"}}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":false,"services":[{"service":"runtime","status":"failed","failSafe":true}]}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("health ok=false should fail the smoke");

        assert!(error
            .to_string()
            .contains("health response ok must be true"));
    }

    #[test]
    fn rejects_app_server_stdio_smoke_out_of_order_responses() {
        let stdout = [
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}"#,
            r#"{"jsonrpc":"2.0","id":"init","result":{"lifecycle":{"state":"ready"}}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("smoke transcript should preserve response order");

        assert!(error
            .to_string()
            .contains("unexpected smoke response order"));
    }

    #[test]
    fn rejects_app_server_stdio_smoke_unknown_response_id() {
        let stdout = [
            r#"{"jsonrpc":"2.0","id":"init","result":{"lifecycle":{"state":"ready"}}}"#,
            r#"{"jsonrpc":"2.0","id":"unknown","result":{"ok":true}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("unknown response ids should fail the smoke");

        assert!(error.to_string().contains("unexpected smoke response id"));
    }

    #[test]
    fn rejects_app_server_stdio_smoke_unknown_notification_method() {
        let stdout = [
            r#"{"jsonrpc":"2.0","method":"fake/event","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":"init","result":{"lifecycle":{"state":"ready"}}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("unknown notification methods should fail the smoke");

        assert!(error
            .to_string()
            .contains("unexpected app-server notification method"));
    }

    #[test]
    fn rejects_app_server_stdio_smoke_queue_overflow_notification() {
        let stdout = [
            r#"{"jsonrpc":"2.0","method":"error","params":{"code":"NOTIFICATION_QUEUE_OVERFLOW","message":"client notification queue exceeded bounded capacity; reconnect required","retryable":true}}"#,
            r#"{"jsonrpc":"2.0","id":"init","result":{"lifecycle":{"state":"ready"}}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}"#,
            r#"{"jsonrpc":"2.0","id":"capabilities","result":{"capabilities":{"session":{"status":"implemented","methods":["turn/start"]}}}}"#,
            r#"{"jsonrpc":"2.0","id":"stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}"#,
        ]
        .join("\n");

        let error = parse_app_server_stdio_smoke_stdout(&stdout)
            .expect_err("queue overflow should fail sidecar smoke parsing");

        assert!(error.to_string().contains("notification queue overflow"));
    }

    #[tokio::test]
    async fn supervisor_response_rejects_unexpected_response_id() {
        let stdout = r#"{"jsonrpc":"2.0","id":"wrong","result":{"ok":true}}"#.to_string() + "\n";
        let mut stdout = BufReader::new(stdout.as_bytes());
        let mut notification_count = 0;

        let error = read_supervisor_response(&mut stdout, "health", &mut notification_count)
            .await
            .expect_err("unexpected supervisor response ids should fail fast");

        assert!(error
            .to_string()
            .contains("unexpected app-server supervisor response id"));
        assert_eq!(notification_count, 0);
    }

    #[tokio::test]
    async fn supervisor_response_rejects_malformed_response_line() {
        let stdout = [
            r#"{"jsonrpc":"2.0","result":{"ok":true}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true}}"#,
        ]
        .join("\n")
            + "\n";
        let mut stdout = BufReader::new(stdout.as_bytes());
        let mut notification_count = 0;

        let error = read_supervisor_response(&mut stdout, "health", &mut notification_count)
            .await
            .expect_err("malformed supervisor response lines should fail fast");

        assert!(error
            .to_string()
            .contains("neither response nor notification"));
        assert_eq!(notification_count, 0);
    }

    #[tokio::test]
    async fn supervisor_response_accepts_codex_thread_started_notification() {
        let stdout = [
            r#"{"jsonrpc":"2.0","method":"thread/started","params":{"threadId":"thread_1"}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true}}"#,
        ]
        .join("\n")
            + "\n";
        let mut stdout = BufReader::new(stdout.as_bytes());
        let mut notification_count = 0;

        let response = read_supervisor_response(&mut stdout, "health", &mut notification_count)
            .await
            .expect("thread/started should be a valid app-server notification");

        assert_eq!(response["id"], "health");
        assert_eq!(notification_count, 1);
    }

    #[tokio::test]
    async fn supervisor_response_rejects_notification_queue_overflow() {
        let stdout = [
            r#"{"jsonrpc":"2.0","method":"error","params":{"code":"NOTIFICATION_QUEUE_OVERFLOW","message":"client notification queue exceeded bounded capacity; reconnect required","retryable":true}}"#,
            r#"{"jsonrpc":"2.0","id":"health","result":{"ok":true}}"#,
        ]
        .join("\n")
            + "\n";
        let mut stdout = BufReader::new(stdout.as_bytes());
        let mut notification_count = 0;

        let error = read_supervisor_response(&mut stdout, "health", &mut notification_count)
            .await
            .expect_err("queue overflow notifications should force supervisor reconnect handling");

        assert!(error.to_string().contains("notification queue overflow"));
        assert_eq!(notification_count, 0);
    }

    #[tokio::test]
    async fn app_server_stdio_smoke_spawns_configured_binary_when_set() {
        if std::env::var_os(APP_SERVER_BINARY_ENV).is_none() {
            return;
        }

        let report = check_app_server_sidecar_stdio()
            .await
            .expect("configured app-server sidecar should complete stdio smoke");

        assert!(report.notification_count >= 3);
        assert_eq!(report.session_status, "implemented");
        assert_eq!(report.runtime_health_status, "degraded");
        assert_eq!(report.shutdown_state, "stopped");
    }

    #[tokio::test]
    async fn app_server_stdio_smoke_times_out_hanging_process() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("sleep 5");

        let error =
            check_app_server_sidecar_stdio_with_command(&mut command, Duration::from_millis(20))
                .await
                .expect_err("hanging sidecar should time out");

        assert!(error.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn app_server_supervisor_reaches_ready_and_shutdown_stops_sidecar() {
        let transcript_dir = tempfile::tempdir().expect("transcript dir");
        let transcript_path = transcript_dir.path().join("supervisor-requests.jsonl");
        let config =
            shell_supervisor_config(&strict_supervisor_script(true, Some(&transcript_path)), 1);
        let handle = spawn_app_server_sidecar_supervisor(config);

        let ready = wait_for_supervisor_state(
            &handle,
            AppServerSupervisorState::Ready,
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(ready.state, AppServerSupervisorState::Ready);
        assert_eq!(ready.notification_count, 2);
        assert_eq!(ready.runtime_health_status.as_deref(), Some("degraded"));

        let stopped = handle.shutdown().await;
        assert_eq!(stopped.state, AppServerSupervisorState::Stopped);
        assert_supervisor_transcript_includes_shutdown(&transcript_path);
    }

    #[tokio::test]
    async fn app_server_supervisor_restarts_and_fails_after_repeated_start_errors() {
        let config = shell_supervisor_config("exit 7", 1);
        let handle = spawn_app_server_sidecar_supervisor(config);

        let failed = wait_for_supervisor_state(
            &handle,
            AppServerSupervisorState::Failed,
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(failed.state, AppServerSupervisorState::Failed);
        assert_eq!(failed.restart_count, 1);
        assert!(
            failed
                .last_error
                .as_deref()
                .unwrap_or_default()
                .contains("stdout closed")
                || failed
                    .last_error
                    .as_deref()
                    .unwrap_or_default()
                    .contains("Broken pipe"),
            "unexpected supervisor error: {failed:?}"
        );

        let _ = handle.shutdown().await;
    }

    #[tokio::test]
    async fn app_server_supervisor_treats_dropped_handle_as_shutdown() {
        let config = shell_supervisor_config(&strict_supervisor_script(false, None), 1);
        let handle = spawn_app_server_sidecar_supervisor(config);
        let status = std::sync::Arc::clone(&handle.status);

        let ready = wait_for_supervisor_state(
            &handle,
            AppServerSupervisorState::Ready,
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(ready.state, AppServerSupervisorState::Ready);

        drop(handle);
        let stopped = wait_for_status_snapshot(
            &status,
            AppServerSupervisorState::Stopped,
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(stopped.state, AppServerSupervisorState::Stopped);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn app_server_supervisor_global_slot_starts_reports_and_shutdowns() {
        let _ = shutdown_app_server_sidecar_supervisor_if_running().await;
        let config = shell_supervisor_config(&strict_supervisor_script(false, None), 1);

        assert!(start_app_server_sidecar_supervisor_once(config));
        assert!(
            !start_app_server_sidecar_supervisor_once(shell_supervisor_config("exit 9", 0)),
            "second supervisor start should be ignored while one is running"
        );

        let ready = wait_for_global_supervisor_state(
            AppServerSupervisorState::Ready,
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(ready.state, AppServerSupervisorState::Ready);
        assert_eq!(ready.runtime_health_status.as_deref(), Some("degraded"));

        let stopped = shutdown_app_server_sidecar_supervisor_if_running().await;
        assert_eq!(stopped.state, AppServerSupervisorState::Stopped);
        assert_eq!(
            app_server_sidecar_supervisor_status().state,
            AppServerSupervisorState::Stopped
        );
    }

    fn shell_supervisor_config(script: &str, max_restarts: usize) -> AppServerSupervisorConfig {
        AppServerSupervisorConfig {
            binary: OsString::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from(script)],
            health_interval: Duration::from_millis(20),
            restart_backoff: Duration::from_millis(10),
            request_timeout: Duration::from_millis(100),
            max_restarts,
        }
    }

    fn strict_supervisor_script(
        ready_notification: bool,
        transcript_path: Option<&Path>,
    ) -> String {
        let transcript_line = transcript_path
            .map(|path| {
                format!(
                    "printf '%s\\n' \"$line\" >> {}\n",
                    shell_quote(&path.to_string_lossy())
                )
            })
            .unwrap_or_default();
        let mut script = String::from("while IFS= read -r line; do\n");
        script.push_str(&transcript_line);
        script.push_str(
            r#"case "$line" in
  *'"id":"supervisor-init"'*'"method":"initialize"'*'"requestedCapabilities"'*)
"#,
        );
        if ready_notification {
            script.push_str(
                r#"    echo '{"jsonrpc":"2.0","method":"lifecycle/changed","params":{"lifecycle":{"state":"ready"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized","params":{"lifecycle":{"state":"ready"},"compatibilityProfiles":[{"id":"codex_app_server_v2","version":"2.0.0-compat","scope":"chat_session_subset","description":"test profile","methods":["initialize"],"events":["notifications/initialized"],"aliases":[],"eventQueue":{"maxPendingNotifications":256,"overflow":"lag_disconnect"}}],"eventQueue":{"maxPendingNotifications":256,"overflow":"lag_disconnect"}}}'
"#,
            );
        }
        script.push_str(
            r#"    echo '{"jsonrpc":"2.0","id":"supervisor-init","result":{"lifecycle":{"state":"ready"}}}'
    ;;
  *'"id":"supervisor-health"'*'"method":"health/check"'*'"includeDetails":true'*)
    echo '{"jsonrpc":"2.0","id":"supervisor-health","result":{"ok":true,"services":[{"service":"runtime","status":"degraded","failSafe":false}]}}'
    ;;
  *'"id":"supervisor-stop"'*'"method":"shutdown"'*'"reason":"client_exit"'*)
    echo '{"jsonrpc":"2.0","id":"supervisor-stop","result":{"accepted":true,"lifecycle":{"state":"stopped"}}}'
    exit 0
    ;;
  *)
    echo '{"jsonrpc":"2.0","id":"unexpected","error":{"code":-32601,"message":"unexpected supervisor request"}}'
    exit 2
    ;;
esac
done"#,
        );
        script
    }

    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', r#"'\''"#))
    }

    fn assert_supervisor_transcript_includes_shutdown(path: &Path) {
        let transcript =
            std::fs::read_to_string(path).expect("supervisor transcript should be readable");
        assert!(
            transcript
                .lines()
                .any(|line| line.contains(r#""id":"supervisor-stop""#)
                    && line.contains(r#""method":"shutdown""#)),
            "supervisor transcript should include shutdown request:\n{transcript}"
        );
    }

    async fn wait_for_supervisor_state(
        handle: &AppServerSupervisorHandle,
        state: AppServerSupervisorState,
        timeout: Duration,
    ) -> AppServerSupervisorStatus {
        wait_for_status_snapshot(&handle.status, state, timeout).await
    }

    async fn wait_for_status_snapshot(
        status: &std::sync::Arc<std::sync::Mutex<AppServerSupervisorStatus>>,
        state: AppServerSupervisorState,
        timeout: Duration,
    ) -> AppServerSupervisorStatus {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let snapshot = supervisor_status_snapshot(status);
            if snapshot.state == state || tokio::time::Instant::now() >= deadline {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn wait_for_global_supervisor_state(
        state: AppServerSupervisorState,
        timeout: Duration,
    ) -> AppServerSupervisorStatus {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let snapshot = app_server_sidecar_supervisor_status();
            if snapshot.state == state || tokio::time::Instant::now() >= deadline {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
