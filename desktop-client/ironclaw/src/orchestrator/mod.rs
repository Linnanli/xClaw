//! Orchestrator for managing sandboxed worker containers.
//!
//! The orchestrator runs in the main agent process and provides:
//! - An internal HTTP API for worker communication (LLM proxy, status, secrets)
//! - Per-job bearer token authentication
//! - Container lifecycle management (create, monitor, stop)
//!
//! ```text
//! ┌───────────────────────────────────────────────┐
//! │              Orchestrator                       │
//! │                                                 │
//! │  Internal API (default :50051, configurable)    │
//! │    POST /worker/{id}/llm/complete               │
//! │    POST /worker/{id}/llm/complete_with_tools    │
//! │    GET  /worker/{id}/job                        │
//! │    GET  /worker/{id}/credentials                │
//! │    POST /worker/{id}/status                     │
//! │    POST /worker/{id}/complete                   │
//! │                                                 │
//! │  ContainerJobManager                            │
//! │    create_job() -> container + token             │
//! │    stop_job()                                    │
//! │    list_jobs()                                   │
//! │                                                 │
//! │  TokenStore                                     │
//! │    per-job bearer tokens (in-memory only)       │
//! │    per-job credential grants (in-memory only)   │
//! └───────────────────────────────────────────────┘
//! ```

pub mod api;
pub mod auth;
pub mod job_manager;
pub mod reaper;

pub use api::OrchestratorApi;
pub use auth::{CredentialGrant, TokenStore};
pub use job_manager::{
    CompletionResult, ContainerHandle, ContainerJobConfig, ContainerJobManager, JobMode,
};
pub use reaper::{ReaperConfig, SandboxReaper};

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::db::Database;
use crate::llm::LlmProvider;
use dasclaw_runtime::secrets::SecretsStore;
use ironclaw_common::AppEvent;

/// Resolve the orchestrator port from the `ORCHESTRATOR_PORT` environment
/// variable, falling back to 50051.
fn resolve_orchestrator_port() -> u16 {
    std::env::var("ORCHESTRATOR_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50051)
}

/// Result of orchestrator setup, containing all handles needed by the agent.
pub struct OrchestratorSetup {
    pub container_job_manager: Option<Arc<ContainerJobManager>>,
    pub job_event_tx: Option<broadcast::Sender<(Uuid, String, AppEvent)>>,
    pub prompt_queue: Arc<Mutex<HashMap<Uuid, VecDeque<api::PendingPrompt>>>>,
    pub docker_status: crate::sandbox::DockerStatus,
}

/// Errors returned by [`setup_orchestrator`] when the orchestrator cannot
/// be brought up safely (ADR-119 F2 — fail-closed boot).
///
/// Replacing the historical silent downgrade: if the operator configures
/// `JobRuntimeMode::LocalContainer` we MUST refuse to boot rather than
/// quietly running the agent without a working sandbox.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorSetupError {
    /// `JobRuntimeMode::LocalContainer` requires Docker but it is not
    /// installed on this host.
    #[error(
        "JobRuntimeMode::LocalContainer requires Docker but it is not installed. {hint}\n\
         Hint: set [job_runtime] mode = \"disabled\" or install Docker."
    )]
    DockerNotInstalled { hint: String },

    /// `JobRuntimeMode::LocalContainer` requires Docker but the daemon is
    /// not currently running.
    #[error(
        "JobRuntimeMode::LocalContainer requires Docker but the daemon is not running. {hint}\n\
         Hint: start Docker or set [job_runtime] mode = \"disabled\"."
    )]
    DockerNotRunning { hint: String },

    /// `JobRuntimeMode::Cloud` is reserved for a future ADR; refuse to
    /// boot rather than silently behaving like `Disabled`.
    #[error(
        "JobRuntimeMode::Cloud (endpoint = {endpoint:?}) is not yet implemented. \
         Use mode = \"disabled\" or mode = \"local_container\"."
    )]
    CloudNotImplemented { endpoint: String },
}

/// Detect Docker availability, create the container job manager, and start
/// the orchestrator internal API in the background.
///
/// Dispatches on [`crate::config::JobRuntimeMode`] (ADR-119 F2):
/// - `Disabled`     → skip docker probe, return empty setup
/// - `LocalContainer` → require working Docker, else fail-closed with
///   [`OrchestratorSetupError::DockerNotInstalled`] / `DockerNotRunning`
/// - `Cloud { .. }` → reserved; fails with [`OrchestratorSetupError::CloudNotImplemented`]
pub async fn setup_orchestrator(
    config: &crate::config::Config,
    llm: &Arc<dyn LlmProvider>,
    db: Option<&Arc<dyn Database>>,
    secrets_store: Option<&Arc<dyn SecretsStore + Send + Sync>>,
) -> Result<OrchestratorSetup, OrchestratorSetupError> {
    let prompt_queue = Arc::new(Mutex::new(
        HashMap::<Uuid, VecDeque<api::PendingPrompt>>::new(),
    ));

    match &config.job_runtime.mode {
        crate::config::JobRuntimeMode::Disabled => Ok(OrchestratorSetup {
            container_job_manager: None,
            job_event_tx: None,
            prompt_queue,
            docker_status: crate::sandbox::DockerStatus::Disabled,
        }),
        crate::config::JobRuntimeMode::LocalContainer => {
            setup_local_container(config, llm, db, secrets_store, prompt_queue).await
        }
        crate::config::JobRuntimeMode::Cloud { endpoint } => {
            Err(OrchestratorSetupError::CloudNotImplemented {
                endpoint: endpoint.clone(),
            })
        }
    }
}

/// Build the local-container orchestrator (probe Docker, spawn API,
/// construct `ContainerJobManager`). Fails closed when Docker is
/// unavailable.
async fn setup_local_container(
    config: &crate::config::Config,
    llm: &Arc<dyn LlmProvider>,
    db: Option<&Arc<dyn Database>>,
    secrets_store: Option<&Arc<dyn SecretsStore + Send + Sync>>,
    prompt_queue: Arc<Mutex<HashMap<Uuid, VecDeque<api::PendingPrompt>>>>,
) -> Result<OrchestratorSetup, OrchestratorSetupError> {
    let detection = crate::sandbox::check_docker().await;
    let docker_status = detection.status;
    match docker_status {
        crate::sandbox::DockerStatus::Available => {
            tracing::info!("Docker is available");
        }
        crate::sandbox::DockerStatus::NotInstalled => {
            return Err(OrchestratorSetupError::DockerNotInstalled {
                hint: detection.platform.install_hint().to_string(),
            });
        }
        crate::sandbox::DockerStatus::NotRunning => {
            return Err(OrchestratorSetupError::DockerNotRunning {
                hint: detection.platform.start_hint().to_string(),
            });
        }
        crate::sandbox::DockerStatus::Disabled => {
            // Should not happen: setup_local_container is only entered
            // for JobRuntimeMode::LocalContainer. Treat as fail-closed.
            return Err(OrchestratorSetupError::DockerNotRunning {
                hint: "internal: docker probe returned Disabled in LocalContainer mode".to_string(),
            });
        }
    }

    let (tx, _) = broadcast::channel(256);
    let job_event_tx = Some(tx);

    let token_store = TokenStore::new();
    let orchestrator_port = resolve_orchestrator_port();
    let job_config = ContainerJobConfig {
        image: config.sandbox.image.clone(),
        memory_limit_mb: config.sandbox.memory_limit_mb,
        cpu_shares: config.sandbox.cpu_shares,
        orchestrator_port,
        claude_code_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
        claude_code_oauth_token: crate::config::ClaudeCodeConfig::extract_oauth_token(),
        claude_code_model: config.claude_code.model.clone(),
        claude_code_max_turns: config.claude_code.max_turns,
        claude_code_memory_limit_mb: config.claude_code.memory_limit_mb,
        claude_code_allowed_tools: config.claude_code.allowed_tools.clone(),
    };
    let jm = Arc::new(ContainerJobManager::new(job_config, token_store.clone()));

    let orchestrator_state = api::OrchestratorState {
        llm: Arc::clone(llm),
        job_manager: Arc::clone(&jm),
        token_store,
        job_event_tx: job_event_tx.clone(),
        prompt_queue: Arc::clone(&prompt_queue),
        store: db.cloned(),
        secrets_store: secrets_store.cloned(),
        user_id: "default".to_string(),
        job_owner_cache: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };

    tokio::spawn(async move {
        if let Err(e) = OrchestratorApi::start(orchestrator_state, orchestrator_port).await {
            tracing::error!("Orchestrator API failed: {}", e);
        }
    });

    if config.claude_code.enabled {
        tracing::info!(
            "Claude Code sandbox mode available (model: {}, max_turns: {})",
            config.claude_code.model,
            config.claude_code.max_turns
        );
    }

    Ok(OrchestratorSetup {
        container_job_manager: Some(jm),
        job_event_tx,
        prompt_queue,
        docker_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::helpers::lock_env;

    #[test]
    fn resolve_orchestrator_port_from_env() {
        let _guard = lock_env();

        // Safety: env-var mutation requires unsafe in edition 2024;
        // lock_env() serializes concurrent access from other test threads.

        // Absent env var → default 50051
        unsafe { std::env::remove_var("ORCHESTRATOR_PORT") };
        assert_eq!(resolve_orchestrator_port(), 50051);

        // Valid custom port
        unsafe { std::env::set_var("ORCHESTRATOR_PORT", "50052") };
        assert_eq!(resolve_orchestrator_port(), 50052);

        // Non-numeric value → fallback to default
        unsafe { std::env::set_var("ORCHESTRATOR_PORT", "not_a_port") };
        assert_eq!(resolve_orchestrator_port(), 50051);

        // Out of u16 range → fallback to default
        unsafe { std::env::set_var("ORCHESTRATOR_PORT", "99999") };
        assert_eq!(resolve_orchestrator_port(), 50051);

        // Cleanup
        unsafe { std::env::remove_var("ORCHESTRATOR_PORT") };
    }

    // ─── ADR-119 F2: fail-closed boot dispatch ────────────────────────────

    fn build_test_config(mode: crate::config::JobRuntimeMode) -> crate::config::Config {
        let tmp = std::env::temp_dir().join(format!("ironclaw-orch-test-{}", uuid::Uuid::new_v4()));
        let mut cfg = crate::config::Config::for_testing(
            tmp.join("db.sqlite"),
            tmp.join("skills"),
            tmp.join("installed_skills"),
        );
        cfg.job_runtime = crate::config::JobRuntimeConfig { mode };
        cfg
    }

    #[tokio::test]
    async fn req_adr119_f2_disabled_mode_returns_empty_setup_without_docker_probe() {
        // When mode = Disabled the orchestrator must NOT probe Docker
        // and must succeed even on hosts where Docker is missing.
        let cfg = build_test_config(crate::config::JobRuntimeMode::Disabled);
        let llm: Arc<dyn LlmProvider> = Arc::new(crate::testing::StubLlm::new("noop"));
        let setup = match setup_orchestrator(&cfg, &llm, None, None).await {
            Ok(s) => s,
            Err(e) => panic!("Disabled mode must succeed, got error: {e}"),
        };
        assert!(setup.container_job_manager.is_none());
        assert!(setup.job_event_tx.is_none());
        assert!(matches!(
            setup.docker_status,
            crate::sandbox::DockerStatus::Disabled
        ));
    }

    #[tokio::test]
    async fn req_adr119_f2_cloud_mode_fails_closed_with_endpoint_in_error() {
        // Cloud is reserved for a future ADR; setup must refuse to boot.
        let cfg = build_test_config(crate::config::JobRuntimeMode::Cloud {
            endpoint: "https://jobs.example.com/v1".to_string(),
        });
        let llm: Arc<dyn LlmProvider> = Arc::new(crate::testing::StubLlm::new("noop"));
        match setup_orchestrator(&cfg, &llm, None, None).await {
            Ok(_) => panic!("Cloud mode must fail-closed"),
            Err(OrchestratorSetupError::CloudNotImplemented { endpoint }) => {
                assert_eq!(endpoint, "https://jobs.example.com/v1");
            }
            Err(other) => panic!("expected CloudNotImplemented, got {other}"),
        }
    }

    #[test]
    fn req_adr119_f2_docker_not_installed_error_renders_actionable_hint() {
        // The Display impl must mention BOTH the platform-specific install
        // hint and the "set mode = disabled" workaround so operators can
        // self-recover from the log alone.
        let err = OrchestratorSetupError::DockerNotInstalled {
            hint: "Install Docker Desktop from https://docker.com".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("LocalContainer"), "msg = {msg}");
        assert!(msg.contains("not installed"), "msg = {msg}");
        assert!(msg.contains("Install Docker Desktop"), "msg = {msg}");
        assert!(msg.contains("disabled"), "msg = {msg}");
    }

    #[test]
    fn req_adr119_f2_docker_not_running_error_renders_actionable_hint() {
        let err = OrchestratorSetupError::DockerNotRunning {
            hint: "Run `open -a Docker`".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("not running"), "msg = {msg}");
        assert!(msg.contains("open -a Docker"), "msg = {msg}");
        assert!(msg.contains("disabled"), "msg = {msg}");
    }
}
