//! Minimal dasclaw app-server host.
//!
//! Phase 1 deliberately starts with lifecycle, initialize, health, and
//! capability reporting. Runtime execution is injected through
//! [`RuntimeBridge`]: the default host is a no-op bridge, while
//! [`DasclawAgentRuntimeBridge`] delegates to `dasclaw_runtime::Agent`.
//! DLP, jobs, skills, MCP, and sandbox execution stay outside this crate.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dasclaw_app_server_protocol::ClientModelConfig;
use dasclaw_app_server_protocol::{
    AgentMessageDeltaEvent, CapabilitiesChangedEvent, CapabilitiesChangedReason,
    CapabilitiesListResponse, CapabilityMatrix, ClientInfo, CompatibilityProfile,
    DEFAULT_MAX_PENDING_NOTIFICATIONS, ErrorCode, ErrorData, ErrorEvent, HealthCheckParams,
    HealthCheckResponse, InitializeParams, InitializeResponse, ItemCompletedEvent,
    ItemStartedEvent, ItemType, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    LifecycleChangedEvent, LifecycleReason, LifecycleSnapshot, LifecycleState,
    LifecycleStatusResponse, ModelProviderInitializeConfig, ModelProviderSelectForNextTurnParams,
    ModelProviderSelectForNextTurnResponse, NotificationQueuePolicy, NotificationsInitializedEvent,
    ProtocolSchemaResponse, ProtocolVersion, ReasoningSummaryTextDeltaEvent, ServerInfo,
    ServerNotification, ServiceHealth, ServiceName, ShutdownParams, ShutdownReason,
    ShutdownResponse, ThreadCreateParams, ThreadCreateResponse, ThreadCreatedEvent,
    ThreadListResponse, ThreadReadParams, ThreadReadResponse, ThreadStartResponse,
    ThreadStartedEvent, ThreadSummary, TurnCancelParams, TurnCancelResponse, TurnCancelledEvent,
    TurnCompletedEvent, TurnDeltaEvent, TurnFailedEvent, TurnInterruptResponse, TurnListParams,
    TurnListResponse, TurnReadParams, TurnReadResponse, TurnStartParams, TurnStartResponse,
    TurnStartedEvent, TurnStatus, TurnSummary,
};
use dasclaw_app_server_protocol::{JSON_RPC_VERSION, method};
use dasclaw_llm_provider::provider::claw_code_provider::ClawCodeLlmProvider;
use dasclaw_llm_provider::provider::config::{CacheRetention, RegistryProviderConfig};
use dasclaw_llm_provider::provider::registry::ProviderProtocol;
use dasclaw_runtime::LlmProviderResponder;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

pub const SERVER_NAME: &str = "dasclaw_app_server";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const STDIO_NOTIFICATION_POLL_INTERVAL: Duration = Duration::from_millis(10);
pub const STDIO_EOF_DRAIN_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct AppServer {
    server: ServerInfo,
    lifecycle: LifecycleSnapshot,
    capabilities: CapabilityMatrix,
    client: Option<ClientInfo>,
    notifications: NotificationBus,
    threads: SessionThreadHost,
    runtime_bridge: Arc<dyn RuntimeBridge>,
    runtime_turn_updates: RuntimeTurnUpdateSink,
    model_provider: ModelProviderState,
    codex_v2_compat_enabled: bool,
}

impl Default for AppServer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Default)]
struct ModelProviderState {
    models: HashMap<String, ClientModelConfig>,
    selected_model_id: Option<String>,
}

impl std::fmt::Debug for ModelProviderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelProviderState")
            .field("model_count", &self.models.len())
            .field("selected_model_id", &self.selected_model_id)
            .finish()
    }
}

impl ModelProviderState {
    fn from_config(config: ModelProviderInitializeConfig) -> Result<Self, AppServerError> {
        if config.models.is_empty() {
            return Err(AppServerError::invalid_request(
                "model_provider",
                "modelProvider.models must contain at least one model",
            ));
        }

        validate_client_model(&config.selected_model, "selectedModel")?;
        let selected_model_id = config.selected_model.model_id.clone();
        let mut models = HashMap::new();
        for model in config.models {
            validate_client_model(&model, "models")?;
            if models.insert(model.model_id.clone(), model).is_some() {
                return Err(AppServerError::invalid_request(
                    "model_provider",
                    "modelProvider.models contains duplicate modelId",
                ));
            }
        }

        if !models.contains_key(&selected_model_id) {
            return Err(AppServerError::invalid_request(
                "model_provider",
                format!(
                    "selected model is not present in modelProvider.models: {selected_model_id}"
                ),
            ));
        }

        Ok(Self {
            models,
            selected_model_id: Some(selected_model_id),
        })
    }

    fn selected_snapshot(&self) -> Result<RuntimeModelProviderSnapshot, AppServerError> {
        let selected_model_id = self.selected_model_id.as_deref().ok_or_else(|| {
            AppServerError::invalid_request(
                "model_provider",
                "model provider config is required before starting a turn",
            )
        })?;

        let model = self.models.get(selected_model_id).ok_or_else(|| {
            AppServerError::invalid_request(
                "model_provider",
                "selected model is not available; refresh model provider config",
            )
        })?;

        RuntimeModelProviderSnapshot::from_client_model(model)
    }

    fn select_for_next_turn(&mut self, model_id: String) -> Result<String, AppServerError> {
        if self.selected_model_id.is_none() {
            return Err(AppServerError::invalid_request(
                "model_provider",
                "model provider config is required before selecting a model",
            ));
        }

        let model_id = model_id.trim();
        if model_id.is_empty() {
            return Err(AppServerError::invalid_request(
                "model_provider",
                "modelId is required",
            ));
        }

        if !self.models.contains_key(model_id) {
            return Err(AppServerError::invalid_request(
                "model_provider",
                format!("unknown modelId: {model_id}"),
            ));
        }

        let selected_model_id = model_id.to_string();
        self.selected_model_id = Some(selected_model_id.clone());
        Ok(selected_model_id)
    }

    fn health(&self) -> ServiceHealth {
        if self.selected_model_id.is_some() {
            ServiceHealth::ready(ServiceName::ModelProvider)
        } else {
            ServiceHealth::unavailable_fail_safe(
                ServiceName::ModelProvider,
                "model provider config is required before starting a turn",
            )
        }
    }
}

fn validate_client_model(model: &ClientModelConfig, field: &str) -> Result<(), AppServerError> {
    validate_required_model_field(&model.model_id, field, "modelId")?;
    validate_required_model_option(
        model.provider.as_deref(),
        field,
        "provider",
        &model.model_id,
    )?;
    validate_required_model_option(
        model.api_base_url.as_deref(),
        field,
        "apiBaseUrl",
        &model.model_id,
    )?;
    validate_required_model_option(model.api_key.as_deref(), field, "apiKey", &model.model_id)?;
    validate_required_model_option(
        model.api_format.as_deref(),
        field,
        "apiFormat",
        &model.model_id,
    )?;
    Ok(())
}

fn validate_required_model_field(
    value: &str,
    field: &str,
    name: &str,
) -> Result<(), AppServerError> {
    if value.trim().is_empty() {
        return Err(AppServerError::invalid_request(
            "model_provider",
            format!("modelProvider.{field}.{name} is required"),
        ));
    }

    Ok(())
}

fn validate_required_model_option(
    value: Option<&str>,
    field: &str,
    name: &str,
    model_id: &str,
) -> Result<(), AppServerError> {
    if value.map(str::trim).is_none_or(str::is_empty) {
        return Err(AppServerError::invalid_request(
            "model_provider",
            format!("modelProvider.{field}.{name} is required for modelId: {model_id}"),
        ));
    }

    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeModelProviderSnapshot {
    pub model_id: String,
    pub provider: String,
    pub api_base_url: String,
    pub api_key: String,
    pub api_format: String,
    pub model_call_mode: dasclaw_runtime::ModelCallMode,
}

impl std::fmt::Debug for RuntimeModelProviderSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeModelProviderSnapshot")
            .field("model_id", &self.model_id)
            .field("provider", &self.provider)
            .field("api_base_url", &self.api_base_url)
            .field("api_key", &"<redacted>")
            .field("api_format", &self.api_format)
            .field("model_call_mode", &self.model_call_mode)
            .finish()
    }
}

impl RuntimeModelProviderSnapshot {
    fn from_client_model(model: &ClientModelConfig) -> Result<Self, AppServerError> {
        Ok(Self {
            model_id: required_snapshot_field(&model.model_id, "modelId", &model.model_id)?,
            provider: required_snapshot_option(
                model.provider.as_deref(),
                "provider",
                &model.model_id,
            )?,
            api_base_url: required_snapshot_option(
                model.api_base_url.as_deref(),
                "apiBaseUrl",
                &model.model_id,
            )?,
            api_key: required_snapshot_option(model.api_key.as_deref(), "apiKey", &model.model_id)?,
            api_format: required_snapshot_option(
                model.api_format.as_deref(),
                "apiFormat",
                &model.model_id,
            )?,
            model_call_mode: parse_model_call_mode(
                model.model_call_mode.as_deref(),
                &model.model_id,
            )?,
        })
    }

    fn provider_protocol(&self) -> Result<ProviderProtocol, RuntimeBridgeError> {
        match self.api_format.trim().to_ascii_lowercase().as_str() {
            "anthropic" => Ok(ProviderProtocol::Anthropic),
            "openai" | "openai_compat" | "openai-compatible" | "openai_completions" => {
                Ok(ProviderProtocol::OpenAiCompletions)
            }
            "ollama" => Ok(ProviderProtocol::Ollama),
            other => Err(RuntimeBridgeError::fatal(format!(
                "unsupported model provider apiFormat for modelId {}: {other}",
                self.model_id
            ))),
        }
    }
}

fn parse_model_call_mode(
    value: Option<&str>,
    model_id: &str,
) -> Result<dasclaw_runtime::ModelCallMode, AppServerError> {
    match value
        .unwrap_or("stream")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "invoke" => Ok(dasclaw_runtime::ModelCallMode::Invoke),
        "stream" => Ok(dasclaw_runtime::ModelCallMode::Stream),
        other => Err(AppServerError::invalid_request(
            "model_provider",
            format!("selected model modelCallMode is invalid for modelId: {model_id}: {other}"),
        )),
    }
}

fn required_snapshot_field(
    value: &str,
    name: &str,
    model_id: &str,
) -> Result<String, AppServerError> {
    if value.trim().is_empty() {
        return Err(AppServerError::invalid_request(
            "model_provider",
            format!("selected model {name} is required for modelId: {model_id}"),
        ));
    }

    Ok(value.trim().to_string())
}

fn required_snapshot_option(
    value: Option<&str>,
    name: &str,
    model_id: &str,
) -> Result<String, AppServerError> {
    let value = value.map(str::trim).ok_or_else(|| {
        AppServerError::invalid_request(
            "model_provider",
            format!("selected model {name} is required for modelId: {model_id}"),
        )
    })?;

    required_snapshot_field(value, name, model_id)
}

impl AppServer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            server: ServerInfo {
                name: SERVER_NAME.to_string(),
                version: SERVER_VERSION.to_string(),
                protocol_version: ProtocolVersion::current(),
            },
            lifecycle: lifecycle_snapshot(
                LifecycleState::Starting,
                LifecycleReason::ProcessStarted,
                None,
                Vec::new(),
            ),
            capabilities: CapabilityMatrix::phase_one(),
            client: None,
            notifications: NotificationBus::new(),
            threads: SessionThreadHost::new(),
            runtime_bridge: Arc::new(NoopRuntimeBridge),
            runtime_turn_updates: RuntimeTurnUpdateSink::new(),
            model_provider: ModelProviderState::default(),
            codex_v2_compat_enabled: false,
        }
    }

    #[must_use]
    pub fn with_runtime_bridge(runtime_bridge: Arc<dyn RuntimeBridge>) -> Self {
        let mut server = Self::new();
        server.runtime_bridge = runtime_bridge;
        server
    }

    #[must_use]
    pub fn with_runtime_responder(responder: Arc<dyn dasclaw_runtime::AgentResponder>) -> Self {
        let bridge = DasclawAgentRuntimeBridge::from_responder(responder);
        Self::with_runtime_bridge(Arc::new(bridge))
    }

    pub fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> Result<InitializeResponse, AppServerError> {
        if !self
            .server
            .protocol_version
            .is_compatible_with(params.protocol_version)
        {
            let previous_state = self.lifecycle.state;
            self.lifecycle = lifecycle_snapshot(
                LifecycleState::Failed,
                LifecycleReason::VersionMismatch,
                Some("client protocol major version is incompatible".to_string()),
                Vec::new(),
            );
            self.emit_lifecycle_changed(previous_state);
            return Err(AppServerError::version_mismatch(self.lifecycle.clone()));
        }

        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }

        let codex_v2_compat_requested = params
            .requested_capabilities
            .iter()
            .any(|capability| capability == CompatibilityProfile::CODEX_APP_SERVER_V2_ID);
        let unavailable_requested_capabilities =
            self.unavailable_requested_capabilities(&params.requested_capabilities);
        let model_provider = params
            .model_provider
            .map(ModelProviderState::from_config)
            .transpose()?;

        if matches!(
            self.lifecycle.state,
            LifecycleState::Ready | LifecycleState::Running | LifecycleState::Degraded
        ) {
            if let Some(model_provider) = model_provider {
                self.model_provider = model_provider;
            }
            self.codex_v2_compat_enabled = codex_v2_compat_requested;
            self.client = Some(params.client);
            return Ok(InitializeResponse {
                server: self.server.clone(),
                lifecycle: self.lifecycle.clone(),
                capabilities: self.capabilities.clone(),
                compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
                unavailable_requested_capabilities,
            });
        }

        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(
            LifecycleState::Initializing,
            LifecycleReason::InitializeRequested,
            Some("initialize request accepted".to_string()),
            Vec::new(),
        );
        self.emit_lifecycle_changed(previous_state);

        self.codex_v2_compat_enabled = codex_v2_compat_requested;
        if let Some(model_provider) = model_provider {
            self.model_provider = model_provider;
        }
        self.client = Some(params.client);
        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(
            LifecycleState::Ready,
            LifecycleReason::RuntimeReady,
            Some("minimal app-server initialized".to_string()),
            Vec::new(),
        );
        self.emit_lifecycle_changed(previous_state);
        self.emit_capabilities_changed(CapabilitiesChangedReason::Initialize);
        self.emit_codex_notifications_initialized(unavailable_requested_capabilities.clone());

        Ok(InitializeResponse {
            server: self.server.clone(),
            lifecycle: self.lifecycle.clone(),
            capabilities: self.capabilities.clone(),
            compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
            unavailable_requested_capabilities,
        })
    }

    #[must_use]
    pub fn health_check(&mut self, params: HealthCheckParams) -> HealthCheckResponse {
        self.drain_runtime_turn_updates();
        let services = if params.include_details {
            self.service_health()
        } else {
            Vec::new()
        };
        let ok = matches!(
            self.lifecycle.state,
            LifecycleState::Ready | LifecycleState::Running | LifecycleState::Degraded
        );

        HealthCheckResponse {
            ok,
            lifecycle: self.lifecycle.clone(),
            services,
        }
    }

    #[must_use]
    pub fn capabilities(&mut self) -> CapabilitiesListResponse {
        self.drain_runtime_turn_updates();
        CapabilitiesListResponse {
            capabilities: self.capabilities.clone(),
            compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
        }
    }

    #[must_use]
    pub fn protocol_schema(&self) -> ProtocolSchemaResponse {
        ProtocolSchemaResponse::phase_one(self.capabilities.clone())
    }

    pub fn thread_create(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<ThreadCreateResponse, AppServerError> {
        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }
        self.require_initialized("session")?;

        let thread_id = self.threads.create(params);
        self.notifications.emit_thread_created(ThreadCreatedEvent {
            thread_id: thread_id.clone(),
        });
        self.emit_codex_thread_started(thread_id.clone());

        Ok(ThreadCreateResponse {
            thread_id,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn thread_start(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<ThreadStartResponse, AppServerError> {
        let created = self.thread_create(params)?;
        Ok(ThreadStartResponse {
            thread_id: created.thread_id,
            lifecycle: created.lifecycle,
        })
    }

    pub fn thread_list(&self) -> Result<ThreadListResponse, AppServerError> {
        self.require_initialized("session")?;
        Ok(ThreadListResponse {
            threads: self.threads.list(),
        })
    }

    pub fn thread_read(
        &self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse, AppServerError> {
        self.require_initialized("session")?;
        Ok(ThreadReadResponse {
            thread: self.thread_summary_or_error(&params.thread_id)?,
        })
    }

    pub fn turn_start(
        &mut self,
        params: TurnStartParams,
    ) -> Result<TurnStartResponse, AppServerError> {
        self.require_initialized("session")?;
        self.require_thread_exists(&params.thread_id)?;
        let model_provider = self.model_provider.selected_snapshot()?;
        let turn_id = self.threads.next_turn_id();
        self.runtime_bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: params.thread_id.clone(),
                turn_id: turn_id.clone(),
                prompt: params.prompt,
                model_provider,
                updates: self.runtime_turn_updates.clone(),
            })
            .map_err(AppServerError::runtime_bridge)?;
        self.threads
            .record_started_turn(&params.thread_id, turn_id.clone());
        self.transition_lifecycle(
            LifecycleState::Running,
            LifecycleReason::RequestInProgress,
            Some("turn request is running".to_string()),
        );
        let thread_id = params.thread_id;
        self.notifications.emit_turn_started(TurnStartedEvent {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            status: TurnStatus::Pending,
        });
        self.emit_codex_item_started(thread_id, turn_id.clone());

        Ok(TurnStartResponse {
            turn_id,
            status: TurnStatus::Pending,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn model_provider_select_for_next_turn(
        &mut self,
        params: ModelProviderSelectForNextTurnParams,
    ) -> Result<ModelProviderSelectForNextTurnResponse, AppServerError> {
        self.require_initialized("model_provider")?;
        let selected_model_id = self.model_provider.select_for_next_turn(params.model_id)?;

        Ok(ModelProviderSelectForNextTurnResponse { selected_model_id })
    }

    pub fn turn_cancel(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<TurnCancelResponse, AppServerError> {
        self.require_initialized("session")?;
        self.drain_runtime_turn_updates();
        self.require_thread_exists(&params.thread_id)?;
        let current = self.turn_summary_or_error(&params.thread_id, &params.turn_id)?;
        if matches!(current.status, TurnStatus::Completed | TurnStatus::Failed) {
            return Err(AppServerError::invalid_request(
                "session",
                format!("turn is already terminal: {}", params.turn_id),
            ));
        }
        if current.status != TurnStatus::Cancelled {
            self.runtime_bridge
                .cancel_turn(RuntimeTurnCancelRequest {
                    thread_id: params.thread_id.clone(),
                    turn_id: params.turn_id.clone(),
                })
                .map_err(AppServerError::runtime_bridge)?;
        }
        let changed = self
            .threads
            .cancel_turn(&params.thread_id, &params.turn_id)
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "session",
                    format!("unknown turn id: {}", params.turn_id),
                )
            })?;
        if changed {
            self.emit_codex_item_completed(
                params.thread_id.clone(),
                params.turn_id.clone(),
                TurnStatus::Cancelled,
            );
            self.notifications.emit_turn_cancelled(TurnCancelledEvent {
                thread_id: params.thread_id,
                turn_id: params.turn_id,
                status: TurnStatus::Cancelled,
            });
            self.transition_ready_if_no_pending_turns();
        }

        Ok(TurnCancelResponse {
            accepted: true,
            status: TurnStatus::Cancelled,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn turn_interrupt(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<TurnInterruptResponse, AppServerError> {
        let _ = self.turn_cancel(params)?;
        Ok(TurnInterruptResponse {})
    }

    pub fn turn_list(
        &mut self,
        params: TurnListParams,
    ) -> Result<TurnListResponse, AppServerError> {
        self.require_initialized("session")?;
        self.drain_runtime_turn_updates();
        self.require_thread_exists(&params.thread_id)?;
        Ok(TurnListResponse {
            turns: self.threads.list_turns(&params.thread_id),
        })
    }

    pub fn turn_read(
        &mut self,
        params: TurnReadParams,
    ) -> Result<TurnReadResponse, AppServerError> {
        self.require_initialized("session")?;
        self.drain_runtime_turn_updates();
        self.require_thread_exists(&params.thread_id)?;
        Ok(TurnReadResponse {
            turn: self.turn_summary_or_error(&params.thread_id, &params.turn_id)?,
        })
    }

    #[must_use]
    pub fn lifecycle_status(&mut self) -> LifecycleStatusResponse {
        self.drain_runtime_turn_updates();
        LifecycleStatusResponse {
            lifecycle: self.lifecycle.clone(),
        }
    }

    #[must_use]
    pub fn server_info(&self) -> ServerInfo {
        self.server.clone()
    }

    #[must_use]
    pub fn shutdown(&mut self, params: ShutdownParams) -> ShutdownResponse {
        if self.lifecycle.state == LifecycleState::Stopped {
            return ShutdownResponse {
                accepted: true,
                lifecycle: self.lifecycle.clone(),
            };
        }

        self.drain_runtime_turn_updates();
        self.runtime_bridge.shutdown();
        for turn in self.threads.cancel_pending_turns() {
            self.emit_codex_item_completed(
                turn.thread_id.clone(),
                turn.turn_id.clone(),
                TurnStatus::Cancelled,
            );
            self.notifications.emit_turn_cancelled(TurnCancelledEvent {
                thread_id: turn.thread_id,
                turn_id: turn.turn_id,
                status: TurnStatus::Cancelled,
            });
        }

        let reason = params.reason.unwrap_or(ShutdownReason::ClientExit);
        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(
            LifecycleState::Stopped,
            LifecycleReason::ShutdownRequested,
            Some(format!("shutdown accepted: {reason:?}")),
            Vec::new(),
        );
        self.emit_lifecycle_changed(previous_state);

        ShutdownResponse {
            accepted: true,
            lifecycle: self.lifecycle.clone(),
        }
    }

    pub fn drain_notifications_with_policy(&mut self) -> NotificationDrain {
        self.drain_runtime_turn_updates();
        self.notifications.drain_with_policy()
    }

    pub fn drain_notifications(&mut self) -> Vec<ServerNotification> {
        self.drain_notifications_with_policy().notifications
    }

    pub fn drain_json_rpc_notifications_with_policy(&mut self) -> JsonRpcNotificationDrain {
        self.drain_runtime_turn_updates();
        self.notifications.drain_json_rpc_with_policy()
    }

    pub fn drain_json_rpc_notifications(&mut self) -> Vec<String> {
        self.drain_json_rpc_notifications_with_policy().lines
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.lifecycle.state == LifecycleState::Stopped
    }

    pub fn handle_json_rpc(&mut self, input: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(input) {
            Ok(Value::Array(_)) => Some(invalid_request_response(
                None,
                "JSON-RPC batch requests are not supported in Phase 1".to_string(),
            )),
            Ok(value) => {
                let id = value.get("id").cloned();
                let is_notification = id.is_none();
                match serde_json::from_value::<JsonRpcRequest>(value) {
                    Ok(request) => self.route_json_rpc(request),
                    Err(error) => response_for_request(
                        invalid_request_response(id, error.to_string()),
                        is_notification,
                    ),
                }
            }
            Err(error) => Some(parse_error_response(error.to_string())),
        };

        response.map(|response| serialize_response(&response))
    }

    fn route_json_rpc(&mut self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let is_notification = request.id.is_none();
        if request.jsonrpc != JSON_RPC_VERSION {
            return response_for_request(
                invalid_request_response(request.id, "jsonrpc must be \"2.0\"".to_string()),
                is_notification,
            );
        }

        let response = match request.method.as_str() {
            method::INITIALIZE => {
                route_with_params(request.id, request.params, |params| self.initialize(params))
            }
            method::PROTOCOL_SCHEMA => {
                route_with_no_params(request.id, request.params, || self.protocol_schema())
            }
            method::HEALTH_CHECK => {
                route_with_optional_params(request.id, request.params, |params| {
                    Ok(self.health_check(params))
                })
            }
            method::CAPABILITIES_LIST => {
                route_with_no_params(request.id, request.params, || self.capabilities())
            }
            method::LIFECYCLE_STATUS => {
                route_with_no_params(request.id, request.params, || self.lifecycle_status())
            }
            method::SHUTDOWN => route_with_optional_params(request.id, request.params, |params| {
                Ok(self.shutdown(params))
            }),
            method::THREAD_CREATE => {
                route_with_params(request.id, request.params, |params: ThreadCreateParams| {
                    self.thread_create(params)
                })
            }
            method::THREAD_START => {
                route_with_params(request.id, request.params, |params: ThreadCreateParams| {
                    self.thread_start(params)
                })
            }
            method::THREAD_LIST => {
                route_with_no_params_result(request.id, request.params, || self.thread_list())
            }
            method::THREAD_READ => {
                route_with_params(request.id, request.params, |params: ThreadReadParams| {
                    self.thread_read(params)
                })
            }
            method::TURN_START => {
                route_with_params(request.id, request.params, |params: TurnStartParams| {
                    self.turn_start(params)
                })
            }
            method::TURN_CANCEL => {
                route_with_params(request.id, request.params, |params: TurnCancelParams| {
                    self.turn_cancel(params)
                })
            }
            method::TURN_INTERRUPT => {
                route_with_params(request.id, request.params, |params: TurnCancelParams| {
                    self.turn_interrupt(params)
                })
            }
            method::TURN_LIST => {
                route_with_params(request.id, request.params, |params: TurnListParams| {
                    self.turn_list(params)
                })
            }
            method::TURN_READ => {
                route_with_params(request.id, request.params, |params: TurnReadParams| {
                    self.turn_read(params)
                })
            }
            method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN => route_with_params(
                request.id,
                request.params,
                |params: ModelProviderSelectForNextTurnParams| {
                    self.model_provider_select_for_next_turn(params)
                },
            ),
            _ => method_not_found_response(request.id, request.method),
        };

        response_for_request(response, is_notification)
    }

    fn service_health(&self) -> Vec<ServiceHealth> {
        vec![
            ServiceHealth::ready(ServiceName::Protocol),
            ServiceHealth::ready(ServiceName::Lifecycle),
            ServiceHealth::ready(ServiceName::Session),
            ServiceHealth::disabled(ServiceName::Logs, "log source is not wired in Phase 1"),
            ServiceHealth::degraded(
                ServiceName::Runtime,
                "runtime bridge boundary is available; default host requires a runtime adapter",
            ),
            ServiceHealth::unavailable_fail_safe(
                ServiceName::DlpPolicy,
                "DLP/policy service is declared but not migrated in Phase 1",
            ),
            self.model_provider.health(),
            ServiceHealth::disabled(ServiceName::Tools, "tool registry is not wired in Phase 1"),
            ServiceHealth::disabled(
                ServiceName::Sandbox,
                "sandbox adapter is not wired in Phase 1",
            ),
            ServiceHealth::disabled(ServiceName::Jobs, "job host is not wired in Phase 1"),
            ServiceHealth::disabled(
                ServiceName::Skills,
                "skills registry is not wired in Phase 1",
            ),
            ServiceHealth::disabled(ServiceName::Mcp, "MCP registry is not wired in Phase 1"),
        ]
    }

    fn emit_lifecycle_changed(&mut self, previous_state: LifecycleState) {
        let event = LifecycleChangedEvent {
            lifecycle: self.lifecycle.clone(),
            previous_state: Some(previous_state),
        };
        self.notifications.emit_lifecycle_changed(event);
    }

    fn emit_capabilities_changed(&mut self, reason: CapabilitiesChangedReason) {
        let event = CapabilitiesChangedEvent {
            capabilities: self.capabilities.clone(),
            reason,
        };
        self.notifications.emit_capabilities_changed(event);
    }

    fn emit_codex_notifications_initialized(
        &mut self,
        unavailable_requested_capabilities: Vec<String>,
    ) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications
            .emit_notifications_initialized(NotificationsInitializedEvent {
                lifecycle: self.lifecycle.clone(),
                compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
                unavailable_requested_capabilities,
                event_queue: NotificationQueuePolicy::bounded_lag_disconnect(),
            });
    }

    fn unavailable_requested_capabilities(&self, requested: &[String]) -> Vec<String> {
        let mut unavailable = Vec::new();
        for capability in requested {
            if capability == CompatibilityProfile::CODEX_APP_SERVER_V2_ID {
                continue;
            }
            if !self.is_capability_implemented(capability) && !unavailable.contains(capability) {
                unavailable.push(capability.clone());
            }
        }
        unavailable
    }

    fn is_capability_implemented(&self, capability: &str) -> bool {
        let status = match capability {
            "protocol" => self.capabilities.protocol.status,
            "lifecycle" => self.capabilities.lifecycle.status,
            "health" => self.capabilities.health.status,
            "session" => self.capabilities.session.status,
            "approval" => self.capabilities.approval.status,
            "dlp_policy" => self.capabilities.dlp_policy.status,
            "model_provider" => self.capabilities.model_provider.status,
            "tools" => self.capabilities.tools.status,
            "jobs" => self.capabilities.jobs.status,
            "skills" => self.capabilities.skills.status,
            "mcp" => self.capabilities.mcp.status,
            "sandbox" => self.capabilities.sandbox.status,
            "logs" => self.capabilities.logs.status,
            _ => return false,
        };
        status == dasclaw_app_server_protocol::CapabilityStatus::Implemented
    }

    fn require_initialized(&self, capability: impl Into<String>) -> Result<(), AppServerError> {
        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }

        if matches!(
            self.lifecycle.state,
            LifecycleState::Ready | LifecycleState::Running | LifecycleState::Degraded
        ) {
            Ok(())
        } else {
            Err(AppServerError::not_initialized(
                capability,
                self.lifecycle.clone(),
            ))
        }
    }

    fn require_thread_exists(&self, thread_id: &str) -> Result<(), AppServerError> {
        if self.threads.contains(thread_id) {
            Ok(())
        } else {
            Err(AppServerError::invalid_request(
                "session",
                format!("unknown thread id: {thread_id}"),
            ))
        }
    }

    fn thread_summary_or_error(&self, thread_id: &str) -> Result<ThreadSummary, AppServerError> {
        self.threads.summary(thread_id).ok_or_else(|| {
            AppServerError::invalid_request("session", format!("unknown thread id: {thread_id}"))
        })
    }

    fn turn_summary_or_error(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<TurnSummary, AppServerError> {
        self.threads
            .turn_summary(thread_id, turn_id)
            .ok_or_else(|| {
                AppServerError::invalid_request("session", format!("unknown turn id: {turn_id}"))
            })
    }

    fn emit_codex_thread_started(&mut self, thread_id: String) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications
            .emit_thread_started(ThreadStartedEvent { thread_id });
    }

    fn emit_codex_item_started(&mut self, thread_id: String, turn_id: String) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications.emit_item_started(ItemStartedEvent {
            thread_id,
            item_id: turn_id.clone(),
            turn_id,
            item_type: ItemType::AgentMessage,
        });
    }

    fn emit_codex_agent_message_delta(
        &mut self,
        thread_id: String,
        turn_id: String,
        delta: String,
    ) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications
            .emit_agent_message_delta(AgentMessageDeltaEvent {
                thread_id,
                item_id: turn_id.clone(),
                turn_id,
                delta,
            });
    }

    fn emit_codex_reasoning_summary_text_delta(
        &mut self,
        thread_id: String,
        turn_id: String,
        summary_index: i64,
        delta: String,
    ) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications
            .emit_reasoning_summary_text_delta(ReasoningSummaryTextDeltaEvent {
                thread_id,
                item_id: format!("{turn_id}:reasoning"),
                turn_id,
                summary_index,
                delta,
            });
    }

    fn emit_codex_item_completed(
        &mut self,
        thread_id: String,
        turn_id: String,
        status: TurnStatus,
    ) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications.emit_item_completed(ItemCompletedEvent {
            thread_id,
            item_id: turn_id.clone(),
            turn_id,
            status,
        });
    }

    fn emit_codex_error(&mut self, thread_id: String, turn_id: String, message: String) {
        if !self.codex_v2_compat_enabled {
            return;
        }
        self.notifications.emit_error(ErrorEvent {
            code: ErrorCode::ServiceDegraded,
            message,
            thread_id: Some(thread_id),
            turn_id: Some(turn_id),
            retryable: true,
        });
    }

    fn transition_lifecycle(
        &mut self,
        state: LifecycleState,
        reason: LifecycleReason,
        message: Option<String>,
    ) {
        if self.lifecycle.state == state && self.lifecycle.reason == reason {
            return;
        }

        let previous_state = self.lifecycle.state;
        self.lifecycle = lifecycle_snapshot(state, reason, message, Vec::new());
        self.emit_lifecycle_changed(previous_state);
    }

    fn transition_ready_if_no_pending_turns(&mut self) {
        if self.lifecycle.state != LifecycleState::Running || self.threads.has_pending_turns() {
            return;
        }

        self.transition_lifecycle(
            LifecycleState::Ready,
            LifecycleReason::RuntimeReady,
            Some("all pending turns settled".to_string()),
        );
    }

    fn drain_runtime_turn_updates(&mut self) {
        let mut terminal_update_applied = false;
        for update in self.runtime_turn_updates.drain() {
            match update.outcome {
                RuntimeTurnOutcome::Delta { delta } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_turn_delta(TurnDeltaEvent {
                            thread_id: update.thread_id.clone(),
                            turn_id: update.turn_id.clone(),
                            delta: delta.clone(),
                        });
                        self.emit_codex_agent_message_delta(
                            update.thread_id,
                            update.turn_id,
                            delta,
                        );
                    }
                }
                RuntimeTurnOutcome::ReasoningSummaryDelta {
                    delta,
                    summary_index,
                } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.emit_codex_reasoning_summary_text_delta(
                            update.thread_id,
                            update.turn_id,
                            summary_index,
                            delta,
                        );
                    }
                }
                outcome => {
                    let update = RuntimeTurnUpdate { outcome, ..update };
                    if let Some(summary) = self.threads.apply_runtime_turn_update(update) {
                        terminal_update_applied = true;
                        match summary.status {
                            TurnStatus::Completed => {
                                self.emit_codex_item_completed(
                                    summary.thread_id.clone(),
                                    summary.turn_id.clone(),
                                    TurnStatus::Completed,
                                );
                                self.notifications.emit_turn_completed(TurnCompletedEvent {
                                    thread_id: summary.thread_id,
                                    turn_id: summary.turn_id,
                                    status: TurnStatus::Completed,
                                    output: summary.output.unwrap_or_default(),
                                });
                            }
                            TurnStatus::Failed => {
                                let error = summary.error.unwrap_or_default();
                                self.emit_codex_item_completed(
                                    summary.thread_id.clone(),
                                    summary.turn_id.clone(),
                                    TurnStatus::Failed,
                                );
                                self.notifications.emit_turn_failed(TurnFailedEvent {
                                    thread_id: summary.thread_id.clone(),
                                    turn_id: summary.turn_id.clone(),
                                    status: TurnStatus::Failed,
                                    error: error.clone(),
                                });
                                self.emit_codex_error(summary.thread_id, summary.turn_id, error);
                            }
                            TurnStatus::Pending | TurnStatus::Cancelled => {}
                        }
                    }
                }
            }
        }
        if terminal_update_applied {
            self.transition_ready_if_no_pending_turns();
        }
    }
}

pub fn run_stdio_server<R, W>(reader: R, writer: W) -> io::Result<()>
where
    R: BufRead + Send + 'static,
    W: Write,
{
    run_stdio_server_with_app_server(AppServer::new(), reader, writer)
}

pub fn run_stdio_server_with_app_server<R, W>(
    mut server: AppServer,
    reader: R,
    mut writer: W,
) -> io::Result<()>
where
    R: BufRead + Send + 'static,
    W: Write,
{
    let (line_sender, line_receiver) = mpsc::channel();
    let _reader_thread = std::thread::spawn(move || {
        for line in reader.lines() {
            if line_sender.send(line).is_err() {
                break;
            }
        }
    });

    let mut input_closed_at = None;
    loop {
        match line_receiver.recv_timeout(STDIO_NOTIFICATION_POLL_INTERVAL) {
            Ok(Ok(line)) => {
                input_closed_at = None;
                if !line.trim().is_empty() {
                    let response = server.handle_json_rpc(&line);
                    let notification_write = write_pending_notifications(&mut server, &mut writer)?;
                    if notification_write.should_disconnect {
                        writer.flush()?;
                        break;
                    }
                    if let Some(response) = response {
                        writeln!(writer, "{response}")?;
                    }
                    writer.flush()?;
                    if server.is_stopped() {
                        break;
                    }
                }
            }
            Ok(Err(error)) => return Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                input_closed_at.get_or_insert_with(Instant::now);
            }
        }

        let notification_write = write_pending_notifications(&mut server, &mut writer)?;
        if notification_write.wrote {
            writer.flush()?;
            if input_closed_at.is_some() {
                input_closed_at = Some(Instant::now());
            }
        }
        if notification_write.should_disconnect {
            break;
        }
        if server.is_stopped() {
            break;
        }
        if input_closed_at.is_some_and(|closed_at| closed_at.elapsed() >= STDIO_EOF_DRAIN_TIMEOUT) {
            break;
        }
    }

    let _ = write_pending_notifications(&mut server, &mut writer)?;
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NotificationWrite {
    wrote: bool,
    should_disconnect: bool,
}

fn write_pending_notifications<W>(
    server: &mut AppServer,
    writer: &mut W,
) -> io::Result<NotificationWrite>
where
    W: Write,
{
    let mut wrote = false;
    let drain = server.drain_json_rpc_notifications_with_policy();
    for notification in drain.lines {
        writeln!(writer, "{notification}")?;
        wrote = true;
    }
    Ok(NotificationWrite {
        wrote,
        should_disconnect: drain.should_disconnect,
    })
}

pub trait RuntimeBridge: std::fmt::Debug + Send + Sync {
    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError>;
    fn cancel_turn(&self, request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError>;
    fn shutdown(&self);
}

#[derive(Debug, Clone)]
pub struct RuntimeTurnStartRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub prompt: String,
    pub model_provider: RuntimeModelProviderSnapshot,
    pub updates: RuntimeTurnUpdateSink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnCancelRequest {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeTurnUpdateSink {
    updates: Arc<Mutex<Vec<RuntimeTurnUpdate>>>,
}

impl RuntimeTurnUpdateSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn complete(&self, thread_id: String, turn_id: String, output: String) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::Completed { output },
        });
    }

    pub fn fail(&self, thread_id: String, turn_id: String, error: String) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::Failed { error },
        });
    }

    pub fn delta(&self, thread_id: String, turn_id: String, delta: String) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::Delta { delta },
        });
    }

    pub fn reasoning_summary_delta(
        &self,
        thread_id: String,
        turn_id: String,
        summary_index: i64,
        delta: String,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ReasoningSummaryDelta {
                delta,
                summary_index,
            },
        });
    }

    fn drain(&self) -> Vec<RuntimeTurnUpdate> {
        self.updates
            .lock()
            .map(|mut updates| std::mem::take(&mut *updates))
            .unwrap_or_default()
    }

    fn push(&self, update: RuntimeTurnUpdate) {
        if let Ok(mut updates) = self.updates.lock() {
            updates.push(update);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnUpdate {
    pub thread_id: String,
    pub turn_id: String,
    pub outcome: RuntimeTurnOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTurnOutcome {
    Delta { delta: String },
    ReasoningSummaryDelta { delta: String, summary_index: i64 },
    Completed { output: String },
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct RuntimeBridgeError {
    message: String,
    retryable: bool,
}

impl RuntimeBridgeError {
    #[must_use]
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    #[must_use]
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct NoopRuntimeBridge;

impl RuntimeBridge for NoopRuntimeBridge {
    fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn cancel_turn(&self, _request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
        Ok(())
    }

    fn shutdown(&self) {}
}

#[derive(Clone)]
pub struct DasclawAgentRuntimeBridge {
    agent_factory: Arc<AgentFactory>,
    in_flight: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

type AgentFactory = dyn Fn(
        CancellationToken,
        RuntimeModelProviderSnapshot,
    ) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
    + Send
    + Sync
    + 'static;

impl std::fmt::Debug for DasclawAgentRuntimeBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DasclawAgentRuntimeBridge")
            .finish_non_exhaustive()
    }
}

impl DasclawAgentRuntimeBridge {
    #[must_use]
    pub fn new(
        agent_factory: impl Fn(CancellationToken) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self::new_with_model_provider(move |token, _snapshot| agent_factory(token))
    }

    #[must_use]
    pub fn new_with_model_provider(
        agent_factory: impl Fn(
            CancellationToken,
            RuntimeModelProviderSnapshot,
        ) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            agent_factory: Arc::new(agent_factory),
            in_flight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn from_model_provider_snapshot() -> Self {
        Self::new_with_model_provider(agent_from_model_provider_snapshot)
    }

    #[must_use]
    pub fn from_responder(responder: Arc<dyn dasclaw_runtime::AgentResponder>) -> Self {
        Self::new(move |token| {
            dasclaw_runtime::Agent::builder()
                .responder_arc(Arc::clone(&responder))
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        })
    }

    fn remove_in_flight_turn(&self, turn_id: &str) {
        if let Ok(mut in_flight) = self.in_flight.lock() {
            in_flight.remove(turn_id);
        }
    }
}

impl RuntimeBridge for DasclawAgentRuntimeBridge {
    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        let token = CancellationToken::new();
        let agent = (self.agent_factory)(token.clone(), request.model_provider.clone())?;
        let mut in_flight = self
            .in_flight
            .lock()
            .map_err(|_| RuntimeBridgeError::retryable("runtime turn registry lock poisoned"))?;
        in_flight.insert(request.turn_id.clone(), token);
        drop(in_flight);

        let bridge = self.clone();
        let thread_id = request.thread_id;
        let turn_id = request.turn_id;
        let runtime_cleanup_turn_id = turn_id.clone();
        let spawn_cleanup_turn_id = turn_id.clone();
        let prompt = request.prompt;
        let updates = request.updates;
        let event_updates = updates.clone();
        let event_thread_id = thread_id.clone();
        let event_turn_id = turn_id.clone();
        let model_call_mode = request.model_provider.model_call_mode;
        thread::Builder::new()
            .name(format!("dasclaw-app-server-{turn_id}"))
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();

                let result = match runtime {
                    Ok(runtime) => runtime.block_on(async move {
                        use futures_util::StreamExt as _;

                        let mut stream = std::sync::Arc::new(agent).stream(
                            &prompt,
                            dasclaw_runtime::AgentRunOptions { model_call_mode },
                        );
                        while let Some(event) = stream.next().await {
                            match event {
                                Ok(dasclaw_runtime::AgentEvent::TextChunk(delta)) => {
                                    if !delta.is_empty() {
                                        event_updates.delta(
                                            event_thread_id.clone(),
                                            event_turn_id.clone(),
                                            delta,
                                        );
                                    }
                                }
                                Ok(dasclaw_runtime::AgentEvent::ReasoningSummaryChunk(delta)) => {
                                    if !delta.is_empty() {
                                        event_updates.reasoning_summary_delta(
                                            event_thread_id.clone(),
                                            event_turn_id.clone(),
                                            0,
                                            delta,
                                        );
                                    }
                                }
                                Ok(dasclaw_runtime::AgentEvent::Completed(output)) => {
                                    return Ok(output.text);
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    return Err(error);
                                }
                            }
                        }
                        Err(dasclaw_runtime::AgentError::LoopFailure(
                            "agent stream ended before completion".to_string(),
                        ))
                    }),
                    Err(error) => Err(dasclaw_runtime::AgentError::LoopFailure(format!(
                        "failed to create tokio runtime: {error}"
                    ))),
                };

                bridge.remove_in_flight_turn(&runtime_cleanup_turn_id);
                match result {
                    Ok(output) => updates.complete(thread_id, turn_id, output),
                    Err(error) => updates.fail(thread_id, turn_id, error.to_string()),
                }
            })
            .map_err(|error| {
                self.remove_in_flight_turn(&spawn_cleanup_turn_id);
                RuntimeBridgeError::retryable(format!(
                    "failed to start dasclaw runtime turn: {error}"
                ))
            })?;

        Ok(())
    }

    fn cancel_turn(&self, request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
        let in_flight = self
            .in_flight
            .lock()
            .map_err(|_| RuntimeBridgeError::retryable("runtime turn registry lock poisoned"))?;
        let Some(token) = in_flight.get(&request.turn_id).cloned() else {
            return Err(RuntimeBridgeError::retryable(format!(
                "runtime turn is not in flight: {}",
                request.turn_id
            )));
        };
        token.cancel();
        Ok(())
    }

    fn shutdown(&self) {
        let tokens = self
            .in_flight
            .lock()
            .map(|mut in_flight| {
                in_flight
                    .drain()
                    .map(|(_turn_id, token)| token)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for token in tokens {
            token.cancel();
        }
    }
}

fn agent_from_model_provider_snapshot(
    token: CancellationToken,
    snapshot: RuntimeModelProviderSnapshot,
) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError> {
    let config = registry_config_from_snapshot(&snapshot)?;
    let provider = ClawCodeLlmProvider::from_registry_config(&config).map_err(|error| {
        RuntimeBridgeError::fatal(redact_snapshot_secret(&error.to_string(), &snapshot))
    })?;
    let responder = LlmProviderResponder::new(Arc::new(provider));

    dasclaw_runtime::Agent::builder()
        .responder(responder)
        .cancellation_token(token)
        .build()
        .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
}

fn registry_config_from_snapshot(
    snapshot: &RuntimeModelProviderSnapshot,
) -> Result<RegistryProviderConfig, RuntimeBridgeError> {
    Ok(RegistryProviderConfig {
        protocol: snapshot.provider_protocol()?,
        provider_id: snapshot.provider.clone(),
        api_key: Some(SecretString::new(snapshot.api_key.clone().into())),
        base_url: snapshot.api_base_url.clone(),
        model: snapshot.model_id.clone(),
        extra_headers: Vec::new(),
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::default(),
        unsupported_params: Vec::new(),
        strict_tools_schema: true,
    })
}

fn redact_snapshot_secret(message: &str, snapshot: &RuntimeModelProviderSnapshot) -> String {
    if snapshot.api_key.is_empty() {
        return message.to_string();
    }

    message.replace(&snapshot.api_key, "<redacted>")
}

#[derive(Debug, Clone, Default)]
pub struct SessionThreadHost {
    next_thread_id: u64,
    next_turn_id: u64,
    threads: Vec<ThreadRecord>,
    turns: Vec<TurnRecord>,
}

impl SessionThreadHost {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_thread_id: 1,
            next_turn_id: 1,
            threads: Vec::new(),
            turns: Vec::new(),
        }
    }

    fn create(&mut self, params: ThreadCreateParams) -> String {
        let thread_id = format!("thread_{}", self.next_thread_id);
        self.next_thread_id += 1;
        self.threads.push(ThreadRecord {
            thread_id: thread_id.clone(),
            title: params.title,
            workspace_root: params.workspace_root,
        });
        thread_id
    }

    fn list(&self) -> Vec<ThreadSummary> {
        self.threads.iter().map(ThreadRecord::to_summary).collect()
    }

    fn summary(&self, thread_id: &str) -> Option<ThreadSummary> {
        self.threads
            .iter()
            .find(|thread| thread.thread_id == thread_id)
            .map(ThreadRecord::to_summary)
    }

    fn next_turn_id(&self) -> String {
        format!("turn_{}", self.next_turn_id)
    }

    fn record_started_turn(&mut self, thread_id: &str, turn_id: String) {
        self.next_turn_id += 1;
        self.turns.push(TurnRecord {
            thread_id: thread_id.to_string(),
            turn_id,
            status: TurnStatus::Pending,
            output: None,
            error: None,
        });
    }

    fn apply_runtime_turn_update(&mut self, update: RuntimeTurnUpdate) -> Option<TurnSummary> {
        let turn = self
            .turns
            .iter_mut()
            .find(|turn| turn.thread_id == update.thread_id && turn.turn_id == update.turn_id)?;
        if turn.status == TurnStatus::Cancelled {
            return None;
        }

        match update.outcome {
            RuntimeTurnOutcome::Delta { .. } | RuntimeTurnOutcome::ReasoningSummaryDelta { .. } => {
                return None;
            }
            RuntimeTurnOutcome::Completed { output } => {
                turn.status = TurnStatus::Completed;
                turn.output = Some(output);
                turn.error = None;
            }
            RuntimeTurnOutcome::Failed { error } => {
                turn.status = TurnStatus::Failed;
                turn.output = None;
                turn.error = Some(error);
            }
        }

        Some(turn.to_summary())
    }

    fn turn_is_pending(&self, thread_id: &str, turn_id: &str) -> bool {
        self.turns.iter().any(|turn| {
            turn.thread_id == thread_id
                && turn.turn_id == turn_id
                && turn.status == TurnStatus::Pending
        })
    }

    fn has_pending_turns(&self) -> bool {
        self.turns
            .iter()
            .any(|turn| turn.status == TurnStatus::Pending)
    }

    fn cancel_turn(&mut self, thread_id: &str, turn_id: &str) -> Option<bool> {
        let turn = self
            .turns
            .iter_mut()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)?;
        if turn.status == TurnStatus::Cancelled {
            return Some(false);
        }

        turn.status = TurnStatus::Cancelled;
        Some(true)
    }

    fn cancel_pending_turns(&mut self) -> Vec<TurnSummary> {
        let mut cancelled = Vec::new();
        for turn in &mut self.turns {
            if turn.status == TurnStatus::Pending {
                turn.status = TurnStatus::Cancelled;
                turn.output = None;
                turn.error = None;
                cancelled.push(turn.to_summary());
            }
        }
        cancelled
    }

    fn list_turns(&self, thread_id: &str) -> Vec<TurnSummary> {
        self.turns
            .iter()
            .filter(|turn| turn.thread_id == thread_id)
            .map(TurnRecord::to_summary)
            .collect()
    }

    fn turn_summary(&self, thread_id: &str, turn_id: &str) -> Option<TurnSummary> {
        self.turns
            .iter()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
            .map(TurnRecord::to_summary)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.threads.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }

    #[must_use]
    pub fn contains(&self, thread_id: &str) -> bool {
        self.threads
            .iter()
            .any(|thread| thread.thread_id == thread_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRecord {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
}

impl ThreadRecord {
    fn to_summary(&self) -> ThreadSummary {
        ThreadSummary {
            thread_id: self.thread_id.clone(),
            title: self.title.clone(),
            workspace_root: self.workspace_root.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecord {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

impl TurnRecord {
    fn to_summary(&self) -> TurnSummary {
        TurnSummary {
            thread_id: self.thread_id.clone(),
            turn_id: self.turn_id.clone(),
            status: self.status,
            output: self.output.clone(),
            error: self.error.clone(),
        }
    }
}

fn response_for_request(
    response: JsonRpcResponse,
    is_notification: bool,
) -> Option<JsonRpcResponse> {
    if is_notification {
        None
    } else {
        Some(response)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NotificationBus {
    pending: Vec<ServerNotification>,
    lag_disconnect_signaled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotificationDrain {
    pub notifications: Vec<ServerNotification>,
    pub should_disconnect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcNotificationDrain {
    pub lines: Vec<String>,
    pub should_disconnect: bool,
}

impl NotificationBus {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit_notifications_initialized(&mut self, event: NotificationsInitializedEvent) {
        self.push(ServerNotification::notifications_initialized(event));
    }

    pub fn emit_lifecycle_changed(&mut self, event: LifecycleChangedEvent) {
        self.push(ServerNotification::lifecycle_changed(event));
    }

    pub fn emit_capabilities_changed(&mut self, event: CapabilitiesChangedEvent) {
        self.push(ServerNotification::capabilities_changed(event));
    }

    pub fn emit_thread_created(&mut self, event: ThreadCreatedEvent) {
        self.push(ServerNotification::thread_created(event));
    }

    pub fn emit_thread_started(&mut self, event: ThreadStartedEvent) {
        self.push(ServerNotification::thread_started(event));
    }

    pub fn emit_turn_started(&mut self, event: TurnStartedEvent) {
        self.push(ServerNotification::turn_started(event));
    }

    pub fn emit_turn_delta(&mut self, event: TurnDeltaEvent) {
        self.push(ServerNotification::turn_delta(event));
    }

    pub fn emit_turn_completed(&mut self, event: TurnCompletedEvent) {
        self.push(ServerNotification::turn_completed(event));
    }

    pub fn emit_turn_failed(&mut self, event: TurnFailedEvent) {
        self.push(ServerNotification::turn_failed(event));
    }

    pub fn emit_turn_cancelled(&mut self, event: TurnCancelledEvent) {
        self.push(ServerNotification::turn_cancelled(event));
    }

    pub fn emit_item_started(&mut self, event: ItemStartedEvent) {
        self.push(ServerNotification::item_started(event));
    }

    pub fn emit_agent_message_delta(&mut self, event: AgentMessageDeltaEvent) {
        self.push(ServerNotification::agent_message_delta(event));
    }

    pub fn emit_reasoning_summary_text_delta(&mut self, event: ReasoningSummaryTextDeltaEvent) {
        self.push(ServerNotification::reasoning_summary_text_delta(event));
    }

    pub fn emit_item_completed(&mut self, event: ItemCompletedEvent) {
        self.push(ServerNotification::item_completed(event));
    }

    pub fn emit_error(&mut self, event: ErrorEvent) {
        self.push(ServerNotification::error(event));
    }

    pub fn drain_with_policy(&mut self) -> NotificationDrain {
        let should_disconnect = self.lag_disconnect_signaled;
        self.lag_disconnect_signaled = false;
        NotificationDrain {
            notifications: std::mem::take(&mut self.pending),
            should_disconnect,
        }
    }

    pub fn drain(&mut self) -> Vec<ServerNotification> {
        self.drain_with_policy().notifications
    }

    pub fn drain_json_rpc_with_policy(&mut self) -> JsonRpcNotificationDrain {
        let drain = self.drain_with_policy();
        JsonRpcNotificationDrain {
            lines: drain
                .notifications
                .into_iter()
                .filter_map(|notification| serde_json::to_string(&notification).ok())
                .collect(),
            should_disconnect: drain.should_disconnect,
        }
    }

    pub fn drain_json_rpc(&mut self) -> Vec<String> {
        self.drain_json_rpc_with_policy().lines
    }

    fn push(&mut self, notification: Result<ServerNotification, serde_json::Error>) {
        if self.lag_disconnect_signaled {
            return;
        }
        if let Ok(notification) = notification {
            if self.pending.len() >= DEFAULT_MAX_PENDING_NOTIFICATIONS as usize {
                self.pending.clear();
                if let Ok(error) = ServerNotification::error(ErrorEvent {
                    code: ErrorCode::NotificationQueueOverflow,
                    message:
                        "client notification queue exceeded bounded capacity; reconnect required"
                            .to_string(),
                    thread_id: None,
                    turn_id: None,
                    retryable: true,
                }) {
                    self.pending.push(error);
                }
                self.lag_disconnect_signaled = true;
                return;
            }
            self.pending.push(notification);
        }
    }
}

fn json_rpc_ok(id: Option<Value>, result: impl Serialize) -> JsonRpcResponse {
    JsonRpcResponse::ok(id.clone(), result)
        .unwrap_or_else(|error| internal_error_response(id, error.to_string()))
}

fn parse_error_response(message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        None,
        -32700,
        "Parse error",
        message,
        ErrorCode::InvalidParams,
    )
}

fn invalid_request_response(id: Option<Value>, message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32600,
        "Invalid Request",
        message,
        ErrorCode::InvalidParams,
    )
}

fn method_not_found_response(id: Option<Value>, method: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32601,
        "Method not found",
        format!("unknown method: {method}"),
        ErrorCode::UnknownMethod,
    )
}

fn invalid_params_response(id: Option<Value>, message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32602,
        "Invalid params",
        message,
        ErrorCode::InvalidParams,
    )
}

fn app_error_response(id: Option<Value>, error: AppServerError) -> JsonRpcResponse {
    match error {
        AppServerError::Protocol { data } => {
            let data = *data;
            JsonRpcResponse::error(
                id,
                JsonRpcError::new(app_error_code(data.code), data.message.clone(), Some(data)),
            )
        }
    }
}

fn internal_error_response(id: Option<Value>, message: String) -> JsonRpcResponse {
    json_rpc_error_response(
        id,
        -32603,
        "Internal error",
        message,
        ErrorCode::InternalError,
    )
}

fn json_rpc_error_response(
    id: Option<Value>,
    json_rpc_code: i64,
    json_rpc_message: &str,
    message: String,
    code: ErrorCode,
) -> JsonRpcResponse {
    JsonRpcResponse::error(
        id,
        JsonRpcError::new(
            json_rpc_code,
            json_rpc_message,
            Some(ErrorData {
                code,
                message,
                lifecycle: None,
                capability: None,
                retryable: false,
            }),
        ),
    )
}

fn route_with_params<T, R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    T: for<'de> Deserialize<'de>,
    R: Serialize,
    F: FnOnce(T) -> Result<R, AppServerError>,
{
    let Some(params) = params else {
        return invalid_params_response(id, "missing params".to_string());
    };

    match serde_json::from_value::<T>(params) {
        Ok(params) => match handler(params) {
            Ok(result) => json_rpc_ok(id, result),
            Err(error) => app_error_response(id, error),
        },
        Err(error) => invalid_params_response(id, error.to_string()),
    }
}

fn route_with_optional_params<T, R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    T: for<'de> Deserialize<'de> + Default,
    R: Serialize,
    F: FnOnce(T) -> Result<R, AppServerError>,
{
    let params = params.unwrap_or(Value::Object(Default::default()));
    route_with_params(id, Some(params), handler)
}

fn route_with_no_params<R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    R: Serialize,
    F: FnOnce() -> R,
{
    if has_non_empty_params(params) {
        return invalid_params_response(id, "method does not accept params".to_string());
    }

    json_rpc_ok(id, handler())
}

fn route_with_no_params_result<R, F>(
    id: Option<Value>,
    params: Option<Value>,
    handler: F,
) -> JsonRpcResponse
where
    R: Serialize,
    F: FnOnce() -> Result<R, AppServerError>,
{
    if has_non_empty_params(params) {
        return invalid_params_response(id, "method does not accept params".to_string());
    }

    match handler() {
        Ok(result) => json_rpc_ok(id, result),
        Err(error) => app_error_response(id, error),
    }
}

fn has_non_empty_params(params: Option<Value>) -> bool {
    match params {
        None | Some(Value::Null) => false,
        Some(Value::Object(map)) => !map.is_empty(),
        Some(Value::Array(values)) => !values.is_empty(),
        Some(_) => true,
    }
}

fn app_error_code(code: ErrorCode) -> i64 {
    match code {
        ErrorCode::VersionMismatch => -32001,
        ErrorCode::CapabilityUnavailable => -32002,
        ErrorCode::NotInitialized => -32003,
        ErrorCode::OperationInProgress => -32004,
        ErrorCode::ServiceDegraded => -32005,
        ErrorCode::NotificationQueueOverflow => -32006,
        ErrorCode::UnknownMethod => -32601,
        ErrorCode::InvalidParams => -32602,
        ErrorCode::InternalError => -32603,
    }
}

fn serialize_response(response: &JsonRpcResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|error| {
        format!(
            r#"{{"jsonrpc":"2.0","error":{{"code":-32603,"message":"Internal error","data":{{"code":"INTERNAL_ERROR","message":"failed to serialize response: {error}","retryable":false}}}}}}"#
        )
    })
}
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppServerError {
    #[error("{data:?}")]
    Protocol { data: Box<ErrorData> },
}

impl AppServerError {
    fn protocol(data: ErrorData) -> Self {
        Self::Protocol {
            data: Box::new(data),
        }
    }

    fn version_mismatch(lifecycle: LifecycleSnapshot) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::VersionMismatch,
            message: "client protocol major version is incompatible".to_string(),
            lifecycle: Some(lifecycle),
            capability: Some("protocol".to_string()),
            retryable: false,
        })
    }

    fn server_stopped(lifecycle: LifecycleSnapshot) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::ServiceDegraded,
            message: "app-server is stopped and cannot be initialized again; restart the process"
                .to_string(),
            lifecycle: Some(lifecycle),
            capability: Some("lifecycle".to_string()),
            retryable: false,
        })
    }

    fn not_initialized(capability: impl Into<String>, lifecycle: LifecycleSnapshot) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::NotInitialized,
            message: "initialize must complete before this method can be called".to_string(),
            lifecycle: Some(lifecycle),
            capability: Some(capability.into()),
            retryable: true,
        })
    }

    fn invalid_request(capability: impl Into<String>, message: impl Into<String>) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::InvalidParams,
            message: message.into(),
            lifecycle: None,
            capability: Some(capability.into()),
            retryable: false,
        })
    }

    fn runtime_bridge(error: RuntimeBridgeError) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::ServiceDegraded,
            message: error.message,
            lifecycle: None,
            capability: Some("runtime".to_string()),
            retryable: error.retryable,
        })
    }
}

pub fn supported_methods() -> &'static [&'static str] {
    &[
        method::INITIALIZE,
        method::PROTOCOL_SCHEMA,
        method::HEALTH_CHECK,
        method::CAPABILITIES_LIST,
        method::LIFECYCLE_STATUS,
        method::SHUTDOWN,
        method::THREAD_CREATE,
        method::THREAD_START,
        method::THREAD_LIST,
        method::THREAD_READ,
        method::TURN_START,
        method::TURN_CANCEL,
        method::TURN_INTERRUPT,
        method::TURN_LIST,
        method::TURN_READ,
        method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN,
    ]
}

fn lifecycle_snapshot(
    state: LifecycleState,
    reason: LifecycleReason,
    message: Option<String>,
    degraded_services: Vec<ServiceHealth>,
) -> LifecycleSnapshot {
    LifecycleSnapshot {
        state,
        reason,
        message,
        since: unix_timestamp_string(),
        degraded_services,
    }
}

fn unix_timestamp_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::Cursor;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use dasclaw_app_server_protocol::{
        CapabilityStatus, ServiceStatus, TransportKind, WorkspaceInfo, WorkspaceTrust,
    };
    use dasclaw_core::messages::FinishReason;
    use dasclaw_core::reasoning_ctx::ReasoningContext;
    use dasclaw_core::response_types::{
        RespondOutput, RespondResult, ResponseMetadata, TokenUsage,
    };
    use dasclaw_core::traits::HostError;
    use secrecy::ExposeSecret;
    use tokio::sync::mpsc;

    use super::*;

    struct InvokeOnlyResponder;

    #[async_trait::async_trait]
    impl dasclaw_runtime::AgentResponder for InvokeOnlyResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            Ok(RespondOutput {
                result: RespondResult::Text("invoke bridge path".to_string()),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                metadata: ResponseMetadata::default(),
            })
        }

        async fn respond_streaming(
            &self,
            _ctx: &mut ReasoningContext,
            _event_tx: mpsc::Sender<dasclaw_runtime::AgentEvent>,
        ) -> Result<RespondOutput, HostError> {
            Err("streaming path should not be used in invoke mode".into())
        }
    }

    fn wait_for_runtime_updates(sink: &RuntimeTurnUpdateSink) -> Vec<RuntimeTurnUpdate> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let updates = sink.drain();
            if !updates.is_empty() {
                return updates;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Vec::new()
    }

    #[test]
    fn initialize_accepts_compatible_client_and_returns_minimal_capabilities() {
        let mut server = AppServer::new();
        let response = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: Some(WorkspaceInfo {
                    root: Some("/tmp/workspace".to_string()),
                    trust: WorkspaceTrust::Unknown,
                }),
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("compatible v0 client should initialize");

        assert_eq!(response.lifecycle.state, LifecycleState::Ready);
        assert_eq!(
            response.capabilities.protocol.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            response.capabilities.dlp_policy.status,
            CapabilityStatus::Declared
        );
    }

    #[test]
    fn initialize_rejects_incompatible_major_version() {
        let mut server = AppServer::new();
        let error = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "future-client".to_string(),
                    version: "1.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect_err("incompatible major version should fail");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::VersionMismatch);
                assert!(!data.retryable);
            }
        }
    }

    #[test]
    fn initialize_reports_requested_capabilities_that_are_not_implemented() {
        let mut server = AppServer::new();
        let response = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec![
                    "protocol".to_string(),
                    "dlp_policy".to_string(),
                    "unknown_future".to_string(),
                    "dlp_policy".to_string(),
                ],
                model_provider: None,
            })
            .expect("initialize should tolerate unavailable requested capabilities");

        assert_eq!(
            response.unavailable_requested_capabilities,
            vec!["dlp_policy".to_string(), "unknown_future".to_string()]
        );
    }

    #[test]
    fn initialize_reports_codex_v2_profile_as_chat_session_subset() {
        let mut server = AppServer::new();
        let response = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec![
                    CompatibilityProfile::CODEX_APP_SERVER_V2_ID.to_string(),
                ],
                model_provider: None,
            })
            .expect("initialize should accept the compatibility profile");
        let profile = response
            .compatibility_profiles
            .iter()
            .find(|profile| profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_ID)
            .expect("initialize response should advertise the Codex v2 subset profile");

        assert!(response.unavailable_requested_capabilities.is_empty());
        assert_eq!(
            profile.scope,
            dasclaw_app_server_protocol::CompatibilityProfileScope::ChatSessionSubset
        );
        assert_eq!(
            profile.methods,
            vec![
                method::INITIALIZE,
                method::THREAD_START,
                method::THREAD_READ,
                method::TURN_START,
                method::TURN_INTERRUPT,
            ]
        );
    }

    #[test]
    fn capabilities_list_reports_codex_v2_profile_as_chat_session_subset() {
        let mut server = AppServer::new();
        let response = server.capabilities();
        let profile = response
            .compatibility_profiles
            .iter()
            .find(|profile| profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_ID)
            .expect("capabilities/list should advertise the Codex v2 subset profile");

        assert_eq!(
            profile.scope,
            dasclaw_app_server_protocol::CompatibilityProfileScope::ChatSessionSubset
        );
    }

    #[test]
    fn initialize_is_idempotent_after_ready_without_reemitting_notifications() {
        let mut server = AppServer::new();
        let params = InitializeParams {
            client: ClientInfo {
                name: "open-cowork".to_string(),
                version: "0.0.0".to_string(),
                transport: TransportKind::Stdio,
            },
            protocol_version: ProtocolVersion::current(),
            workspace: None,
            requested_capabilities: Vec::new(),
            model_provider: None,
        };

        let first = server
            .initialize(params.clone())
            .expect("first initialize should succeed");
        assert_eq!(first.lifecycle.state, LifecycleState::Ready);
        assert_eq!(server.drain_notifications().len(), 3);

        let second = server
            .initialize(params)
            .expect("second initialize should be idempotent");
        assert_eq!(second.lifecycle.state, LifecycleState::Ready);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn initialize_after_shutdown_is_rejected_without_leaving_stopped() {
        let mut server = AppServer::new();
        let _ = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        let error = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect_err("stopped process should require a restart");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::ServiceDegraded);
                assert_eq!(
                    data.lifecycle.expect("lifecycle should be attached").state,
                    LifecycleState::Stopped
                );
                assert!(!data.retryable);
            }
        }
    }

    #[test]
    fn health_reports_runtime_boundary_without_claiming_future_services() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: Some(test_model_provider_config()),
            })
            .expect("server should initialize");

        let health = server.health_check(HealthCheckParams {
            include_details: true,
        });

        assert!(health.ok);
        assert!(
            health
                .services
                .iter()
                .any(|service| service.service == ServiceName::DlpPolicy && service.fail_safe)
        );
        assert!(
            health
                .services
                .iter()
                .any(|service| service.service == ServiceName::Runtime)
        );
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Session && service.status == ServiceStatus::Ready
        }));
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Runtime && service.status == ServiceStatus::Degraded
        }));
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::ModelProvider && service.status == ServiceStatus::Ready
        }));
    }

    #[test]
    fn health_reports_fail_safe_when_model_provider_is_missing() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("server should initialize without starting turns");

        let health = server.health_check(HealthCheckParams {
            include_details: true,
        });

        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::ModelProvider
                && service.status == ServiceStatus::Unavailable
                && service.fail_safe
        }));
    }

    #[test]
    fn initialize_rejects_invalid_model_provider_config() {
        let mut server = AppServer::new();
        let mut selected_model = test_model_config("gpt-test");
        selected_model.api_key = None;
        let error = server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: Some(ModelProviderInitializeConfig {
                    models: vec![selected_model.clone()],
                    selected_model,
                }),
            })
            .expect_err("selected model without apiKey must be rejected");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::InvalidParams);
                assert_eq!(data.capability.as_deref(), Some("model_provider"));
            }
        }
        assert_eq!(
            server.lifecycle_status().lifecycle.state,
            LifecycleState::Starting
        );
    }

    #[test]
    fn model_provider_select_for_next_turn_accepts_known_model_and_rejects_unknown() {
        let mut server = initialized_server();
        let response = server
            .model_provider_select_for_next_turn(ModelProviderSelectForNextTurnParams {
                model_id: "gpt-next".to_string(),
            })
            .expect("known model should be selected for the next turn");

        assert_eq!(response.selected_model_id, "gpt-next");

        let error = server
            .model_provider_select_for_next_turn(ModelProviderSelectForNextTurnParams {
                model_id: "missing-model".to_string(),
            })
            .expect_err("unknown model should be rejected");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::InvalidParams);
                assert_eq!(data.capability.as_deref(), Some("model_provider"));
            }
        }
    }

    #[test]
    fn model_provider_select_for_next_turn_fails_safe_without_config() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize may complete before a model provider is injected");

        let error = server
            .model_provider_select_for_next_turn(ModelProviderSelectForNextTurnParams {
                model_id: "gpt-next".to_string(),
            })
            .expect_err("selecting without config must fail safe");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::InvalidParams);
                assert_eq!(data.capability.as_deref(), Some("model_provider"));
            }
        }
    }

    #[test]
    fn health_can_omit_service_details_for_lightweight_probes() {
        let mut server = AppServer::new();
        let health = server.health_check(HealthCheckParams {
            include_details: false,
        });

        assert!(!health.ok);
        assert_eq!(health.lifecycle.state, LifecycleState::Starting);
        assert!(health.services.is_empty());
    }

    #[test]
    fn shutdown_moves_lifecycle_to_stopped() {
        let mut server = AppServer::new();
        let response = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        assert!(response.accepted);
        assert_eq!(response.lifecycle.state, LifecycleState::Stopped);
    }

    #[test]
    fn shutdown_is_idempotent_after_stopped_without_reemitting_notifications() {
        let mut server = AppServer::new();
        let first = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(first.lifecycle.state, LifecycleState::Stopped);
        assert_eq!(server.drain_notifications().len(), 1);

        let second = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(second.lifecycle.state, LifecycleState::Stopped);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn shutdown_cancels_pending_session_turns() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let shutdown = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(shutdown.lifecycle.state, LifecycleState::Stopped);

        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].method, "turn/cancelled");
        assert_eq!(notifications[0].params["turnId"], started.turn_id);
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "stopped");
    }

    #[test]
    fn server_info_exposes_protocol_version() {
        let server = AppServer::new();
        let info = server.server_info();

        assert_eq!(info.name, SERVER_NAME);
        assert_eq!(info.protocol_version, ProtocolVersion::current());
    }

    #[test]
    fn protocol_schema_exposes_current_methods_events_and_capabilities() {
        let server = AppServer::new();
        let schema = server.protocol_schema();
        let method_names = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>();

        assert_eq!(schema.protocol_version, ProtocolVersion::current());
        assert!(method_names.contains(&method::INITIALIZE));
        assert!(method_names.contains(&method::PROTOCOL_SCHEMA));
        assert_eq!(
            schema.capabilities.protocol.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(schema.capabilities.logs.status, CapabilityStatus::Declared);
    }

    #[test]
    fn protocol_schema_methods_are_all_routable() {
        let server = AppServer::new();
        let schema = server.protocol_schema();

        for method in schema.methods {
            assert!(
                supported_methods().contains(&method.method.as_str()),
                "protocol/schema advertised an unroutable method: {}",
                method.method
            );
        }
    }

    #[test]
    fn initialize_queues_lifecycle_and_capability_notifications() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: Some(test_model_provider_config()),
            })
            .expect("server should initialize");

        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 3);
        assert_eq!(notifications[0].method, "lifecycle/changed");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[2].method, "capabilities/changed");
    }

    #[test]
    fn codex_v2_initialize_queues_notifications_initialized_after_capabilities() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec![
                    CompatibilityProfile::CODEX_APP_SERVER_V2_ID.to_string(),
                    "mcp".to_string(),
                ],
                model_provider: None,
            })
            .expect("server should initialize");

        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 4);
        assert_eq!(notifications[0].method, "lifecycle/changed");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[2].method, "capabilities/changed");
        assert_eq!(notifications[3].method, "notifications/initialized");
        assert_eq!(
            notifications[3].params["compatibilityProfiles"][0]["id"],
            CompatibilityProfile::CODEX_APP_SERVER_V2_ID
        );
        assert_eq!(
            notifications[3].params["unavailableRequestedCapabilities"],
            serde_json::json!(["mcp"])
        );
        assert_eq!(
            notifications[3].params["eventQueue"]["maxPendingNotifications"],
            DEFAULT_MAX_PENDING_NOTIFICATIONS
        );
    }

    #[test]
    fn draining_notifications_is_one_shot() {
        let mut server = AppServer::new();
        let _response = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        assert_eq!(server.drain_notifications().len(), 1);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn notification_bus_drains_json_rpc_notifications() {
        let mut bus = NotificationBus::new();
        bus.emit_lifecycle_changed(LifecycleChangedEvent {
            lifecycle: lifecycle_snapshot(
                LifecycleState::Ready,
                LifecycleReason::RuntimeReady,
                None,
                Vec::new(),
            ),
            previous_state: Some(LifecycleState::Initializing),
        });

        let lines = bus.drain_json_rpc();
        assert_eq!(lines.len(), 1);

        let value: Value = serde_json::from_str(&lines[0]).expect("notification should be JSON");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["method"], "lifecycle/changed");
        assert!(bus.drain_json_rpc().is_empty());
    }

    #[test]
    fn notification_bus_signals_lag_disconnect_when_queue_is_full() {
        let mut bus = NotificationBus::new();
        for _ in 0..DEFAULT_MAX_PENDING_NOTIFICATIONS {
            bus.emit_thread_created(ThreadCreatedEvent {
                thread_id: "thread_1".to_string(),
            });
        }
        bus.emit_thread_created(ThreadCreatedEvent {
            thread_id: "thread_overflow".to_string(),
        });
        bus.emit_thread_created(ThreadCreatedEvent {
            thread_id: "ignored_after_overflow".to_string(),
        });

        let drain = bus.drain_with_policy();

        assert!(drain.should_disconnect);
        assert_eq!(drain.notifications.len(), 1);
        assert_eq!(drain.notifications[0].method, "error");
        assert_eq!(
            drain.notifications[0].params["code"],
            serde_json::json!("NOTIFICATION_QUEUE_OVERFLOW")
        );
        assert_eq!(drain.notifications[0].params["retryable"], true);
        assert!(!bus.drain_with_policy().should_disconnect);
    }

    #[test]
    fn json_rpc_notification_drain_preserves_lag_disconnect_policy() {
        let mut bus = NotificationBus::new();
        for _ in 0..=DEFAULT_MAX_PENDING_NOTIFICATIONS {
            bus.emit_thread_created(ThreadCreatedEvent {
                thread_id: "thread_1".to_string(),
            });
        }

        let drain = bus.drain_json_rpc_with_policy();
        let value: Value =
            serde_json::from_str(&drain.lines[0]).expect("overflow notification should be JSON");

        assert!(drain.should_disconnect);
        assert_eq!(drain.lines.len(), 1);
        assert_eq!(value["method"], "error");
        assert_eq!(value["params"]["code"], "NOTIFICATION_QUEUE_OVERFLOW");
    }

    #[test]
    fn implemented_capability_methods_are_routable() {
        let matrix = CapabilityMatrix::phase_one();
        let implemented_methods = [
            matrix.protocol.methods,
            matrix.lifecycle.methods,
            matrix.health.methods,
            matrix.session.methods,
            matrix.model_provider.methods,
            matrix.logs.methods,
        ]
        .concat();

        for method in implemented_methods {
            assert!(
                supported_methods().contains(&method.as_str()),
                "implemented method must be routable: {method}"
            );
        }
    }

    #[test]
    fn json_rpc_initialize_routes_to_server_method() {
        let mut server = AppServer::new();
        let request = format!(
            r#"{{
                "jsonrpc":"2.0",
                "id":"req_1",
                "method":"{}",
                "params":{{
                    "client":{{"name":"open-cowork","version":"0.0.0","transport":"stdio"}},
                    "protocolVersion":{{"major":0,"minor":1,"patch":0}},
                    "requestedCapabilities":[]
                }}
            }}"#,
            method::INITIALIZE
        );
        let response = server
            .handle_json_rpc(&request)
            .expect("request should return a response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], "req_1");
        assert_eq!(value["result"]["lifecycle"]["state"], "ready");
        assert_eq!(
            value["result"]["capabilities"]["protocol"]["status"],
            "implemented"
        );
    }

    #[test]
    fn json_rpc_health_reports_fail_safe_dlp_policy() {
        let mut server = AppServer::new();
        let request = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"{}","params":{{"includeDetails":true}}}}"#,
            method::HEALTH_CHECK
        );
        let response = server
            .handle_json_rpc(&request)
            .expect("request should return a response");
        let health: Value = serde_json::from_str(&response).expect("response should be JSON");
        let services = health["result"]["services"]
            .as_array()
            .expect("health services should be an array");

        assert!(services.iter().any(|service| {
            service["service"] == "dlp_policy"
                && service["status"] == serde_json::json!(ServiceStatus::Unavailable)
                && service["failSafe"] == true
        }));
    }

    #[test]
    fn json_rpc_protocol_schema_routes_without_initialize() {
        let mut server = AppServer::new();
        let request = format!(
            r#"{{"jsonrpc":"2.0","id":"schema","method":"{}"}}"#,
            method::PROTOCOL_SCHEMA
        );
        let response = server
            .handle_json_rpc(&request)
            .expect("request should return a response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "schema");
        assert_eq!(value["result"]["protocolVersion"]["major"], 0);
        assert_eq!(
            value["result"]["capabilities"]["protocol"]["status"],
            "implemented"
        );
        assert!(
            value["result"]["methods"]
                .as_array()
                .expect("methods should be an array")
                .iter()
                .any(|method| method["method"] == "protocol/schema")
        );
    }

    #[test]
    fn json_rpc_thread_create_requires_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
            )
            .expect("thread/create should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["error"]["code"], -32003);
        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["capability"], "session");
        assert_eq!(value["error"]["data"]["retryable"], true);
    }

    #[test]
    fn json_rpc_model_provider_select_for_next_turn_routes_to_server_method() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"model","method":"modelProvider/selectForNextTurn","params":{"modelId":"gpt-next"}}"#,
            )
            .expect("modelProvider/selectForNextTurn should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "model");
        assert_eq!(value["result"]["selectedModelId"], "gpt-next");
    }

    #[test]
    fn json_rpc_thread_create_returns_thread_id_after_initialize() {
        let mut server = AppServer::new();
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: Some(test_model_provider_config()),
            })
            .expect("initialize should succeed");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
            )
            .expect("thread/create should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["result"]["threadId"], "thread_1");
        assert_eq!(value["result"]["lifecycle"]["state"], "ready");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, "thread/created");
    }

    #[test]
    fn json_rpc_thread_list_requires_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["retryable"], true);
    }

    #[test]
    fn json_rpc_thread_list_and_read_return_in_memory_threads() {
        let mut server = initialized_server();
        let first = server
            .thread_create(ThreadCreateParams {
                title: Some("First".to_string()),
                workspace_root: Some("/tmp/workspace".to_string()),
            })
            .expect("first thread should be created");
        let _second = server
            .thread_create(ThreadCreateParams {
                title: Some("Second".to_string()),
                workspace_root: None,
            })
            .expect("second thread should be created");

        let list = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list should return a response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"thread","method":"thread/read","params":{{"threadId":"{}"}}}}"#,
                first.thread_id
            ))
            .expect("thread/read should return a response");
        let list_value: Value = serde_json::from_str(&list).expect("list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");

        assert_eq!(
            list_value["result"]["threads"]
                .as_array()
                .expect("threads should be an array")
                .len(),
            2
        );
        assert_eq!(read_value["result"]["thread"]["threadId"], "thread_1");
        assert_eq!(read_value["result"]["thread"]["title"], "First");
        assert_eq!(
            read_value["result"]["thread"]["workspaceRoot"],
            "/tmp/workspace"
        );
    }

    #[test]
    fn json_rpc_thread_read_rejects_unknown_thread() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/read","params":{"threadId":"missing"}}"#,
            )
            .expect("thread/read should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_requires_initialize() {
        let mut server = AppServer::new();
        let start = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{"threadId":"thread_1","prompt":"hello"}}"#,
            )
            .expect("turn/start should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(start_value["error"]["data"]["retryable"], true);
    }

    #[test]
    fn json_rpc_turn_start_rejects_unknown_thread_after_initialize() {
        let mut server = initialized_server();
        let start = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{"threadId":"missing","prompt":"hello"}}"#,
            )
            .expect("turn/start should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["error"]["code"], -32602);
        assert_eq!(start_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(start_value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_and_cancel_manage_pending_in_memory_turns() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{{"threadId":"{}","prompt":"hello"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["result"]["turnId"], "turn_1");
        assert_eq!(start_value["result"]["status"], "pending");
        assert_eq!(start_value["result"]["lifecycle"]["state"], "running");
        let started = server.drain_notifications();
        assert_eq!(started.len(), 2);
        assert_eq!(started[0].method, "lifecycle/changed");
        assert_eq!(started[0].params["lifecycle"]["state"], "running");
        assert_eq!(started[1].method, "turn/started");

        let cancel = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-cancel","method":"turn/cancel","params":{{"threadId":"{}","turnId":"turn_1"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/cancel should return a structured response");
        let cancel_value: Value = serde_json::from_str(&cancel).expect("cancel response JSON");

        assert_eq!(cancel_value["result"]["accepted"], true);
        assert_eq!(cancel_value["result"]["status"], "cancelled");
        assert_eq!(cancel_value["result"]["lifecycle"]["state"], "ready");
        let cancelled = server.drain_notifications();
        assert_eq!(cancelled.len(), 2);
        assert_eq!(cancelled[0].method, "turn/cancelled");
        assert_eq!(cancelled[1].method, "lifecycle/changed");
        assert_eq!(cancelled[1].params["lifecycle"]["state"], "ready");
    }

    #[test]
    fn turn_start_invokes_runtime_bridge_before_recording_pending_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");

        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello runtime".to_string(),
            })
            .expect("turn should start through runtime bridge");

        assert_eq!(start.turn_id, "turn_1");
        assert_eq!(start.status, TurnStatus::Pending);
        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].thread_id, thread.thread_id);
        assert_eq!(calls[0].turn_id, "turn_1");
        assert_eq!(calls[0].prompt, "hello runtime");
        assert_eq!(calls[0].model_provider.model_id, "gpt-test");
        assert_eq!(
            calls[0].model_provider.api_base_url,
            "http://localhost:11434/v1"
        );
        assert_eq!(calls[0].model_provider.api_key, "test-api-key");

        let turns = server
            .turn_list(TurnListParams {
                thread_id: calls[0].thread_id.clone(),
            })
            .expect("turn/list should still use session bookkeeping");
        assert_eq!(turns.turns.len(), 1);
        assert_eq!(turns.turns[0].status, TurnStatus::Pending);
    }

    #[test]
    fn turn_start_without_model_provider_fails_before_runtime_bridge() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = AppServer::with_runtime_bridge(bridge.clone());
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize may complete before turns are started");
        let _ = server.drain_notifications();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created before model selection is needed");

        let error = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello runtime".to_string(),
            })
            .expect_err("missing model provider must fail safe before runtime");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::InvalidParams);
                assert_eq!(data.capability.as_deref(), Some("model_provider"));
            }
        }
        assert!(bridge.calls.lock().expect("calls lock").is_empty());
        let turns = server
            .turn_list(TurnListParams {
                thread_id: thread.thread_id,
            })
            .expect("turn/list should still work after fail-safe rejection");
        assert!(turns.turns.is_empty());
    }

    #[test]
    fn model_provider_selection_applies_to_next_turn_snapshot_only() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");

        let first = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "first".to_string(),
            })
            .expect("first turn should use initial model");
        let response = server
            .model_provider_select_for_next_turn(ModelProviderSelectForNextTurnParams {
                model_id: "gpt-next".to_string(),
            })
            .expect("known model should be selected");
        let second = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                prompt: "second".to_string(),
            })
            .expect("second turn should use newly selected model");

        assert_eq!(response.selected_model_id, "gpt-next");
        assert_eq!(first.turn_id, "turn_1");
        assert_eq!(second.turn_id, "turn_2");
        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].model_provider.model_id, "gpt-test");
        assert_eq!(calls[1].model_provider.model_id, "gpt-next");
    }

    #[test]
    fn json_rpc_turn_start_runtime_bridge_error_does_not_record_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_result(Err(
            RuntimeBridgeError::retryable("runtime unavailable"),
        )));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{{"threadId":"{}","prompt":"hello"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a structured runtime error");
        let value: Value = serde_json::from_str(&response).expect("start response JSON");

        assert_eq!(value["error"]["code"], -32005);
        assert_eq!(value["error"]["data"]["code"], "SERVICE_DEGRADED");
        assert_eq!(value["error"]["data"]["capability"], "runtime");
        assert_eq!(value["error"]["data"]["retryable"], true);
        let turns = server
            .turn_list(TurnListParams {
                thread_id: thread.thread_id,
            })
            .expect("turn/list should still work after bridge error");
        assert!(turns.turns.is_empty());
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn json_rpc_turn_lifecycle_integration_streams_multi_delta_then_completed() {
        let bridge = Arc::new(DelayedSequencedRuntimeBridge::new([
            RuntimeTurnOutcome::Delta {
                delta: "Hel".to_string(),
            },
            RuntimeTurnOutcome::Delta {
                delta: "lo".to_string(),
            },
            RuntimeTurnOutcome::Completed {
                output: "Hello".to_string(),
            },
        ]));
        let mut server = AppServer::with_runtime_bridge(bridge);

        let initialize = json_rpc_value(
            server
                .handle_json_rpc(initialized_request_json())
                .expect("initialize should return a JSON-RPC response"),
        );
        assert_eq!(initialize["result"]["lifecycle"]["state"], "ready");
        let initialize_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&initialize_notifications),
            vec![
                "lifecycle/changed",
                "lifecycle/changed",
                "capabilities/changed"
            ]
        );

        let thread = json_rpc_value(
            server
                .handle_json_rpc(
                    r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
                )
                .expect("thread/create should return a JSON-RPC response"),
        );
        let thread_id = thread["result"]["threadId"]
            .as_str()
            .expect("threadId should be returned")
            .to_string();
        let thread_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&thread_notifications),
            vec!["thread/created"]
        );

        let turn = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{thread_id}","input":"hello"}}}}"#
                ))
                .expect("turn/start should return a JSON-RPC response"),
        );
        let turn_id = turn["result"]["turnId"]
            .as_str()
            .expect("turnId should be returned")
            .to_string();
        assert_eq!(turn["result"]["status"], "pending");

        let immediate_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&immediate_notifications),
            vec!["lifecycle/changed", "turn/started"],
            "turn/start must return pending before delayed runtime terminal notifications"
        );
        assert_eq!(
            immediate_notifications[0]["params"]["lifecycle"]["state"],
            "running"
        );

        let turn_notifications =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(1));
        assert_eq!(
            methods_from_values(&turn_notifications),
            vec![
                "turn/delta",
                "turn/delta",
                "turn/completed",
                "lifecycle/changed"
            ]
        );
        assert_eq!(turn_notifications[0]["params"]["delta"], "Hel");
        assert_eq!(turn_notifications[1]["params"]["delta"], "lo");
        assert_eq!(turn_notifications[2]["params"]["output"], "Hello");
        assert_eq!(
            turn_notifications[3]["params"]["lifecycle"]["state"],
            "ready"
        );

        let read = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"read","method":"turn/read","params":{{"threadId":"{thread_id}","turnId":"{turn_id}"}}}}"#
                ))
                .expect("turn/read should return a JSON-RPC response"),
        );
        assert_eq!(read["result"]["turn"]["status"], "completed");
        assert_eq!(read["result"]["turn"]["output"], "Hello");
        assert!(server.drain_json_rpc_notifications().is_empty());
    }

    #[test]
    fn json_rpc_turn_lifecycle_integration_reports_runtime_failed_turn() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([RuntimeTurnOutcome::Failed {
            error: "runtime failed".to_string(),
        }]));
        let mut server = AppServer::with_runtime_bridge(bridge);
        let _initialize = json_rpc_value(
            server
                .handle_json_rpc(initialized_request_json())
                .expect("initialize should return a JSON-RPC response"),
        );
        let _ = server.drain_json_rpc_notifications();
        let thread = json_rpc_value(
            server
                .handle_json_rpc(
                    r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
                )
                .expect("thread/create should return a JSON-RPC response"),
        );
        let thread_id = thread["result"]["threadId"]
            .as_str()
            .expect("threadId should be returned")
            .to_string();
        let _ = server.drain_json_rpc_notifications();

        let turn = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","prompt":"hello"}}}}"#,
                    thread_id
                ))
                .expect("turn/start should return a JSON-RPC response"),
        );
        let turn_id = turn["result"]["turnId"]
            .as_str()
            .expect("turnId should be returned")
            .to_string();

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&notifications),
            vec![
                "lifecycle/changed",
                "turn/started",
                "turn/failed",
                "lifecycle/changed"
            ]
        );
        assert_eq!(notifications[0]["params"]["lifecycle"]["state"], "running");
        assert_eq!(notifications[2]["params"]["error"], "runtime failed");
        assert_eq!(notifications[3]["params"]["lifecycle"]["state"], "ready");

        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"read","method":"turn/read","params":{{"threadId":"{thread_id}","turnId":"{turn_id}"}}}}"#
            ))
            .expect("turn/read should return a JSON-RPC response");
        let read = json_rpc_value(read);
        assert_eq!(read["result"]["turn"]["status"], "failed");
        assert_eq!(read["result"]["turn"]["error"], "runtime failed");
    }

    #[test]
    fn json_rpc_turn_lifecycle_integration_cancels_pending_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = AppServer::with_runtime_bridge(bridge.clone());
        let _initialize = json_rpc_value(
            server
                .handle_json_rpc(initialized_request_json())
                .expect("initialize should return a JSON-RPC response"),
        );
        let _ = server.drain_json_rpc_notifications();
        let thread = json_rpc_value(
            server
                .handle_json_rpc(
                    r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
                )
                .expect("thread/create should return a JSON-RPC response"),
        );
        let thread_id = thread["result"]["threadId"]
            .as_str()
            .expect("threadId should be returned")
            .to_string();
        let _ = server.drain_json_rpc_notifications();

        let turn = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","prompt":"hello"}}}}"#,
                    thread_id
                ))
                .expect("turn/start should return a JSON-RPC response"),
        );
        let turn_id = turn["result"]["turnId"]
            .as_str()
            .expect("turnId should be returned")
            .to_string();
        assert_eq!(
            methods_from_values(&json_rpc_values(server.drain_json_rpc_notifications())),
            vec!["lifecycle/changed", "turn/started"]
        );

        let cancel = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"cancel","method":"turn/cancel","params":{{"threadId":"{thread_id}","turnId":"{turn_id}"}}}}"#
                ))
                .expect("turn/cancel should return a JSON-RPC response"),
        );
        assert_eq!(cancel["result"]["accepted"], true);
        assert_eq!(cancel["result"]["status"], "cancelled");
        assert_eq!(
            methods_from_values(&json_rpc_values(server.drain_json_rpc_notifications())),
            vec!["turn/cancelled", "lifecycle/changed"]
        );

        let cancel_calls = bridge.cancel_calls.lock().expect("cancel calls lock");
        assert_eq!(cancel_calls.len(), 1);
        assert_eq!(cancel_calls[0].turn_id, turn_id);

        let read = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"read","method":"turn/read","params":{{"threadId":"{thread_id}","turnId":"{}"}}}}"#,
                    cancel_calls[0].turn_id
                ))
                .expect("turn/read should return a JSON-RPC response"),
        );
        assert_eq!(read["result"]["turn"]["status"], "cancelled");
    }

    #[test]
    fn turn_read_applies_runtime_completion_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 5);
        assert_eq!(notifications[0].method, "thread/created");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[2].method, "turn/started");
        assert_eq!(notifications[3].method, "turn/completed");
        assert_eq!(notifications[3].params["output"], "hello from runtime");
        assert_eq!(notifications[4].method, "lifecycle/changed");
        assert_eq!(notifications[4].params["lifecycle"]["state"], "ready");

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: start.turn_id,
            })
            .expect("turn/read should apply completion update");

        assert_eq!(read.turn.status, TurnStatus::Completed);
        assert_eq!(read.turn.output.as_deref(), Some("hello from runtime"));
        assert_eq!(read.turn.error, None);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn turn_list_applies_runtime_failure_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Failed {
                error: "runtime failed".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 5);
        assert_eq!(notifications[0].method, "thread/created");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[2].method, "turn/started");
        assert_eq!(notifications[3].method, "turn/failed");
        assert_eq!(notifications[3].params["error"], "runtime failed");
        assert_eq!(notifications[4].method, "lifecycle/changed");
        assert_eq!(notifications[4].params["lifecycle"]["state"], "ready");

        let list = server
            .turn_list(TurnListParams {
                thread_id: thread.thread_id,
            })
            .expect("turn/list should apply failure update");

        assert_eq!(list.turns.len(), 1);
        assert_eq!(list.turns[0].turn_id, start.turn_id);
        assert_eq!(list.turns[0].status, TurnStatus::Failed);
        assert_eq!(list.turns[0].output, None);
        assert_eq!(list.turns[0].error.as_deref(), Some("runtime failed"));
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn lifecycle_stays_running_until_all_pending_turns_settle() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let first = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "first".to_string(),
            })
            .expect("first turn should start");
        let second = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "second".to_string(),
            })
            .expect("second turn should start");
        let _ = server.drain_notifications();

        server.runtime_turn_updates.complete(
            thread.thread_id.clone(),
            first.turn_id,
            "first done".to_string(),
        );
        let _after_first = server
            .turn_list(TurnListParams {
                thread_id: thread.thread_id.clone(),
            })
            .expect("turn/list should apply first completion");
        let first_notifications = server.drain_notifications();

        assert_eq!(server.lifecycle.state, LifecycleState::Running);
        assert_eq!(
            first_notifications
                .iter()
                .map(|notification| notification.method.as_str())
                .collect::<Vec<_>>(),
            vec!["turn/completed"]
        );

        server.runtime_turn_updates.complete(
            thread.thread_id.clone(),
            second.turn_id,
            "second done".to_string(),
        );
        let _after_second = server
            .turn_list(TurnListParams {
                thread_id: thread.thread_id,
            })
            .expect("turn/list should apply second completion");
        let second_notifications = server.drain_notifications();

        assert_eq!(server.lifecycle.state, LifecycleState::Ready);
        assert_eq!(
            second_notifications
                .iter()
                .map(|notification| notification.method.as_str())
                .collect::<Vec<_>>(),
            vec!["turn/completed", "lifecycle/changed"]
        );
        assert_eq!(
            second_notifications[1].params["lifecycle"]["state"],
            "ready"
        );
    }

    #[test]
    fn lifecycle_status_applies_runtime_terminal_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server
            .runtime_turn_updates
            .complete(thread.thread_id, turn.turn_id, "done".to_string());
        let status = server.lifecycle_status();
        let notifications = server.drain_notifications();

        assert_eq!(status.lifecycle.state, LifecycleState::Ready);
        assert_eq!(
            notifications
                .iter()
                .map(|notification| notification.method.as_str())
                .collect::<Vec<_>>(),
            vec!["turn/completed", "lifecycle/changed"]
        );
    }

    #[test]
    fn health_check_applies_runtime_terminal_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server.runtime_turn_updates.fail(
            thread.thread_id,
            turn.turn_id,
            "runtime failed".to_string(),
        );
        let health = server.health_check(HealthCheckParams {
            include_details: false,
        });
        let notifications = server.drain_notifications();

        assert!(health.ok);
        assert_eq!(health.lifecycle.state, LifecycleState::Ready);
        assert_eq!(
            notifications
                .iter()
                .map(|notification| notification.method.as_str())
                .collect::<Vec<_>>(),
            vec!["turn/failed", "lifecycle/changed"]
        );
    }

    #[test]
    fn capabilities_list_applies_runtime_terminal_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server
            .runtime_turn_updates
            .complete(thread.thread_id, turn.turn_id, "done".to_string());
        let capabilities = server.capabilities();
        let notifications = server.drain_notifications();

        assert_eq!(
            capabilities.capabilities.session.status,
            dasclaw_app_server_protocol::CapabilityStatus::Implemented
        );
        assert_eq!(
            notifications
                .iter()
                .map(|notification| notification.method.as_str())
                .collect::<Vec<_>>(),
            vec!["turn/completed", "lifecycle/changed"]
        );
        assert_eq!(notifications[1].params["lifecycle"]["state"], "ready");
    }

    #[test]
    fn drain_notifications_emits_runtime_delta_without_finishing_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Delta {
                delta: "hel".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 4);
        assert_eq!(notifications[0].method, "thread/created");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[2].method, "turn/started");
        assert_eq!(notifications[3].method, "turn/delta");
        assert_eq!(notifications[3].params["delta"], "hel");

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: start.turn_id,
            })
            .expect("turn/read should apply delta update");

        assert_eq!(read.turn.status, TurnStatus::Pending);
        assert_eq!(read.turn.output, None);
        assert_eq!(read.turn.error, None);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn drain_json_rpc_notifications_emits_runtime_updates() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "json rpc output".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let _start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                prompt: "hello".to_string(),
            })
            .expect("turn should start");

        let lines = server.drain_json_rpc_notifications();
        assert_eq!(lines.len(), 5);
        let values = lines
            .iter()
            .map(|line| serde_json::from_str::<Value>(line).expect("notification JSON"))
            .collect::<Vec<_>>();
        assert_eq!(values[0]["method"], "thread/created");
        assert_eq!(values[1]["method"], "lifecycle/changed");
        assert_eq!(values[1]["params"]["lifecycle"]["state"], "running");
        assert_eq!(values[2]["method"], "turn/started");
        assert_eq!(values[3]["method"], "turn/completed");
        assert_eq!(values[3]["params"]["output"], "json rpc output");
        assert_eq!(values[4]["method"], "lifecycle/changed");
        assert_eq!(values[4]["params"]["lifecycle"]["state"], "ready");
    }

    #[test]
    fn turn_cancel_invokes_runtime_bridge_before_recording_cancelled_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let cancelled = server
            .turn_cancel(TurnCancelParams {
                thread_id: thread.thread_id.clone(),
                turn_id: start.turn_id.clone(),
            })
            .expect("turn/cancel should call runtime bridge first");

        assert_eq!(cancelled.status, TurnStatus::Cancelled);
        let cancel_calls = bridge.cancel_calls.lock().expect("cancel calls lock");
        assert_eq!(cancel_calls.len(), 1);
        assert_eq!(cancel_calls[0].thread_id, thread.thread_id);
        assert_eq!(cancel_calls[0].turn_id, start.turn_id);
    }

    #[test]
    fn turn_cancel_runtime_bridge_error_does_not_record_cancelled_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_cancel_result(Err(
            RuntimeBridgeError::retryable("cancel unavailable"),
        )));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let error = server
            .turn_cancel(TurnCancelParams {
                thread_id: thread.thread_id.clone(),
                turn_id: start.turn_id.clone(),
            })
            .expect_err("runtime cancel failure should surface");
        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::ServiceDegraded);
                assert_eq!(data.capability.as_deref(), Some("runtime"));
                assert!(data.retryable);
            }
        }

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: start.turn_id,
            })
            .expect("turn/read should still find the pending turn");
        assert_eq!(read.turn.status, TurnStatus::Pending);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn dasclaw_runtime_bridge_creates_a_distinct_cancel_token_per_turn_start() {
        let captured_tokens = Arc::new(Mutex::new(Vec::new()));
        let factory_tokens = Arc::clone(&captured_tokens);
        let bridge = DasclawAgentRuntimeBridge::new(move |token| {
            factory_tokens
                .lock()
                .expect("factory tokens lock")
                .push(token);
            Err(RuntimeBridgeError::fatal("test factory stops before run"))
        });

        for turn_id in ["turn_a", "turn_b"] {
            let error = bridge
                .start_turn(RuntimeTurnStartRequest {
                    thread_id: "thread_1".to_string(),
                    turn_id: turn_id.to_string(),
                    prompt: "hello".to_string(),
                    model_provider: test_runtime_model_snapshot(),
                    updates: RuntimeTurnUpdateSink::new(),
                })
                .expect_err("test factory should stop before spawning");
            assert_eq!(error.message, "test factory stops before run");
        }

        let tokens = captured_tokens.lock().expect("captured tokens lock");
        assert_eq!(tokens.len(), 2);
        tokens[0].cancel();
        assert!(tokens[0].is_cancelled());
        assert!(!tokens[1].is_cancelled());
    }

    #[test]
    fn dasclaw_runtime_bridge_passes_model_snapshot_to_agent_factory() {
        let captured_snapshots = Arc::new(Mutex::new(Vec::new()));
        let factory_snapshots = Arc::clone(&captured_snapshots);
        let bridge = DasclawAgentRuntimeBridge::new_with_model_provider(move |_token, snapshot| {
            factory_snapshots
                .lock()
                .expect("factory snapshots lock")
                .push(snapshot);
            Err(RuntimeBridgeError::fatal("test factory stops before run"))
        });

        let error = bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "hello".to_string(),
                model_provider: test_runtime_model_snapshot(),
                updates: RuntimeTurnUpdateSink::new(),
            })
            .expect_err("test factory should stop before spawning");

        assert_eq!(error.message, "test factory stops before run");
        let snapshots = captured_snapshots.lock().expect("captured snapshots lock");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].model_id, "gpt-test");
        assert_eq!(snapshots[0].api_key, "test-api-key");
    }

    #[test]
    fn runtime_model_provider_snapshot_debug_redacts_api_key() {
        let snapshot = test_runtime_model_snapshot();
        let debug = format!("{snapshot:?}");

        assert!(debug.contains("RuntimeModelProviderSnapshot"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("test-api-key"));
    }

    #[test]
    fn registry_config_from_snapshot_uses_only_snapshot_values() {
        let snapshot = RuntimeModelProviderSnapshot {
            model_id: "snapshot-model".to_string(),
            provider: "snapshot-provider".to_string(),
            api_base_url: "http://snapshot-host/v1".to_string(),
            api_key: "snapshot-api-key".to_string(),
            api_format: "anthropic".to_string(),
            model_call_mode: dasclaw_runtime::ModelCallMode::Stream,
        };

        let config =
            registry_config_from_snapshot(&snapshot).expect("snapshot should build config");

        assert!(matches!(config.protocol, ProviderProtocol::Anthropic));
        assert_eq!(config.provider_id, "snapshot-provider");
        assert_eq!(config.base_url, "http://snapshot-host/v1");
        assert_eq!(config.model, "snapshot-model");
        let api_key = config.api_key.expect("api key should come from snapshot");
        assert_eq!(api_key.expose_secret(), "snapshot-api-key");
    }

    #[test]
    fn runtime_model_provider_snapshot_parses_model_call_mode() {
        let mut model = test_model_config("gpt-test");
        model.model_call_mode = Some("invoke".to_string());

        let snapshot = RuntimeModelProviderSnapshot::from_client_model(&model)
            .expect("modelCallMode=invoke should parse");

        assert_eq!(
            snapshot.model_call_mode,
            dasclaw_runtime::ModelCallMode::Invoke
        );
    }

    #[test]
    fn dasclaw_runtime_bridge_uses_snapshot_model_call_mode() {
        let bridge = DasclawAgentRuntimeBridge::from_responder(Arc::new(InvokeOnlyResponder));
        let updates = RuntimeTurnUpdateSink::new();
        let mut snapshot = test_runtime_model_snapshot();
        snapshot.model_call_mode = dasclaw_runtime::ModelCallMode::Invoke;

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "hello".to_string(),
                model_provider: snapshot,
                updates: updates.clone(),
            })
            .expect("turn should start");

        let updates = wait_for_runtime_updates(&updates);
        assert_eq!(updates.len(), 2, "expected delta + completion: {updates:?}");
        assert!(matches!(
            &updates[0].outcome,
            RuntimeTurnOutcome::Delta { delta } if delta == "invoke bridge path"
        ));
        assert!(matches!(
            &updates[1].outcome,
            RuntimeTurnOutcome::Completed { output } if output == "invoke bridge path"
        ));
    }

    #[test]
    fn snapshot_runtime_bridge_rejects_unknown_api_format_without_leaking_key() {
        let bridge = DasclawAgentRuntimeBridge::from_model_provider_snapshot();
        let mut snapshot = test_runtime_model_snapshot();
        snapshot.api_format = "future-format".to_string();
        snapshot.api_key = "super-secret-key".to_string();

        let error = bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "hello".to_string(),
                model_provider: snapshot,
                updates: RuntimeTurnUpdateSink::new(),
            })
            .expect_err("unknown api format should be rejected before spawning");

        assert!(
            error
                .message
                .contains("unsupported model provider apiFormat")
        );
        assert!(!error.message.contains("super-secret-key"));
    }

    #[test]
    fn dasclaw_runtime_bridge_cancel_targets_only_the_requested_turn_token() {
        let bridge =
            DasclawAgentRuntimeBridge::new(|_| Err(RuntimeBridgeError::fatal("unused factory")));
        let first_token = CancellationToken::new();
        let second_token = CancellationToken::new();
        {
            let mut in_flight = bridge.in_flight.lock().expect("in-flight lock");
            in_flight.insert("turn_a".to_string(), first_token.clone());
            in_flight.insert("turn_b".to_string(), second_token.clone());
        }

        bridge
            .cancel_turn(RuntimeTurnCancelRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_b".to_string(),
            })
            .expect("turn_b should be cancellable");

        assert!(!first_token.is_cancelled());
        assert!(second_token.is_cancelled());
    }

    #[test]
    fn dasclaw_runtime_bridge_streams_delta_and_completion_from_real_agent() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(|token| {
            dasclaw_runtime::Agent::builder()
                .responder(ChunkedRuntimeResponder::new(["Hel", "lo"]))
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        }));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        let methods = notifications
            .iter()
            .map(|notification| notification.method.as_str())
            .collect::<Vec<_>>();
        assert!(methods.contains(&"thread/created"));
        assert!(methods.contains(&"turn/started"));
        assert!(methods.contains(&"turn/delta"));
        assert!(methods.contains(&"turn/completed"));
        let deltas = notifications
            .iter()
            .filter(|notification| notification.method == "turn/delta")
            .map(|notification| notification.params["delta"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(deltas, vec!["Hel", "lo"]);

        let completed = notifications
            .iter()
            .find(|notification| notification.method == "turn/completed")
            .expect("completion notification should be emitted");
        assert_eq!(completed.params["output"], "Hello");
        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn_id,
            })
            .expect("turn/read should observe completed runtime result");
        assert_eq!(read.turn.status, TurnStatus::Completed);
        assert_eq!(read.turn.output.as_deref(), Some("Hello"));
    }

    #[test]
    fn dasclaw_runtime_bridge_routes_reasoning_outside_agent_message_delta() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(|token| {
            dasclaw_runtime::Agent::builder()
                .responder(ScriptedRuntimeResponder::new(
                    [
                        dasclaw_runtime::AgentEvent::ReasoningSummaryChunk(
                            "private scratch".to_string(),
                        ),
                        dasclaw_runtime::AgentEvent::TextChunk("final answer".to_string()),
                    ],
                    "final answer",
                ))
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        }));
        let mut server = initialized_codex_v2_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        let agent_deltas = notifications
            .iter()
            .filter(|notification| {
                notification.method == "turn/delta"
                    || notification.method == "item/agentMessage/delta"
            })
            .map(|notification| notification.params["delta"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(agent_deltas, vec!["final answer", "final answer"]);
        assert!(
            agent_deltas
                .iter()
                .all(|delta| !delta.contains("<think>") && !delta.contains("scratch"))
        );

        let reasoning_deltas = notifications
            .iter()
            .filter(|notification| notification.method == "item/reasoning/summaryTextDelta")
            .map(|notification| notification.params["delta"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(reasoning_deltas, vec!["private scratch"]);

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn_id,
            })
            .expect("turn/read should observe completed runtime result");
        assert_eq!(read.turn.output.as_deref(), Some("final answer"));
    }

    #[test]
    fn app_server_runtime_responder_constructor_wires_real_agent_bridge() {
        let responder = Arc::new(ChunkedRuntimeResponder::new(["res", "ponder"]));
        let mut server = AppServer::with_runtime_responder(responder);
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: Some(test_model_provider_config()),
            })
            .expect("initialize should succeed");
        let _ = server.drain_notifications();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start through runtime responder");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "turn/delta"),
            "runtime responder should stream deltas: {notifications:?}"
        );
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "turn/completed"),
            "runtime responder should complete the turn: {notifications:?}"
        );

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn_id,
            })
            .expect("turn/read should observe runtime responder result");
        assert_eq!(read.turn.status, TurnStatus::Completed);
        assert_eq!(read.turn.output.as_deref(), Some("responder"));
    }

    #[test]
    fn dasclaw_runtime_bridge_cancel_preserves_cancelled_turn_after_runtime_stops() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(|token| {
            dasclaw_runtime::Agent::builder()
                .responder(CancelAwareRuntimeResponder::new(token.clone()))
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        }));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let cancelled = server
            .turn_cancel(TurnCancelParams {
                thread_id: thread.thread_id.clone(),
                turn_id: started.turn_id.clone(),
            })
            .expect("turn/cancel should cancel runtime token");
        assert!(cancelled.accepted);
        assert_eq!(cancelled.status, TurnStatus::Cancelled);

        let notifications = drain_for(&mut server, Duration::from_millis(150));
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "turn/cancelled"),
            "cancel notification should be emitted: {notifications:?}"
        );
        assert!(
            notifications
                .iter()
                .all(|notification| notification.method != "turn/failed"),
            "runtime stopped update must not override cancellation: {notifications:?}"
        );

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn_id,
            })
            .expect("turn/read should keep cancelled status");
        assert_eq!(read.turn.status, TurnStatus::Cancelled);
        assert_eq!(read.turn.output, None);
        assert_eq!(read.turn.error, None);
    }

    #[test]
    fn shutdown_cancels_real_runtime_bridge_turn_without_failure_notification() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(|token| {
            dasclaw_runtime::Agent::builder()
                .responder(CancelAwareRuntimeResponder::new(token.clone()))
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        }));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let shutdown = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(shutdown.lifecycle.state, LifecycleState::Stopped);

        let notifications = drain_for(&mut server, Duration::from_millis(150));
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "turn/cancelled"),
            "shutdown should cancel pending runtime turn: {notifications:?}"
        );
        assert!(
            notifications
                .iter()
                .all(|notification| notification.method != "turn/failed"),
            "runtime stopped update must not emit failure after shutdown: {notifications:?}"
        );

        let read = server
            .threads
            .turn_summary(&thread.thread_id, &started.turn_id);
        assert_eq!(
            read.expect("turn should still be recorded").status,
            TurnStatus::Cancelled
        );
    }

    #[test]
    fn json_rpc_turn_list_and_read_return_in_memory_turn_status() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn should be started");
        let _ = server.turn_cancel(TurnCancelParams {
            thread_id: thread.thread_id.clone(),
            turn_id: start.turn_id.clone(),
        });

        let list = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turns","method":"turn/list","params":{{"threadId":"{}"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/list should return a response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/read","params":{{"threadId":"{}","turnId":"{}"}}}}"#,
                thread.thread_id, start.turn_id
            ))
            .expect("turn/read should return a response");
        let list_value: Value = serde_json::from_str(&list).expect("list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");

        assert_eq!(
            list_value["result"]["turns"]
                .as_array()
                .expect("turns should be an array")
                .len(),
            1
        );
        assert_eq!(read_value["result"]["turn"]["turnId"], "turn_1");
        assert_eq!(read_value["result"]["turn"]["status"], "cancelled");
    }

    #[test]
    fn json_rpc_turn_read_rejects_unknown_turn() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/read","params":{{"threadId":"{}","turnId":"missing"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/read should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("read response JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn session_methods_after_shutdown_return_stopped_lifecycle_error() {
        let mut server = initialized_server();
        let _ = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list after shutdown should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["code"], -32005);
        assert_eq!(value["error"]["data"]["code"], "SERVICE_DEGRADED");
        assert_eq!(value["error"]["data"]["lifecycle"]["state"], "stopped");
        assert_eq!(value["error"]["data"]["retryable"], false);
    }

    #[test]
    fn json_rpc_turn_cancel_rejects_unknown_turn_after_thread_exists() {
        let mut server = initialized_server();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-cancel","method":"turn/cancel","params":{{"threadId":"{}","turnId":"missing"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/cancel should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("cancel response JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_accepts_codex_v2_input_alias() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"hello input","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let value: Value = serde_json::from_str(&response).expect("turn response JSON");
        let calls = bridge.calls.lock().expect("calls lock");

        assert_eq!(value["result"]["turnId"], "turn_1");
        assert_eq!(calls[0].prompt, "hello input");
    }

    #[test]
    fn json_rpc_accepts_codex_v2_thread_start_and_turn_interrupt_aliases() {
        let mut server = initialized_server();
        let thread_response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"title":"Draft"}}"#,
            )
            .expect("thread/start should return a JSON-RPC response");
        let thread: Value = serde_json::from_str(&thread_response).expect("thread response JSON");
        let thread_id = thread["result"]["threadId"]
            .as_str()
            .expect("thread id should be present")
            .to_string();
        let _ = server.drain_notifications();

        let turn_response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{thread_id}","input":[{{"type":"text","text":"hello","text_elements":[]}}]}}}}"#
            ))
            .expect("turn/start should return a JSON-RPC response");
        let turn: Value = serde_json::from_str(&turn_response).expect("turn response JSON");
        let turn_id = turn["result"]["turnId"]
            .as_str()
            .expect("turn id should be present")
            .to_string();
        let _ = server.drain_notifications();

        let interrupt = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"interrupt","method":"turn/interrupt","params":{{"threadId":"{thread_id}","turnId":"{turn_id}"}}}}"#
            ))
            .expect("turn/interrupt should return a JSON-RPC response");
        let value: Value = serde_json::from_str(&interrupt).expect("interrupt response JSON");

        assert_eq!(value["result"], serde_json::json!({}));
    }

    #[test]
    fn codex_v2_requested_profile_emits_item_notifications_without_replacing_legacy_events() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([
            RuntimeTurnOutcome::Delta {
                delta: "hel".to_string(),
            },
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ]));
        let mut server = AppServer::with_runtime_bridge(bridge);
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec![
                    CompatibilityProfile::CODEX_APP_SERVER_V2_ID.to_string(),
                ],
                model_provider: Some(test_model_provider_config()),
            })
            .expect("initialize should succeed");
        let initialize_notifications = server.drain_notifications();
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let thread_notifications = server.drain_notifications();

        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        let methods = notifications
            .iter()
            .map(|notification| notification.method.as_str())
            .collect::<Vec<_>>();

        assert_eq!(thread_notifications.len(), 2);
        assert_eq!(thread_notifications[0].method, "thread/created");
        assert_eq!(thread_notifications[1].method, "thread/started");
        assert_eq!(started.turn_id, "turn_1");
        assert_eq!(
            methods,
            [
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "turn/delta",
                "item/agentMessage/delta",
                "item/completed",
                "turn/completed",
                "lifecycle/changed",
            ]
        );
        assert_eq!(notifications[0].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[7].params["lifecycle"]["state"], "ready");

        let profile = CompatibilityProfile::codex_app_server_v2();
        let emitted_methods = initialize_notifications
            .iter()
            .chain(thread_notifications.iter())
            .chain(notifications.iter())
            .map(|notification| notification.method.as_str())
            .collect::<Vec<_>>();
        for method in emitted_methods {
            assert!(
                profile.events.iter().any(|event| event == method),
                "codex_app_server_v2 profile did not declare emitted event: {method}"
            );
        }
    }

    #[test]
    fn codex_v2_stdio_notifications_are_declared_by_advertised_profile() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([
            RuntimeTurnOutcome::Delta {
                delta: "hel".to_string(),
            },
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ]));
        let server = AppServer::with_runtime_bridge(bridge);
        let input = [
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "initialize",
                "method": "initialize",
                "params": {
                    "client": {
                        "name": "codex",
                        "version": "2.0.0",
                        "transport": "stdio"
                    },
                    "protocolVersion": ProtocolVersion::current(),
                    "requestedCapabilities": [CompatibilityProfile::CODEX_APP_SERVER_V2_ID],
                    "modelProvider": test_model_provider_config()
                }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "thread",
                "method": "thread/create",
                "params": {"title": "Draft"}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "turn",
                "method": "turn/start",
                "params": {"threadId": "thread_1", "prompt": "hello"}
            }),
        ]
        .into_iter()
        .map(|request| serde_json::to_string(&request).expect("request should serialize"))
        .collect::<Vec<_>>()
        .join("\n")
            + "\n";
        let mut stdout = Vec::new();

        run_stdio_server_with_app_server(
            server,
            std::io::BufReader::new(Cursor::new(input)),
            &mut stdout,
        )
        .expect("real stdio loop should write v2 profile notifications");

        let lines = String::from_utf8(stdout).expect("stdio output should be UTF-8");
        let values = lines
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("stdio line should be JSON"))
            .collect::<Vec<_>>();
        let initialize_response = values
            .iter()
            .find(|value| value.get("id").and_then(Value::as_str) == Some("initialize"))
            .expect("initialize response should be present");
        let advertised_events = initialize_response["result"]["compatibilityProfiles"]
            .as_array()
            .expect("initialize response should advertise compatibility profiles")
            .iter()
            .find(|profile| {
                profile["id"].as_str() == Some(CompatibilityProfile::CODEX_APP_SERVER_V2_ID)
            })
            .expect("codex v2 compatibility profile should be advertised")["events"]
            .as_array()
            .expect("codex v2 compatibility profile should advertise events")
            .iter()
            .map(|event| event.as_str().expect("event should be a string"))
            .collect::<BTreeSet<_>>();
        let emitted_methods = values
            .iter()
            .filter(|value| value.get("id").is_none() && value.get("method").is_some())
            .map(|value| value["method"].as_str().expect("method should be a string"))
            .collect::<Vec<_>>();

        assert!(emitted_methods.contains(&"notifications/initialized"));
        assert!(emitted_methods.contains(&"capabilities/changed"));
        assert!(emitted_methods.contains(&"thread/created"));
        assert!(emitted_methods.contains(&"thread/started"));
        assert!(emitted_methods.contains(&"lifecycle/changed"));
        assert!(emitted_methods.contains(&"item/agentMessage/delta"));
        for method in emitted_methods {
            assert!(
                advertised_events.contains(method),
                "stdio emitted notification not declared by advertised codex_app_server_v2 profile: {method}"
            );
        }
    }

    #[test]
    fn legacy_profile_does_not_emit_codex_v2_item_notifications() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                prompt: "hello".to_string(),
            })
            .expect("turn should start");
        let methods = server
            .drain_notifications()
            .into_iter()
            .map(|notification| notification.method)
            .collect::<Vec<_>>();

        assert_eq!(started.turn_id, "turn_1");
        assert_eq!(
            methods,
            vec![
                "lifecycle/changed",
                "turn/started",
                "turn/completed",
                "lifecycle/changed"
            ]
        );
    }

    fn initialized_server() -> AppServer {
        initialized_server_with_bridge(Arc::new(NoopRuntimeBridge))
    }

    fn initialized_server_with_bridge(bridge: Arc<dyn RuntimeBridge>) -> AppServer {
        initialized_server_with_bridge_and_capabilities(bridge, Vec::new())
    }

    fn initialized_codex_v2_server_with_bridge(bridge: Arc<dyn RuntimeBridge>) -> AppServer {
        initialized_server_with_bridge_and_capabilities(
            bridge,
            vec![CompatibilityProfile::CODEX_APP_SERVER_V2_ID.to_string()],
        )
    }

    fn initialized_server_with_bridge_and_capabilities(
        bridge: Arc<dyn RuntimeBridge>,
        requested_capabilities: Vec<String>,
    ) -> AppServer {
        let mut server = AppServer::with_runtime_bridge(bridge);
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "open-cowork".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities,
                model_provider: Some(test_model_provider_config()),
            })
            .expect("initialize should succeed");
        let _ = server.drain_notifications();
        server
    }

    fn test_model_provider_config() -> ModelProviderInitializeConfig {
        let selected_model = test_model_config("gpt-test");
        ModelProviderInitializeConfig {
            models: vec![selected_model.clone(), test_model_config("gpt-next")],
            selected_model,
        }
    }

    fn test_model_config(model_id: &str) -> ClientModelConfig {
        ClientModelConfig {
            model_id: model_id.to_string(),
            display_name: Some(model_id.to_string()),
            provider: Some("openai".to_string()),
            api_base_url: Some("http://localhost:11434/v1".to_string()),
            api_key: Some("test-api-key".to_string()),
            api_format: Some("openai".to_string()),
            model_call_mode: None,
            source: Some("test".to_string()),
            capabilities: Vec::new(),
        }
    }

    fn test_runtime_model_snapshot() -> RuntimeModelProviderSnapshot {
        RuntimeModelProviderSnapshot {
            model_id: "gpt-test".to_string(),
            provider: "openai".to_string(),
            api_base_url: "http://localhost:11434/v1".to_string(),
            api_key: "test-api-key".to_string(),
            api_format: "openai".to_string(),
            model_call_mode: dasclaw_runtime::ModelCallMode::Stream,
        }
    }

    fn initialized_request_json() -> &'static str {
        r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"open-cowork","version":"0.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":[],"modelProvider":{"models":[{"modelId":"gpt-test","displayName":"gpt-test","provider":"openai","apiBaseUrl":"http://localhost:11434/v1","apiKey":"test-api-key","apiFormat":"openai","source":"test"},{"modelId":"gpt-next","displayName":"gpt-next","provider":"openai","apiBaseUrl":"http://localhost:11434/v1","apiKey":"test-api-key","apiFormat":"openai","source":"test"}],"selectedModel":{"modelId":"gpt-test","displayName":"gpt-test","provider":"openai","apiBaseUrl":"http://localhost:11434/v1","apiKey":"test-api-key","apiFormat":"openai","source":"test"}}}}"#
    }

    fn json_rpc_value(line: String) -> Value {
        serde_json::from_str(&line).expect("JSON-RPC line should decode")
    }

    fn json_rpc_values(lines: Vec<String>) -> Vec<Value> {
        lines.into_iter().map(json_rpc_value).collect()
    }

    fn methods_from_values(values: &[Value]) -> Vec<&str> {
        values
            .iter()
            .filter_map(|value| value.get("method").and_then(Value::as_str))
            .collect()
    }

    fn drain_json_rpc_until_method(
        server: &mut AppServer,
        method: &str,
        timeout: Duration,
    ) -> Vec<Value> {
        let deadline = Instant::now() + timeout;
        let mut notifications = Vec::new();
        loop {
            notifications.extend(json_rpc_values(server.drain_json_rpc_notifications()));
            if notifications
                .iter()
                .any(|notification| notification["method"] == method)
                || Instant::now() >= deadline
            {
                return notifications;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn drain_until_method(
        server: &mut AppServer,
        method: &str,
        timeout: Duration,
    ) -> Vec<ServerNotification> {
        let deadline = Instant::now() + timeout;
        let mut notifications = Vec::new();
        loop {
            notifications.extend(server.drain_notifications());
            if notifications
                .iter()
                .any(|notification| notification.method == method)
                || Instant::now() >= deadline
            {
                return notifications;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn drain_for(server: &mut AppServer, duration: Duration) -> Vec<ServerNotification> {
        let deadline = Instant::now() + duration;
        let mut notifications = Vec::new();
        while Instant::now() < deadline {
            notifications.extend(server.drain_notifications());
            std::thread::sleep(Duration::from_millis(10));
        }
        notifications.extend(server.drain_notifications());
        notifications
    }

    struct ChunkedRuntimeResponder {
        chunks: Vec<String>,
    }

    impl ChunkedRuntimeResponder {
        fn new(chunks: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                chunks: chunks.into_iter().map(String::from).collect(),
            }
        }
    }

    #[async_trait::async_trait]
    impl dasclaw_runtime::AgentResponder for ChunkedRuntimeResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            Ok(text_output(&self.chunks.concat()))
        }

        async fn respond_streaming(
            &self,
            _ctx: &mut ReasoningContext,
            event_tx: mpsc::Sender<dasclaw_runtime::AgentEvent>,
        ) -> Result<RespondOutput, HostError> {
            for chunk in &self.chunks {
                let _ = event_tx
                    .send(dasclaw_runtime::AgentEvent::TextChunk(chunk.clone()))
                    .await;
            }
            Ok(text_output(&self.chunks.concat()))
        }
    }

    struct ScriptedRuntimeResponder {
        events: Vec<dasclaw_runtime::AgentEvent>,
        output: String,
    }

    impl ScriptedRuntimeResponder {
        fn new(
            events: impl IntoIterator<Item = dasclaw_runtime::AgentEvent>,
            output: impl Into<String>,
        ) -> Self {
            Self {
                events: events.into_iter().collect(),
                output: output.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl dasclaw_runtime::AgentResponder for ScriptedRuntimeResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            Ok(text_output(&self.output))
        }

        async fn respond_streaming(
            &self,
            _ctx: &mut ReasoningContext,
            event_tx: mpsc::Sender<dasclaw_runtime::AgentEvent>,
        ) -> Result<RespondOutput, HostError> {
            for event in &self.events {
                let _ = event_tx.send(event.clone()).await;
            }
            Ok(text_output(&self.output))
        }
    }

    struct CancelAwareRuntimeResponder {
        token: CancellationToken,
    }

    impl CancelAwareRuntimeResponder {
        fn new(token: CancellationToken) -> Self {
            Self { token }
        }
    }

    #[async_trait::async_trait]
    impl dasclaw_runtime::AgentResponder for CancelAwareRuntimeResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            Ok(text_output("unexpected response"))
        }

        async fn check_signals(&self) -> dasclaw_runtime::LoopSignal {
            for _ in 0..200 {
                if self.token.is_cancelled() {
                    return dasclaw_runtime::LoopSignal::Stop;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            dasclaw_runtime::LoopSignal::Continue
        }
    }

    fn text_output(text: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::Text(text.to_string()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        }
    }

    #[derive(Debug, Default)]
    struct RecordingRuntimeBridge {
        calls: Mutex<Vec<RuntimeTurnStartRequest>>,
        cancel_calls: Mutex<Vec<RuntimeTurnCancelRequest>>,
        result: Mutex<Option<Result<(), RuntimeBridgeError>>>,
        cancel_result: Mutex<Option<Result<(), RuntimeBridgeError>>>,
        completion: Mutex<Option<RuntimeTurnOutcome>>,
    }

    impl RecordingRuntimeBridge {
        fn with_result(result: Result<(), RuntimeBridgeError>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                cancel_calls: Mutex::new(Vec::new()),
                result: Mutex::new(Some(result)),
                cancel_result: Mutex::new(None),
                completion: Mutex::new(None),
            }
        }

        fn with_completion(completion: RuntimeTurnOutcome) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                cancel_calls: Mutex::new(Vec::new()),
                result: Mutex::new(None),
                cancel_result: Mutex::new(None),
                completion: Mutex::new(Some(completion)),
            }
        }

        fn with_cancel_result(result: Result<(), RuntimeBridgeError>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                cancel_calls: Mutex::new(Vec::new()),
                result: Mutex::new(None),
                cancel_result: Mutex::new(Some(result)),
                completion: Mutex::new(None),
            }
        }
    }

    #[derive(Debug)]
    struct SequencedRuntimeBridge {
        outcomes: Vec<RuntimeTurnOutcome>,
    }

    impl SequencedRuntimeBridge {
        fn new(outcomes: impl IntoIterator<Item = RuntimeTurnOutcome>) -> Self {
            Self {
                outcomes: outcomes.into_iter().collect(),
            }
        }
    }

    impl RuntimeBridge for SequencedRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            for outcome in &self.outcomes {
                match outcome {
                    RuntimeTurnOutcome::Delta { delta } => {
                        request.updates.delta(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            delta.clone(),
                        );
                    }
                    RuntimeTurnOutcome::ReasoningSummaryDelta {
                        delta,
                        summary_index,
                    } => {
                        request.updates.reasoning_summary_delta(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            *summary_index,
                            delta.clone(),
                        );
                    }
                    RuntimeTurnOutcome::Completed { output } => {
                        request.updates.complete(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            output.clone(),
                        );
                    }
                    RuntimeTurnOutcome::Failed { error } => {
                        request.updates.fail(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            error.clone(),
                        );
                    }
                }
            }
            Ok(())
        }

        fn cancel_turn(
            &self,
            _request: RuntimeTurnCancelRequest,
        ) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn shutdown(&self) {}
    }

    #[derive(Debug)]
    struct DelayedSequencedRuntimeBridge {
        outcomes: Vec<RuntimeTurnOutcome>,
    }

    impl DelayedSequencedRuntimeBridge {
        fn new(outcomes: impl IntoIterator<Item = RuntimeTurnOutcome>) -> Self {
            Self {
                outcomes: outcomes.into_iter().collect(),
            }
        }
    }

    impl RuntimeBridge for DelayedSequencedRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            let outcomes = self.outcomes.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(25));
                for outcome in outcomes {
                    match outcome {
                        RuntimeTurnOutcome::Delta { delta } => {
                            request.updates.delta(
                                request.thread_id.clone(),
                                request.turn_id.clone(),
                                delta,
                            );
                        }
                        RuntimeTurnOutcome::ReasoningSummaryDelta {
                            delta,
                            summary_index,
                        } => {
                            request.updates.reasoning_summary_delta(
                                request.thread_id.clone(),
                                request.turn_id.clone(),
                                summary_index,
                                delta,
                            );
                        }
                        RuntimeTurnOutcome::Completed { output } => {
                            request.updates.complete(
                                request.thread_id.clone(),
                                request.turn_id.clone(),
                                output,
                            );
                        }
                        RuntimeTurnOutcome::Failed { error } => {
                            request.updates.fail(
                                request.thread_id.clone(),
                                request.turn_id.clone(),
                                error,
                            );
                        }
                    }
                }
            });
            Ok(())
        }

        fn cancel_turn(
            &self,
            _request: RuntimeTurnCancelRequest,
        ) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn shutdown(&self) {}
    }

    impl RuntimeBridge for RecordingRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            if let Some(completion) = self.completion.lock().expect("completion lock").clone() {
                match completion {
                    RuntimeTurnOutcome::Delta { delta } => {
                        request.updates.delta(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            delta,
                        );
                    }
                    RuntimeTurnOutcome::ReasoningSummaryDelta {
                        delta,
                        summary_index,
                    } => {
                        request.updates.reasoning_summary_delta(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            summary_index,
                            delta,
                        );
                    }
                    RuntimeTurnOutcome::Completed { output } => {
                        request.updates.complete(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            output,
                        );
                    }
                    RuntimeTurnOutcome::Failed { error } => {
                        request.updates.fail(
                            request.thread_id.clone(),
                            request.turn_id.clone(),
                            error,
                        );
                    }
                }
            }
            self.calls.lock().expect("calls lock").push(request);
            self.result
                .lock()
                .expect("result lock")
                .clone()
                .unwrap_or(Ok(()))
        }

        fn cancel_turn(&self, request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError> {
            self.cancel_calls
                .lock()
                .expect("cancel calls lock")
                .push(request);
            self.cancel_result
                .lock()
                .expect("cancel result lock")
                .clone()
                .unwrap_or(Ok(()))
        }

        fn shutdown(&self) {}
    }

    #[test]
    fn json_rpc_rejects_unknown_method() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"bad","method":"thread/send"}"#)
            .expect("request should return a response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["error"]["code"], -32601);
        assert_eq!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
    }

    #[test]
    fn json_rpc_notification_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","method":"{}","params":{{"reason":"test"}}}}"#,
            method::SHUTDOWN
        ));

        assert!(response.is_none());
        assert!(server.is_stopped());
        assert_eq!(server.drain_notifications().len(), 1);
    }

    #[test]
    fn invalid_json_rpc_notification_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(r#"{"jsonrpc":"1.0","method":"health/check"}"#);

        assert!(response.is_none());
    }

    #[test]
    fn invalid_json_rpc_version_request_returns_invalid_request_error() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"1.0","id":"bad-version","method":"health/check"}"#)
            .expect("request with id should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "bad-version");
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert!(value["result"].is_null());
    }

    #[test]
    fn json_rpc_parse_error_returns_error_without_id() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc("{not-json")
            .expect("parse errors should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert!(value["id"].is_null());
        assert_eq!(value["error"]["code"], -32700);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn json_rpc_invalid_request_shape_returns_invalid_request_error() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"missing-method"}"#)
            .expect("invalid request shapes should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "missing-method");
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn json_rpc_invalid_notification_shape_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(r#"{"jsonrpc":"2.0"}"#);

        assert!(response.is_none());
    }

    #[test]
    fn json_rpc_batch_request_is_rejected_explicitly() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"[{"jsonrpc":"2.0","id":"one","method":"health/check"}]"#)
            .expect("batch requests should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert!(value["id"].is_null());
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn no_params_method_rejects_non_empty_params() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"bad-params","method":"lifecycle/status","params":{"unexpected":true}}"#,
            )
            .expect("request with id should return an error response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "bad-params");
        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn unknown_method_notification_without_id_does_not_return_response() {
        let mut server = AppServer::new();
        let response = server.handle_json_rpc(r#"{"jsonrpc":"2.0","method":"thread/send"}"#);

        assert!(response.is_none());
    }
}
