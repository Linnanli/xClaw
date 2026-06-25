//! Minimal dasclaw app-server host.
//!
//! Phase 1 deliberately starts with lifecycle, initialize, health, and
//! capability reporting. Runtime execution is injected through
//! [`RuntimeBridge`]: the default host is a no-op bridge, while
//! [`DasclawAgentRuntimeBridge`] delegates to `dasclaw_runtime::Agent`.
//! DLP, jobs, skills, MCP, and sandbox execution stay outside this crate.

pub mod app_services;
pub mod command_service;
pub mod config_service;
pub mod fs_service;
pub mod hook_service;
pub mod job_service;
pub mod log_service;
pub mod mcp_service;
pub mod repo_service;
pub mod search_service;
pub mod skills_service;

mod blocking_runtime;
mod sandbox_protocol;
mod thread_lifecycle;

pub use sandbox_protocol::RuntimeSandboxContext;

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dasclaw_app_server_protocol::ClientModelConfig;
use dasclaw_app_server_protocol::{
    AgentMessageDeltaEvent, AppServerApprovalDecision, ApprovalResponsePayload,
    AutoApprovalReviewCompletedEvent, AutoApprovalReviewStartedEvent, CapabilitiesChangedEvent,
    CapabilitiesChangedReason, CapabilitiesListResponse, CapabilityMatrix, ClientInfo,
    CommandExecOutputDeltaNotification, CommandExecParams, CommandExecResizeParams,
    CommandExecResizeResponse, CommandExecResponse, CommandExecTerminateParams,
    CommandExecTerminateResponse, CommandExecWriteParams, CommandExecWriteResponse,
    CommandExecutionApprovalRequest, CommandExecutionOutputDeltaEvent,
    CommandExecutionTerminalInteractionEvent, CompatibilityProfile, ConfigBatchWriteParams,
    ConfigReadParams, ConfigReadResponse, ConfigRequirementsReadResponse, ConfigValueWriteParams,
    ConfigWarningNotification, ConfigWriteResponse, DEFAULT_MAX_PENDING_NOTIFICATIONS,
    DeprecationNoticeNotification, DynamicToolCallOutputContentItem, DynamicToolCallParams,
    DynamicToolCallResponse, ErrorCode, ErrorData, ErrorEvent, FileChangeApprovalDecision,
    FileChangeOutputDeltaEvent, FileChangePatchUpdatedEvent, FileChangeRequestApprovalParams,
    FileChangeRequestApprovalResponse, FileUpdateChange, FsChangedNotification, FsCopyParams,
    FsCopyResponse, FsCreateDirectoryParams, FsCreateDirectoryResponse, FsGetMetadataParams,
    FsGetMetadataResponse, FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams,
    FsReadFileResponse, FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse,
    FsWatchParams, FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, FuzzyFileSearchParams,
    FuzzyFileSearchResponse, GetConversationSummaryParams, GetConversationSummaryResponse,
    GitDiffToRemoteParams, GitDiffToRemoteResponse, GuardianApprovalReview,
    GuardianWarningNotification, HealthCheckParams, HealthCheckResponse, HookCompletedNotification,
    HookStartedNotification, InitializeParams, InitializeResponse, ItemCompletedEvent,
    ItemStartedEvent, JobListParams, JobListResponse, JobReadParams, JobReadResponse,
    JsonRpcClientResponse, JsonRpcError, JsonRpcIncoming, JsonRpcRequest, JsonRpcResponse,
    JsonRpcServerRequest, LifecycleChangedEvent, LifecycleReason, LifecycleSnapshot,
    LifecycleState, LifecycleStatusResponse, ListMcpServerStatusParams,
    ListMcpServerStatusResponse, LogEntryEvent, McpResourceReadParams, McpResourceReadResponse,
    McpServerOauthLoginCompletedNotification, McpServerOauthLoginParams,
    McpServerOauthLoginResponse, McpServerReloadParams, McpServerReloadResponse,
    McpServerStartupState, McpServerStatusUpdatedNotification, McpServerToolCallParams,
    McpServerToolCallResponse, McpToolCallProgressNotification, ModelListParams, ModelListResponse,
    ModelProviderInitializeConfig, ModelProviderSelectForNextTurnParams,
    ModelProviderSelectForNextTurnResponse, ModelRerouteReason, ModelReroutedNotification,
    ModelVerification, ModelVerificationNotification, NotificationQueuePolicy,
    NotificationsInitializedEvent, PermissionsApprovalDecision, PermissionsRequestApprovalParams,
    PermissionsRequestApprovalResponse, PlanDeltaEvent, ProtocolSchemaResponse, ProtocolVersion,
    RawResponseItemCompletedEvent, ReasoningSummaryPartAddedEvent, ReasoningSummaryTextDeltaEvent,
    ReasoningTextDeltaEvent, RuntimeToolApprovalAvailability, SandboxMode, ServerInfo,
    ServerNotification, ServerRequestResolutionOutcome, ServerRequestResolvedEvent, ServiceHealth,
    ServiceName, ShutdownParams, ShutdownReason, ShutdownResponse, SkillsChangedNotification,
    SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListParams, SkillsListResponse,
    ThreadArchiveParams, ThreadArchiveResponse, ThreadArchivedEvent, ThreadCompactStartParams,
    ThreadCompactStartResponse, ThreadCompactedEvent, ThreadForkParams, ThreadForkResponse,
    ThreadGoalClearParams, ThreadGoalClearResponse, ThreadGoalClearedEvent, ThreadGoalGetParams,
    ThreadGoalGetResponse, ThreadGoalSetParams, ThreadGoalSetResponse, ThreadGoalUpdatedEvent,
    ThreadInjectItemsParams, ThreadInjectItemsResponse, ThreadListParams, ThreadListResponse,
    ThreadLoadedListParams, ThreadLoadedListResponse, ThreadMetadataUpdateResponse,
    ThreadNameUpdatedEvent, ThreadReadParams, ThreadReadResponse, ThreadResumeParams,
    ThreadResumeResponse, ThreadRollbackParams, ThreadRollbackResponse, ThreadSetNameParams,
    ThreadSetNameResponse, ThreadStartParams, ThreadStartResponse, ThreadStartedEvent,
    ThreadStatusChangedEvent, ThreadTokenUsageUpdatedEvent, ThreadTurnsListParams,
    ThreadTurnsListResponse, ThreadUnarchiveParams, ThreadUnarchiveResponse, ThreadUnarchivedEvent,
    ThreadUnsubscribeParams, ThreadUnsubscribeResponse, ThreadUnsubscribeStatus,
    TokenUsageBreakdown, ToolRequestUserInputParams, ToolRequestUserInputQuestion,
    ToolRequestUserInputResponse, TurnCompletedEvent, TurnDiffUpdatedEvent, TurnInterruptParams,
    TurnInterruptResponse, TurnPlanStep, TurnPlanUpdatedEvent, TurnReadParams, TurnReadResponse,
    TurnStartParams, TurnStartResponse, TurnStartedEvent, TurnStatus, TurnSteerParams,
    TurnSteerResponse, UserInput, WarningNotification,
};
use dasclaw_app_server_protocol::{
    CodexSessionSource, CodexThread, CodexThreadItem, CodexThreadStatus, CodexTurn, CodexTurnError,
    CodexTurnStatus, ConversationSummary, ReviewDelivery, ReviewStartParams, ReviewStartResponse,
    ReviewTarget,
};
use dasclaw_app_server_protocol::{JSON_RPC_VERSION, method, server_request};
use dasclaw_core::agentic_loop::{AgentPlanStep, AgentPlanStepStatus};
use dasclaw_core::messages::ReasoningSummary;
use dasclaw_llm_provider::provider::claw_code_provider::ClawCodeLlmProvider;
use dasclaw_llm_provider::provider::config::{CacheRetention, RegistryProviderConfig};
use dasclaw_llm_provider::provider::registry::ProviderProtocol;
use dasclaw_runtime::LlmProviderResponder;
use hook_service::AppServerHookNotification;
use search_service::SearchNotification;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thread_lifecycle::{
    ThreadCreation, ThreadFork, ThreadLifecycleHost, ThreadSummary, TurnSummary,
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub const SERVER_NAME: &str = "dasclaw_app_server";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const STDIO_NOTIFICATION_POLL_INTERVAL: Duration = Duration::from_millis(10);
pub const STDIO_EOF_DRAIN_TIMEOUT: Duration = Duration::from_millis(100);
pub const SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct AppServer {
    server: ServerInfo,
    lifecycle: LifecycleSnapshot,
    capabilities: CapabilityMatrix,
    client: Option<ClientInfo>,
    notifications: NotificationBus,
    threads: ThreadLifecycleHost,
    runtime_bridge: Arc<dyn RuntimeBridge>,
    runtime_features: RuntimeBridgeFeatures,
    runtime_turn_updates: RuntimeTurnUpdateSink,
    pending_server_requests: PendingServerRequestStore,
    model_provider: ModelProviderState,
    app_services: app_services::AppServerServices,
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

    fn selected_provider_id(&self) -> String {
        self.selected_model_id
            .as_deref()
            .and_then(|selected_model_id| self.models.get(selected_model_id))
            .and_then(|model| model.provider.clone())
            .unwrap_or_default()
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

    fn model_list_response(&self) -> Result<ModelListResponse, AppServerError> {
        let selected_model_id = self.selected_model_id.as_deref().ok_or_else(|| {
            AppServerError::invalid_request(
                "model_provider",
                "model provider config is required before listing models",
            )
        })?;
        let mut models = self
            .models
            .values()
            .map(|model| {
                dasclaw_app_server_protocol::CodexModel::from_client_model(
                    model,
                    model.model_id == selected_model_id,
                )
            })
            .collect::<Vec<_>>();
        models.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(ModelListResponse {
            data: models,
            next_cursor: None,
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingServerRequestKind {
    CommandApproval,
    #[allow(dead_code)]
    DynamicToolCall,
    #[allow(dead_code)]
    ToolUserInput,
    #[allow(dead_code)]
    FileChangeApproval,
    #[allow(dead_code)]
    PermissionsApproval,
}

#[derive(Debug, Clone)]
struct PendingServerRequest {
    request_id: String,
    runtime_request_id: String,
    kind: PendingServerRequestKind,
    thread_id: String,
    turn_id: String,
    created_at: Instant,
}

#[derive(Debug, Clone, Default)]
struct PendingServerRequestStore {
    requests: Vec<PendingServerRequest>,
}

impl PendingServerRequestStore {
    fn new() -> Self {
        Self::default()
    }

    fn insert(&mut self, request: PendingServerRequest) {
        self.requests
            .retain(|existing| existing.request_id != request.request_id);
        self.requests.push(request);
    }

    fn remove(&mut self, id: &Value) -> Option<PendingServerRequest> {
        let request_id = json_rpc_id_to_request_id(id);
        let index = self
            .requests
            .iter()
            .position(|request| request.request_id == request_id)?;
        Some(self.requests.remove(index))
    }

    fn drain_turn(&mut self, thread_id: &str, turn_id: &str) -> Vec<PendingServerRequest> {
        let mut drained = Vec::new();
        let mut retained = Vec::new();
        for request in self.requests.drain(..) {
            if request.thread_id == thread_id && request.turn_id == turn_id {
                drained.push(request);
            } else {
                retained.push(request);
            }
        }
        self.requests = retained;
        drained
    }

    fn drain_all(&mut self) -> Vec<PendingServerRequest> {
        std::mem::take(&mut self.requests)
    }

    fn expired(&mut self, timeout: Duration) -> Vec<PendingServerRequest> {
        let now = Instant::now();
        let mut expired = Vec::new();
        let mut retained = Vec::new();
        for request in self.requests.drain(..) {
            if now.duration_since(request.created_at) >= timeout {
                expired.push(request);
            } else {
                retained.push(request);
            }
        }
        self.requests = retained;
        expired
    }

    #[cfg(test)]
    fn age_all(&mut self, age: Duration) {
        for request in &mut self.requests {
            request.created_at = request
                .created_at
                .checked_sub(age)
                .unwrap_or(request.created_at);
        }
    }
}

fn json_rpc_id_to_request_id(id: &Value) -> String {
    id.as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| id.to_string())
}

fn decode_server_request_response(
    kind: PendingServerRequestKind,
    result: Value,
) -> Result<RuntimeServerRequestResponse, String> {
    match kind {
        PendingServerRequestKind::CommandApproval => {
            let payload = serde_json::from_value::<ApprovalResponsePayload>(result)
                .map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::Approval(
                approval_decision_to_runtime(payload.decision),
            ))
        }
        PendingServerRequestKind::DynamicToolCall => {
            serde_json::from_value::<DynamicToolCallResponse>(result)
                .map(RuntimeServerRequestResponse::DynamicTool)
                .map_err(|error| error.to_string())
        }
        PendingServerRequestKind::ToolUserInput => {
            serde_json::from_value::<ToolRequestUserInputResponse>(result)
                .map(RuntimeServerRequestResponse::ToolUserInput)
                .map_err(|error| error.to_string())
        }
        PendingServerRequestKind::FileChangeApproval => {
            let payload = serde_json::from_value::<FileChangeRequestApprovalResponse>(result)
                .map_err(|error| error.to_string())?;
            Ok(RuntimeServerRequestResponse::FileChange(payload.decision))
        }
        PendingServerRequestKind::PermissionsApproval => {
            serde_json::from_value::<PermissionsRequestApprovalResponse>(result)
                .map(RuntimeServerRequestResponse::Permissions)
                .map_err(|error| error.to_string())
        }
    }
}

fn approval_decision_to_runtime(
    decision: AppServerApprovalDecision,
) -> dasclaw_runtime::ApprovalDecision {
    match decision {
        AppServerApprovalDecision::Approve => dasclaw_runtime::ApprovalDecision::Approve,
        AppServerApprovalDecision::ApproveAlways => {
            dasclaw_runtime::ApprovalDecision::ApproveAlways
        }
        AppServerApprovalDecision::Reject { reason } => {
            dasclaw_runtime::ApprovalDecision::Reject { reason }
        }
    }
}

const PERMISSIONS_REQUEST_REJECTED_REASON: &str = "permissions request rejected";

fn failed_runtime_server_request_response(
    kind: PendingServerRequestKind,
    reason: &str,
) -> RuntimeServerRequestResponse {
    match kind {
        PendingServerRequestKind::CommandApproval => {
            RuntimeServerRequestResponse::Approval(dasclaw_runtime::ApprovalDecision::Reject {
                reason: Some(reason.to_string()),
            })
        }
        PendingServerRequestKind::DynamicToolCall => {
            RuntimeServerRequestResponse::DynamicTool(DynamicToolCallResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: reason.to_string(),
                }],
                success: false,
            })
        }
        PendingServerRequestKind::ToolUserInput => {
            RuntimeServerRequestResponse::ToolUserInput(ToolRequestUserInputResponse {
                answers: HashMap::new(),
            })
        }
        PendingServerRequestKind::FileChangeApproval => {
            RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Cancel)
        }
        PendingServerRequestKind::PermissionsApproval => {
            RuntimeServerRequestResponse::Permissions(denied_permissions_response())
        }
    }
}

fn server_request_resolution_outcome(
    payload: &RuntimeServerRequestResponse,
) -> (ServerRequestResolutionOutcome, Option<String>) {
    match payload {
        RuntimeServerRequestResponse::Approval(dasclaw_runtime::ApprovalDecision::Reject {
            reason,
        }) => (ServerRequestResolutionOutcome::Rejected, reason.clone()),
        RuntimeServerRequestResponse::Approval(_) => {
            (ServerRequestResolutionOutcome::Approved, None)
        }
        RuntimeServerRequestResponse::DynamicTool(response) => {
            if response.success {
                (ServerRequestResolutionOutcome::Approved, None)
            } else {
                (
                    ServerRequestResolutionOutcome::Failed,
                    Some("dynamic tool call response reported failure".to_string()),
                )
            }
        }
        RuntimeServerRequestResponse::ToolUserInput(_) => {
            (ServerRequestResolutionOutcome::Approved, None)
        }
        RuntimeServerRequestResponse::Permissions(response) => match response.decision {
            PermissionsApprovalDecision::Approve => {
                (ServerRequestResolutionOutcome::Approved, None)
            }
            PermissionsApprovalDecision::Reject => (
                ServerRequestResolutionOutcome::Rejected,
                Some(PERMISSIONS_REQUEST_REJECTED_REASON.to_string()),
            ),
        },
        RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Accept)
        | RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::AcceptForSession) => {
            (ServerRequestResolutionOutcome::Approved, None)
        }
        RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Decline)
        | RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Cancel) => {
            (ServerRequestResolutionOutcome::Rejected, None)
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

fn parse_reasoning_summary(
    value: Option<&str>,
    model_id: &str,
) -> Result<ReasoningSummary, AppServerError> {
    match value.unwrap_or("none").trim().to_ascii_lowercase().as_str() {
        "none" => Ok(ReasoningSummary::None),
        "auto" => Ok(ReasoningSummary::Auto),
        "concise" => Ok(ReasoningSummary::Concise),
        "detailed" => Ok(ReasoningSummary::Detailed),
        other => Err(AppServerError::invalid_request(
            "turn/start",
            format!("turn/start reasoningSummary is invalid for modelId: {model_id}: {other}"),
        )),
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
            threads: ThreadLifecycleHost::new(),
            runtime_bridge: Arc::new(NoopRuntimeBridge),
            runtime_features: RuntimeBridgeFeatures::default(),
            runtime_turn_updates: RuntimeTurnUpdateSink::new(),
            pending_server_requests: PendingServerRequestStore::new(),
            model_provider: ModelProviderState::default(),
            app_services: app_services::AppServerServices::default(),
        }
    }

    pub fn with_thread_snapshot_path(
        mut self,
        path: impl Into<PathBuf>,
    ) -> Result<Self, AppServerError> {
        self.threads = ThreadLifecycleHost::with_snapshot_path(path)?;
        Ok(self)
    }

    #[must_use]
    pub fn with_runtime_bridge(runtime_bridge: Arc<dyn RuntimeBridge>) -> Self {
        let mut server = Self::new();
        let features = runtime_bridge.features();
        if features.tools {
            server.capabilities = server.capabilities.with_runtime_command_tool_events_ready();
        }
        if features.approval || features.tools {
            server.capabilities = server
                .capabilities
                .with_runtime_tool_approval(features.runtime_tool_approval_availability());
        }
        if features.sandbox {
            server.capabilities = server.capabilities.with_runtime_sandbox_ready();
        }
        if features.thread_compact {
            server.capabilities = server.capabilities.with_thread_compact_ready();
        }
        if features.turn_steer {
            server.capabilities = server.capabilities.with_runtime_turn_steer_ready();
        }
        server.runtime_features = features;
        server.runtime_bridge = runtime_bridge;
        server
    }

    #[must_use]
    pub fn with_app_services(mut self, services: app_services::AppServerServices) -> Self {
        self.app_services = services;
        self.refresh_service_capabilities();
        self
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
        self.refresh_service_capabilities();
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
            self.client = Some(params.client);
            return Ok(InitializeResponse {
                server: self.server.clone(),
                lifecycle: self.lifecycle.clone(),
                capabilities: self.capabilities.clone(),
                compatibility_profiles: compatibility_profiles(),
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
        self.app_services.collect_startup_notifications();
        self.drain_service_updates();

        Ok(InitializeResponse {
            server: self.server.clone(),
            lifecycle: self.lifecycle.clone(),
            capabilities: self.capabilities.clone(),
            compatibility_profiles: compatibility_profiles(),
            unavailable_requested_capabilities,
        })
    }

    #[must_use]
    pub fn health_check(&mut self, params: HealthCheckParams) -> HealthCheckResponse {
        self.drain_runtime_turn_updates();
        self.expire_pending_server_requests();
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
        self.refresh_service_capabilities();
        CapabilitiesListResponse {
            capabilities: self.capabilities.clone(),
            compatibility_profiles: compatibility_profiles(),
        }
    }

    #[must_use]
    pub fn protocol_schema(&self) -> ProtocolSchemaResponse {
        ProtocolSchemaResponse::phase_one(self.service_capability_snapshot())
    }

    pub fn config_requirements_read(
        &self,
    ) -> Result<ConfigRequirementsReadResponse, AppServerError> {
        self.require_initialized("sandbox")?;
        Ok(ConfigRequirementsReadResponse {
            allowed_sandbox_modes: vec![SandboxMode::ReadOnly, SandboxMode::WorkspaceWrite],
        })
    }

    pub fn config_read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, AppServerError> {
        self.require_initialized("config")?;
        self.app_services.config.read(params)
    }

    pub fn config_value_write(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        self.require_initialized("config")?;
        self.app_services.config.write_value(params)
    }

    pub fn config_batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        self.require_initialized("config")?;
        self.app_services.config.write_batch(params)
    }

    pub fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<GitDiffToRemoteResponse, AppServerError> {
        self.require_initialized("repo")?;
        self.app_services.repo.git_diff_to_remote(params)
    }

    pub fn fuzzy_file_search(
        &self,
        params: FuzzyFileSearchParams,
    ) -> Result<FuzzyFileSearchResponse, AppServerError> {
        self.require_initialized("search")?;
        self.app_services.search.fuzzy_file_search(params)
    }

    fn create_thread_record(
        &mut self,
        params: ThreadStartParams,
    ) -> Result<String, AppServerError> {
        if self.lifecycle.state == LifecycleState::Stopped {
            return Err(AppServerError::server_stopped(self.lifecycle.clone()));
        }
        self.require_initialized("session")?;

        let thread_id = self.threads.create(ThreadCreation {
            cwd: params.cwd,
            sandbox: params.sandbox,
            permission_profile: params.permission_profile,
            title: None,
            ephemeral: true,
            forked_from_id: None,
        })?;
        self.emit_codex_thread_started(thread_id.clone())?;

        Ok(thread_id)
    }

    #[cfg(test)]
    fn create_thread_for_test(
        &mut self,
        params: TestThreadParams,
    ) -> Result<TestThreadHandle, AppServerError> {
        let thread_id = self.create_thread_record(ThreadStartParams {
            cwd: params.cwd,
            sandbox: None,
            permission_profile: None,
        })?;
        Ok(TestThreadHandle {
            thread_id,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn thread_start(
        &mut self,
        params: ThreadStartParams,
    ) -> Result<ThreadStartResponse, AppServerError> {
        let selected_model = self.model_provider.selected_snapshot()?;
        let thread_id = self.create_thread_record(params)?;
        let thread = self.codex_thread_view(&thread_id, true)?;
        let model = selected_model.model_id;
        let model_provider = selected_model.provider;
        let cwd = thread.cwd.clone();
        Ok(ThreadStartResponse {
            thread,
            model,
            model_provider,
            cwd,
        })
    }

    pub fn get_conversation_summary(
        &self,
        params: GetConversationSummaryParams,
    ) -> Result<GetConversationSummaryResponse, AppServerError> {
        self.require_initialized("session")?;
        let summary = match params {
            GetConversationSummaryParams::ConversationId { conversation_id } => {
                self.thread_summary_or_error_with_capability(&conversation_id, "session")?
            }
            GetConversationSummaryParams::RolloutPath { rollout_path } => {
                self.threads.summary_by_path(&rollout_path).ok_or_else(|| {
                    AppServerError::invalid_request(
                        "session",
                        format!("unknown rollout path: {rollout_path}"),
                    )
                })?
            }
        };

        Ok(GetConversationSummaryResponse {
            summary: self.conversation_summary_view(summary)?,
        })
    }

    pub fn review_start(
        &mut self,
        params: ReviewStartParams,
    ) -> Result<ReviewStartResponse, AppServerError> {
        self.require_initialized("review")?;
        self.thread_summary_or_error_with_capability(&params.thread_id, "review")?;

        let prompt = review_start_prompt(&params.target)?;
        let review_thread_id = match params.delivery.unwrap_or(ReviewDelivery::Inline) {
            ReviewDelivery::Inline => params.thread_id,
            ReviewDelivery::Detached => {
                self.thread_fork(ThreadForkParams {
                    thread_id: params.thread_id,
                    ephemeral: true,
                    exclude_turns: false,
                    persist_extended_history: false,
                    cwd: None,
                })
                .map_err(|error| error.with_capability("review"))?
                .thread
                .id
            }
        };
        let turn = self
            .turn_start(TurnStartParams {
                thread_id: review_thread_id.clone(),
                input: vec![dasclaw_app_server_protocol::UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })?
            .turn;

        Ok(ReviewStartResponse {
            turn,
            review_thread_id,
        })
    }

    pub fn thread_list(
        &self,
        _params: ThreadListParams,
    ) -> Result<ThreadListResponse, AppServerError> {
        self.require_initialized("session")?;
        let data = self
            .threads
            .list()
            .into_iter()
            .map(|thread| self.codex_thread_view(&thread.thread_id, true))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ThreadListResponse {
            data,
            next_cursor: None,
            backwards_cursor: None,
        })
    }

    pub fn thread_read(
        &self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse, AppServerError> {
        self.require_initialized("session")?;
        Ok(ThreadReadResponse {
            thread: self.codex_thread_view(&params.thread_id, true)?,
        })
    }

    pub fn thread_resume(
        &mut self,
        params: ThreadResumeParams,
    ) -> Result<ThreadResumeResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        if params.history.is_some() {
            return Err(history_import_unsupported());
        }
        let summary =
            self.thread_summary_or_error_with_capability(&params.thread_id, "thread_lifecycle")?;
        if summary.archived {
            return Err(AppServerError::invalid_request(
                "thread_lifecycle",
                "cannot resume archived thread",
            ));
        }
        if params.path.is_some() && params.path != summary.path {
            return Err(history_import_unsupported());
        }
        let selected_model = self.model_provider.selected_snapshot()?;
        self.threads.set_subscribed(&params.thread_id, true)?;
        let thread = self.codex_thread_view(&params.thread_id, !params.exclude_turns)?;
        let cwd = thread.cwd.clone();
        self.emit_thread_status_changed(&thread.id, thread.status.clone());
        Ok(ThreadResumeResponse {
            thread,
            model: selected_model.model_id,
            model_provider: selected_model.provider,
            cwd,
        })
    }

    pub fn thread_fork(
        &mut self,
        params: ThreadForkParams,
    ) -> Result<ThreadForkResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        self.thread_summary_or_error_with_capability(&params.thread_id, "thread_lifecycle")?;
        let selected_model = self.model_provider.selected_snapshot()?;
        let thread_id = self.threads.fork(
            &params.thread_id,
            ThreadFork {
                cwd: params.cwd,
                ephemeral: params.ephemeral,
                exclude_turns: params.exclude_turns,
            },
        )?;
        self.emit_codex_thread_started(thread_id.clone())?;
        let thread = self.codex_thread_view(&thread_id, true)?;
        let cwd = thread.cwd.clone();
        Ok(ThreadForkResponse {
            thread,
            model: selected_model.model_id,
            model_provider: selected_model.provider,
            cwd,
        })
    }

    pub fn thread_archive(
        &mut self,
        params: ThreadArchiveParams,
    ) -> Result<ThreadArchiveResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        self.threads.set_archived(&params.thread_id, true)?;
        self.notifications
            .emit_thread_archived(ThreadArchivedEvent {
                thread_id: params.thread_id,
            });
        Ok(ThreadArchiveResponse {})
    }

    pub fn thread_unarchive(
        &mut self,
        params: ThreadUnarchiveParams,
    ) -> Result<ThreadUnarchiveResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        self.threads.set_archived(&params.thread_id, false)?;
        self.notifications
            .emit_thread_unarchived(ThreadUnarchivedEvent {
                thread_id: params.thread_id.clone(),
            });
        Ok(ThreadUnarchiveResponse {
            thread: self.codex_thread_view(&params.thread_id, true)?,
        })
    }

    pub fn thread_unsubscribe(
        &mut self,
        params: ThreadUnsubscribeParams,
    ) -> Result<ThreadUnsubscribeResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        let status = match self.threads.summary(&params.thread_id) {
            None => ThreadUnsubscribeStatus::NotLoaded,
            Some(summary) if !summary.subscribed => ThreadUnsubscribeStatus::NotSubscribed,
            Some(_) => {
                self.threads.set_subscribed(&params.thread_id, false)?;
                ThreadUnsubscribeStatus::Unsubscribed
            }
        };
        Ok(ThreadUnsubscribeResponse { status })
    }

    pub fn thread_name_set(
        &mut self,
        params: ThreadSetNameParams,
    ) -> Result<ThreadSetNameResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        let name = params.name.and_then(|name| {
            let trimmed = name.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        });
        self.threads.set_name(&params.thread_id, name.clone())?;
        self.notifications
            .emit_thread_name_updated(ThreadNameUpdatedEvent {
                thread_id: params.thread_id.clone(),
                thread_name: name,
            });
        Ok(ThreadSetNameResponse {})
    }

    pub fn thread_metadata_update_value(
        &mut self,
        params: Value,
    ) -> Result<ThreadMetadataUpdateResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        let thread_id = params
            .get("threadId")
            .and_then(Value::as_str)
            .ok_or_else(|| AppServerError::invalid_request("thread_lifecycle", "missing threadId"))?
            .to_string();
        if let Some(git_info) = params.get("gitInfo") {
            let patch = match git_info {
                Value::Null => None,
                value => Some(value.clone()),
            };
            self.threads.update_git_info(&thread_id, patch)?;
        } else {
            self.thread_summary_or_error_with_capability(&thread_id, "thread_lifecycle")?;
        }
        Ok(ThreadMetadataUpdateResponse {
            thread: self.codex_thread_view(&thread_id, true)?,
        })
    }

    pub fn thread_rollback(
        &mut self,
        params: ThreadRollbackParams,
    ) -> Result<ThreadRollbackResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        if params.num_turns == 0 {
            return Err(AppServerError::invalid_request(
                "thread_lifecycle",
                "numTurns must be greater than 0",
            ));
        }
        self.threads
            .rollback(&params.thread_id, params.num_turns as usize)?;
        let thread = self.codex_thread_view(&params.thread_id, true)?;
        self.emit_thread_status_changed(&thread.id, thread.status.clone());
        Ok(ThreadRollbackResponse { thread })
    }

    pub fn thread_loaded_list(
        &self,
        params: ThreadLoadedListParams,
    ) -> Result<ThreadLoadedListResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        let limit = params.limit.unwrap_or(50);
        if limit == 0 {
            return Err(AppServerError::invalid_request(
                "thread_lifecycle",
                "limit must be greater than 0",
            ));
        }
        let start = match params.cursor {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| {
                AppServerError::invalid_request("thread_lifecycle", "invalid cursor")
            })?,
            None => 0,
        };
        let loaded = self
            .threads
            .list()
            .into_iter()
            .filter(|thread| thread.subscribed && !thread.archived)
            .map(|thread| thread.thread_id)
            .collect::<Vec<_>>();
        let end = start.saturating_add(limit as usize).min(loaded.len());
        let data = loaded
            .get(start..end)
            .map_or_else(Vec::new, |threads| threads.to_vec());
        let next_cursor = (end < loaded.len()).then(|| end.to_string());
        Ok(ThreadLoadedListResponse { data, next_cursor })
    }

    pub fn thread_inject_items(
        &mut self,
        params: ThreadInjectItemsParams,
    ) -> Result<ThreadInjectItemsResponse, AppServerError> {
        self.require_initialized("thread_lifecycle")?;
        self.threads.inject_items(&params.thread_id, params.items)?;
        Ok(ThreadInjectItemsResponse {})
    }

    pub fn thread_goal_set(
        &mut self,
        params: ThreadGoalSetParams,
    ) -> Result<ThreadGoalSetResponse, AppServerError> {
        self.require_initialized("thread_goal")?;
        if params.objective.trim().is_empty() {
            return Err(AppServerError::invalid_request(
                "thread_goal",
                "objective must not be empty",
            ));
        }
        let goal = self.threads.set_goal(
            &params.thread_id,
            params.objective.trim().to_string(),
            params.status,
            params.token_budget,
        )?;
        self.notifications
            .emit_thread_goal_updated(ThreadGoalUpdatedEvent {
                thread_id: params.thread_id,
                turn_id: None,
                goal: goal.clone(),
            });
        Ok(ThreadGoalSetResponse { goal })
    }

    pub fn thread_goal_get(
        &self,
        params: ThreadGoalGetParams,
    ) -> Result<ThreadGoalGetResponse, AppServerError> {
        self.require_initialized("thread_goal")?;
        Ok(ThreadGoalGetResponse {
            goal: self.threads.get_goal(&params.thread_id)?,
        })
    }

    pub fn thread_goal_clear(
        &mut self,
        params: ThreadGoalClearParams,
    ) -> Result<ThreadGoalClearResponse, AppServerError> {
        self.require_initialized("thread_goal")?;
        if self.threads.clear_goal(&params.thread_id)? {
            self.notifications
                .emit_thread_goal_cleared(ThreadGoalClearedEvent {
                    thread_id: params.thread_id,
                });
        }
        Ok(ThreadGoalClearResponse {})
    }

    pub fn thread_compact_start(
        &mut self,
        params: ThreadCompactStartParams,
    ) -> Result<ThreadCompactStartResponse, AppServerError> {
        self.require_initialized("thread_compact")?;
        self.thread_summary_or_error_with_capability(&params.thread_id, "thread_compact")?;
        if !self.runtime_features.thread_compact {
            return Err(AppServerError::capability_unavailable(
                "thread_compact",
                "runtime bridge does not support thread compact",
            ));
        }
        let result = self
            .runtime_bridge
            .compact_thread(RuntimeThreadCompactRequest {
                thread_id: params.thread_id.clone(),
            })
            .map_err(AppServerError::runtime_bridge)?;
        self.threads
            .record_compacted_turn(&params.thread_id, result.turn_id.clone())?;
        self.notifications
            .emit_thread_compacted(ThreadCompactedEvent {
                thread_id: params.thread_id,
                turn_id: result.turn_id,
            });
        Ok(ThreadCompactStartResponse {})
    }

    pub fn turn_start(
        &mut self,
        params: TurnStartParams,
    ) -> Result<TurnStartResponse, AppServerError> {
        self.require_initialized("session")?;
        let thread_summary = self
            .threads
            .summary(&params.thread_id)
            .ok_or_else(|| AppServerError::invalid_request("session", "thread not found"))?;
        let model_provider = self.model_provider.selected_snapshot()?;
        let mut prompt = params.prompt_text();
        if prompt.trim().is_empty() {
            return Err(AppServerError::invalid_request(
                "turn/start",
                "turn/start input must include at least one text item",
            ));
        }
        let reasoning_summary =
            parse_reasoning_summary(params.summary.as_deref(), &model_provider.model_id)?;
        let turn_cwd = params
            .cwd
            .as_deref()
            .or(thread_summary.workspace_root.as_deref())
            .map(std::path::Path::new)
            .unwrap_or_else(|| std::path::Path::new("."));
        let sandbox_context = crate::sandbox_protocol::resolve_turn_context(
            params.sandbox_policy,
            params.permission_profile,
            thread_summary.sandbox_context,
            turn_cwd,
            "turn/start",
        )?;
        let sandbox_warning = warning_from_sandbox_context(&params.thread_id, &sandbox_context)?;
        let turn_id = self.threads.next_turn_id();
        self.threads
            .record_started_turn(&params.thread_id, turn_id.clone())?;
        match self.run_hook_lifecycle_event(dasclaw_hooks::HookEvent::Inbound {
            user_id: "app-server".to_string(),
            channel: "chat".to_string(),
            content: prompt.clone(),
            thread_id: Some(params.thread_id.clone()),
        }) {
            Ok(Some(modified)) => {
                prompt = modified;
            }
            Ok(None) => {}
            Err(error) => {
                if let Err(cancel_error) = self.threads.cancel_turn(&params.thread_id, &turn_id) {
                    return Err(AppServerError::service_degraded(
                        "thread_lifecycle",
                        format!(
                            "inbound hook rejected turn start: {}; failed to cancel pending turn: {}",
                            error.public_message(),
                            cancel_error.public_message()
                        ),
                    ));
                }
                return Err(error);
            }
        }
        if let Some(warning) = sandbox_warning {
            self.notifications.emit_warning(warning);
        }
        self.runtime_bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: params.thread_id.clone(),
                turn_id: turn_id.clone(),
                prompt,
                cwd: turn_cwd.to_path_buf(),
                model_provider,
                reasoning_summary,
                sandbox_context,
                updates: self.runtime_turn_updates.clone(),
                hook_registry: self.app_services.hook_registry.clone(),
            })
            .map_err(|error| {
                if let Err(cancel_error) = self.threads.cancel_turn(&params.thread_id, &turn_id) {
                    return AppServerError::service_degraded(
                        "thread_lifecycle",
                        format!(
                            "runtime start failed: {error}; failed to cancel pending turn: {}",
                            cancel_error.public_message()
                        ),
                    );
                }
                AppServerError::runtime_bridge(error)
            })?;
        self.transition_lifecycle(
            LifecycleState::Running,
            LifecycleReason::RequestInProgress,
            Some("turn request is running".to_string()),
        );
        let thread_id = params.thread_id;
        let turn = self.codex_turn_view(&thread_id, &turn_id)?;
        self.notifications.emit_turn_started(TurnStartedEvent {
            thread_id: thread_id.clone(),
            turn: turn.clone(),
        });
        self.emit_codex_item_started(thread_id.clone(), turn_id.clone());

        Ok(TurnStartResponse { turn })
    }

    pub fn turn_steer(
        &mut self,
        params: TurnSteerParams,
    ) -> Result<TurnSteerResponse, AppServerError> {
        self.require_initialized("session")?;
        self.drain_runtime_turn_updates();
        self.require_thread_exists(&params.thread_id)?;
        if !self.runtime_features.turn_steer {
            return Err(AppServerError::capability_unavailable(
                "turn_steer",
                "runtime bridge does not support turn steer",
            ));
        }
        if params.input.is_empty() {
            return Err(AppServerError::invalid_request(
                "turn/steer",
                "turn/steer input must not be empty",
            ));
        }
        let prompt = prompt_text_from_user_input(&params.input);
        if prompt.trim().is_empty() {
            return Err(AppServerError::invalid_request(
                "turn/steer",
                "turn/steer input must include at least one text item",
            ));
        }
        let active_turn =
            self.turn_summary_or_error(&params.thread_id, &params.expected_turn_id)?;
        if active_turn.status != TurnStatus::Pending {
            return Err(AppServerError::invalid_request(
                "session",
                format!("turn is not active: {}", params.expected_turn_id),
            ));
        }

        let turn_id = params.expected_turn_id;
        self.runtime_bridge
            .steer_turn(RuntimeTurnSteerRequest {
                thread_id: params.thread_id,
                turn_id: turn_id.clone(),
                input: params.input.clone(),
                prompt,
                responsesapi_client_metadata: params.responsesapi_client_metadata,
            })
            .map_err(AppServerError::runtime_bridge)?;

        Ok(TurnSteerResponse { turn_id })
    }

    pub fn model_provider_select_for_next_turn(
        &mut self,
        params: ModelProviderSelectForNextTurnParams,
    ) -> Result<ModelProviderSelectForNextTurnResponse, AppServerError> {
        self.require_initialized("model_provider")?;
        let selected_model_id = self.model_provider.select_for_next_turn(params.model_id)?;

        Ok(ModelProviderSelectForNextTurnResponse { selected_model_id })
    }

    pub fn model_list(
        &self,
        _params: ModelListParams,
    ) -> Result<ModelListResponse, AppServerError> {
        self.require_initialized("model_provider")?;
        self.model_provider.model_list_response()
    }

    pub fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.read_file(params)
    }

    pub fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.write_file(params)
    }

    pub fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.create_directory(params)
    }

    pub fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.get_metadata(params)
    }

    pub fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.read_directory(params)
    }

    pub fn fs_remove(&self, params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.remove(params)
    }

    pub fn fs_copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.copy(params)
    }

    pub fn fs_watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.watch(params)
    }

    pub fn fs_unwatch(&self, params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        self.require_initialized("filesystem")?;
        self.app_services.filesystem.unwatch(params)
    }

    pub fn command_exec(
        &self,
        params: CommandExecParams,
    ) -> Result<CommandExecResponse, AppServerError> {
        self.require_initialized("command_exec")?;
        self.app_services.command.exec(params)
    }

    pub fn command_exec_write(
        &self,
        params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError> {
        self.require_initialized("command_exec")?;
        self.app_services.command.write(params)
    }

    pub fn command_exec_terminate(
        &self,
        params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError> {
        self.require_initialized("command_exec")?;
        self.app_services.command.terminate(params)
    }

    pub fn command_exec_resize(
        &self,
        params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError> {
        self.require_initialized("command_exec")?;
        self.app_services.command.resize(params)
    }

    pub fn jobs_list(&self, params: JobListParams) -> Result<JobListResponse, AppServerError> {
        self.require_initialized("jobs")?;
        self.app_services.jobs.list(params)
    }

    pub fn jobs_read(&self, params: JobReadParams) -> Result<JobReadResponse, AppServerError> {
        self.require_initialized("jobs")?;
        self.app_services.jobs.read(params)
    }

    pub fn skills_list(
        &self,
        params: SkillsListParams,
    ) -> Result<SkillsListResponse, AppServerError> {
        self.require_initialized("skills")?;
        self.app_services.skills.list(params)
    }

    pub fn skills_config_write(
        &mut self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError> {
        self.require_initialized("skills")?;
        let response = self.app_services.skills.write_config(params)?;
        self.notifications
            .emit_skills_changed(SkillsChangedNotification {});
        Ok(response)
    }

    pub fn mcp_server_status_list(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError> {
        self.require_initialized("mcp")?;
        self.app_services.mcp.list_status(params)
    }

    pub fn mcp_server_reload(
        &mut self,
        params: McpServerReloadParams,
    ) -> Result<McpServerReloadResponse, AppServerError> {
        self.require_initialized("mcp")?;
        let requested = params.name.clone();
        match self.app_services.mcp.reload(params) {
            Ok(response) => {
                for name in &response.reloaded {
                    self.notifications.emit_mcp_startup_status_updated(
                        McpServerStatusUpdatedNotification {
                            name: name.clone(),
                            status: McpServerStartupState::Ready,
                            error: None,
                        },
                    );
                }
                Ok(response)
            }
            Err(error) => {
                if let Some(name) = requested {
                    self.notifications.emit_mcp_startup_status_updated(
                        McpServerStatusUpdatedNotification {
                            name,
                            status: McpServerStartupState::Failed,
                            error: Some(error.public_message().to_string()),
                        },
                    );
                }
                Err(error)
            }
        }
    }

    pub fn mcp_server_tool_call(
        &self,
        params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError> {
        self.require_initialized("mcp")?;
        self.app_services.mcp.call_tool(params)
    }

    pub fn mcp_server_resource_read(
        &self,
        params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError> {
        self.require_initialized("mcp")?;
        self.app_services.mcp.read_resource(params)
    }

    pub fn mcp_server_oauth_login(
        &mut self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError> {
        self.require_initialized("mcp")?;
        let name = params.name.clone();
        match self.app_services.mcp.oauth_login(params) {
            Ok(response) => {
                self.notifications.emit_mcp_oauth_login_completed(
                    McpServerOauthLoginCompletedNotification {
                        name,
                        success: true,
                        error: None,
                    },
                );
                Ok(response)
            }
            Err(error) => {
                self.notifications.emit_mcp_oauth_login_completed(
                    McpServerOauthLoginCompletedNotification {
                        name,
                        success: false,
                        error: Some(error.public_message().to_string()),
                    },
                );
                Err(error)
            }
        }
    }

    fn cancel_turn_record(&mut self, params: TurnInterruptParams) -> Result<(), AppServerError> {
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
            .cancel_turn(&params.thread_id, &params.turn_id)?
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "session",
                    format!("unknown turn id: {}", params.turn_id),
                )
            })?;
        if changed {
            let pending_approvals = self
                .pending_server_requests
                .drain_turn(&params.thread_id, &params.turn_id);
            self.emit_failed_pending_server_request_resolutions(
                pending_approvals,
                "turn cancelled",
            );
            self.emit_codex_item_completed(params.thread_id.clone(), params.turn_id.clone(), None);
            let turn = self.codex_turn_view(&params.thread_id, &params.turn_id)?;
            self.notifications.emit_turn_completed(TurnCompletedEvent {
                thread_id: params.thread_id.clone(),
                turn,
            });
            self.transition_ready_if_no_pending_turns();
        }

        Ok(())
    }

    #[cfg(test)]
    fn interrupt_turn_for_test(
        &mut self,
        params: TestTurnInterruptParams,
    ) -> Result<TestTurnInterruptResult, AppServerError> {
        self.cancel_turn_record(TurnInterruptParams {
            thread_id: params.thread_id,
            turn_id: params.turn_id,
        })?;
        Ok(TestTurnInterruptResult {
            accepted: true,
            status: TurnStatus::Cancelled,
            lifecycle: self.lifecycle.clone(),
        })
    }

    pub fn turn_interrupt(
        &mut self,
        params: TurnInterruptParams,
    ) -> Result<TurnInterruptResponse, AppServerError> {
        self.cancel_turn_record(params)?;
        Ok(TurnInterruptResponse {})
    }

    pub fn thread_turns_list(
        &mut self,
        params: ThreadTurnsListParams,
    ) -> Result<ThreadTurnsListResponse, AppServerError> {
        self.require_initialized("session")?;
        self.drain_runtime_turn_updates();
        self.require_thread_exists(&params.thread_id)?;
        Ok(ThreadTurnsListResponse {
            data: self
                .threads
                .list_turns(&params.thread_id)
                .into_iter()
                .map(codex_turn_from_summary)
                .collect(),
            next_cursor: None,
            backwards_cursor: None,
        })
    }

    #[cfg(test)]
    fn list_turns_for_test(
        &mut self,
        params: TestTurnsSnapshotParams,
    ) -> Result<TestTurnsSnapshot, AppServerError> {
        self.require_initialized("session")?;
        self.drain_runtime_turn_updates();
        self.require_thread_exists(&params.thread_id)?;
        Ok(TestTurnsSnapshot {
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
            turn: self.codex_turn_view(&params.thread_id, &params.turn_id)?,
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
        let cancelled_turns = match self.threads.cancel_pending_turns() {
            Ok(turns) => turns,
            Err(error) => {
                let message = error.public_message().to_string();
                self.notifications.emit_error(ErrorEvent {
                    code: ErrorCode::ServiceDegraded,
                    message: message.clone(),
                    thread_id: None,
                    turn_id: None,
                    retryable: true,
                });
                self.transition_lifecycle(
                    LifecycleState::Degraded,
                    LifecycleReason::InternalError,
                    Some(format!("shutdown rejected: {message}")),
                );
                return ShutdownResponse {
                    accepted: false,
                    lifecycle: self.lifecycle.clone(),
                };
            }
        };
        self.runtime_bridge.shutdown();
        let pending_approvals = self.pending_server_requests.drain_all();
        self.emit_failed_pending_server_request_resolutions(pending_approvals, "server shutdown");
        for turn in cancelled_turns {
            self.emit_codex_item_completed(turn.thread_id.clone(), turn.turn_id.clone(), None);
            self.notifications.emit_turn_completed(TurnCompletedEvent {
                thread_id: turn.thread_id.clone(),
                turn: codex_turn_from_summary(turn.clone()),
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
        self.drain_service_updates();
        self.drain_runtime_turn_updates();
        self.expire_pending_server_requests();
        self.notifications.drain_with_policy()
    }

    pub fn drain_notifications(&mut self) -> Vec<ServerNotification> {
        self.drain_notifications_with_policy().notifications
    }

    pub fn drain_json_rpc_notifications_with_policy(&mut self) -> JsonRpcNotificationDrain {
        self.drain_service_updates();
        self.drain_runtime_turn_updates();
        self.expire_pending_server_requests();
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
        self.drain_runtime_turn_updates();
        self.expire_pending_server_requests();
        let response = match serde_json::from_str::<Value>(input) {
            Ok(Value::Array(_)) => Some(invalid_request_response(
                None,
                "JSON-RPC batch requests are not supported in Phase 1".to_string(),
            )),
            Ok(value) => {
                let id = value.get("id").cloned();
                let is_notification = id.is_none();
                if value.get("method").is_some() {
                    match serde_json::from_value::<JsonRpcRequest>(value) {
                        Ok(request) => self.route_json_rpc(request),
                        Err(error) => response_for_request(
                            invalid_request_response(id, error.to_string()),
                            is_notification,
                        ),
                    }
                } else if value.get("result").is_some() || value.get("error").is_some() {
                    match serde_json::from_value::<JsonRpcClientResponse>(value) {
                        Ok(response) => {
                            self.handle_client_response(response);
                            None
                        }
                        Err(error) => response_for_request(
                            invalid_request_response(id, error.to_string()),
                            is_notification,
                        ),
                    }
                } else {
                    match serde_json::from_value::<JsonRpcRequest>(value) {
                        Ok(request) => self.route_json_rpc(request),
                        Err(error) => response_for_request(
                            invalid_request_response(id, error.to_string()),
                            is_notification,
                        ),
                    }
                }
            }
            Err(error) => Some(parse_error_response(error.to_string())),
        };

        response.map(|response| serialize_response(&response))
    }

    fn try_spawn_detached_command_exec(
        &self,
        input: &str,
        response_sender: mpsc::Sender<DetachedResponse>,
    ) -> DetachedCommandExecDispatch {
        let incoming = match serde_json::from_str::<JsonRpcIncoming>(input) {
            Ok(JsonRpcIncoming::Request(request)) => request,
            Ok(JsonRpcIncoming::ClientResponse(_)) | Err(_) => {
                return DetachedCommandExecDispatch::NotDetached;
            }
        };
        if incoming.method != method::COMMAND_EXEC {
            return DetachedCommandExecDispatch::NotDetached;
        }
        if incoming.jsonrpc != JSON_RPC_VERSION {
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

        let is_notification = incoming.id.is_none();
        let process_id = params.process_id.clone();
        if let Err(error) = self.require_initialized("command_exec") {
            if is_notification {
                return DetachedCommandExecDispatch::NotDetached;
            }
            return DetachedCommandExecDispatch::ImmediateResponse(serialize_response(
                &app_error_response(incoming.id, error),
            ));
        }

        let id = incoming.id;
        let command = Arc::clone(&self.app_services.command);
        thread::spawn(move || {
            let response = match command.exec(params) {
                Ok(result) => json_rpc_ok(id.clone(), result),
                Err(error) => app_error_response(id.clone(), error),
            };
            let _ = if is_notification {
                response_sender.send(DetachedResponse::Completed)
            } else {
                response_sender.send(DetachedResponse::Response {
                    json: serialize_response(&response),
                })
            };
        });
        DetachedCommandExecDispatch::Spawned { process_id }
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
            method::CONFIG_REQUIREMENTS_READ => {
                route_with_no_params_result(request.id, request.params, || {
                    self.config_requirements_read()
                })
            }
            method::CONFIG_READ => {
                route_with_params(request.id, request.params, |params: ConfigReadParams| {
                    self.config_read(params)
                })
            }
            method::CONFIG_VALUE_WRITE => route_with_params(
                request.id,
                request.params,
                |params: ConfigValueWriteParams| self.config_value_write(params),
            ),
            method::CONFIG_BATCH_WRITE => route_with_params(
                request.id,
                request.params,
                |params: ConfigBatchWriteParams| self.config_batch_write(params),
            ),
            method::GIT_DIFF_TO_REMOTE => route_with_params(
                request.id,
                request.params,
                |params: GitDiffToRemoteParams| self.git_diff_to_remote(params),
            ),
            method::FUZZY_FILE_SEARCH => route_with_params(
                request.id,
                request.params,
                |params: FuzzyFileSearchParams| self.fuzzy_file_search(params),
            ),
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
            method::GET_CONVERSATION_SUMMARY => route_with_params(
                request.id,
                request.params,
                |params: GetConversationSummaryParams| self.get_conversation_summary(params),
            ),
            method::REVIEW_START => {
                route_with_params(request.id, request.params, |params: ReviewStartParams| {
                    self.review_start(params)
                })
            }
            method::THREAD_START => {
                route_with_params(request.id, request.params, |params: ThreadStartParams| {
                    self.thread_start(params)
                })
            }
            method::THREAD_LIST => {
                let params = request
                    .params
                    .unwrap_or_else(|| Value::Object(Default::default()));
                route_with_params(request.id, Some(params), |params| self.thread_list(params))
            }
            method::THREAD_READ => {
                route_with_params(request.id, request.params, |params: ThreadReadParams| {
                    self.thread_read(params)
                })
            }
            method::THREAD_RESUME => {
                route_with_params(request.id, request.params, |params: ThreadResumeParams| {
                    self.thread_resume(params)
                })
            }
            method::THREAD_FORK => {
                route_with_params(request.id, request.params, |params: ThreadForkParams| {
                    self.thread_fork(params)
                })
            }
            method::THREAD_ARCHIVE => {
                route_with_params(request.id, request.params, |params: ThreadArchiveParams| {
                    self.thread_archive(params)
                })
            }
            method::THREAD_UNARCHIVE => route_with_params(
                request.id,
                request.params,
                |params: ThreadUnarchiveParams| self.thread_unarchive(params),
            ),
            method::THREAD_UNSUBSCRIBE => route_with_params(
                request.id,
                request.params,
                |params: ThreadUnsubscribeParams| self.thread_unsubscribe(params),
            ),
            method::THREAD_NAME_SET => {
                route_with_params(request.id, request.params, |params: ThreadSetNameParams| {
                    self.thread_name_set(params)
                })
            }
            method::THREAD_METADATA_UPDATE => {
                route_with_params(request.id, request.params, |params: Value| {
                    self.thread_metadata_update_value(params)
                })
            }
            method::THREAD_ROLLBACK => route_with_params(
                request.id,
                request.params,
                |params: ThreadRollbackParams| self.thread_rollback(params),
            ),
            method::THREAD_LOADED_LIST => route_with_optional_params(
                request.id,
                request.params,
                |params: ThreadLoadedListParams| self.thread_loaded_list(params),
            ),
            method::THREAD_INJECT_ITEMS => route_with_params(
                request.id,
                request.params,
                |params: ThreadInjectItemsParams| self.thread_inject_items(params),
            ),
            method::THREAD_GOAL_SET => {
                route_with_params(request.id, request.params, |params: ThreadGoalSetParams| {
                    self.thread_goal_set(params)
                })
            }
            method::THREAD_GOAL_GET => {
                route_with_params(request.id, request.params, |params: ThreadGoalGetParams| {
                    self.thread_goal_get(params)
                })
            }
            method::THREAD_GOAL_CLEAR => route_with_params(
                request.id,
                request.params,
                |params: ThreadGoalClearParams| self.thread_goal_clear(params),
            ),
            method::THREAD_COMPACT_START => route_with_params(
                request.id,
                request.params,
                |params: ThreadCompactStartParams| self.thread_compact_start(params),
            ),
            method::TURN_START => {
                route_with_params(request.id, request.params, |params: TurnStartParams| {
                    self.turn_start(params)
                })
            }
            method::TURN_STEER => {
                route_with_params(request.id, request.params, |params: TurnSteerParams| {
                    self.turn_steer(params)
                })
            }
            method::TURN_INTERRUPT => {
                route_with_params(request.id, request.params, |params: TurnInterruptParams| {
                    self.turn_interrupt(params)
                })
            }
            method::THREAD_TURNS_LIST => route_with_params(
                request.id,
                request.params,
                |params: ThreadTurnsListParams| self.thread_turns_list(params),
            ),
            method::TURN_READ => {
                route_with_params(request.id, request.params, |params: TurnReadParams| {
                    self.turn_read(params)
                })
            }
            method::MODEL_LIST => {
                route_with_optional_params(request.id, request.params, |params| {
                    self.model_list(params)
                })
            }
            method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN => route_with_params(
                request.id,
                request.params,
                |params: ModelProviderSelectForNextTurnParams| {
                    self.model_provider_select_for_next_turn(params)
                },
            ),
            method::JOBS_LIST => {
                route_with_optional_params(request.id, request.params, |params: JobListParams| {
                    self.jobs_list(params)
                })
            }
            method::JOBS_READ => {
                route_with_params(request.id, request.params, |params: JobReadParams| {
                    self.jobs_read(params)
                })
            }
            method::SKILLS_LIST => route_with_optional_params(
                request.id,
                request.params,
                |params: SkillsListParams| self.skills_list(params),
            ),
            method::SKILLS_CONFIG_WRITE => route_with_params(
                request.id,
                request.params,
                |params: SkillsConfigWriteParams| self.skills_config_write(params),
            ),
            method::MCP_SERVER_STATUS_LIST => route_with_optional_params(
                request.id,
                request.params,
                |params: ListMcpServerStatusParams| self.mcp_server_status_list(params),
            ),
            method::CONFIG_MCP_SERVER_RELOAD => route_with_optional_params(
                request.id,
                request.params,
                |params: McpServerReloadParams| self.mcp_server_reload(params),
            ),
            method::MCP_SERVER_TOOL_CALL => route_with_params(
                request.id,
                request.params,
                |params: McpServerToolCallParams| self.mcp_server_tool_call(params),
            ),
            method::MCP_SERVER_RESOURCE_READ => route_with_params(
                request.id,
                request.params,
                |params: McpResourceReadParams| self.mcp_server_resource_read(params),
            ),
            method::MCP_SERVER_OAUTH_LOGIN => route_with_params(
                request.id,
                request.params,
                |params: McpServerOauthLoginParams| self.mcp_server_oauth_login(params),
            ),
            method::FS_READ_FILE => {
                route_with_params(request.id, request.params, |params: FsReadFileParams| {
                    self.fs_read_file(params)
                })
            }
            method::FS_WRITE_FILE => {
                route_with_params(request.id, request.params, |params: FsWriteFileParams| {
                    self.fs_write_file(params)
                })
            }
            method::FS_CREATE_DIRECTORY => route_with_params(
                request.id,
                request.params,
                |params: FsCreateDirectoryParams| self.fs_create_directory(params),
            ),
            method::FS_GET_METADATA => {
                route_with_params(request.id, request.params, |params: FsGetMetadataParams| {
                    self.fs_get_metadata(params)
                })
            }
            method::FS_READ_DIRECTORY => route_with_params(
                request.id,
                request.params,
                |params: FsReadDirectoryParams| self.fs_read_directory(params),
            ),
            method::FS_REMOVE => {
                route_with_params(request.id, request.params, |params: FsRemoveParams| {
                    self.fs_remove(params)
                })
            }
            method::FS_COPY => {
                route_with_params(request.id, request.params, |params: FsCopyParams| {
                    self.fs_copy(params)
                })
            }
            method::FS_WATCH => {
                route_with_params(request.id, request.params, |params: FsWatchParams| {
                    self.fs_watch(params)
                })
            }
            method::FS_UNWATCH => {
                route_with_params(request.id, request.params, |params: FsUnwatchParams| {
                    self.fs_unwatch(params)
                })
            }
            method::COMMAND_EXEC => {
                route_with_params(request.id, request.params, |params: CommandExecParams| {
                    self.command_exec(params)
                })
            }
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
            method::APPROVAL_RESPOND => {
                let id = request.id.clone();
                route_with_params(
                    request.id,
                    request.params,
                    |params: ApprovalResponsePayload| {
                        self.apply_approval_response_id(id.unwrap_or(Value::Null), params)
                    },
                )
            }
            _ => method_not_found_response(request.id, request.method),
        };

        response_for_request(response, is_notification)
    }

    fn handle_client_response(&mut self, response: JsonRpcClientResponse) {
        if response.jsonrpc != JSON_RPC_VERSION {
            self.fail_pending_server_request_response(
                response.id,
                "jsonrpc must be \"2.0\"".to_string(),
            );
            return;
        }

        if let Some(error) = response.error {
            self.fail_pending_server_request_response(response.id, error.message);
            return;
        }

        let Some(result) = response.result else {
            self.fail_pending_server_request_response(response.id, "missing result".to_string());
            return;
        };

        let _ = self.apply_server_request_response_id(response.id, result);
    }

    fn apply_approval_response_id(
        &mut self,
        response_id: Value,
        payload: ApprovalResponsePayload,
    ) -> Result<(), AppServerError> {
        let result = serde_json::to_value(payload).map_err(|error| {
            AppServerError::invalid_request(
                method::APPROVAL_RESPOND,
                format!("failed to encode approval response: {error}"),
            )
        })?;
        self.apply_server_request_response_id(response_id, result)
    }

    fn apply_server_request_response_id(
        &mut self,
        response_id: Value,
        result: Value,
    ) -> Result<(), AppServerError> {
        let Some(pending) = self.pending_server_requests.remove(&response_id) else {
            self.emit_failed_server_request_resolution(
                response_id,
                "unknown or expired server request".to_string(),
            );
            return Ok(());
        };

        match decode_server_request_response(pending.kind, result) {
            Ok(payload) => self.apply_server_request_resolution(pending, payload),
            Err(reason) => self.fail_pending_server_request(pending, reason),
        }
        Ok(())
    }

    fn apply_server_request_resolution(
        &mut self,
        pending: PendingServerRequest,
        payload: RuntimeServerRequestResponse,
    ) {
        let (success_outcome, resolution_reason) = server_request_resolution_outcome(&payload);

        let result = self
            .runtime_bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: pending.runtime_request_id.clone(),
                payload,
            });

        match result {
            Ok(()) => {
                self.notifications
                    .emit_server_request_resolved(ServerRequestResolvedEvent {
                        request_id: pending.request_id,
                        thread_id: Some(pending.thread_id),
                        turn_id: Some(pending.turn_id),
                        outcome: success_outcome,
                        reason: resolution_reason,
                    });
                if self.threads.has_pending_turns() {
                    self.transition_lifecycle(
                        LifecycleState::Running,
                        LifecycleReason::RequestInProgress,
                        Some("approval resolved; turn request is running".to_string()),
                    );
                }
            }
            Err(error) => {
                let thread_id = pending.thread_id.clone();
                let turn_id = pending.turn_id.clone();
                let message = error.message;
                self.notifications
                    .emit_server_request_resolved(ServerRequestResolvedEvent {
                        request_id: pending.request_id,
                        thread_id: Some(thread_id.clone()),
                        turn_id: Some(turn_id.clone()),
                        outcome: ServerRequestResolutionOutcome::Failed,
                        reason: Some(message.clone()),
                    });
                self.fail_pending_turn(thread_id, turn_id, message);
            }
        }
    }

    fn fail_pending_server_request_response(&mut self, response_id: Value, reason: String) {
        let Some(pending) = self.pending_server_requests.remove(&response_id) else {
            self.emit_failed_server_request_resolution(response_id, reason);
            return;
        };

        self.fail_pending_server_request(pending, reason);
    }

    fn fail_pending_server_request(&mut self, pending: PendingServerRequest, reason: String) {
        let runtime_reason = format!("server request response failed: {reason}");
        self.resolve_failed_runtime_server_request(&pending, &reason);
        self.notifications
            .emit_server_request_resolved(ServerRequestResolvedEvent {
                request_id: pending.request_id,
                thread_id: Some(pending.thread_id.clone()),
                turn_id: Some(pending.turn_id.clone()),
                outcome: ServerRequestResolutionOutcome::Failed,
                reason: Some(reason),
            });
        self.fail_pending_turn(pending.thread_id, pending.turn_id, runtime_reason);
    }

    fn resolve_failed_runtime_server_request(&self, pending: &PendingServerRequest, reason: &str) {
        if pending.kind == PendingServerRequestKind::CommandApproval {
            let _ = self
                .runtime_bridge
                .resolve_approval(RuntimeApprovalDecision {
                    request_id: pending.runtime_request_id.clone(),
                    decision: dasclaw_runtime::ApprovalDecision::Reject {
                        reason: Some(format!("approval response failed: {reason}")),
                    },
                });
            return;
        }

        let payload = failed_runtime_server_request_response(pending.kind, reason);
        let _ = self
            .runtime_bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: pending.runtime_request_id.clone(),
                payload,
            });
    }

    fn fail_pending_turn(&mut self, thread_id: String, turn_id: String, error: String) {
        let update = RuntimeTurnUpdate {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            outcome: RuntimeTurnOutcome::Failed { error },
        };
        let summary = match self.threads.apply_runtime_turn_update(update) {
            Ok(Some(summary)) => summary,
            Ok(None) => return,
            Err(error) => {
                self.emit_codex_error(thread_id, turn_id, error.public_message().to_string());
                return;
            }
        };
        if summary.status != TurnStatus::Failed {
            return;
        }

        let error = summary.error.clone().unwrap_or_default();
        self.emit_codex_item_completed(summary.thread_id.clone(), summary.turn_id.clone(), None);
        self.notifications.emit_turn_completed(TurnCompletedEvent {
            thread_id: summary.thread_id.clone(),
            turn: codex_turn_from_summary(summary.clone()),
        });
        self.emit_codex_error(summary.thread_id, summary.turn_id, error);
        self.transition_ready_if_no_pending_turns();
    }

    fn emit_failed_server_request_resolution(&mut self, response_id: Value, reason: String) {
        let request_id = json_rpc_id_to_request_id(&response_id);
        self.notifications
            .emit_server_request_resolved(ServerRequestResolvedEvent {
                request_id,
                thread_id: None,
                turn_id: None,
                outcome: ServerRequestResolutionOutcome::Failed,
                reason: Some(reason),
            });
    }

    fn emit_failed_pending_server_request_resolutions(
        &mut self,
        pending: Vec<PendingServerRequest>,
        reason: &str,
    ) {
        for request in pending {
            self.notifications
                .emit_server_request_resolved(ServerRequestResolvedEvent {
                    request_id: request.request_id,
                    thread_id: Some(request.thread_id),
                    turn_id: Some(request.turn_id),
                    outcome: ServerRequestResolutionOutcome::Failed,
                    reason: Some(reason.to_string()),
                });
        }
    }

    fn emit_approval_server_request(
        &mut self,
        thread_id: String,
        turn_id: String,
        request: RuntimeApprovalRequest,
    ) {
        if !self.threads.turn_is_pending(&thread_id, &turn_id) {
            return;
        }

        let request_id = format!("approval_{}", request.request_id);
        let runtime_request_id = request.request_id.clone();
        let item_id = format!("{turn_id}:tool:{}", request.tool_call_id);
        let server_request = match JsonRpcServerRequest::new(
            request_id.clone(),
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
            CommandExecutionApprovalRequest {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                item_id,
                tool_call_id: request.tool_call_id,
                tool_name: request.tool_name,
                command: request.command,
                description: request.description,
                display_parameters: request.display_parameters,
                allow_always: request.allow_always,
            },
        ) {
            Ok(request) => request,
            Err(error) => {
                let message = format!("failed to serialize approval server request: {error}");
                let _ = self
                    .runtime_bridge
                    .resolve_approval(RuntimeApprovalDecision {
                        request_id: runtime_request_id,
                        decision: dasclaw_runtime::ApprovalDecision::Reject {
                            reason: Some(message.clone()),
                        },
                    });
                self.fail_pending_turn(thread_id, turn_id, message);
                return;
            }
        };

        self.pending_server_requests.insert(PendingServerRequest {
            request_id: request_id.clone(),
            runtime_request_id,
            kind: PendingServerRequestKind::CommandApproval,
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            created_at: Instant::now(),
        });
        self.transition_lifecycle(
            LifecycleState::AwaitingApproval,
            LifecycleReason::ApprovalPending,
            Some("turn request is awaiting approval".to_string()),
        );
        self.notifications.emit_server_request(server_request);
    }

    fn emit_dynamic_tool_call_server_request(
        &mut self,
        thread_id: String,
        turn_id: String,
        request: RuntimeDynamicToolCallRequest,
    ) {
        if !self.threads.turn_is_pending(&thread_id, &turn_id) {
            return;
        }

        let request_id = request.request_id.clone();
        let runtime_request_id = request.request_id.clone();
        let server_request = match JsonRpcServerRequest::new(
            request_id.clone(),
            server_request::ITEM_TOOL_CALL,
            DynamicToolCallParams {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                call_id: request.call_id,
                namespace: request.namespace,
                tool: request.tool,
                arguments: request.arguments,
            },
        ) {
            Ok(request) => request,
            Err(error) => {
                let message =
                    format!("failed to serialize dynamic tool call server request: {error}");
                self.fail_pending_turn(thread_id, turn_id, message);
                return;
            }
        };

        self.pending_server_requests.insert(PendingServerRequest {
            request_id: request_id.clone(),
            runtime_request_id,
            kind: PendingServerRequestKind::DynamicToolCall,
            thread_id,
            turn_id,
            created_at: Instant::now(),
        });
        self.notifications.emit_server_request(server_request);
    }

    fn emit_tool_user_input_server_request(
        &mut self,
        thread_id: String,
        turn_id: String,
        request: RuntimeToolUserInputRequest,
    ) {
        if !self.threads.turn_is_pending(&thread_id, &turn_id) {
            return;
        }

        let request_id = request.request_id.clone();
        let runtime_request_id = request.request_id.clone();
        let server_request = match JsonRpcServerRequest::new(
            request_id.clone(),
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            ToolRequestUserInputParams {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                item_id: request.item_id,
                questions: request.questions,
            },
        ) {
            Ok(request) => request,
            Err(error) => {
                let message =
                    format!("failed to serialize tool user input server request: {error}");
                self.fail_pending_turn(thread_id, turn_id, message);
                return;
            }
        };

        self.pending_server_requests.insert(PendingServerRequest {
            request_id: request_id.clone(),
            runtime_request_id,
            kind: PendingServerRequestKind::ToolUserInput,
            thread_id,
            turn_id,
            created_at: Instant::now(),
        });
        self.notifications.emit_server_request(server_request);
    }

    fn emit_file_change_approval_server_request(
        &mut self,
        thread_id: String,
        turn_id: String,
        request: RuntimeFileChangeApprovalRequest,
    ) {
        if !self.threads.turn_is_pending(&thread_id, &turn_id) {
            return;
        }

        let request_id = request.request_id.clone();
        let runtime_request_id = request.request_id.clone();
        let server_request = match JsonRpcServerRequest::new(
            request_id.clone(),
            server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
            FileChangeRequestApprovalParams {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                item_id: request.item_id,
                reason: request.reason,
                grant_root: request.grant_root,
            },
        ) {
            Ok(request) => request,
            Err(error) => {
                let message =
                    format!("failed to serialize file change approval server request: {error}");
                self.fail_pending_turn(thread_id, turn_id, message);
                return;
            }
        };

        self.pending_server_requests.insert(PendingServerRequest {
            request_id: request_id.clone(),
            runtime_request_id,
            kind: PendingServerRequestKind::FileChangeApproval,
            thread_id,
            turn_id,
            created_at: Instant::now(),
        });
        self.notifications.emit_server_request(server_request);
    }

    fn emit_permissions_approval_server_request(
        &mut self,
        thread_id: String,
        turn_id: String,
        request: RuntimePermissionsApprovalRequest,
    ) {
        if !self.threads.turn_is_pending(&thread_id, &turn_id) {
            return;
        }

        let request_id = request.request_id.clone();
        let runtime_request_id = request.request_id.clone();
        let server_request = match JsonRpcServerRequest::new(
            request_id.clone(),
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
            PermissionsRequestApprovalParams {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                item_id: request.item_id,
                cwd: request.cwd,
                reason: request.reason,
                permissions: request.permissions,
            },
        ) {
            Ok(request) => request,
            Err(error) => {
                let message =
                    format!("failed to serialize permissions approval server request: {error}");
                self.fail_pending_turn(thread_id, turn_id, message);
                return;
            }
        };

        self.pending_server_requests.insert(PendingServerRequest {
            request_id: request_id.clone(),
            runtime_request_id,
            kind: PendingServerRequestKind::PermissionsApproval,
            thread_id,
            turn_id,
            created_at: Instant::now(),
        });
        self.notifications.emit_server_request(server_request);
    }

    fn expire_pending_server_requests(&mut self) {
        let expired = self.pending_server_requests.expired(SERVER_REQUEST_TIMEOUT);
        for pending in expired {
            let thread_id = pending.thread_id.clone();
            let turn_id = pending.turn_id.clone();
            if pending.kind == PendingServerRequestKind::CommandApproval {
                let _ = self
                    .runtime_bridge
                    .resolve_approval(RuntimeApprovalDecision {
                        request_id: pending.runtime_request_id.clone(),
                        decision: dasclaw_runtime::ApprovalDecision::Reject {
                            reason: Some("approval request timed out".to_string()),
                        },
                    });
            } else {
                self.resolve_failed_runtime_server_request(&pending, "server request timed out");
            }
            self.notifications
                .emit_server_request_resolved(ServerRequestResolvedEvent {
                    request_id: pending.request_id,
                    thread_id: Some(thread_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    outcome: ServerRequestResolutionOutcome::TimedOut,
                    reason: Some("server request timed out".to_string()),
                });
            self.fail_pending_turn(thread_id, turn_id, "approval request timed out".to_string());
        }
    }

    #[cfg(test)]
    fn expire_pending_server_requests_for_tests(&mut self, age: Duration) {
        self.pending_server_requests.age_all(age);
        self.expire_pending_server_requests();
    }

    #[cfg(test)]
    fn insert_pending_server_request_for_test(
        &mut self,
        request_id: &str,
        kind: PendingServerRequestKind,
        runtime_request_id: &str,
        thread_id: &str,
        turn_id: &str,
    ) {
        self.pending_server_requests.insert(PendingServerRequest {
            request_id: request_id.to_string(),
            runtime_request_id: runtime_request_id.to_string(),
            kind,
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            created_at: Instant::now(),
        });
    }

    fn service_health(&self) -> Vec<ServiceHealth> {
        let mut services = vec![
            ServiceHealth::ready(ServiceName::Protocol),
            ServiceHealth::ready(ServiceName::Lifecycle),
            ServiceHealth::ready(ServiceName::Session),
            ServiceHealth::degraded(
                ServiceName::Runtime,
                "runtime bridge boundary is available; default host requires a runtime adapter",
            ),
            ServiceHealth::unavailable_fail_safe(
                ServiceName::DlpPolicy,
                "DLP/policy service is declared but not migrated in Phase 1",
            ),
            self.model_provider.health(),
            self.tools_health(),
            self.sandbox_health(),
        ];
        services.extend(self.app_services.health());
        services
    }

    fn drain_service_updates(&mut self) {
        for entry in self.app_services.drain_log_entries() {
            self.notifications.emit_log_entry(entry);
        }
        for event in self.app_services.drain_mcp_tool_call_progress_events() {
            self.notifications.emit_mcp_tool_call_progress(event);
        }
        for event in self.app_services.drain_fs_changed_events() {
            self.notifications.emit_fs_changed(event);
        }
        for event in self.app_services.drain_command_exec_output_delta_events() {
            self.notifications.emit_command_exec_output_delta(event);
        }
        for event in self.app_services.drain_search_events() {
            match event {
                SearchNotification::Updated(event) => {
                    self.notifications
                        .emit_fuzzy_file_search_session_updated(event);
                }
                SearchNotification::Completed(event) => {
                    self.notifications
                        .emit_fuzzy_file_search_session_completed(event);
                }
            }
        }
        for event in self.app_services.drain_hook_notifications() {
            match event {
                AppServerHookNotification::Started(event) => {
                    self.notifications.emit_hook_started(event);
                }
                AppServerHookNotification::Completed(event) => {
                    self.notifications.emit_hook_completed(event);
                }
                AppServerHookNotification::Warning(event) => {
                    self.notifications.emit_warning(event);
                }
                AppServerHookNotification::GuardianWarning(event) => {
                    self.notifications.emit_guardian_warning(event);
                }
                AppServerHookNotification::ConfigWarning(event) => {
                    self.notifications.emit_config_warning(event);
                }
                AppServerHookNotification::DeprecationNotice(event) => {
                    self.notifications.emit_deprecation_notice(event);
                }
            }
        }
    }

    fn refresh_service_capabilities(&mut self) {
        self.capabilities = self.service_capability_snapshot();
    }

    fn service_capability_snapshot(&self) -> CapabilityMatrix {
        let mut capabilities = self.capabilities.clone();
        let service_baseline = CapabilityMatrix::phase_one();
        capabilities.logs = service_baseline.logs;
        capabilities.jobs = service_baseline.jobs;
        capabilities.skills = service_baseline.skills;
        capabilities.mcp = service_baseline.mcp;
        let mut availability = self.app_services.availability();
        availability.r6.review = true;
        let (model_reroutes, model_verifications) =
            self.runtime_features.real_model_r6_producer_availability();
        availability.r6.model_reroutes = model_reroutes;
        availability.r6.model_verifications = model_verifications;
        availability.r6.warnings = generic_warning_producer_available();
        availability.r6.guardian_warnings = self.runtime_features.auto_approval_review;
        capabilities.with_app_services(availability)
    }

    fn tools_health(&self) -> ServiceHealth {
        if self.runtime_features.tools {
            ServiceHealth::ready(ServiceName::Tools)
        } else {
            ServiceHealth::disabled(ServiceName::Tools, "tool registry is not wired in Phase 1")
        }
    }

    fn sandbox_health(&self) -> ServiceHealth {
        if self.runtime_features.sandbox {
            ServiceHealth::ready(ServiceName::Sandbox)
        } else {
            ServiceHealth::disabled(
                ServiceName::Sandbox,
                "sandbox adapter is not wired in Phase 1",
            )
        }
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
        self.notifications
            .emit_notifications_initialized(NotificationsInitializedEvent {
                lifecycle: self.lifecycle.clone(),
                compatibility_profiles: vec![
                    CompatibilityProfile::codex_app_server_v2_chat_session_subset(),
                ],
                unavailable_requested_capabilities,
                event_queue: NotificationQueuePolicy::bounded_lag_disconnect(),
            });
    }

    fn unavailable_requested_capabilities(&self, requested: &[String]) -> Vec<String> {
        let mut unavailable = Vec::new();
        for capability in requested {
            if capability == CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID {
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
            "filesystem" => self.capabilities.filesystem.status,
            "command_exec" => self.capabilities.command_exec.status,
            "thread_lifecycle" => self.capabilities.thread_lifecycle.status,
            "thread_goal" => self.capabilities.thread_goal.status,
            "thread_compact" => self.capabilities.thread_compact.status,
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
            LifecycleState::Ready
                | LifecycleState::Running
                | LifecycleState::AwaitingApproval
                | LifecycleState::Degraded
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
        self.thread_summary_or_error_with_capability(thread_id, "session")
    }

    fn thread_summary_or_error_with_capability(
        &self,
        thread_id: &str,
        capability: &str,
    ) -> Result<ThreadSummary, AppServerError> {
        self.threads.summary(thread_id).ok_or_else(|| {
            AppServerError::invalid_request(capability, format!("unknown thread id: {thread_id}"))
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

    fn emit_codex_thread_started(&mut self, thread_id: String) -> Result<(), AppServerError> {
        let thread = self.codex_thread_view(&thread_id, false)?;
        self.notifications
            .emit_thread_started(ThreadStartedEvent { thread });
        Ok(())
    }

    fn emit_thread_status_changed(&mut self, thread_id: &str, status: CodexThreadStatus) {
        self.notifications
            .emit_thread_status_changed(ThreadStatusChangedEvent {
                thread_id: thread_id.to_string(),
                status,
            });
    }

    fn codex_thread_view(
        &self,
        thread_id: &str,
        include_turns: bool,
    ) -> Result<CodexThread, AppServerError> {
        let summary = self.thread_summary_or_error(thread_id)?;
        let turn_summaries = self.threads.list_turns(thread_id);
        let has_pending_turn = turn_summaries
            .iter()
            .any(|turn| turn.status == TurnStatus::Pending);
        let turns = if include_turns {
            turn_summaries
                .into_iter()
                .map(codex_turn_from_summary)
                .collect()
        } else {
            Vec::new()
        };

        Ok(CodexThread {
            id: summary.thread_id,
            forked_from_id: summary.forked_from_id,
            preview: summary.title.clone().unwrap_or_default(),
            ephemeral: summary.ephemeral,
            model_provider: self.model_provider.selected_provider_id(),
            created_at: summary.created_at,
            updated_at: summary.updated_at,
            status: if has_pending_turn {
                CodexThreadStatus::Active {
                    active_flags: Vec::new(),
                }
            } else {
                CodexThreadStatus::Idle
            },
            path: summary.path,
            cwd: summary.workspace_root.unwrap_or_else(|| ".".to_string()),
            cli_version: SERVER_VERSION.to_string(),
            source: CodexSessionSource::AppServer,
            agent_nickname: None,
            agent_role: None,
            git_info: summary.git_info,
            name: summary.title,
            turns,
        })
    }

    fn conversation_summary_view(
        &self,
        summary: ThreadSummary,
    ) -> Result<ConversationSummary, AppServerError> {
        Ok(ConversationSummary {
            conversation_id: summary.thread_id.clone(),
            path: summary.path.unwrap_or_default(),
            preview: self.thread_preview(&summary.thread_id)?,
            timestamp: Some(summary.created_at.to_string()),
            updated_at: Some(summary.updated_at.to_string()),
            model_provider: self.model_provider.selected_provider_id(),
            cwd: summary.workspace_root.unwrap_or_else(|| ".".to_string()),
            cli_version: SERVER_VERSION.to_string(),
            source: CodexSessionSource::Unknown,
            git_info: summary.git_info,
        })
    }

    fn thread_preview(&self, thread_id: &str) -> Result<String, AppServerError> {
        Ok(self
            .thread_summary_or_error(thread_id)?
            .title
            .unwrap_or_default())
    }

    fn codex_turn_view(&self, thread_id: &str, turn_id: &str) -> Result<CodexTurn, AppServerError> {
        let summary = self.turn_summary_or_error(thread_id, turn_id)?;
        Ok(codex_turn_from_summary(summary))
    }

    fn emit_codex_item_started(&mut self, thread_id: String, turn_id: String) {
        self.notifications.emit_item_started(ItemStartedEvent {
            thread_id,
            turn_id: turn_id.clone(),
            item: CodexThreadItem::started_agent_message(turn_id),
        });
    }

    fn emit_codex_agent_message_delta(
        &mut self,
        thread_id: String,
        turn_id: String,
        delta: String,
    ) {
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
        text: Option<String>,
    ) {
        self.notifications.emit_item_completed(ItemCompletedEvent {
            thread_id,
            turn_id: turn_id.clone(),
            item: CodexThreadItem::completed_agent_message(turn_id, text.unwrap_or_default()),
        });
    }

    fn emit_codex_error(&mut self, thread_id: String, turn_id: String, message: String) {
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
        if self.threads.has_pending_turns()
            || !matches!(
                self.lifecycle.state,
                LifecycleState::Running | LifecycleState::AwaitingApproval
            )
        {
            return;
        }

        self.transition_lifecycle(
            LifecycleState::Ready,
            LifecycleReason::RuntimeReady,
            Some("all pending turns settled".to_string()),
        );
    }

    fn run_hook_lifecycle_event(
        &self,
        event: dasclaw_hooks::HookEvent,
    ) -> Result<Option<String>, AppServerError> {
        let Some(registry) = self.app_services.hook_registry.clone() else {
            return Ok(None);
        };
        let hook_point = event.hook_point();
        let runtime =
            blocking_runtime::BlockingTokioRuntime::new("dasclaw-app-server-hook", "hooks")?;
        runtime.block_on("run hook lifecycle event", async move {
            registry
                .run(&event)
                .await
                .map(|outcome| match outcome {
                    dasclaw_hooks::HookOutcome::Continue { modified } => modified,
                    dasclaw_hooks::HookOutcome::Reject { .. } => None,
                })
                .map_err(|error| {
                    hook_lifecycle_error_to_app_server_error(hook_point.as_str(), error)
                })
        })
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
                RuntimeTurnOutcome::ReasoningSummaryPartAdded { update: part } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_reasoning_summary_part_added(
                            ReasoningSummaryPartAddedEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                item_id: part.item_id,
                                summary_index: part.summary_index,
                            },
                        );
                    }
                }
                RuntimeTurnOutcome::ReasoningTextDelta { update: reasoning } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications
                            .emit_reasoning_text_delta(ReasoningTextDeltaEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                item_id: reasoning.item_id,
                                content_index: reasoning.content_index,
                                delta: reasoning.delta,
                            });
                    }
                }
                RuntimeTurnOutcome::PlanUpdated { update: plan } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications
                            .emit_turn_plan_updated(TurnPlanUpdatedEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                explanation: plan.explanation,
                                plan: plan.plan,
                            });
                    }
                }
                RuntimeTurnOutcome::DiffUpdated { update: diff } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications
                            .emit_turn_diff_updated(TurnDiffUpdatedEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                diff: diff.diff,
                            });
                    }
                }
                RuntimeTurnOutcome::RawResponseItemCompleted { update: raw } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_raw_response_item_completed(
                            RawResponseItemCompletedEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                item: raw.item,
                            },
                        );
                    }
                }
                RuntimeTurnOutcome::PlanDelta { update: delta } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_plan_delta(PlanDeltaEvent {
                            thread_id: update.thread_id,
                            turn_id: update.turn_id,
                            item_id: delta.item_id,
                            delta: delta.delta,
                        });
                    }
                }
                RuntimeTurnOutcome::ApprovalRequested { request } => {
                    self.emit_approval_server_request(update.thread_id, update.turn_id, request);
                }
                RuntimeTurnOutcome::DynamicToolCallRequested { request } => {
                    self.emit_dynamic_tool_call_server_request(
                        update.thread_id,
                        update.turn_id,
                        request,
                    );
                }
                RuntimeTurnOutcome::ToolUserInputRequested { request } => {
                    self.emit_tool_user_input_server_request(
                        update.thread_id,
                        update.turn_id,
                        request,
                    );
                }
                RuntimeTurnOutcome::FileChangeApprovalRequested { request } => {
                    self.emit_file_change_approval_server_request(
                        update.thread_id,
                        update.turn_id,
                        request,
                    );
                }
                RuntimeTurnOutcome::PermissionsApprovalRequested { request } => {
                    self.emit_permissions_approval_server_request(
                        update.thread_id,
                        update.turn_id,
                        request,
                    );
                }
                RuntimeTurnOutcome::ToolResult {
                    update: tool_result,
                } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications
                            .emit_command_execution_terminal_interaction(
                                CommandExecutionTerminalInteractionEvent {
                                    thread_id: update.thread_id,
                                    turn_id: update.turn_id,
                                    item_id: tool_result.item_id,
                                    message: tool_result.content,
                                    is_error: tool_result.is_error,
                                },
                            );
                    }
                }
                RuntimeTurnOutcome::CommandOutputDelta { update: output } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_command_execution_output_delta(
                            CommandExecutionOutputDeltaEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                item_id: output.item_id,
                                delta: output.delta,
                            },
                        );
                    }
                }
                RuntimeTurnOutcome::FileChangeOutputDelta { update: output } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_file_change_output_delta(
                            FileChangeOutputDeltaEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                item_id: output.item_id,
                                delta: output.delta,
                            },
                        );
                    }
                }
                RuntimeTurnOutcome::FileChangePatchUpdated { update: patch } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_file_change_patch_updated(
                            FileChangePatchUpdatedEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                item_id: patch.item_id,
                                changes: patch.changes,
                            },
                        );
                    }
                }
                RuntimeTurnOutcome::AutoApprovalReviewStarted { update: review } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_auto_approval_review_started(
                            AutoApprovalReviewStartedEvent {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                review_id: review.review_id,
                                target_item_id: review.target_item_id,
                                review: review.review,
                                action: review.action,
                            },
                        );
                    }
                }
                RuntimeTurnOutcome::AutoApprovalReviewCompleted { update: review } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications.emit_auto_approval_review_completed(
                            AutoApprovalReviewCompletedEvent {
                                thread_id: update.thread_id.clone(),
                                turn_id: update.turn_id.clone(),
                                review_id: review.review_id.clone(),
                                target_item_id: review.target_item_id.clone(),
                                review: review.review.clone(),
                                action: review.action.clone(),
                            },
                        );
                        if let Some(warning) =
                            guardian_warning_from_auto_approval_review(&update.thread_id, &review)
                        {
                            self.notifications.emit_guardian_warning(warning);
                        }
                    }
                }
                RuntimeTurnOutcome::ModelRerouted {
                    from_model,
                    to_model,
                    reason,
                } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications
                            .emit_model_rerouted(ModelReroutedNotification {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                from_model,
                                to_model,
                                reason,
                            });
                    }
                }
                RuntimeTurnOutcome::ModelVerification { verifications } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        self.notifications
                            .emit_model_verification(ModelVerificationNotification {
                                thread_id: update.thread_id,
                                turn_id: update.turn_id,
                                verifications,
                            });
                    }
                }
                RuntimeTurnOutcome::TokenUsageUpdated { usage } => {
                    if self
                        .threads
                        .turn_is_pending(&update.thread_id, &update.turn_id)
                    {
                        match self
                            .threads
                            .apply_token_usage(&update.thread_id, token_usage_breakdown(usage))
                        {
                            Ok(token_usage) => {
                                self.notifications.emit_thread_token_usage_updated(
                                    ThreadTokenUsageUpdatedEvent {
                                        thread_id: update.thread_id,
                                        turn_id: update.turn_id,
                                        token_usage,
                                    },
                                );
                            }
                            Err(error) => {
                                self.emit_codex_error(
                                    update.thread_id,
                                    update.turn_id,
                                    error.public_message().to_string(),
                                );
                            }
                        }
                    }
                }
                mut outcome => {
                    let thread_id = update.thread_id.clone();
                    let turn_id = update.turn_id.clone();
                    if let RuntimeTurnOutcome::Completed { output } = &mut outcome {
                        match self.run_hook_lifecycle_event(dasclaw_hooks::HookEvent::Outbound {
                            user_id: "app-server".to_string(),
                            channel: "chat".to_string(),
                            content: output.clone(),
                            thread_id: Some(thread_id.clone()),
                        }) {
                            Ok(Some(modified)) => {
                                *output = modified;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                self.drain_service_updates();
                                self.fail_pending_turn(
                                    thread_id,
                                    turn_id,
                                    format!(
                                        "outbound hook rejected turn output: {}",
                                        error.public_message()
                                    ),
                                );
                                continue;
                            }
                        }
                        self.drain_service_updates();
                    }
                    let update = RuntimeTurnUpdate { outcome, ..update };
                    match self.threads.apply_runtime_turn_update(update) {
                        Ok(Some(summary)) => {
                            terminal_update_applied = true;
                            match summary.status {
                                TurnStatus::Completed => {
                                    self.emit_codex_item_completed(
                                        summary.thread_id.clone(),
                                        summary.turn_id.clone(),
                                        summary.output.clone(),
                                    );
                                    self.notifications.emit_turn_completed(TurnCompletedEvent {
                                        thread_id: summary.thread_id.clone(),
                                        turn: codex_turn_from_summary(summary.clone()),
                                    });
                                }
                                TurnStatus::Failed => {
                                    let error = summary.error.clone().unwrap_or_default();
                                    self.emit_codex_item_completed(
                                        summary.thread_id.clone(),
                                        summary.turn_id.clone(),
                                        None,
                                    );
                                    self.notifications.emit_turn_completed(TurnCompletedEvent {
                                        thread_id: summary.thread_id.clone(),
                                        turn: codex_turn_from_summary(summary.clone()),
                                    });
                                    self.emit_codex_error(
                                        summary.thread_id,
                                        summary.turn_id,
                                        error,
                                    );
                                }
                                TurnStatus::Pending | TurnStatus::Cancelled => {}
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.emit_codex_error(
                                thread_id,
                                turn_id,
                                error.public_message().to_string(),
                            );
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
    let (detached_response_sender, detached_response_receiver) =
        mpsc::channel::<DetachedResponse>();
    let _reader_thread = std::thread::spawn(move || {
        for line in reader.lines() {
            if line_sender.send(line).is_err() {
                break;
            }
        }
    });

    let mut input_closed_at = None;
    let mut detached_inflight = 0_usize;
    loop {
        match line_receiver.recv_timeout(STDIO_NOTIFICATION_POLL_INTERVAL) {
            Ok(Ok(line)) => {
                input_closed_at = None;
                if !line.trim().is_empty() {
                    let response = match server
                        .try_spawn_detached_command_exec(&line, detached_response_sender.clone())
                    {
                        DetachedCommandExecDispatch::Spawned { process_id } => {
                            detached_inflight += 1;
                            if let Some(process_id) = process_id {
                                let detached_written = wait_for_detached_command_process(
                                    &server,
                                    &process_id,
                                    &detached_response_receiver,
                                    &mut writer,
                                )?;
                                detached_inflight =
                                    detached_inflight.saturating_sub(detached_written);
                            }
                            None
                        }
                        DetachedCommandExecDispatch::ImmediateResponse(response) => Some(response),
                        DetachedCommandExecDispatch::NotDetached => server.handle_json_rpc(&line),
                    };
                    let notification_write = write_pending_notifications(&mut server, &mut writer)?;
                    let detached_written =
                        write_detached_responses(&detached_response_receiver, &mut writer)?;
                    detached_inflight = detached_inflight.saturating_sub(detached_written);
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
        let detached_written = write_detached_responses(&detached_response_receiver, &mut writer)?;
        detached_inflight = detached_inflight.saturating_sub(detached_written);
        if notification_write.wrote || detached_written > 0 {
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
        if detached_inflight == 0
            && input_closed_at
                .is_some_and(|closed_at| closed_at.elapsed() >= STDIO_EOF_DRAIN_TIMEOUT)
        {
            break;
        }
    }

    let _ = write_pending_notifications(&mut server, &mut writer)?;
    let _ = write_detached_responses(&detached_response_receiver, &mut writer)?;
    writer.flush()?;
    Ok(())
}

enum DetachedResponse {
    Response { json: String },
    Completed,
}

enum DetachedCommandExecDispatch {
    NotDetached,
    Spawned { process_id: Option<String> },
    ImmediateResponse(String),
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

fn write_detached_responses<W>(
    receiver: &mpsc::Receiver<DetachedResponse>,
    writer: &mut W,
) -> io::Result<usize>
where
    W: Write,
{
    let mut wrote = 0_usize;
    while let Ok(response) = receiver.try_recv() {
        let DetachedResponse::Response { json } = response else {
            wrote += 1;
            continue;
        };
        writeln!(writer, "{json}")?;
        wrote += 1;
    }
    Ok(wrote)
}

fn wait_for_detached_command_process<W>(
    server: &AppServer,
    process_id: &str,
    detached_response_receiver: &mpsc::Receiver<DetachedResponse>,
    writer: &mut W,
) -> io::Result<usize>
where
    W: Write,
{
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut detached_written = 0_usize;
    while Instant::now() < deadline {
        if server.app_services.command.has_active_process(process_id) {
            break;
        }
        let written = write_detached_responses(detached_response_receiver, writer)?;
        detached_written += written;
        if written > 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok(detached_written)
}

pub trait RuntimeBridge: std::fmt::Debug + Send + Sync {
    fn features(&self) -> RuntimeBridgeFeatures {
        RuntimeBridgeFeatures::default()
    }

    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError>;
    fn cancel_turn(&self, request: RuntimeTurnCancelRequest) -> Result<(), RuntimeBridgeError>;
    fn steer_turn(&self, _request: RuntimeTurnSteerRequest) -> Result<(), RuntimeBridgeError> {
        Err(RuntimeBridgeError::fatal(
            "runtime bridge does not support turn steer",
        ))
    }
    fn compact_thread(
        &self,
        _request: RuntimeThreadCompactRequest,
    ) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
        Err(RuntimeBridgeError::fatal(
            "runtime bridge does not support thread compact",
        ))
    }
    fn resolve_approval(
        &self,
        _decision: RuntimeApprovalDecision,
    ) -> Result<(), RuntimeBridgeError> {
        Err(RuntimeBridgeError::fatal(
            "runtime bridge does not support approval resolution",
        ))
    }
    fn resolve_server_request(
        &self,
        resolution: RuntimeServerRequestResolution,
    ) -> Result<(), RuntimeBridgeError> {
        match resolution.payload {
            RuntimeServerRequestResponse::Approval(decision) => {
                self.resolve_approval(RuntimeApprovalDecision {
                    request_id: resolution.request_id,
                    decision,
                })
            }
            RuntimeServerRequestResponse::DynamicTool(_)
            | RuntimeServerRequestResponse::ToolUserInput(_)
            | RuntimeServerRequestResponse::FileChange(_)
            | RuntimeServerRequestResponse::Permissions(_) => Err(RuntimeBridgeError::fatal(
                "runtime bridge does not support this server request response kind",
            )),
        }
    }
    fn shutdown(&self);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeBridgeFeatures {
    pub approval: bool,
    pub tools: bool,
    pub sandbox: bool,
    pub model_reroutes: bool,
    pub model_verifications: bool,
    pub dynamic_tool_call: bool,
    pub tool_user_input: bool,
    pub permissions_approval: bool,
    pub file_change_approval: bool,
    pub file_change_events: bool,
    pub auto_approval_review: bool,
    pub thread_compact: bool,
    pub turn_steer: bool,
}

impl RuntimeBridgeFeatures {
    fn runtime_tool_approval_availability(self) -> RuntimeToolApprovalAvailability {
        RuntimeToolApprovalAvailability {
            command_approval: self.approval,
            dynamic_tool_call: self.dynamic_tool_call,
            tool_user_input: self.tool_user_input,
            permissions_approval: self.permissions_approval,
            file_change_approval: self.file_change_approval,
            file_change_events: self.file_change_events,
            auto_approval_review: self.auto_approval_review,
        }
    }

    fn real_model_r6_producer_availability(self) -> (bool, bool) {
        // Keep these false until the runtime bridge exposes a real producer
        // contract for model/rerouted and model/verification events.
        (false, false)
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeTurnStartRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub prompt: String,
    pub cwd: PathBuf,
    pub model_provider: RuntimeModelProviderSnapshot,
    pub reasoning_summary: ReasoningSummary,
    pub sandbox_context: RuntimeSandboxContext,
    pub updates: RuntimeTurnUpdateSink,
    pub hook_registry: Option<Arc<dasclaw_hooks::HookRegistry>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnCancelRequest {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnSteerRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub input: Vec<UserInput>,
    pub prompt: String,
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeThreadCompactRequest {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeThreadCompactResult {
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApprovalRequest {
    pub request_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub command: Option<String>,
    pub description: String,
    pub display_parameters: Value,
    pub allow_always: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeApprovalDecision {
    pub request_id: String,
    pub decision: dasclaw_runtime::ApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDynamicToolCallRequest {
    pub request_id: String,
    pub call_id: String,
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeToolUserInputRequest {
    pub request_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileChangeApprovalRequest {
    pub request_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePermissionsApprovalRequest {
    pub request_id: String,
    pub item_id: String,
    pub cwd: String,
    pub reason: Option<String>,
    pub permissions: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileChangeOutputDeltaUpdate {
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileChangePatchUpdatedUpdate {
    pub item_id: String,
    pub changes: Vec<FileUpdateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAutoApprovalReviewUpdate {
    pub review_id: String,
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeServerRequestResolution {
    pub request_id: String,
    pub payload: RuntimeServerRequestResponse,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeServerRequestResponse {
    Approval(dasclaw_runtime::ApprovalDecision),
    DynamicTool(DynamicToolCallResponse),
    ToolUserInput(ToolRequestUserInputResponse),
    FileChange(FileChangeApprovalDecision),
    Permissions(PermissionsRequestApprovalResponse),
}

#[derive(Debug)]
enum PendingRuntimeClientRequest {
    DynamicTool {
        turn_id: String,
        sender: oneshot::Sender<DynamicToolCallResponse>,
    },
    ToolUserInput {
        turn_id: String,
        sender: oneshot::Sender<ToolRequestUserInputResponse>,
    },
    FileChange {
        turn_id: String,
        sender: oneshot::Sender<FileChangeApprovalDecision>,
    },
    Permissions {
        turn_id: String,
        sender: oneshot::Sender<PermissionsRequestApprovalResponse>,
    },
}

impl PendingRuntimeClientRequest {
    fn turn_id(&self) -> &str {
        match self {
            Self::DynamicTool { turn_id, .. }
            | Self::ToolUserInput { turn_id, .. }
            | Self::FileChange { turn_id, .. }
            | Self::Permissions { turn_id, .. } => turn_id,
        }
    }

    fn fail(self) {
        match self {
            Self::DynamicTool { sender, .. } => {
                let _ = sender.send(DynamicToolCallResponse {
                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                        text: "runtime request was cancelled".to_string(),
                    }],
                    success: false,
                });
            }
            Self::ToolUserInput { sender, .. } => {
                let _ = sender.send(ToolRequestUserInputResponse {
                    answers: HashMap::new(),
                });
            }
            Self::FileChange { sender, .. } => {
                let _ = sender.send(FileChangeApprovalDecision::Cancel);
            }
            Self::Permissions { sender, .. } => {
                let _ = sender.send(PermissionsRequestApprovalResponse {
                    decision: PermissionsApprovalDecision::Reject,
                    permissions: Value::Null,
                    scope: dasclaw_app_server_protocol::PermissionGrantScope::Turn,
                    strict_auto_review: None,
                });
            }
        }
    }
}

type PendingRuntimeClientRequests = Arc<Mutex<HashMap<String, PendingRuntimeClientRequest>>>;

#[derive(Clone)]
pub struct RuntimeClientRequestContext {
    thread_id: String,
    turn_id: String,
    cwd: PathBuf,
    updates: RuntimeTurnUpdateSink,
    pending_requests: PendingRuntimeClientRequests,
    hook_registry: Option<Arc<dasclaw_hooks::HookRegistry>>,
}

impl RuntimeClientRequestContext {
    pub async fn request_dynamic_tool(
        &self,
        call: &dasclaw_core::messages::ToolCall,
        namespace: Option<String>,
        tool: String,
    ) -> DynamicToolCallResponse {
        let (sender, receiver) = oneshot::channel();
        let request_id = call.id.clone();
        if self
            .insert_pending_request(
                request_id.clone(),
                PendingRuntimeClientRequest::DynamicTool {
                    turn_id: self.turn_id.clone(),
                    sender,
                },
            )
            .is_err()
        {
            return failed_dynamic_tool_response("runtime request registry lock poisoned");
        }

        self.updates.dynamic_tool_call_requested(
            self.thread_id.clone(),
            self.turn_id.clone(),
            RuntimeDynamicToolCallRequest {
                request_id,
                call_id: call.id.clone(),
                namespace,
                tool,
                arguments: call.arguments.clone(),
            },
        );

        receiver.await.unwrap_or_else(|_| {
            failed_dynamic_tool_response("dynamic tool request was cancelled before response")
        })
    }

    pub async fn request_user_input(
        &self,
        call: &dasclaw_core::messages::ToolCall,
        questions: Vec<ToolRequestUserInputQuestion>,
    ) -> ToolRequestUserInputResponse {
        let (sender, receiver) = oneshot::channel();
        let request_id = call.id.clone();
        if self
            .insert_pending_request(
                request_id.clone(),
                PendingRuntimeClientRequest::ToolUserInput {
                    turn_id: self.turn_id.clone(),
                    sender,
                },
            )
            .is_err()
        {
            return ToolRequestUserInputResponse {
                answers: HashMap::new(),
            };
        }

        self.updates.tool_user_input_requested(
            self.thread_id.clone(),
            self.turn_id.clone(),
            RuntimeToolUserInputRequest {
                request_id,
                item_id: runtime_tool_item_id(&self.turn_id, &call.id),
                questions,
            },
        );

        receiver.await.unwrap_or(ToolRequestUserInputResponse {
            answers: HashMap::new(),
        })
    }

    pub async fn request_permissions(
        &self,
        call: &dasclaw_core::messages::ToolCall,
        permissions: Value,
        reason: Option<String>,
    ) -> PermissionsRequestApprovalResponse {
        let (sender, receiver) = oneshot::channel();
        let request_id = call.id.clone();
        if self
            .insert_pending_request(
                request_id.clone(),
                PendingRuntimeClientRequest::Permissions {
                    turn_id: self.turn_id.clone(),
                    sender,
                },
            )
            .is_err()
        {
            return denied_permissions_response();
        }

        self.updates.permissions_approval_requested(
            self.thread_id.clone(),
            self.turn_id.clone(),
            RuntimePermissionsApprovalRequest {
                request_id,
                item_id: runtime_tool_item_id(&self.turn_id, &call.id),
                cwd: self.cwd.to_string_lossy().to_string(),
                reason,
                permissions,
            },
        );

        receiver
            .await
            .unwrap_or_else(|_| denied_permissions_response())
    }

    pub async fn request_file_change_approval(
        &self,
        call: &dasclaw_core::messages::ToolCall,
    ) -> FileChangeApprovalDecision {
        let (sender, receiver) = oneshot::channel();
        let request_id = call.id.clone();
        if self
            .insert_pending_request(
                request_id.clone(),
                PendingRuntimeClientRequest::FileChange {
                    turn_id: self.turn_id.clone(),
                    sender,
                },
            )
            .is_err()
        {
            return FileChangeApprovalDecision::Cancel;
        }

        self.updates.file_change_approval_requested(
            self.thread_id.clone(),
            self.turn_id.clone(),
            RuntimeFileChangeApprovalRequest {
                request_id,
                item_id: runtime_tool_item_id(&self.turn_id, &call.id),
                reason: string_field(&call.arguments, "reason")
                    .or_else(|| Some(format!("file change requested by {}", call.name))),
                grant_root: string_field(&call.arguments, "path"),
            },
        );

        receiver.await.unwrap_or(FileChangeApprovalDecision::Cancel)
    }

    fn insert_pending_request(
        &self,
        request_id: String,
        request: PendingRuntimeClientRequest,
    ) -> Result<(), RuntimeBridgeError> {
        let mut pending = self.pending_requests.lock().map_err(|_| {
            RuntimeBridgeError::retryable("runtime client request registry lock poisoned")
        })?;
        if pending.contains_key(&request_id) {
            return Err(RuntimeBridgeError::retryable(format!(
                "runtime client request is already pending: {request_id}"
            )));
        }
        pending.insert(request_id, request);
        Ok(())
    }
}

#[derive(Clone)]
pub struct RuntimeClientToolExecutor {
    context: RuntimeClientRequestContext,
    fallback: Option<Arc<dyn dasclaw_runtime::ToolExecutor>>,
}

impl RuntimeClientToolExecutor {
    #[must_use]
    pub fn new(
        context: RuntimeClientRequestContext,
        fallback: Option<Arc<dyn dasclaw_runtime::ToolExecutor>>,
    ) -> Self {
        Self { context, fallback }
    }

    async fn execute_dynamic_tool(
        &self,
        call: &dasclaw_core::messages::ToolCall,
        namespace: Option<String>,
        tool: String,
    ) -> dasclaw_core::messages::ToolResult {
        let response = self
            .context
            .request_dynamic_tool(call, namespace, tool)
            .await;
        dynamic_tool_response_to_tool_result(call, response)
    }

    async fn execute_user_input(
        &self,
        call: &dasclaw_core::messages::ToolCall,
    ) -> dasclaw_core::messages::ToolResult {
        let questions = match serde_json::from_value::<Vec<ToolRequestUserInputQuestion>>(
            call.arguments
                .get("questions")
                .cloned()
                .unwrap_or(Value::Array(Vec::new())),
        ) {
            Ok(questions) if !questions.is_empty() => questions,
            _ => {
                return error_tool_result(
                    call,
                    "request_user_input requires a non-empty questions array",
                );
            }
        };
        let response = self.context.request_user_input(call, questions).await;
        let is_error = response.answers.is_empty();
        json_tool_result(call, &response, is_error)
    }

    async fn execute_permissions(
        &self,
        call: &dasclaw_core::messages::ToolCall,
    ) -> dasclaw_core::messages::ToolResult {
        let permissions = call
            .arguments
            .get("permissions")
            .cloned()
            .unwrap_or_else(|| call.arguments.clone());
        let reason = string_field(&call.arguments, "reason");
        let response = self
            .context
            .request_permissions(call, permissions, reason)
            .await;
        let is_error = response.decision == PermissionsApprovalDecision::Reject
            || response.permissions.is_null();
        json_tool_result(call, &response, is_error)
    }

    async fn execute_file_change(
        &self,
        call: &dasclaw_core::messages::ToolCall,
    ) -> Result<dasclaw_core::messages::ToolResult, dasclaw_core::traits::HostError> {
        let decision = self.context.request_file_change_approval(call).await;
        match decision {
            FileChangeApprovalDecision::Accept | FileChangeApprovalDecision::AcceptForSession => {
                match &self.fallback {
                    Some(fallback) => fallback.execute(call).await,
                    None => Ok(error_tool_result(
                        call,
                        "file change accepted but no file-changing executor is configured",
                    )),
                }
            }
            FileChangeApprovalDecision::Decline => {
                Ok(error_tool_result(call, "file change denied by user"))
            }
            FileChangeApprovalDecision::Cancel => {
                Ok(error_tool_result(call, "file change cancelled by user"))
            }
        }
    }

    async fn run_before_tool_call_hook(
        &self,
        call: &dasclaw_core::messages::ToolCall,
    ) -> Result<(), String> {
        let Some(registry) = &self.context.hook_registry else {
            return Ok(());
        };

        let event = dasclaw_hooks::HookEvent::ToolCall {
            tool_name: call.name.clone(),
            parameters: call.arguments.clone(),
            user_id: "app-server".to_string(),
            thread_id: Some(self.context.thread_id.clone()),
            turn_id: Some(self.context.turn_id.clone()),
            context: "chat".to_string(),
        };

        registry
            .run(&event)
            .await
            .map(|_| ())
            .map_err(|error| match error {
                dasclaw_hooks::HookError::Rejected { reason } => reason,
                other => other.to_string(),
            })
    }
}

#[async_trait::async_trait]
impl dasclaw_runtime::ToolExecutor for RuntimeClientToolExecutor {
    async fn execute(
        &self,
        call: &dasclaw_core::messages::ToolCall,
    ) -> Result<dasclaw_core::messages::ToolResult, dasclaw_core::traits::HostError> {
        if let Err(reason) = self.run_before_tool_call_hook(call).await {
            return Ok(error_tool_result(call, &reason));
        }

        match classify_runtime_client_tool(&call.name) {
            RuntimeClientToolKind::DynamicTool { namespace, tool } => {
                Ok(self.execute_dynamic_tool(call, namespace, tool).await)
            }
            RuntimeClientToolKind::UserInput => Ok(self.execute_user_input(call).await),
            RuntimeClientToolKind::Permissions => Ok(self.execute_permissions(call).await),
            RuntimeClientToolKind::FileChange => self.execute_file_change(call).await,
            RuntimeClientToolKind::Fallback => match &self.fallback {
                Some(fallback) => fallback.execute(call).await,
                None => Ok(error_tool_result(
                    call,
                    &format!("unknown tool: {}", call.name),
                )),
            },
        }
    }
}

enum RuntimeClientToolKind {
    DynamicTool {
        namespace: Option<String>,
        tool: String,
    },
    UserInput,
    Permissions,
    FileChange,
    Fallback,
}

fn classify_runtime_client_tool(name: &str) -> RuntimeClientToolKind {
    match name {
        "request_user_input" => RuntimeClientToolKind::UserInput,
        "request_permissions" => RuntimeClientToolKind::Permissions,
        "apply_patch" | "write_file" => RuntimeClientToolKind::FileChange,
        "open_url" => RuntimeClientToolKind::DynamicTool {
            namespace: Some("client".to_string()),
            tool: "open_url".to_string(),
        },
        _ => match name.strip_prefix("client.") {
            Some(tool) => RuntimeClientToolKind::DynamicTool {
                namespace: Some("client".to_string()),
                tool: tool.to_string(),
            },
            None => RuntimeClientToolKind::Fallback,
        },
    }
}

fn failed_dynamic_tool_response(reason: &str) -> DynamicToolCallResponse {
    DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputText {
            text: reason.to_string(),
        }],
        success: false,
    }
}

fn denied_permissions_response() -> PermissionsRequestApprovalResponse {
    PermissionsRequestApprovalResponse {
        decision: PermissionsApprovalDecision::Reject,
        permissions: Value::Null,
        scope: dasclaw_app_server_protocol::PermissionGrantScope::Turn,
        strict_auto_review: None,
    }
}

fn dynamic_tool_response_to_tool_result(
    call: &dasclaw_core::messages::ToolCall,
    response: DynamicToolCallResponse,
) -> dasclaw_core::messages::ToolResult {
    json_tool_result(call, &response, !response.success)
}

fn json_tool_result<T: Serialize>(
    call: &dasclaw_core::messages::ToolCall,
    payload: &T,
    is_error: bool,
) -> dasclaw_core::messages::ToolResult {
    let content = serde_json::to_string(payload)
        .unwrap_or_else(|error| format!("failed to serialize tool response: {error}"));
    dasclaw_core::messages::ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        content,
        is_error,
    }
}

fn error_tool_result(
    call: &dasclaw_core::messages::ToolCall,
    message: &str,
) -> dasclaw_core::messages::ToolResult {
    dasclaw_core::messages::ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        content: message.to_string(),
        is_error: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeToolResultUpdate {
    pub item_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandOutputDeltaUpdate {
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReasoningSummaryPartAddedUpdate {
    pub item_id: String,
    pub summary_index: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReasoningTextDeltaUpdate {
    pub item_id: String,
    pub content_index: i64,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnPlanUpdate {
    pub explanation: Option<String>,
    pub plan: Vec<TurnPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnDiffUpdate {
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeRawResponseItemCompletedUpdate {
    pub item: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePlanDeltaUpdate {
    pub item_id: String,
    pub delta: String,
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

    pub fn reasoning_summary_part_added(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeReasoningSummaryPartAddedUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ReasoningSummaryPartAdded { update },
        });
    }

    pub fn reasoning_text_delta(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeReasoningTextDeltaUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ReasoningTextDelta { update },
        });
    }

    pub fn turn_plan_updated(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeTurnPlanUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::PlanUpdated { update },
        });
    }

    pub fn turn_diff_updated(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeTurnDiffUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::DiffUpdated { update },
        });
    }

    pub fn raw_response_item_completed(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeRawResponseItemCompletedUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::RawResponseItemCompleted { update },
        });
    }

    pub fn plan_delta(&self, thread_id: String, turn_id: String, update: RuntimePlanDeltaUpdate) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::PlanDelta { update },
        });
    }

    pub fn approval_requested(
        &self,
        thread_id: String,
        turn_id: String,
        request: RuntimeApprovalRequest,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ApprovalRequested { request },
        });
    }

    pub fn tool_result(&self, thread_id: String, turn_id: String, update: RuntimeToolResultUpdate) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ToolResult { update },
        });
    }

    pub fn command_output_delta(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeCommandOutputDeltaUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::CommandOutputDelta { update },
        });
    }

    pub fn dynamic_tool_call_requested(
        &self,
        thread_id: String,
        turn_id: String,
        request: RuntimeDynamicToolCallRequest,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::DynamicToolCallRequested { request },
        });
    }

    pub fn tool_user_input_requested(
        &self,
        thread_id: String,
        turn_id: String,
        request: RuntimeToolUserInputRequest,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ToolUserInputRequested { request },
        });
    }

    pub fn file_change_approval_requested(
        &self,
        thread_id: String,
        turn_id: String,
        request: RuntimeFileChangeApprovalRequest,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::FileChangeApprovalRequested { request },
        });
    }

    pub fn permissions_approval_requested(
        &self,
        thread_id: String,
        turn_id: String,
        request: RuntimePermissionsApprovalRequest,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::PermissionsApprovalRequested { request },
        });
    }

    pub fn file_change_output_delta(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeFileChangeOutputDeltaUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::FileChangeOutputDelta { update },
        });
    }

    pub fn file_change_patch_updated(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeFileChangePatchUpdatedUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::FileChangePatchUpdated { update },
        });
    }

    pub fn auto_approval_review_started(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeAutoApprovalReviewUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::AutoApprovalReviewStarted { update },
        });
    }

    pub fn auto_approval_review_completed(
        &self,
        thread_id: String,
        turn_id: String,
        update: RuntimeAutoApprovalReviewUpdate,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::AutoApprovalReviewCompleted { update },
        });
    }

    pub fn token_usage_updated(
        &self,
        thread_id: String,
        turn_id: String,
        usage: dasclaw_core::response_types::TokenUsage,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::TokenUsageUpdated { usage },
        });
    }

    pub fn model_rerouted(
        &self,
        thread_id: String,
        turn_id: String,
        from_model: String,
        to_model: String,
        reason: ModelRerouteReason,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ModelRerouted {
                from_model,
                to_model,
                reason,
            },
        });
    }

    pub fn model_verification(
        &self,
        thread_id: String,
        turn_id: String,
        verifications: Vec<ModelVerification>,
    ) {
        self.push(RuntimeTurnUpdate {
            thread_id,
            turn_id,
            outcome: RuntimeTurnOutcome::ModelVerification { verifications },
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

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeTurnUpdate {
    pub thread_id: String,
    pub turn_id: String,
    pub outcome: RuntimeTurnOutcome,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeTurnOutcome {
    Delta {
        delta: String,
    },
    ReasoningSummaryDelta {
        delta: String,
        summary_index: i64,
    },
    ReasoningSummaryPartAdded {
        update: RuntimeReasoningSummaryPartAddedUpdate,
    },
    ReasoningTextDelta {
        update: RuntimeReasoningTextDeltaUpdate,
    },
    PlanUpdated {
        update: RuntimeTurnPlanUpdate,
    },
    DiffUpdated {
        update: RuntimeTurnDiffUpdate,
    },
    RawResponseItemCompleted {
        update: RuntimeRawResponseItemCompletedUpdate,
    },
    PlanDelta {
        update: RuntimePlanDeltaUpdate,
    },
    ApprovalRequested {
        request: RuntimeApprovalRequest,
    },
    ToolResult {
        update: RuntimeToolResultUpdate,
    },
    CommandOutputDelta {
        update: RuntimeCommandOutputDeltaUpdate,
    },
    DynamicToolCallRequested {
        request: RuntimeDynamicToolCallRequest,
    },
    ToolUserInputRequested {
        request: RuntimeToolUserInputRequest,
    },
    FileChangeApprovalRequested {
        request: RuntimeFileChangeApprovalRequest,
    },
    PermissionsApprovalRequested {
        request: RuntimePermissionsApprovalRequest,
    },
    FileChangeOutputDelta {
        update: RuntimeFileChangeOutputDeltaUpdate,
    },
    FileChangePatchUpdated {
        update: RuntimeFileChangePatchUpdatedUpdate,
    },
    AutoApprovalReviewStarted {
        update: RuntimeAutoApprovalReviewUpdate,
    },
    AutoApprovalReviewCompleted {
        update: RuntimeAutoApprovalReviewUpdate,
    },
    TokenUsageUpdated {
        usage: dasclaw_core::response_types::TokenUsage,
    },
    ModelRerouted {
        from_model: String,
        to_model: String,
        reason: ModelRerouteReason,
    },
    ModelVerification {
        verifications: Vec<ModelVerification>,
    },
    Completed {
        output: String,
    },
    Failed {
        error: String,
    },
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
    active_agents: Arc<Mutex<HashMap<String, Arc<dasclaw_runtime::Agent>>>>,
    pending_approvals: Arc<Mutex<HashMap<String, Arc<dasclaw_runtime::Agent>>>>,
    pending_client_requests: PendingRuntimeClientRequests,
    features: RuntimeBridgeFeatures,
}

type AgentFactory = dyn Fn(
        CancellationToken,
        RuntimeModelProviderSnapshot,
        ReasoningSummary,
        RuntimeClientRequestContext,
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
        Self::new_with_model_provider(move |token, _snapshot, _reasoning_summary| {
            agent_factory(token)
        })
    }

    #[must_use]
    pub fn new_with_model_provider(
        agent_factory: impl Fn(
            CancellationToken,
            RuntimeModelProviderSnapshot,
            ReasoningSummary,
        ) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self::new_with_runtime_context(move |token, snapshot, reasoning_summary, _context| {
            agent_factory(token, snapshot, reasoning_summary)
        })
    }

    #[must_use]
    pub fn new_with_runtime_context(
        agent_factory: impl Fn(
            CancellationToken,
            RuntimeModelProviderSnapshot,
            ReasoningSummary,
            RuntimeClientRequestContext,
        ) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            agent_factory: Arc::new(agent_factory),
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            active_agents: Arc::new(Mutex::new(HashMap::new())),
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            pending_client_requests: Arc::new(Mutex::new(HashMap::new())),
            features: RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: false,
                dynamic_tool_call: true,
                tool_user_input: true,
                permissions_approval: true,
                file_change_approval: true,
                turn_steer: true,
                ..RuntimeBridgeFeatures::default()
            },
        }
    }

    #[must_use]
    pub fn new_with_model_provider_and_features(
        agent_factory: impl Fn(
            CancellationToken,
            RuntimeModelProviderSnapshot,
            ReasoningSummary,
        ) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError>
        + Send
        + Sync
        + 'static,
        features: RuntimeBridgeFeatures,
    ) -> Self {
        let mut bridge = Self::new_with_model_provider(agent_factory);
        bridge.features = features;
        bridge
    }

    #[must_use]
    pub fn from_model_provider_snapshot() -> Self {
        Self::new_with_runtime_context(agent_from_model_provider_snapshot)
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

    fn cleanup_runtime_turn(&self, turn_id: &str, agent: &Arc<dasclaw_runtime::Agent>) {
        self.remove_in_flight_turn(turn_id);
        if let Ok(mut active_agents) = self.active_agents.lock() {
            active_agents.remove(turn_id);
        }
        if let Ok(mut pending_approvals) = self.pending_approvals.lock() {
            pending_approvals.retain(|_, pending_agent| !Arc::ptr_eq(pending_agent, agent));
        }
        self.fail_pending_client_requests_for_turn(turn_id);
    }

    fn fail_pending_client_requests_for_turn(&self, turn_id: &str) {
        let pending = self
            .pending_client_requests
            .lock()
            .map(|mut requests| {
                let mut drained = Vec::new();
                let mut retained = HashMap::new();
                for (request_id, request) in requests.drain() {
                    if request.turn_id() == turn_id {
                        drained.push(request);
                    } else {
                        retained.insert(request_id, request);
                    }
                }
                *requests = retained;
                drained
            })
            .unwrap_or_default();
        for request in pending {
            request.fail();
        }
    }

    fn fail_all_pending_client_requests(&self) {
        let pending = self
            .pending_client_requests
            .lock()
            .map(|mut requests| {
                requests
                    .drain()
                    .map(|(_, request)| request)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for request in pending {
            request.fail();
        }
    }

    fn resolve_pending_client_request(
        &self,
        resolution: RuntimeServerRequestResolution,
    ) -> Result<(), RuntimeBridgeError> {
        let pending = self
            .pending_client_requests
            .lock()
            .map_err(|_| {
                RuntimeBridgeError::retryable("runtime client request registry lock poisoned")
            })?
            .remove(&resolution.request_id);

        let Some(pending) = pending else {
            return Err(RuntimeBridgeError::retryable(format!(
                "runtime client request is not pending: {}",
                resolution.request_id
            )));
        };

        match (pending, resolution.payload) {
            (
                PendingRuntimeClientRequest::DynamicTool { sender, .. },
                RuntimeServerRequestResponse::DynamicTool(response),
            ) => sender
                .send(response)
                .map_err(|_| RuntimeBridgeError::retryable("runtime dynamic tool waiter closed")),
            (
                PendingRuntimeClientRequest::ToolUserInput { sender, .. },
                RuntimeServerRequestResponse::ToolUserInput(response),
            ) => sender
                .send(response)
                .map_err(|_| RuntimeBridgeError::retryable("runtime user input waiter closed")),
            (
                PendingRuntimeClientRequest::FileChange { sender, .. },
                RuntimeServerRequestResponse::FileChange(response),
            ) => sender
                .send(response)
                .map_err(|_| RuntimeBridgeError::retryable("runtime file change waiter closed")),
            (
                PendingRuntimeClientRequest::Permissions { sender, .. },
                RuntimeServerRequestResponse::Permissions(response),
            ) => sender
                .send(response)
                .map_err(|_| RuntimeBridgeError::retryable("runtime permissions waiter closed")),
            (pending, _) => {
                pending.fail();
                Err(RuntimeBridgeError::fatal(
                    "runtime client request response kind mismatch",
                ))
            }
        }
    }
}

impl RuntimeBridge for DasclawAgentRuntimeBridge {
    fn features(&self) -> RuntimeBridgeFeatures {
        self.features
    }

    fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
        if !request.sandbox_context.is_empty() {
            return Err(RuntimeBridgeError::fatal(
                "runtime bridge does not support sandbox context enforcement",
            ));
        }
        let token = CancellationToken::new();
        let client_request_context = RuntimeClientRequestContext {
            thread_id: request.thread_id.clone(),
            turn_id: request.turn_id.clone(),
            cwd: request.cwd.clone(),
            updates: request.updates.clone(),
            pending_requests: Arc::clone(&self.pending_client_requests),
            hook_registry: request.hook_registry.clone(),
        };
        let agent = Arc::new((self.agent_factory)(
            token.clone(),
            request.model_provider.clone(),
            request.reasoning_summary,
            client_request_context,
        )?);
        let mut in_flight = self
            .in_flight
            .lock()
            .map_err(|_| RuntimeBridgeError::retryable("runtime turn registry lock poisoned"))?;
        in_flight.insert(request.turn_id.clone(), token);
        drop(in_flight);
        let mut active_agents = self
            .active_agents
            .lock()
            .map_err(|_| RuntimeBridgeError::retryable("runtime agent registry lock poisoned"))?;
        active_agents.insert(request.turn_id.clone(), Arc::clone(&agent));
        drop(active_agents);

        let bridge = self.clone();
        let thread_id = request.thread_id;
        let turn_id = request.turn_id;
        let runtime_cleanup_turn_id = turn_id.clone();
        let spawn_cleanup_turn_id = turn_id.clone();
        let spawn_cleanup_agent = Arc::clone(&agent);
        let runtime_cleanup_agent = Arc::clone(&agent);
        let stream_agent = Arc::clone(&agent);
        let prompt = request.prompt;
        let cwd = request.cwd;
        let updates = request.updates;
        let event_updates = updates.clone();
        let event_thread_id = thread_id.clone();
        let event_turn_id = turn_id.clone();
        let requested_model = request.model_provider.model_id.clone();
        let model_call_mode = request.model_provider.model_call_mode;
        let event_bridge = bridge.clone();
        thread::Builder::new()
            .name(format!("dasclaw-app-server-{turn_id}"))
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();

                let result = match runtime {
                    Ok(runtime) => runtime.block_on(async move {
                        use futures_util::StreamExt as _;

                        let mut stream = Arc::clone(&stream_agent).stream(
                            &prompt,
                            dasclaw_runtime::AgentRunOptions { model_call_mode },
                        );
                        let mut tool_item_keys_by_name = HashMap::<String, String>::new();
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
                                Ok(dasclaw_runtime::AgentEvent::ReasoningSummaryPartAdded {
                                    item_id,
                                    summary_index,
                                }) => {
                                    event_updates.reasoning_summary_part_added(
                                        event_thread_id.clone(),
                                        event_turn_id.clone(),
                                        RuntimeReasoningSummaryPartAddedUpdate {
                                            item_id: item_id.unwrap_or_else(|| {
                                                format!("{event_turn_id}:reasoning")
                                            }),
                                            summary_index,
                                        },
                                    );
                                }
                                Ok(dasclaw_runtime::AgentEvent::ReasoningRawTextChunk {
                                    item_id,
                                    content_index,
                                    delta,
                                }) => {
                                    if !delta.is_empty() {
                                        event_updates.reasoning_text_delta(
                                            event_thread_id.clone(),
                                            event_turn_id.clone(),
                                            RuntimeReasoningTextDeltaUpdate {
                                                item_id: item_id.unwrap_or_else(|| {
                                                    format!("{event_turn_id}:reasoning")
                                                }),
                                                content_index,
                                                delta,
                                            },
                                        );
                                    }
                                }
                                Ok(dasclaw_runtime::AgentEvent::PlanDelta { item_id, delta }) => {
                                    if !delta.is_empty() {
                                        event_updates.plan_delta(
                                            event_thread_id.clone(),
                                            event_turn_id.clone(),
                                            RuntimePlanDeltaUpdate {
                                                item_id: item_id.unwrap_or_else(|| {
                                                    format!("{event_turn_id}:plan")
                                                }),
                                                delta,
                                            },
                                        );
                                    }
                                }
                                Ok(dasclaw_runtime::AgentEvent::TurnPlanUpdated {
                                    explanation,
                                    plan,
                                }) => {
                                    event_updates.turn_plan_updated(
                                        event_thread_id.clone(),
                                        event_turn_id.clone(),
                                        RuntimeTurnPlanUpdate {
                                            explanation,
                                            plan: plan
                                                .into_iter()
                                                .map(turn_plan_step_from_agent)
                                                .collect(),
                                        },
                                    );
                                }
                                Ok(dasclaw_runtime::AgentEvent::TurnDiffUpdated { diff }) => {
                                    event_updates.turn_diff_updated(
                                        event_thread_id.clone(),
                                        event_turn_id.clone(),
                                        RuntimeTurnDiffUpdate { diff },
                                    );
                                }
                                Ok(dasclaw_runtime::AgentEvent::RawResponseItemCompleted {
                                    item,
                                }) => {
                                    event_updates.raw_response_item_completed(
                                        event_thread_id.clone(),
                                        event_turn_id.clone(),
                                        RuntimeRawResponseItemCompletedUpdate { item },
                                    );
                                }
                                Ok(dasclaw_runtime::AgentEvent::Completed(output)) => {
                                    return Ok(output);
                                }
                                Ok(dasclaw_runtime::AgentEvent::ApprovalNeeded {
                                    request_id,
                                    tool_name,
                                    tool_arguments,
                                    description,
                                    display_parameters,
                                    allow_always,
                                }) => {
                                    let request_id = request_id.to_string();
                                    let tool_call_id = tool_call_id_from_arguments(&tool_arguments)
                                        .unwrap_or_else(|| tool_name.clone());
                                    let command = command_from_arguments(&tool_arguments);
                                    if let Ok(mut pending_approvals) =
                                        event_bridge.pending_approvals.lock()
                                    {
                                        pending_approvals
                                            .insert(request_id.clone(), Arc::clone(&stream_agent));
                                    }
                                    if let Some(command) = command {
                                        event_updates.approval_requested(
                                            event_thread_id.clone(),
                                            event_turn_id.clone(),
                                            RuntimeApprovalRequest {
                                                request_id: request_id.clone(),
                                                tool_call_id,
                                                tool_name,
                                                command: Some(command),
                                                description,
                                                display_parameters,
                                                allow_always,
                                            },
                                        );
                                    } else {
                                        event_updates.permissions_approval_requested(
                                            event_thread_id.clone(),
                                            event_turn_id.clone(),
                                            RuntimePermissionsApprovalRequest {
                                                request_id,
                                                item_id: runtime_tool_item_id(
                                                    &event_turn_id,
                                                    &tool_call_id,
                                                ),
                                                cwd: cwd.to_string_lossy().to_string(),
                                                reason: Some(description),
                                                permissions: display_parameters,
                                            },
                                        );
                                    }
                                }
                                Ok(dasclaw_runtime::AgentEvent::ToolCallStart {
                                    name,
                                    arguments,
                                }) => {
                                    let call_id = tool_call_id_from_arguments(&arguments)
                                        .unwrap_or_else(|| name.clone());
                                    tool_item_keys_by_name.insert(name.clone(), call_id.clone());
                                    event_updates.command_output_delta(
                                        event_thread_id.clone(),
                                        event_turn_id.clone(),
                                        RuntimeCommandOutputDeltaUpdate {
                                            item_id: runtime_tool_item_id(&event_turn_id, &call_id),
                                            delta: tool_call_started_delta(&name),
                                        },
                                    );
                                }
                                Ok(dasclaw_runtime::AgentEvent::ToolResult {
                                    name,
                                    content,
                                    is_error,
                                }) => {
                                    let item_key = tool_item_keys_by_name
                                        .remove(&name)
                                        .unwrap_or_else(|| name.clone());
                                    event_updates.tool_result(
                                        event_thread_id.clone(),
                                        event_turn_id.clone(),
                                        RuntimeToolResultUpdate {
                                            item_id: runtime_tool_item_id(
                                                &event_turn_id,
                                                &item_key,
                                            ),
                                            content,
                                            is_error,
                                        },
                                    );
                                }
                                Ok(dasclaw_runtime::AgentEvent::FinishReason(_)) => {}
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

                bridge.cleanup_runtime_turn(&runtime_cleanup_turn_id, &runtime_cleanup_agent);
                match result {
                    Ok(output) => {
                        emit_response_model_metadata(
                            &updates,
                            &thread_id,
                            &turn_id,
                            &requested_model,
                            &output.metadata,
                        );
                        updates.token_usage_updated(
                            thread_id.clone(),
                            turn_id.clone(),
                            output.usage,
                        );
                        updates.complete(thread_id, turn_id, output.text);
                    }
                    Err(error) => updates.fail(thread_id, turn_id, error.to_string()),
                }
            })
            .map_err(|error| {
                self.cleanup_runtime_turn(&spawn_cleanup_turn_id, &spawn_cleanup_agent);
                RuntimeBridgeError::retryable(format!(
                    "failed to start dasclaw runtime turn: {error}"
                ))
            })?;

        Ok(())
    }

    fn steer_turn(&self, request: RuntimeTurnSteerRequest) -> Result<(), RuntimeBridgeError> {
        let active_agents = self
            .active_agents
            .lock()
            .map_err(|_| RuntimeBridgeError::retryable("runtime agent registry lock poisoned"))?;
        let Some(agent) = active_agents.get(&request.turn_id).cloned() else {
            return Err(RuntimeBridgeError::retryable(format!(
                "runtime turn is not active: {}",
                request.turn_id
            )));
        };
        drop(active_agents);

        agent
            .inject_user_message(request.prompt)
            .map_err(|error| RuntimeBridgeError::retryable(error.to_string()))
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
        self.fail_pending_client_requests_for_turn(&request.turn_id);
        Ok(())
    }

    fn resolve_approval(
        &self,
        decision: RuntimeApprovalDecision,
    ) -> Result<(), RuntimeBridgeError> {
        let request_id = uuid::Uuid::parse_str(&decision.request_id).map_err(|error| {
            RuntimeBridgeError::fatal(format!("invalid approval request id: {error}"))
        })?;
        let agent = self
            .pending_approvals
            .lock()
            .map_err(|_| RuntimeBridgeError::retryable("runtime approval registry lock poisoned"))?
            .remove(&decision.request_id)
            .ok_or_else(|| {
                RuntimeBridgeError::retryable(format!(
                    "runtime approval request is not pending: {}",
                    decision.request_id
                ))
            })?;
        agent
            .respond_to_approval(request_id, decision.decision)
            .map_err(|error| RuntimeBridgeError::retryable(error.to_string()))
    }

    fn resolve_server_request(
        &self,
        resolution: RuntimeServerRequestResolution,
    ) -> Result<(), RuntimeBridgeError> {
        match &resolution.payload {
            RuntimeServerRequestResponse::Approval(decision) => {
                self.resolve_approval(RuntimeApprovalDecision {
                    request_id: resolution.request_id,
                    decision: decision.clone(),
                })
            }
            RuntimeServerRequestResponse::Permissions(response) => {
                if self
                    .resolve_pending_client_request(resolution.clone())
                    .is_ok()
                {
                    return Ok(());
                }

                let decision = match response.decision {
                    PermissionsApprovalDecision::Approve => {
                        dasclaw_runtime::ApprovalDecision::Approve
                    }
                    PermissionsApprovalDecision::Reject => {
                        dasclaw_runtime::ApprovalDecision::Reject {
                            reason: Some(PERMISSIONS_REQUEST_REJECTED_REASON.to_string()),
                        }
                    }
                };
                self.resolve_approval(RuntimeApprovalDecision {
                    request_id: resolution.request_id,
                    decision,
                })
            }
            RuntimeServerRequestResponse::DynamicTool(_)
            | RuntimeServerRequestResponse::ToolUserInput(_)
            | RuntimeServerRequestResponse::FileChange(_) => {
                self.resolve_pending_client_request(resolution)
            }
        }
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
        self.fail_all_pending_client_requests();
        if let Ok(mut active_agents) = self.active_agents.lock() {
            active_agents.clear();
        }
        if let Ok(mut pending_approvals) = self.pending_approvals.lock() {
            pending_approvals.clear();
        }
    }
}

fn runtime_tool_item_id(turn_id: &str, tool_name: &str) -> String {
    format!("{turn_id}:tool:{tool_name}")
}

fn tool_call_started_delta(tool_name: &str) -> String {
    format!("tool call started: {tool_name}")
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn tool_call_id_from_arguments(arguments: &Value) -> Option<String> {
    string_field(arguments, "id").or_else(|| string_field(arguments, "tool_call_id"))
}

fn command_from_arguments(arguments: &Value) -> Option<String> {
    string_field(arguments, "cmd").or_else(|| string_field(arguments, "command"))
}

fn turn_plan_step_from_agent(step: AgentPlanStep) -> TurnPlanStep {
    TurnPlanStep {
        step: step.step,
        status: match step.status {
            AgentPlanStepStatus::Pending => {
                dasclaw_app_server_protocol::TurnPlanStepStatus::Pending
            }
            AgentPlanStepStatus::InProgress => {
                dasclaw_app_server_protocol::TurnPlanStepStatus::InProgress
            }
            AgentPlanStepStatus::Completed => {
                dasclaw_app_server_protocol::TurnPlanStepStatus::Completed
            }
        },
    }
}

fn prompt_text_from_user_input(input: &[UserInput]) -> String {
    input
        .iter()
        .filter_map(|input| match input {
            UserInput::Text { text, .. } => Some(text.as_str()),
            UserInput::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_from_model_provider_snapshot(
    token: CancellationToken,
    snapshot: RuntimeModelProviderSnapshot,
    reasoning_summary: ReasoningSummary,
    context: RuntimeClientRequestContext,
) -> Result<dasclaw_runtime::Agent, RuntimeBridgeError> {
    let config = registry_config_from_snapshot(&snapshot)?;
    let provider = ClawCodeLlmProvider::from_registry_config(&config).map_err(|error| {
        RuntimeBridgeError::fatal(redact_snapshot_secret(&error.to_string(), &snapshot))
    })?;
    let responder =
        LlmProviderResponder::new(Arc::new(provider)).with_reasoning_summary(reasoning_summary);

    dasclaw_runtime::Agent::builder()
        .responder(responder)
        .tool_executor(RuntimeClientToolExecutor::new(context, None))
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

fn emit_response_model_metadata(
    updates: &RuntimeTurnUpdateSink,
    thread_id: &str,
    turn_id: &str,
    _requested_model: &str,
    metadata: &dasclaw_core::response_types::ResponseMetadata,
) {
    if !metadata.model_verifications.is_empty() {
        updates.model_verification(
            thread_id.to_string(),
            turn_id.to_string(),
            metadata
                .model_verifications
                .iter()
                .map(app_server_model_verification)
                .collect(),
        );
    }
}

fn app_server_model_verification(
    verification: &dasclaw_core::response_types::ResponseModelVerification,
) -> ModelVerification {
    match verification {
        dasclaw_core::response_types::ResponseModelVerification::TrustedAccessForCyber => {
            ModelVerification::TrustedAccessForCyber
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestThreadParams {
    cwd: Option<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestThreadHandle {
    thread_id: String,
    lifecycle: LifecycleSnapshot,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestTurnInterruptParams {
    thread_id: String,
    turn_id: String,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestTurnInterruptResult {
    accepted: bool,
    status: TurnStatus,
    lifecycle: LifecycleSnapshot,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestTurnsSnapshotParams {
    thread_id: String,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestTurnsSnapshot {
    turns: Vec<TurnSummary>,
}

fn codex_turn_from_summary(summary: TurnSummary) -> CodexTurn {
    let items = if summary.items.is_empty() {
        match summary.status {
            TurnStatus::Pending => vec![CodexThreadItem::started_agent_message(
                summary.turn_id.clone(),
            )],
            TurnStatus::Completed => vec![CodexThreadItem::completed_agent_message(
                summary.turn_id.clone(),
                summary.output.clone().unwrap_or_default(),
            )],
            TurnStatus::Failed | TurnStatus::Cancelled => {
                vec![CodexThreadItem::completed_agent_message(
                    summary.turn_id.clone(),
                    summary.output.clone().unwrap_or_default(),
                )]
            }
        }
    } else {
        summary.items
    };
    CodexTurn {
        id: summary.turn_id,
        items,
        status: match summary.status {
            TurnStatus::Pending => CodexTurnStatus::InProgress,
            TurnStatus::Completed => CodexTurnStatus::Completed,
            TurnStatus::Failed => CodexTurnStatus::Failed,
            TurnStatus::Cancelled => CodexTurnStatus::Interrupted,
        },
        error: summary.error.map(|message| CodexTurnError {
            message,
            codex_error_info: None,
            additional_details: None,
        }),
        started_at: None,
        completed_at: None,
        duration_ms: None,
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
    pending_server_requests: Vec<JsonRpcServerRequest>,
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

    pub fn emit_thread_started(&mut self, event: ThreadStartedEvent) {
        self.push(ServerNotification::thread_started(event));
    }

    pub fn emit_thread_status_changed(&mut self, event: ThreadStatusChangedEvent) {
        self.push(ServerNotification::thread_status_changed(event));
    }

    pub fn emit_thread_archived(&mut self, event: ThreadArchivedEvent) {
        self.push(ServerNotification::thread_archived(event));
    }

    pub fn emit_thread_unarchived(&mut self, event: ThreadUnarchivedEvent) {
        self.push(ServerNotification::thread_unarchived(event));
    }

    pub fn emit_thread_name_updated(&mut self, event: ThreadNameUpdatedEvent) {
        self.push(ServerNotification::thread_name_updated(event));
    }

    pub fn emit_thread_goal_updated(&mut self, event: ThreadGoalUpdatedEvent) {
        self.push(ServerNotification::thread_goal_updated(event));
    }

    pub fn emit_thread_goal_cleared(&mut self, event: ThreadGoalClearedEvent) {
        self.push(ServerNotification::thread_goal_cleared(event));
    }

    pub fn emit_thread_token_usage_updated(&mut self, event: ThreadTokenUsageUpdatedEvent) {
        self.push(ServerNotification::thread_token_usage_updated(event));
    }

    pub fn emit_thread_compacted(&mut self, event: ThreadCompactedEvent) {
        self.push(ServerNotification::thread_compacted(event));
    }

    pub fn emit_turn_started(&mut self, event: TurnStartedEvent) {
        self.push(ServerNotification::turn_started(event));
    }

    pub fn emit_turn_completed(&mut self, event: TurnCompletedEvent) {
        self.push(ServerNotification::turn_completed(event));
    }

    pub fn emit_turn_plan_updated(&mut self, event: TurnPlanUpdatedEvent) {
        self.push(ServerNotification::turn_plan_updated(event));
    }

    pub fn emit_turn_diff_updated(&mut self, event: TurnDiffUpdatedEvent) {
        self.push(ServerNotification::turn_diff_updated(event));
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

    pub fn emit_reasoning_summary_part_added(&mut self, event: ReasoningSummaryPartAddedEvent) {
        self.push(ServerNotification::reasoning_summary_part_added(event));
    }

    pub fn emit_reasoning_text_delta(&mut self, event: ReasoningTextDeltaEvent) {
        self.push(ServerNotification::reasoning_text_delta(event));
    }

    pub fn emit_raw_response_item_completed(&mut self, event: RawResponseItemCompletedEvent) {
        self.push(ServerNotification::raw_response_item_completed(event));
    }

    pub fn emit_plan_delta(&mut self, event: PlanDeltaEvent) {
        self.push(ServerNotification::plan_delta(event));
    }

    pub fn emit_item_completed(&mut self, event: ItemCompletedEvent) {
        self.push(ServerNotification::item_completed(event));
    }

    pub fn emit_server_request_resolved(&mut self, event: ServerRequestResolvedEvent) {
        self.push(ServerNotification::server_request_resolved(event));
    }

    pub fn emit_command_execution_output_delta(&mut self, event: CommandExecutionOutputDeltaEvent) {
        self.push(ServerNotification::command_execution_output_delta(event));
    }

    pub fn emit_command_execution_terminal_interaction(
        &mut self,
        event: CommandExecutionTerminalInteractionEvent,
    ) {
        self.push(ServerNotification::command_execution_terminal_interaction(
            event,
        ));
    }

    pub fn emit_file_change_output_delta(&mut self, event: FileChangeOutputDeltaEvent) {
        self.push(ServerNotification::file_change_output_delta(event));
    }

    pub fn emit_file_change_patch_updated(&mut self, event: FileChangePatchUpdatedEvent) {
        self.push(ServerNotification::file_change_patch_updated(event));
    }

    pub fn emit_auto_approval_review_started(&mut self, event: AutoApprovalReviewStartedEvent) {
        self.push(ServerNotification::auto_approval_review_started(event));
    }

    pub fn emit_auto_approval_review_completed(&mut self, event: AutoApprovalReviewCompletedEvent) {
        self.push(ServerNotification::auto_approval_review_completed(event));
    }

    pub fn emit_model_rerouted(&mut self, event: ModelReroutedNotification) {
        self.push(ServerNotification::model_rerouted(event));
    }

    pub fn emit_model_verification(&mut self, event: ModelVerificationNotification) {
        self.push(ServerNotification::model_verification(event));
    }

    pub fn emit_hook_started(&mut self, event: HookStartedNotification) {
        self.push(ServerNotification::hook_started(event));
    }

    pub fn emit_hook_completed(&mut self, event: HookCompletedNotification) {
        self.push(ServerNotification::hook_completed(event));
    }

    pub fn emit_warning(&mut self, event: WarningNotification) {
        self.push(ServerNotification::warning(event));
    }

    pub fn emit_guardian_warning(&mut self, event: GuardianWarningNotification) {
        self.push(ServerNotification::guardian_warning(event));
    }

    pub fn emit_config_warning(&mut self, event: ConfigWarningNotification) {
        self.push(ServerNotification::config_warning(event));
    }

    pub fn emit_deprecation_notice(&mut self, event: DeprecationNoticeNotification) {
        self.push(ServerNotification::deprecation_notice(event));
    }

    pub fn emit_log_entry(&mut self, event: LogEntryEvent) {
        self.push(ServerNotification::log_entry(event));
    }

    pub fn emit_skills_changed(&mut self, event: SkillsChangedNotification) {
        self.push(ServerNotification::skills_changed(event));
    }

    pub fn emit_mcp_oauth_login_completed(
        &mut self,
        event: McpServerOauthLoginCompletedNotification,
    ) {
        self.push(ServerNotification::mcp_server_oauth_login_completed(event));
    }

    pub fn emit_mcp_tool_call_progress(&mut self, event: McpToolCallProgressNotification) {
        self.push(ServerNotification::mcp_tool_call_progress(event));
    }

    pub fn emit_fs_changed(&mut self, event: FsChangedNotification) {
        self.push(ServerNotification::fs_changed(event));
    }

    pub fn emit_command_exec_output_delta(&mut self, event: CommandExecOutputDeltaNotification) {
        self.push(ServerNotification::command_exec_output_delta(event));
    }

    pub fn emit_mcp_startup_status_updated(&mut self, event: McpServerStatusUpdatedNotification) {
        self.push(ServerNotification::mcp_server_startup_status_updated(event));
    }

    pub fn emit_fuzzy_file_search_session_updated(
        &mut self,
        event: dasclaw_app_server_protocol::FuzzyFileSearchSessionUpdatedNotification,
    ) {
        self.push(ServerNotification::fuzzy_file_search_session_updated(event));
    }

    pub fn emit_fuzzy_file_search_session_completed(
        &mut self,
        event: dasclaw_app_server_protocol::FuzzyFileSearchSessionCompletedNotification,
    ) {
        self.push(ServerNotification::fuzzy_file_search_session_completed(
            event,
        ));
    }

    pub fn emit_server_request(&mut self, request: JsonRpcServerRequest) {
        if self.lag_disconnect_signaled {
            return;
        }
        if self.total_pending_len() >= DEFAULT_MAX_PENDING_NOTIFICATIONS as usize {
            self.pending.clear();
            self.pending_server_requests.clear();
            if let Ok(error) = ServerNotification::error(ErrorEvent {
                code: ErrorCode::NotificationQueueOverflow,
                message: "client notification queue exceeded bounded capacity; reconnect required"
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
        self.pending_server_requests.push(request);
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
        let server_requests = std::mem::take(&mut self.pending_server_requests);
        let mut lines = server_requests
            .into_iter()
            .filter_map(|request| serde_json::to_string(&request).ok())
            .collect::<Vec<_>>();
        lines.extend(
            drain
                .notifications
                .into_iter()
                .filter_map(|notification| serde_json::to_string(&notification).ok()),
        );
        JsonRpcNotificationDrain {
            lines,
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
            if self.total_pending_len() >= DEFAULT_MAX_PENDING_NOTIFICATIONS as usize {
                self.pending.clear();
                self.pending_server_requests.clear();
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

    fn total_pending_len(&self) -> usize {
        self.pending.len() + self.pending_server_requests.len()
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

fn history_import_unsupported() -> AppServerError {
    AppServerError::invalid_request(
        "thread_lifecycle",
        "history import is not supported by dasclaw app-server yet",
    )
}

fn token_usage_breakdown(usage: dasclaw_core::response_types::TokenUsage) -> TokenUsageBreakdown {
    let input_tokens = u64::from(usage.input_tokens);
    let output_tokens = u64::from(usage.output_tokens);
    let cached_input_tokens = u64::from(usage.cache_creation_input_tokens)
        .saturating_add(u64::from(usage.cache_read_input_tokens));
    TokenUsageBreakdown {
        total_tokens: input_tokens.saturating_add(output_tokens),
        input_tokens,
        cached_input_tokens,
        output_tokens,
        reasoning_output_tokens: 0,
    }
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

    fn with_capability(self, capability: impl Into<String>) -> Self {
        match self {
            Self::Protocol { mut data } => {
                data.capability = Some(capability.into());
                Self::Protocol { data }
            }
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

    fn capability_unavailable(capability: impl Into<String>, message: impl Into<String>) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::CapabilityUnavailable,
            message: message.into(),
            lifecycle: None,
            capability: Some(capability.into()),
            retryable: false,
        })
    }

    fn service_degraded(capability: impl Into<String>, message: impl Into<String>) -> Self {
        Self::protocol(ErrorData {
            code: ErrorCode::ServiceDegraded,
            message: message.into(),
            lifecycle: None,
            capability: Some(capability.into()),
            retryable: false,
        })
    }

    fn public_message(&self) -> &str {
        match self {
            Self::Protocol { data } => &data.message,
        }
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

fn hook_lifecycle_error_to_app_server_error(
    hook_point: &str,
    error: dasclaw_hooks::HookError,
) -> AppServerError {
    let message = match error {
        dasclaw_hooks::HookError::Rejected { reason } => {
            format!("{hook_point} hook rejected request: {reason}")
        }
        dasclaw_hooks::HookError::ExecutionFailed { reason } => {
            format!("{hook_point} hook failed: {reason}")
        }
        dasclaw_hooks::HookError::Timeout { timeout } => {
            format!("{hook_point} hook timed out after {timeout:?}")
        }
    };
    AppServerError::service_degraded("hooks", message)
}

fn warning_from_sandbox_context(
    thread_id: &str,
    context: &RuntimeSandboxContext,
) -> Result<Option<WarningNotification>, AppServerError> {
    let Some(policy) = context.policy.as_ref() else {
        return Ok(None);
    };
    let policy = crate::sandbox_protocol::codex_policy_from_workspace(policy)?;
    let Some(message) = dasclaw_sandboxing::system_bwrap_warning(&policy) else {
        return Ok(None);
    };
    Ok(Some(WarningNotification {
        thread_id: Some(thread_id.to_string()),
        message,
    }))
}

fn generic_warning_producer_available() -> bool {
    cfg!(target_os = "linux")
}

fn guardian_warning_from_auto_approval_review(
    thread_id: &str,
    update: &RuntimeAutoApprovalReviewUpdate,
) -> Option<GuardianWarningNotification> {
    let status = normalized_policy_label(&update.review.status);
    let risk = update
        .review
        .risk_level
        .as_deref()
        .map(normalized_policy_label);
    let should_warn = matches!(
        status.as_str(),
        "blocked" | "denied" | "rejected" | "timeout" | "timedout" | "failed" | "aborted"
    ) || matches!(risk.as_deref(), Some("high") | Some("critical"));
    if !should_warn {
        return None;
    }

    let message = update.review.rationale.clone().unwrap_or_else(|| {
        format!(
            "Guardian review {} for {}",
            update.review.status, update.action
        )
    });
    Some(GuardianWarningNotification {
        thread_id: thread_id.to_string(),
        message,
    })
}

fn normalized_policy_label(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

fn compatibility_profiles() -> Vec<CompatibilityProfile> {
    vec![CompatibilityProfile::codex_app_server_v2_chat_session_subset()]
}

fn review_start_prompt(target: &ReviewTarget) -> Result<String, AppServerError> {
    let label = match target {
        ReviewTarget::UncommittedChanges => "uncommitted changes".to_string(),
        ReviewTarget::BaseBranch { branch } => {
            format!("changes against base branch {}", branch.trim())
        }
        ReviewTarget::Commit { sha, title } => {
            let mut label = format!("commit {}", sha.trim());
            if let Some(title) = title
                .as_ref()
                .map(|title| title.trim())
                .filter(|title| !title.is_empty())
            {
                label.push_str(": ");
                label.push_str(title);
            }
            label
        }
        ReviewTarget::Custom { instructions } => {
            let trimmed = instructions.trim();
            if trimmed.is_empty() {
                return Err(AppServerError::invalid_request(
                    "review",
                    "custom review instructions must not be empty",
                ));
            }
            trimmed.to_string()
        }
    };

    Ok(format!(
        "Review request: {label}\n\nFocus on correctness, regressions, safety, and missing tests. Return findings first with file and line references."
    ))
}

pub fn supported_methods() -> &'static [&'static str] {
    &[
        method::INITIALIZE,
        method::PROTOCOL_SCHEMA,
        method::CONFIG_REQUIREMENTS_READ,
        method::CONFIG_READ,
        method::CONFIG_VALUE_WRITE,
        method::CONFIG_BATCH_WRITE,
        method::GIT_DIFF_TO_REMOTE,
        method::FUZZY_FILE_SEARCH,
        method::HEALTH_CHECK,
        method::CAPABILITIES_LIST,
        method::LIFECYCLE_STATUS,
        method::SHUTDOWN,
        method::GET_CONVERSATION_SUMMARY,
        method::REVIEW_START,
        method::THREAD_START,
        method::THREAD_LIST,
        method::THREAD_READ,
        method::THREAD_RESUME,
        method::THREAD_FORK,
        method::THREAD_ARCHIVE,
        method::THREAD_UNARCHIVE,
        method::THREAD_UNSUBSCRIBE,
        method::THREAD_NAME_SET,
        method::THREAD_METADATA_UPDATE,
        method::THREAD_ROLLBACK,
        method::THREAD_LOADED_LIST,
        method::THREAD_INJECT_ITEMS,
        method::THREAD_GOAL_SET,
        method::THREAD_GOAL_GET,
        method::THREAD_GOAL_CLEAR,
        method::THREAD_COMPACT_START,
        method::TURN_START,
        method::TURN_STEER,
        method::TURN_INTERRUPT,
        method::THREAD_TURNS_LIST,
        method::TURN_READ,
        method::MODEL_LIST,
        method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN,
        method::JOBS_LIST,
        method::JOBS_READ,
        method::SKILLS_LIST,
        method::SKILLS_CONFIG_WRITE,
        method::MCP_SERVER_OAUTH_LOGIN,
        method::CONFIG_MCP_SERVER_RELOAD,
        method::MCP_SERVER_STATUS_LIST,
        method::MCP_SERVER_RESOURCE_READ,
        method::MCP_SERVER_TOOL_CALL,
        method::FS_READ_FILE,
        method::FS_WRITE_FILE,
        method::FS_CREATE_DIRECTORY,
        method::FS_GET_METADATA,
        method::FS_READ_DIRECTORY,
        method::FS_REMOVE,
        method::FS_COPY,
        method::FS_WATCH,
        method::FS_UNWATCH,
        method::COMMAND_EXEC,
        method::COMMAND_EXEC_WRITE,
        method::COMMAND_EXEC_TERMINATE,
        method::COMMAND_EXEC_RESIZE,
        method::APPROVAL_RESPOND,
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
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use dasclaw_app_server_protocol::{
        CapabilityStatus, CommandExecOutputDeltaNotification, CommandExecOutputStream,
        CommandExecTerminalSize, DynamicToolCallOutputContentItem, FsChangedKind,
        FsChangedNotification, HookEventName, HookExecutionMode, HookHandlerType, HookOutputEntry,
        HookOutputEntryKind, HookRunStatus, HookRunSummary, HookScope, HookSource, ServiceStatus,
        SkillMetadata, SkillScope, SkillsListEntry, TransportKind, UserInput, WorkspaceInfo,
        WorkspaceTrust, event, server_request,
    };
    use dasclaw_core::messages::{FinishReason, ToolCall, ToolDefinition, ToolResult};
    use dasclaw_core::reasoning_ctx::ReasoningContext;
    use dasclaw_core::response_types::{
        RespondOutput, RespondResult, ResponseMetadata, ResponseModelVerification, TokenUsage,
    };
    use dasclaw_core::traits::HostError;
    use dasclaw_observability::{Observer, ObserverEvent};
    use secrecy::ExposeSecret;
    use tokio::sync::mpsc;

    use super::*;

    const TEST_MODEL_API_BASE_URL: &str = "https://api.test/v1";

    #[test]
    fn app_server_noop_services_remain_disabled_and_declared() {
        let mut server = AppServer::new();

        let capabilities = server.capabilities();
        assert_eq!(
            capabilities.capabilities.logs.status,
            CapabilityStatus::Declared
        );
        assert_eq!(
            capabilities.capabilities.jobs.status,
            CapabilityStatus::Declared
        );
        assert_eq!(
            capabilities.capabilities.skills.status,
            CapabilityStatus::Declared
        );
        assert_eq!(
            capabilities.capabilities.mcp.status,
            CapabilityStatus::Declared
        );

        let health = server.health_check(HealthCheckParams {
            include_details: true,
        });
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Logs && service.status == ServiceStatus::Disabled
        }));
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Jobs && service.status == ServiceStatus::Disabled
        }));
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Skills && service.status == ServiceStatus::Disabled
        }));
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Mcp && service.status == ServiceStatus::Disabled
        }));
    }

    #[test]
    fn real_r6_services_advertise_only_wired_notification_producers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let server = initialized_server_with_root(temp.path());
        let capabilities = &server.capabilities;

        assert_eq!(capabilities.config.status, CapabilityStatus::Implemented);
        assert_eq!(capabilities.repo.status, CapabilityStatus::Implemented);
        assert_eq!(capabilities.search.status, CapabilityStatus::Implemented);
        assert_eq!(capabilities.review.status, CapabilityStatus::Implemented);
        assert!(capabilities.model_provider.events.is_empty());
        assert_eq!(capabilities.hooks.status, CapabilityStatus::Implemented);
        assert_eq!(
            capabilities.hooks.events,
            vec![
                event::HOOK_STARTED.to_string(),
                event::HOOK_COMPLETED.to_string()
            ]
        );
        assert_eq!(capabilities.warnings.status, CapabilityStatus::Implemented);
        assert_eq!(capabilities.warnings.events, expected_warning_events(false));
    }

    #[test]
    fn runtime_features_gate_r1_capability_advertising() {
        let bridge = Arc::new(R1RuntimeBridge::default());
        let server = initialized_server_with_bridge(bridge);

        assert_eq!(
            server.capabilities.approval.status,
            CapabilityStatus::Implemented
        );
        assert!(
            server
                .capabilities
                .approval
                .events
                .contains(&server_request::ITEM_TOOL_CALL.to_string())
        );
        assert!(
            server
                .capabilities
                .approval
                .events
                .contains(&server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL.to_string())
        );
        assert!(
            server
                .capabilities
                .tools
                .events
                .contains(&event::ITEM_FILE_CHANGE_PATCH_UPDATED.to_string())
        );
    }

    #[test]
    fn runtime_features_gate_turn_steer_capability_advertising() {
        let default_bridge = Arc::new(RecordingRuntimeBridge::default());
        let default_server = initialized_server_with_bridge(default_bridge);
        assert!(
            !default_server
                .capabilities
                .session
                .methods
                .contains(&method::TURN_STEER.to_string()),
            "default runtime bridge must not advertise steer capability"
        );

        let steer_bridge = Arc::new(RecordingRuntimeBridge::with_turn_steer());
        let steer_server = initialized_server_with_bridge(steer_bridge);
        assert!(
            steer_server
                .capabilities
                .session
                .methods
                .contains(&method::TURN_STEER.to_string()),
            "steer-capable runtime bridge should advertise turn/steer"
        );
    }

    #[test]
    fn default_dasclaw_runtime_bridge_advertises_turn_steer_after_real_injection_exists() {
        let bridge = DasclawAgentRuntimeBridge::from_model_provider_snapshot();
        assert!(
            bridge.features().turn_steer,
            "default runtime bridge should advertise turn steer once it can inject input into a running turn"
        );
    }

    #[test]
    fn dasclaw_runtime_bridge_steer_turn_injects_input_into_running_turn() {
        struct SteerAwareResponder {
            calls: Mutex<usize>,
            second_call_users: Arc<Mutex<Vec<String>>>,
        }

        #[async_trait::async_trait]
        impl dasclaw_runtime::AgentResponder for SteerAwareResponder {
            async fn respond(
                &self,
                ctx: &mut ReasoningContext,
            ) -> Result<RespondOutput, HostError> {
                let mut calls = self.calls.lock().expect("calls lock");
                *calls += 1;
                if *calls == 1 {
                    return Ok(tool_call_output("echo", "call_1"));
                }

                let users = ctx
                    .messages
                    .iter()
                    .filter(|message| message.role == dasclaw_core::messages::Role::User)
                    .map(|message| message.content.clone())
                    .collect::<Vec<_>>();
                *self
                    .second_call_users
                    .lock()
                    .expect("second call users lock") = users;
                Ok(text_output("steered answer"))
            }
        }

        struct BlockingExecutor {
            release: Arc<tokio::sync::Notify>,
        }

        #[async_trait::async_trait]
        impl dasclaw_runtime::ToolExecutor for BlockingExecutor {
            async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
                self.release.notified().await;
                Ok(ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    content: "tool output".to_string(),
                    is_error: false,
                })
            }
        }

        let second_call_users = Arc::new(Mutex::new(Vec::new()));
        let release_tool = Arc::new(tokio::sync::Notify::new());
        let users_for_factory = Arc::clone(&second_call_users);
        let release_for_factory = Arc::clone(&release_tool);
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(move |token| {
            dasclaw_runtime::Agent::builder()
                .responder(SteerAwareResponder {
                    calls: Mutex::new(0),
                    second_call_users: Arc::clone(&users_for_factory),
                })
                .tool_executor(BlockingExecutor {
                    release: Arc::clone(&release_for_factory),
                })
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        }));
        let mut server = initialized_codex_v2_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("first prompt".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let tool_start = drain_until_method(
            &mut server,
            event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
            Duration::from_secs(2),
        );
        assert!(
            tool_start.iter().any(|notification| {
                notification.method == event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA
                    && notification.params["turnId"] == started.turn.id
            }),
            "runtime tool start should be observable before steering: {tool_start:?}"
        );

        server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id.clone(),
                expected_turn_id: started.turn.id.clone(),
                input: text_input("extra context one".to_string()),
                responsesapi_client_metadata: None,
            })
            .expect("real runtime bridge should accept first turn/steer");
        server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id.clone(),
                expected_turn_id: started.turn.id.clone(),
                input: text_input("extra context two".to_string()),
                responsesapi_client_metadata: None,
            })
            .expect("real runtime bridge should accept second turn/steer");
        release_tool.notify_one();

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "turn/completed"),
            "turn should complete after steered second iteration: {notifications:?}"
        );
        assert_eq!(
            *second_call_users.lock().expect("second call users lock"),
            vec![
                "first prompt".to_string(),
                "extra context one".to_string(),
                "extra context two".to_string(),
            ]
        );
    }

    #[test]
    fn default_runtime_bridge_advertises_r2_request_owner_capabilities() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::from_model_provider_snapshot());
        let server = initialized_server_with_bridge(bridge);

        assert_eq!(
            server.capabilities.approval.status,
            CapabilityStatus::Implemented
        );
        for event in [
            server_request::ITEM_TOOL_CALL,
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
            server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
        ] {
            assert!(
                server
                    .capabilities
                    .approval
                    .events
                    .contains(&event.to_string()),
                "default runtime bridge should advertise R2 request owner event {event}"
            );
        }
    }

    #[test]
    fn r1_runtime_updates_emit_server_requests_and_notifications() {
        let bridge = Arc::new(R1ProducingBridge);
        let mut server = initialized_server_with_bridge(bridge);
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let _turn = server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("run r1 producers".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let values = json_rpc_values(server.drain_json_rpc_notifications());
        let methods = values
            .iter()
            .filter_map(|value| value["method"].as_str())
            .collect::<Vec<_>>();

        assert!(methods.contains(&"item/tool/call"));
        assert!(methods.contains(&"item/tool/requestUserInput"));
        assert!(methods.contains(&"item/fileChange/requestApproval"));
        assert!(methods.contains(&"item/permissions/requestApproval"));
        assert!(methods.contains(&"item/fileChange/outputDelta"));
        assert!(methods.contains(&"item/fileChange/patchUpdated"));
        assert!(methods.contains(&"item/autoApprovalReview/started"));
        assert!(methods.contains(&"item/autoApprovalReview/completed"));
        assert!(values.iter().any(|value| {
            value["method"] == "item/tool/call"
                && value["params"]["tool"] == "open_url"
                && value["params"]["arguments"]["url"] == "https://example.test"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/fileChange/patchUpdated"
                && value["params"]["changes"][0]["path"] == "src/lib.rs"
        }));
    }

    #[test]
    fn auto_approval_review_completed_approved_low_does_not_emit_guardian_warning() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([
            RuntimeTurnOutcome::AutoApprovalReviewCompleted {
                update: auto_approval_review_update("approved", Some("low"), Some("within policy")),
            },
        ]));
        let mut server = initialized_server_with_bridge(bridge);
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;

        server
            .turn_start(TurnStartParams {
                thread_id,
                input: text_input("review low risk".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let values = json_rpc_values(server.drain_json_rpc_notifications());
        let methods = methods_from_values(&values);
        assert!(methods.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED));
        assert!(!methods.contains(&event::GUARDIAN_WARNING));
    }

    #[test]
    fn auto_approval_review_completed_warns_for_blocked_and_high_risk_reviews() {
        for (status, risk) in [
            ("blocked", Some("low")),
            ("approved", Some("high")),
            ("approved", Some("critical")),
            ("timedOut", None),
            ("timed_out", None),
            ("aborted", None),
        ] {
            let bridge = Arc::new(SequencedRuntimeBridge::new([
                RuntimeTurnOutcome::AutoApprovalReviewCompleted {
                    update: auto_approval_review_update(
                        status,
                        risk,
                        Some("guardian stopped risky write"),
                    ),
                },
            ]));
            let mut server = initialized_server_with_bridge(bridge);
            let thread_id = server
                .create_thread_for_test(TestThreadParams { cwd: None })
                .expect("thread should be created")
                .thread_id;

            server
                .turn_start(TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: text_input(format!("review {status} {risk:?}")),
                    cwd: None,
                    model: None,
                    summary: None,
                    sandbox_policy: None,
                    permission_profile: None,
                })
                .expect("turn should start");

            let values = json_rpc_values(server.drain_json_rpc_notifications());
            let methods = methods_from_values(&values);
            assert!(
                methods.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED),
                "completed review should still be emitted for {status}/{risk:?}: {methods:?}"
            );
            assert!(
                methods.contains(&event::GUARDIAN_WARNING),
                "guardian warning should be emitted for {status}/{risk:?}: {methods:?}"
            );
            let warning = values
                .iter()
                .find(|value| value["method"] == event::GUARDIAN_WARNING)
                .expect("guardian warning");
            assert_eq!(warning["params"]["threadId"], thread_id);
            assert_eq!(warning["params"]["message"], "guardian stopped risky write");
        }
    }

    #[test]
    fn guardian_warning_message_falls_back_when_review_has_no_rationale() {
        let update = auto_approval_review_update("failed", None, None);
        let warning =
            guardian_warning_from_auto_approval_review("thread-1", &update).expect("warning");

        assert_eq!(warning.thread_id, "thread-1");
        assert_eq!(warning.message, "Guardian review failed for apply_patch");
    }

    #[test]
    fn r6_guardian_warning_readiness_requires_auto_approval_review_support() {
        let mut unsupported = initialized_server_with_bridge(Arc::new(NoopRuntimeBridge));
        let unsupported_capabilities = unsupported.capabilities().capabilities;
        assert!(
            !unsupported_capabilities
                .warnings
                .events
                .contains(&event::GUARDIAN_WARNING.to_string())
        );

        let mut supported =
            initialized_server_with_bridge(Arc::new(AutoApprovalReviewFeatureBridge));
        let supported_capabilities = supported.capabilities().capabilities;
        assert!(
            supported_capabilities
                .warnings
                .events
                .contains(&event::GUARDIAN_WARNING.to_string())
        );
    }

    #[test]
    fn r5_runtime_updates_emit_plan_diff_raw_and_plan_delta_notifications() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([
            RuntimeTurnOutcome::PlanUpdated {
                update: RuntimeTurnPlanUpdate {
                    explanation: Some("Adjusting plan".to_string()),
                    plan: vec![dasclaw_app_server_protocol::TurnPlanStep {
                        step: "inspect current producer".to_string(),
                        status: dasclaw_app_server_protocol::TurnPlanStepStatus::InProgress,
                    }],
                },
            },
            RuntimeTurnOutcome::DiffUpdated {
                update: RuntimeTurnDiffUpdate {
                    diff: "--- a/file\n+++ b/file\n".to_string(),
                },
            },
            RuntimeTurnOutcome::RawResponseItemCompleted {
                update: RuntimeRawResponseItemCompletedUpdate {
                    item: serde_json::json!({
                        "id": "raw_item_1",
                        "type": "message",
                        "content": "raw provider item",
                    }),
                },
            },
            RuntimeTurnOutcome::PlanDelta {
                update: RuntimePlanDeltaUpdate {
                    item_id: "plan_item_1".to_string(),
                    delta: "add verification".to_string(),
                },
            },
        ]));
        let mut server = initialized_server_with_bridge(bridge);
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("run r5 producers".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let values = json_rpc_values(server.drain_json_rpc_notifications());

        assert!(values.iter().any(|value| {
            value["method"] == event::TURN_PLAN_UPDATED
                && value["params"]["threadId"] == thread_id
                && value["params"]["turnId"] == turn.turn.id
                && value["params"]["explanation"] == "Adjusting plan"
                && value["params"]["plan"][0]["step"] == "inspect current producer"
                && value["params"]["plan"][0]["status"] == "inProgress"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::TURN_DIFF_UPDATED
                && value["params"]["threadId"] == thread_id
                && value["params"]["turnId"] == turn.turn.id
                && value["params"]["diff"] == "--- a/file\n+++ b/file\n"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::RAW_RESPONSE_ITEM_COMPLETED
                && value["params"]["threadId"] == thread_id
                && value["params"]["turnId"] == turn.turn.id
                && value["params"]["item"]["id"] == "raw_item_1"
                && value["params"]["item"]["content"] == "raw provider item"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::ITEM_PLAN_DELTA
                && value["params"]["threadId"] == thread_id
                && value["params"]["turnId"] == turn.turn.id
                && value["params"]["itemId"] == "plan_item_1"
                && value["params"]["delta"] == "add verification"
        }));
    }

    #[test]
    fn app_server_ready_services_drive_capabilities_and_health_together() {
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);

        let capabilities = server.capabilities();
        assert_eq!(
            capabilities.capabilities.logs.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            capabilities.capabilities.jobs.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            capabilities.capabilities.skills.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            capabilities.capabilities.mcp.status,
            CapabilityStatus::Implemented
        );

        let health = server.health_check(HealthCheckParams {
            include_details: true,
        });
        for service_name in [
            ServiceName::Logs,
            ServiceName::Jobs,
            ServiceName::Skills,
            ServiceName::Mcp,
        ] {
            assert!(health.services.iter().any(|service| {
                service.service == service_name && service.status == ServiceStatus::Ready
            }));
        }
    }

    #[test]
    fn app_server_noop_json_rpc_routes_return_capability_unavailable() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"jobs","method":"jobs/list","params":{}}"#)
            .expect("jobs/list should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("jobs/list response JSON");

        assert_eq!(value["id"], "jobs");
        assert_ne!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert_eq!(value["error"]["data"]["capability"], "jobs");
    }

    #[test]
    fn app_server_p5_json_rpc_routes_return_capability_unavailable() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"fs-read","method":"fs/readFile","params":{"path":"/tmp/missing.txt"}}"#,
            )
            .expect("fs/readFile should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("fs/readFile response JSON");

        assert_eq!(value["id"], "fs-read");
        assert_ne!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert_eq!(value["error"]["data"]["capability"], "filesystem");
    }

    #[test]
    fn app_server_ready_json_rpc_routes_reach_service_owner() {
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![SkillsListEntry {
                cwd: "/workspace".into(),
                skills: vec![SkillMetadata {
                    name: "review".into(),
                    description: "Review local code".into(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: "/workspace/.codex/skills/review/SKILL.md".into(),
                    scope: SkillScope::Repo,
                    enabled: true,
                }],
                errors: vec![],
            }]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let jobs = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"jobs","method":"jobs/list","params":{}}"#)
            .expect("jobs/list should return a structured response");
        let skills = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"skills","method":"skills/list","params":{"cwds":["/workspace"]}}"#,
            )
            .expect("skills/list should return a structured response");
        let write = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"write","method":"skills/config/write","params":{"name":"review","enabled":false}}"#,
            )
            .expect("skills/config/write should return a structured response");
        let notifications = server.drain_notifications();

        let jobs_value: Value = serde_json::from_str(&jobs).expect("jobs/list response JSON");
        let skills_value: Value = serde_json::from_str(&skills).expect("skills/list response JSON");
        let write_value: Value = serde_json::from_str(&write).expect("skills write response JSON");

        assert_eq!(jobs_value["result"]["data"], serde_json::json!([]));
        assert_eq!(
            skills_value["result"]["data"][0]["skills"][0]["name"],
            "review"
        );
        assert_eq!(write_value["result"]["effectiveEnabled"], false);
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == event::SKILLS_CHANGED)
        );
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

        let capabilities = server.capabilities();
        assert_eq!(
            capabilities.capabilities.filesystem.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            capabilities.capabilities.command_exec.status,
            CapabilityStatus::Implemented
        );

        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let read = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"fs-read","method":"fs/readFile","params":{"path":"/tmp/example.txt"}}"#,
            )
            .expect("fs/readFile should return a structured response");
        let exec = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"cmd","method":"command/exec","params":{"command":["printf","test"]}}"#,
            )
            .expect("command/exec should return a structured response");

        let read_value: Value = serde_json::from_str(&read).expect("fs/readFile response JSON");
        let exec_value: Value = serde_json::from_str(&exec).expect("command/exec response JSON");
        assert_eq!(read_value["result"]["dataBase64"], "dGVzdA==");
        assert_eq!(exec_value["result"]["stdout"], "test");
    }

    #[test]
    fn config_read_returns_redacted_config_and_layers() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_app_server_config(
            temp.path(),
            serde_json::json!({
                "model": "gpt-test",
                "api_key": "secret-api-key",
                "private_key": "secret-private-key",
                "provider": {
                    "token": "secret-token",
                    "name": "openai"
                }
            }),
        );
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":10,"method":"config/read","params":{"includeLayers":true}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["result"]["config"]["model"], "gpt-test");
        assert_eq!(value["result"]["config"]["api_key"], "<redacted>");
        assert_eq!(value["result"]["config"]["private_key"], "<redacted>");
        assert_eq!(value["result"]["config"]["provider"]["token"], "<redacted>");
        assert_eq!(value["result"]["config"]["provider"]["name"], "openai");
        assert!(
            !value["result"]["layers"]
                .as_array()
                .expect("layers")
                .is_empty()
        );
        assert!(!value.to_string().contains("secret-api-key"));
        assert!(!value.to_string().contains("secret-private-key"));
        assert!(!value.to_string().contains("secret-token"));
    }

    #[test]
    fn config_read_runtime_warning_drains_to_json_rpc_notification() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        write_app_server_config(temp.path(), serde_json::json!({"unknown_key": true}));

        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":10,"method":"config/read","params":{"includeLayers":false}}"#,
            )
            .expect("response");
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(methods_from_values(&notifications), vec!["configWarning"]);
        assert_eq!(
            notifications[0]["params"]["summary"],
            "unsupported config key"
        );
        assert!(
            notifications[0]["params"]["details"]
                .as_str()
                .expect("details")
                .contains("unknown_key")
        );
    }

    #[test]
    fn config_write_rejects_key_outside_policy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":11,"method":"config/value/write","params":{"keyPath":"api_key","value":"secret","mergeStrategy":"replace"}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("not writable")
        );
    }

    #[test]
    fn config_write_rejects_version_mismatch_with_stable_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_app_server_config(temp.path(), serde_json::json!({"model": "gpt-test"}));
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":12,"method":"config/value/write","params":{"keyPath":"model","value":"gpt-next","mergeStrategy":"replace","expectedVersion":"stale-version"}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert_eq!(value["error"]["message"], "config version mismatch");
    }

    #[test]
    fn config_write_rejects_nested_secret_key_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":12,"method":"config/value/write","params":{"keyPath":"model.api_key","value":"secret","mergeStrategy":"replace"}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("secret")
        );
    }

    #[test]
    fn config_batch_write_rejects_nested_secret_key_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":13,"method":"config/batchWrite","params":{"edits":[{"keyPath":"model.token","value":"secret","mergeStrategy":"replace"}]}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("secret")
        );
    }

    #[test]
    fn config_read_uses_cwd_project_config_inside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = temp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        write_app_server_config(temp.path(), serde_json::json!({"model": "root-model"}));
        write_app_server_config(&project, serde_json::json!({"model": "project-model"}));
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":14,"method":"config/read","params":{{"includeLayers":true,"cwd":{}}}}}"#,
                serde_json::to_string(&project.to_string_lossy()).expect("cwd json")
            ))
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["result"]["config"]["model"], "project-model");
        assert_eq!(
            value["result"]["layers"][0]["config"]["model"],
            "project-model"
        );
    }

    #[test]
    fn get_conversation_summary_returns_thread_card_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cwd = temp.path().join("workspace");
        fs::create_dir_all(&cwd).expect("workspace dir");
        let cwd_json = serde_json::to_string(&cwd.to_string_lossy()).expect("cwd json");
        let mut server = initialized_server_with_root(temp.path());

        let start_response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{{"cwd":{cwd_json}}}}}"#
            ))
            .expect("thread/start should return a response");
        let start_value: serde_json::Value =
            serde_json::from_str(&start_response).expect("thread/start response JSON");
        let thread = &start_value["result"]["thread"];
        let thread_id = thread["id"].as_str().expect("thread id");

        let summary_response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"summary","method":"{}","params":{{"conversationId":"{}"}}}}"#,
                method::GET_CONVERSATION_SUMMARY,
                thread_id
            ))
            .expect("getConversationSummary should return a response");
        let summary_value: serde_json::Value =
            serde_json::from_str(&summary_response).expect("summary response JSON");
        let summary = &summary_value["result"]["summary"];

        assert_eq!(summary["conversationId"], thread["id"]);
        assert_eq!(summary["path"], "");
        assert_eq!(summary["preview"], "");
        assert_eq!(summary["timestamp"], thread["createdAt"].to_string());
        assert_eq!(summary["updatedAt"], thread["updatedAt"].to_string());
        assert_eq!(summary["modelProvider"], "openai");
        assert_eq!(summary["cwd"], cwd.to_string_lossy().as_ref());
        assert_eq!(summary["cliVersion"], SERVER_VERSION);
        assert_eq!(summary["source"], "unknown");
        assert_eq!(summary["gitInfo"], serde_json::Value::Null);
    }

    #[test]
    fn get_conversation_summary_returns_rollout_path_thread_card_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let snapshot_path = temp.path().join("threads.json");
        let rollout_path = temp.path().join("rollout.jsonl");
        let rollout_path = rollout_path.to_string_lossy();
        let mut server = initialized_server_with_thread_snapshot(
            temp.path(),
            &snapshot_path,
            serde_json::json!({
                "version": 1,
                "nextThreadId": 2,
                "nextTurnId": 1,
                "threads": [{
                    "threadId": "thread_1",
                    "title": "Loaded conversation",
                    "workspaceRoot": "/workspace",
                    "sandbox": null,
                    "permissionProfile": null,
                    "forkedFromId": null,
                    "ephemeral": false,
                    "archived": false,
                    "subscribed": false,
                    "path": rollout_path.as_ref(),
                    "createdAt": 11,
                    "updatedAt": 22,
                    "gitInfo": {
                        "sha": "abc123",
                        "branch": "main",
                        "originUrl": "https://example.invalid/repo.git"
                    },
                    "goal": null,
                    "compactedTurnId": null,
                    "tokenUsage": null
                }],
                "turns": []
            }),
        );

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"summary","method":"{}","params":{{"rolloutPath":{}}}}}"#,
                method::GET_CONVERSATION_SUMMARY,
                serde_json::to_string(rollout_path.as_ref()).expect("rollout path json")
            ))
            .expect("getConversationSummary should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("summary response JSON");
        let summary = &value["result"]["summary"];

        assert_eq!(summary["conversationId"], "thread_1");
        assert_eq!(summary["path"], rollout_path.as_ref());
        assert_eq!(summary["preview"], "Loaded conversation");
        assert_eq!(summary["timestamp"], "11");
        assert_eq!(summary["updatedAt"], "22");
        assert_eq!(summary["modelProvider"], "openai");
        assert_eq!(summary["cwd"], "/workspace");
        assert_eq!(summary["cliVersion"], SERVER_VERSION);
        assert_eq!(summary["source"], "unknown");
        assert_eq!(summary["gitInfo"]["sha"], "abc123");
        assert_eq!(summary["gitInfo"]["branch"], "main");
        assert_eq!(
            summary["gitInfo"]["originUrl"],
            "https://example.invalid/repo.git"
        );
    }

    #[test]
    fn get_conversation_summary_rejects_unknown_conversation_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"summary","method":"{}","params":{{"conversationId":"missing"}}}}"#,
                method::GET_CONVERSATION_SUMMARY
            ))
            .expect("getConversationSummary should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("summary response JSON");

        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn get_conversation_summary_rejects_unknown_rollout_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"summary","method":"{}","params":{{"rolloutPath":"/tmp/missing.jsonl"}}}}"#,
                method::GET_CONVERSATION_SUMMARY
            ))
            .expect("getConversationSummary should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("summary response JSON");

        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn get_conversation_summary_requires_initialized_server() {
        let mut server = AppServer::new();

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"summary","method":"{}","params":{{"conversationId":"thread_1"}}}}"#,
                method::GET_CONVERSATION_SUMMARY
            ))
            .expect("getConversationSummary should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("summary response JSON");

        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn config_read_rejects_cwd_outside_root() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let mut server = initialized_server_with_root(root.path());
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":15,"method":"config/read","params":{{"includeLayers":false,"cwd":{}}}}}"#,
                serde_json::to_string(&outside.path().to_string_lossy()).expect("cwd json")
            ))
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("outside")
        );
    }

    #[test]
    fn config_write_rejects_file_path_outside_root_as_capability_unavailable() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("app-server-config.json");
        let mut server = initialized_server_with_root(root.path());
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":16,"method":"config/value/write","params":{{"keyPath":"model","value":"gpt-test","mergeStrategy":"replace","filePath":{}}}}}"#,
                serde_json::to_string(&outside_file.to_string_lossy()).expect("file path json")
            ))
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("outside")
        );
    }

    #[cfg(unix)]
    #[test]
    fn config_read_rejects_user_config_symlink_escape() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        write_app_server_config(
            outside.path(),
            serde_json::json!({"model": "outside-model"}),
        );
        std::os::unix::fs::symlink(
            outside.path().join(".dasclaw"),
            root.path().join(".dasclaw"),
        )
        .expect("symlink .dasclaw");
        let mut server = initialized_server_with_root(root.path());

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":17,"method":"config/read","params":{"includeLayers":false}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("outside")
        );
        assert!(!value.to_string().contains("outside-model"));
    }

    #[cfg(unix)]
    #[test]
    fn config_write_rejects_user_config_symlink_escape() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        write_app_server_config(
            outside.path(),
            serde_json::json!({"model": "outside-model"}),
        );
        std::os::unix::fs::symlink(
            outside.path().join(".dasclaw"),
            root.path().join(".dasclaw"),
        )
        .expect("symlink .dasclaw");
        let mut server = initialized_server_with_root(root.path());

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":18,"method":"config/value/write","params":{"keyPath":"model","value":"inside-model","mergeStrategy":"replace"}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");

        let outside_config = fs::read_to_string(
            outside
                .path()
                .join(".dasclaw")
                .join("app-server-config.json"),
        )
        .expect("outside config");
        assert!(outside_config.contains("outside-model"));
        assert!(!outside_config.contains("inside-model"));
    }

    #[cfg(unix)]
    #[test]
    fn config_read_rejects_project_config_symlink_escape() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let project = root.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        write_app_server_config(
            outside.path(),
            serde_json::json!({"model": "outside-model"}),
        );
        std::os::unix::fs::symlink(outside.path().join(".dasclaw"), project.join(".dasclaw"))
            .expect("symlink project .dasclaw");
        let mut server = initialized_server_with_root(root.path());

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":19,"method":"config/read","params":{{"includeLayers":true,"cwd":{}}}}}"#,
                serde_json::to_string(&project.to_string_lossy()).expect("cwd json")
            ))
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert!(!value.to_string().contains("outside-model"));
    }

    #[test]
    fn config_write_rejects_secret_keys_inside_object_value() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":20,"method":"config/value/write","params":{"keyPath":"model","value":{"api_key":"secret"},"mergeStrategy":"replace"}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("secret")
        );
    }

    #[test]
    fn config_batch_write_rejects_secret_keys_inside_nested_object_value() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":21,"method":"config/batchWrite","params":{"edits":[{"keyPath":"profile","value":{"nested":{"token":"secret"}},"mergeStrategy":"upsert"}]}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "config");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("secret")
        );
    }

    #[test]
    fn config_write_persists_complete_json_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(temp.path());
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":22,"method":"config/value/write","params":{"keyPath":"model","value":"gpt-test","mergeStrategy":"replace"}}"#,
            )
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["result"]["config"]["model"], "gpt-test");

        let written =
            fs::read_to_string(temp.path().join(".dasclaw").join("app-server-config.json"))
                .expect("written config");
        let parsed: serde_json::Value = serde_json::from_str(&written).expect("complete json");
        assert_eq!(parsed["model"], "gpt-test");
    }

    #[test]
    fn git_diff_to_remote_returns_merge_base_sha_and_patch() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_git_fixture(temp.path());
        std::fs::write(temp.path().join("src.txt"), "changed\n").expect("write");
        let expected_sha =
            run_git_fixture_output(temp.path(), &["merge-base", "HEAD", "@{upstream}"])
                .trim()
                .to_string();

        let mut server = initialized_server_with_root(temp.path());
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "gitDiffToRemote",
            "params": { "cwd": temp.path().to_string_lossy() }
        });
        let response = server
            .handle_json_rpc(&request.to_string())
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["result"]["sha"], expected_sha);
        assert!(
            value["result"]["diff"]
                .as_str()
                .expect("diff")
                .contains("changed")
        );
    }

    #[test]
    fn fuzzy_file_search_returns_files_and_session_notifications() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("mkdir");
        std::fs::write(temp.path().join("src/config_service.rs"), "").expect("write");

        let mut server = initialized_server_with_root(temp.path());
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 40,
            "method": "fuzzyFileSearch",
            "params": {
                "query": "cfg",
                "roots": [temp.path().to_string_lossy()],
                "cancellationToken": "session-a"
            }
        });
        let response = server
            .handle_json_rpc(&request.to_string())
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["result"]["files"][0]["fileName"], "config_service.rs");

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        let methods = methods_from_values(&notifications);
        assert!(methods.contains(&"fuzzyFileSearch/sessionUpdated"));
        assert!(methods.contains(&"fuzzyFileSearch/sessionCompleted"));
    }

    #[test]
    fn fuzzy_file_search_rejects_root_outside_service_root() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let mut server = initialized_server_with_root(root.path());

        let value = fuzzy_file_search_json_rpc(
            &mut server,
            "cfg",
            vec![outside.path().to_string_lossy().to_string()],
        );

        assert_eq!(value["error"]["data"]["capability"], "search");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
    }

    #[test]
    fn fuzzy_file_search_rejects_empty_root() {
        let root = tempfile::tempdir().expect("root tempdir");
        let mut server = initialized_server_with_root(root.path());

        let value = fuzzy_file_search_json_rpc(&mut server, "cfg", vec![String::new()]);

        assert_eq!(value["error"]["data"]["capability"], "search");
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[cfg(unix)]
    #[test]
    fn fuzzy_file_search_rejects_symlink_root_escape() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        std::fs::write(outside.path().join("cfg.rs"), "").expect("write outside file");
        std::os::unix::fs::symlink(outside.path(), root.path().join("outside-link"))
            .expect("symlink outside");
        let mut server = initialized_server_with_root(root.path());

        let value = fuzzy_file_search_json_rpc(
            &mut server,
            "cfg",
            vec![
                root.path()
                    .join("outside-link")
                    .to_string_lossy()
                    .to_string(),
            ],
        );

        assert_eq!(value["error"]["data"]["capability"], "search");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
    }

    #[cfg(unix)]
    #[test]
    fn fuzzy_file_search_does_not_follow_symlink_entries() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        std::fs::write(outside.path().join("cfg.rs"), "").expect("write outside file");
        std::os::unix::fs::symlink(outside.path(), root.path().join("outside-link"))
            .expect("symlink outside");
        let mut server = initialized_server_with_root(root.path());

        let value = fuzzy_file_search_json_rpc(
            &mut server,
            "cfg",
            vec![root.path().to_string_lossy().to_string()],
        );

        assert_eq!(
            value["result"]["files"]
                .as_array()
                .expect("files array")
                .len(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn git_diff_to_remote_rejects_symlink_cwd_escape() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        init_git_fixture(outside.path());
        std::os::unix::fs::symlink(outside.path(), root.path().join("outside-link"))
            .expect("symlink outside repo");

        let mut server = initialized_server_with_root(root.path());
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":31,"method":"gitDiffToRemote","params":{{"cwd":{}}}}}"#,
                serde_json::to_string(&root.path().join("outside-link").to_string_lossy())
                    .expect("cwd json")
            ))
            .expect("response");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["data"]["capability"], "repo");
        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("message")
                .contains("outside")
        );
    }

    fn init_git_fixture(root: &std::path::Path) {
        std::fs::write(root.join("src.txt"), "base\n").expect("write base");
        run_git_fixture(root, &["init"]);
        run_git_fixture(root, &["config", "user.email", "test@example.com"]);
        run_git_fixture(root, &["config", "user.name", "Test User"]);
        run_git_fixture(root, &["add", "."]);
        run_git_fixture(root, &["commit", "-m", "base"]);
        let branch = run_git_fixture_output(root, &["branch", "--show-current"])
            .trim()
            .to_string();
        let remote = root.join(".remote.git");
        let remote_path = remote.to_string_lossy();
        run_git_fixture(root, &["clone", "--bare", ".", remote_path.as_ref()]);
        run_git_fixture(root, &["remote", "add", "origin", remote_path.as_ref()]);
        run_git_fixture(root, &["push", "-u", "origin", &branch]);
    }

    fn fuzzy_file_search_json_rpc(
        server: &mut AppServer,
        query: &str,
        roots: Vec<String>,
    ) -> serde_json::Value {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 41,
            "method": "fuzzyFileSearch",
            "params": {
                "query": query,
                "roots": roots
            }
        });
        let response = server
            .handle_json_rpc(&request.to_string())
            .expect("response");
        serde_json::from_str(&response).expect("json")
    }

    fn run_git_fixture(root: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .expect("git fixture command");
        assert!(status.success());
    }

    fn run_git_fixture_output(root: &std::path::Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("git fixture command");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

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
        let mut server = AppServer::new().with_app_services(services);

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

    #[test]
    fn app_server_ready_p5_routes_require_initialize_before_service_owner() {
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::ready(),
            app_services::TestCommandExecService::ready_buffered(),
        );
        let mut server = AppServer::new().with_app_services(services);

        let read = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"fs-read","method":"fs/readFile","params":{"path":"/tmp/example.txt"}}"#,
            )
            .expect("fs/readFile should return a structured response");
        let exec = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"cmd","method":"command/exec","params":{"command":["printf","test"]}}"#,
            )
            .expect("command/exec should return a structured response");

        let read_value: Value = serde_json::from_str(&read).expect("fs/readFile response JSON");
        let exec_value: Value = serde_json::from_str(&exec).expect("command/exec response JSON");
        assert_eq!(read_value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(exec_value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert!(read_value.get("result").is_none());
        assert!(exec_value.get("result").is_none());
        assert_ne!(read_value["result"]["dataBase64"], "dGVzdA==");
        assert_ne!(exec_value["result"]["stdout"], "test");
    }

    #[test]
    fn app_server_p5_service_events_drain_to_notifications() {
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::ready_with_changed_events(vec![FsChangedNotification {
                watch_id: "watch_1".to_string(),
                path: "/tmp/example.txt".to_string(),
                kind: FsChangedKind::Modified,
            }]),
            app_services::TestCommandExecService::ready_with_output_delta_events(vec![
                CommandExecOutputDeltaNotification {
                    process_id: "proc_1".to_string(),
                    stream: CommandExecOutputStream::Stdout,
                    delta_base64: "dGVzdA==".to_string(),
                    cap_reached: false,
                },
            ]),
        );
        let mut server = AppServer::new().with_app_services(services);

        let notifications = server.drain_notifications();
        let fs_changed = notifications
            .iter()
            .filter(|notification| notification.method == event::FS_CHANGED)
            .collect::<Vec<_>>();
        let output_delta = notifications
            .iter()
            .filter(|notification| notification.method == event::COMMAND_EXEC_OUTPUT_DELTA)
            .collect::<Vec<_>>();

        assert_eq!(notifications.len(), 2);
        assert_eq!(fs_changed.len(), 1);
        assert_eq!(fs_changed[0].params["watchId"], "watch_1");
        assert_eq!(fs_changed[0].params["path"], "/tmp/example.txt");
        assert_eq!(fs_changed[0].params["kind"], "modified");
        assert_eq!(output_delta.len(), 1);
        assert_eq!(output_delta[0].params["processId"], "proc_1");
        assert_eq!(output_delta[0].params["stream"], "stdout");
        assert_eq!(output_delta[0].params["deltaBase64"], "dGVzdA==");
        assert_eq!(output_delta[0].params["capReached"], false);
        assert!(server.drain_notifications().is_empty());
    }

    #[test]
    fn app_server_real_job_service_json_rpc_routes_runtime_contexts() {
        let manager = Arc::new(dasclaw_runtime::context::ContextManager::new(4));
        let job_id = run_async_test(async {
            let job_id = manager
                .create_job("Audit protocol", "Verify app-server JSON-RPC")
                .await
                .unwrap();
            manager
                .update_context(job_id, |ctx| {
                    ctx.transition_to(dasclaw_runtime::JobState::InProgress, None)
                })
                .await
                .unwrap()
                .unwrap();
            job_id
        });
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            job_service::AppServerJobService::new(Arc::clone(&manager)),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let list = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"jobs","method":"jobs/list","params":{}}"#)
            .expect("jobs/list should return a structured response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"read","method":"jobs/read","params":{{"jobId":"{job_id}"}}}}"#
            ))
            .expect("jobs/read should return a structured response");

        let list_value: Value = serde_json::from_str(&list).expect("jobs/list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("jobs/read response JSON");
        assert_eq!(list_value["result"]["data"][0]["jobId"], job_id.to_string());
        assert_eq!(list_value["result"]["data"][0]["state"], "in_progress");
        assert_eq!(read_value["result"]["job"]["title"], "Audit protocol");
        assert_eq!(
            read_value["result"]["job"]["description"],
            "Verify app-server JSON-RPC"
        );
    }

    #[test]
    fn app_server_real_job_service_json_rpc_rejects_invalid_pagination() {
        let manager = Arc::new(dasclaw_runtime::context::ContextManager::new(4));
        run_async_test(async {
            manager
                .create_job("Audit protocol", "Verify app-server JSON-RPC")
                .await
                .unwrap();
        });
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            job_service::AppServerJobService::new(manager),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let bad_cursor = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"bad-cursor","method":"jobs/list","params":{"cursor":"not-a-number"}}"#,
            )
            .expect("jobs/list should return a structured response");
        let zero_limit = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"zero-limit","method":"jobs/list","params":{"limit":0}}"#,
            )
            .expect("jobs/list should return a structured response");

        let bad_cursor_value: Value =
            serde_json::from_str(&bad_cursor).expect("bad cursor response JSON");
        let zero_limit_value: Value =
            serde_json::from_str(&zero_limit).expect("zero limit response JSON");
        assert_eq!(bad_cursor_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(zero_limit_value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn app_server_real_skills_service_json_rpc_lists_writes_and_notifies() {
        let temp = TestDir::new("app_server_real_skills_json_rpc");
        let cwd = temp.path().join("repo");
        let skill_path = cwd.join(".codex/skills/review/SKILL.md");
        write_skill_md(&skill_path, "review", "Review local code");
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let services = app_services::AppServerServices {
            skills: Arc::new(skills_service::AppServerSkillsService::new()),
            ..services
        };
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let list = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "skills",
                    "method": "skills/list",
                    "params": {
                        "cwds": [cwd.to_string_lossy()],
                    },
                })
                .to_string(),
            )
            .expect("skills/list should return a structured response");
        let write = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "write",
                    "method": "skills/config/write",
                    "params": {
                        "path": skill_path.to_string_lossy(),
                        "enabled": false,
                    },
                })
                .to_string(),
            )
            .expect("skills/config/write should return a structured response");
        let after_write = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "skills2",
                    "method": "skills/list",
                    "params": {
                        "cwds": [cwd.to_string_lossy()],
                    },
                })
                .to_string(),
            )
            .expect("skills/list after write should return a structured response");
        let notifications = server.drain_notifications();

        let list_value: Value = serde_json::from_str(&list).expect("skills/list response JSON");
        let write_value: Value = serde_json::from_str(&write).expect("skills write response JSON");
        let after_value: Value =
            serde_json::from_str(&after_write).expect("skills/list after write response JSON");
        assert_eq!(
            list_value["result"]["data"][0]["skills"][0]["name"],
            "review"
        );
        assert_eq!(write_value["result"]["effectiveEnabled"], false);
        assert_eq!(
            after_value["result"]["data"][0]["skills"][0]["enabled"],
            false
        );
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == event::SKILLS_CHANGED)
        );
    }

    #[test]
    fn app_server_real_skills_config_write_by_path_does_not_require_prior_list() {
        let temp = TestDir::new("app_server_real_skills_write_without_list");
        let cwd = temp.path().join("repo");
        let skill_path = cwd.join(".codex/skills/review/SKILL.md");
        write_skill_md(&skill_path, "review", "Review local code");
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            skills_service::AppServerSkillsService::new(),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let write = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "write",
                    "method": "skills/config/write",
                    "params": {
                        "path": skill_path.to_string_lossy(),
                        "enabled": false,
                    },
                })
                .to_string(),
            )
            .expect("skills/config/write should return a structured response");
        let after_write = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "skills",
                    "method": "skills/list",
                    "params": {
                        "cwds": [cwd.to_string_lossy()],
                    },
                })
                .to_string(),
            )
            .expect("skills/list after write should return a structured response");

        let write_value: Value = serde_json::from_str(&write).expect("skills write response JSON");
        let after_value: Value =
            serde_json::from_str(&after_write).expect("skills/list after write response JSON");
        assert_eq!(write_value["result"]["effectiveEnabled"], false);
        assert_eq!(
            after_value["result"]["data"][0]["skills"][0]["enabled"],
            false
        );
    }

    #[test]
    fn app_server_real_p5_filesystem_json_rpc_routes_read_write_and_containment() {
        let temp = TestDir::new("app_server_real_p5_filesystem_routes");
        let outside = TestDir::new("app_server_real_p5_filesystem_outside");
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            fs_service::AppServerFsService::new(temp.path().to_path_buf()),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let create_dir = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"mkdir","method":"fs/createDirectory","params":{"path":"dir"}}"#,
            )
            .expect("fs/createDirectory should return a structured response");
        let write = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"write","method":"fs/writeFile","params":{"path":"dir/source.txt","dataBase64":"cm91dGUtZmlsZQ=="}}"#,
            )
            .expect("fs/writeFile should return a structured response");
        let read = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"read","method":"fs/readFile","params":{"path":"dir/source.txt"}}"#,
            )
            .expect("fs/readFile should return a structured response");
        let list_before_copy = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"list","method":"fs/readDirectory","params":{"path":"dir"}}"#,
            )
            .expect("fs/readDirectory should return a structured response");
        let metadata = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"stat","method":"fs/getMetadata","params":{"path":"dir/source.txt"}}"#,
            )
            .expect("fs/getMetadata should return a structured response");
        let copy = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"copy","method":"fs/copy","params":{"sourcePath":"dir/source.txt","destinationPath":"dir/copied.txt"}}"#,
            )
            .expect("fs/copy should return a structured response");
        let remove = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"remove","method":"fs/remove","params":{"path":"dir/source.txt"}}"#,
            )
            .expect("fs/remove should return a structured response");
        let list_after_remove = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"list2","method":"fs/readDirectory","params":{"path":"dir"}}"#,
            )
            .expect("fs/readDirectory after remove should return a structured response");
        let traversal = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"traversal","method":"fs/readFile","params":{"path":"../escape.txt"}}"#,
            )
            .expect("fs/readFile traversal should return a structured response");
        let outside_path = outside.path().join("outside.txt");
        std::fs::write(&outside_path, "outside").expect("outside fixture should be writable");
        let outside_read = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "outside",
                    "method": "fs/readFile",
                    "params": {
                        "path": outside_path.to_string_lossy(),
                    },
                })
                .to_string(),
            )
            .expect("fs/readFile outside root should return a structured response");

        let create_dir_value: Value =
            serde_json::from_str(&create_dir).expect("mkdir response JSON");
        let write_value: Value = serde_json::from_str(&write).expect("write response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");
        let list_before_value: Value =
            serde_json::from_str(&list_before_copy).expect("list response JSON");
        let metadata_value: Value =
            serde_json::from_str(&metadata).expect("metadata response JSON");
        let copy_value: Value = serde_json::from_str(&copy).expect("copy response JSON");
        let remove_value: Value = serde_json::from_str(&remove).expect("remove response JSON");
        let list_after_value: Value =
            serde_json::from_str(&list_after_remove).expect("list after remove response JSON");
        let traversal_value: Value =
            serde_json::from_str(&traversal).expect("traversal response JSON");
        let outside_value: Value =
            serde_json::from_str(&outside_read).expect("outside response JSON");

        assert!(create_dir_value.get("error").is_none());
        assert!(write_value.get("error").is_none());
        assert_eq!(read_value["result"]["dataBase64"], "cm91dGUtZmlsZQ==");
        assert_eq!(
            list_before_value["result"]["entries"][0]["fileName"],
            "source.txt"
        );
        assert_eq!(metadata_value["result"]["isFile"], true);
        assert_eq!(metadata_value["result"]["isDirectory"], false);
        assert!(copy_value.get("error").is_none());
        assert!(remove_value.get("error").is_none());
        assert_eq!(
            list_after_value["result"]["entries"][0]["fileName"],
            "copied.txt"
        );
        assert_eq!(
            list_after_value["result"]["entries"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            traversal_value["error"]["data"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );
        assert_eq!(traversal_value["error"]["data"]["capability"], "filesystem");
        assert_eq!(
            outside_value["error"]["data"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );
        assert_eq!(outside_value["error"]["data"]["capability"], "filesystem");
    }

    #[test]
    fn app_server_real_p5_command_exec_json_rpc_route_returns_buffered_stdout() {
        let temp = TestDir::new("app_server_real_p5_command_exec_routes");
        let outside = TestDir::new("app_server_real_p5_command_exec_outside");
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            command_service::AppServerCommandExecService::new(temp.path().to_path_buf()),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let exec = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"cmd","method":"command/exec","params":{"command":["echo","route-stdout"],"processId":"route_proc_1"}}"#,
            )
            .expect("command/exec should return a structured response");
        let unsupported_options = [
            (
                "stream",
                serde_json::json!({
                    "command": ["echo", "nope"],
                    "processId": "route_proc_stream",
                    "streamStdoutStderr": true,
                }),
            ),
            (
                "timeout",
                serde_json::json!({
                    "command": ["echo", "nope"],
                    "processId": "route_proc_timeout",
                    "disableTimeout": true,
                }),
            ),
            (
                "size",
                serde_json::json!({
                    "command": ["echo", "nope"],
                    "processId": "route_proc_size",
                    "size": {"cols": 80, "rows": 24},
                }),
            ),
        ];
        let unsupported = unsupported_options.map(|(id, params)| {
            server
                .handle_json_rpc(
                    &serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": "command/exec",
                        "params": params,
                    })
                    .to_string(),
                )
                .expect("unsupported command/exec option should return a structured error")
        });
        let invalid_sandbox_policy = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "sandbox",
                    "method": "command/exec",
                    "params": {
                        "command": ["echo", "nope"],
                        "processId": "route_proc_sandbox",
                        "sandboxPolicy": {"mode": "unrestricted"},
                    },
                })
                .to_string(),
            )
            .expect("invalid sandboxPolicy should return a structured error");
        let cwd_outside = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "cwd",
                    "method": "command/exec",
                    "params": {
                        "command": ["echo", "nope"],
                        "cwd": outside.path().to_string_lossy(),
                    },
                })
                .to_string(),
            )
            .expect("command/exec outside cwd should return a structured response");
        let notifications = server.drain_notifications();

        let exec_value: Value = serde_json::from_str(&exec).expect("command/exec response JSON");
        let unsupported_values = unsupported.map(|response| {
            serde_json::from_str::<Value>(&response)
                .expect("unsupported command/exec response JSON")
        });
        let invalid_sandbox_policy_value: Value =
            serde_json::from_str(&invalid_sandbox_policy).expect("invalid sandboxPolicy JSON");
        let cwd_value: Value =
            serde_json::from_str(&cwd_outside).expect("outside cwd response JSON");
        assert_eq!(exec_value["result"]["exitCode"], 0);
        assert!(
            exec_value["result"]["stdout"]
                .as_str()
                .unwrap()
                .contains("route-stdout")
        );
        assert_eq!(exec_value["result"]["stderr"], "");
        for value in unsupported_values {
            assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
            assert_eq!(value["error"]["data"]["capability"], "command_exec");
        }
        assert_eq!(
            invalid_sandbox_policy_value["error"]["data"]["code"],
            "INVALID_PARAMS"
        );
        assert_eq!(
            invalid_sandbox_policy_value["error"]["data"]["capability"],
            "command_exec"
        );
        assert!(
            invalid_sandbox_policy_value["error"]["data"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("invalid sandboxPolicy")
                || invalid_sandbox_policy_value["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("invalid sandboxPolicy")
        );
        assert_eq!(cwd_value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert_eq!(cwd_value["error"]["data"]["capability"], "command_exec");
        assert!(
            !notifications
                .iter()
                .any(|notification| notification.method == event::COMMAND_EXEC_OUTPUT_DELTA)
        );
        assert!(server.drain_notifications().is_empty());
    }

    #[cfg(unix)]
    fn wait_for_command_service_output_delta(
        service: &dyn app_services::CommandExecService,
        process_id: &str,
        needle: &str,
    ) {
        use base64::Engine as _;

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            for event in service.drain_output_delta_events() {
                if event.process_id != process_id {
                    continue;
                }
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(&event.delta_base64)
                    .expect("valid command output delta base64");
                let text = String::from_utf8_lossy(&decoded);
                if text.contains(needle) {
                    return;
                }
            }

            assert!(
                Instant::now() < deadline,
                "timed out waiting for command output delta {needle:?} from {process_id}"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

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
        let service = Arc::clone(&services.command);
        let readiness_service = Arc::clone(&services.command);
        let params = CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf ready; sleep 30".to_string(),
            ],
            cwd: None,
            timeout_ms: Some(10_000),
            disable_timeout: None,
            output_bytes_cap: None,
            disable_output_cap: None,
            env: Default::default(),
            process_id: Some("route_stream_terminate".to_string()),
            sandbox_policy: None,
            permission_profile: None,
            size: Some(CommandExecTerminalSize { cols: 80, rows: 24 }),
            stream_stdin: Some(false),
            stream_stdout_stderr: Some(true),
            tty: Some(true),
        };
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();
        let exec_thread = thread::spawn(move || service.exec(params).expect("exec response"));
        wait_for_command_service_output_delta(
            &*readiness_service,
            "route_stream_terminate",
            "ready",
        );

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
        let service = Arc::clone(&services.command);
        let readiness_service = Arc::clone(&services.command);
        let params = CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf ready; IFS= read -r line".to_string(),
            ],
            cwd: None,
            timeout_ms: Some(10_000),
            disable_timeout: None,
            output_bytes_cap: None,
            disable_output_cap: None,
            env: Default::default(),
            process_id: Some("route_stream_resize".to_string()),
            sandbox_policy: None,
            permission_profile: None,
            size: Some(CommandExecTerminalSize { cols: 80, rows: 24 }),
            stream_stdin: Some(true),
            stream_stdout_stderr: Some(true),
            tty: Some(true),
        };
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();
        let exec_thread = thread::spawn(move || service.exec(params).expect("exec response"));
        wait_for_command_service_output_delta(&*readiness_service, "route_stream_resize", "ready");

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

    #[cfg(unix)]
    #[test]
    fn app_server_real_p5_command_exec_json_rpc_route_returns_stderr_and_non_zero_exit() {
        let temp = TestDir::new("app_server_real_p5_command_exec_stderr_route");
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            command_service::AppServerCommandExecService::new(temp.path().to_path_buf()),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let exec = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"cmd-stderr","method":"command/exec","params":{"command":["sh","-c","printf route-stderr >&2; exit 7"],"processId":"route_proc_stderr"}}"#,
            )
            .expect("command/exec should return a structured response");

        let exec_value: Value = serde_json::from_str(&exec).expect("command/exec response JSON");
        assert_ne!(exec_value["result"]["exitCode"], 0);
        assert_eq!(exec_value["result"]["exitCode"], 7);
        assert_eq!(exec_value["result"]["stdout"], "");
        assert_eq!(exec_value["result"]["stderr"], "route-stderr");
        assert!(exec_value.get("error").is_none());
    }

    #[test]
    fn app_server_real_mcp_service_json_rpc_status_reload_and_tool_call() {
        let test_mcp = mcp_service::test_support::TestMcpHttpServer::start(9);
        let mut bearer = dasclaw_mcp::McpServerConfig::new("bearer", &test_mcp.url);
        bearer.headers.insert(
            "Authorization".to_string(),
            "Bearer secret-token".to_string(),
        );
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            mcp_service::AppServerMcpService::from_servers(vec![bearer]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let status = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"mcp-status","method":"mcpServerStatus/list","params":{}}"#)
            .expect("mcpServerStatus/list should return a structured response");
        let reload = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"mcp-reload","method":"config/mcpServer/reload","params":{"name":"bearer"}}"#)
            .expect("config/mcpServer/reload should return a structured response");
        let reload_notifications = server.drain_notifications();
        let tool_call = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"mcp-tool","method":"mcpServer/tool/call","params":{"threadId":"thread_1","server":"bearer","tool":"search","arguments":{}}}"#)
            .expect("mcpServer/tool/call should return a structured response");
        let bad_cursor = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"mcp-bad-cursor","method":"mcpServerStatus/list","params":{"cursor":"bad"}}"#)
            .expect("mcpServerStatus/list should return a structured response");

        let status_value: Value = serde_json::from_str(&status).expect("mcp status JSON");
        let reload_value: Value = serde_json::from_str(&reload).expect("mcp reload JSON");
        let tool_value: Value = serde_json::from_str(&tool_call).expect("mcp tool JSON");
        let bad_cursor_value: Value =
            serde_json::from_str(&bad_cursor).expect("mcp bad cursor JSON");
        assert_eq!(status_value["result"]["data"][0]["name"], "bearer");
        assert_eq!(
            status_value["result"]["data"][0]["authStatus"],
            "bearerToken"
        );
        assert!(
            !status_value.to_string().contains("secret-token"),
            "MCP status must not leak authorization headers"
        );
        assert_eq!(reload_value["result"]["reloaded"][0], "bearer");
        assert!(reload_notifications.iter().any(|notification| {
            notification.method == event::MCP_SERVER_STARTUP_STATUS_UPDATED
                && notification.params["name"] == "bearer"
                && notification.params["status"] == "ready"
        }));
        assert_eq!(tool_value["result"]["content"][0]["text"], "ok");
        assert_eq!(
            tool_value["result"]["structuredContent"],
            serde_json::json!({"echoed": true})
        );
        assert_eq!(
            tool_value["result"]["_meta"],
            serde_json::json!({"trace": "abc"})
        );
        assert_eq!(bad_cursor_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert!(
            test_mcp
                .methods()
                .iter()
                .any(|method| method == "tools/call")
        );
    }

    #[test]
    fn app_server_real_mcp_service_advertises_real_tool_resource_and_progress_methods() {
        let service = mcp_service::AppServerMcpService::from_servers(vec![
            dasclaw_mcp::McpServerConfig::new("github", "https://github.example/mcp"),
        ]);
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            service,
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);

        let capabilities = server.capabilities();
        assert_eq!(
            capabilities.capabilities.mcp.methods,
            vec![
                method::CONFIG_MCP_SERVER_RELOAD.to_string(),
                method::MCP_SERVER_STATUS_LIST.to_string(),
                method::MCP_SERVER_RESOURCE_READ.to_string(),
                method::MCP_SERVER_TOOL_CALL.to_string(),
            ]
        );
        assert_eq!(
            capabilities.capabilities.mcp.events,
            vec![
                event::ITEM_MCP_TOOL_CALL_PROGRESS.to_string(),
                event::MCP_SERVER_STARTUP_STATUS_UPDATED.to_string(),
            ]
        );
        assert!(
            !capabilities
                .capabilities
                .mcp
                .methods
                .contains(&method::MCP_SERVER_OAUTH_LOGIN.to_string())
        );
    }

    #[test]
    fn app_server_mcp_resource_read_fail_safe_error_redacts_uri_secrets() {
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            mcp_service::AppServerMcpService::from_servers(vec![
                dasclaw_mcp::McpServerConfig::new("github", "https://github.example/mcp"),
            ]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"resource","method":"mcpServer/resource/read","params":{"server":"github","uri":"https://repo.example/private?access_token=secret-token"}}"#,
            )
            .expect("mcpServer/resource/read should return a structured response");

        let value: Value = serde_json::from_str(&response).expect("resource read response JSON");
        assert_eq!(value["error"]["data"]["code"], "SERVICE_DEGRADED");
        assert!(
            !value.to_string().contains("secret-token"),
            "resource-read errors must not echo URI secrets"
        );
        assert!(
            !value.to_string().contains("access_token"),
            "resource-read errors must not echo sensitive URI query keys"
        );
    }

    #[test]
    fn app_server_mcp_oauth_login_fails_safe_and_emits_completion_notification() {
        let github = dasclaw_mcp::McpServerConfig::new("github", "http://127.0.0.1:9/mcp")
            .with_oauth(dasclaw_mcp::OAuthConfig::new("client-id"));
        let services = app_services::AppServerServices::for_tests(
            app_services::TestLogService::ready(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            mcp_service::AppServerMcpService::from_servers(vec![github]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);
        server
            .handle_json_rpc(initialized_request_json())
            .expect("initialize should return a response");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"oauth","method":"mcpServer/oauth/login","params":{"name":"github","scopes":["repo"],"timeoutSecs":30}}"#,
            )
            .expect("mcpServer/oauth/login should return a structured response");
        let notifications = server.drain_notifications();
        let value: Value = serde_json::from_str(&response).expect("oauth response JSON");

        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert!(notifications.iter().any(|notification| {
            notification.method == event::MCP_SERVER_OAUTH_LOGIN_COMPLETED
                && notification.params["name"] == "github"
                && notification.params["success"] == false
        }));
    }

    #[test]
    fn app_server_log_service_emits_log_entry_notification() {
        let log_service = log_service::AppServerLogService::new();
        let services = app_services::AppServerServices::for_tests(
            log_service.clone(),
            app_services::TestJobService::ready(vec![]),
            app_services::TestSkillsService::ready(vec![]),
            app_services::TestMcpService::ready(vec![]),
            app_services::TestFsService::disabled(),
            app_services::TestCommandExecService::disabled(),
        );
        let mut server = AppServer::new().with_app_services(services);

        log_service.record_event(&ObserverEvent::Error {
            component: "app_server.tests".to_string(),
            message: "test log message".to_string(),
        });

        let notifications = server.drain_notifications();
        assert!(notifications.iter().any(|notification| {
            notification.method == event::LOG_ENTRY
                && notification.params["level"] == "warn"
                && notification.params["target"] == "observability.error"
                && notification.params["message"] == "test log message"
                && notification.params["fields"]["component"] == "app_server.tests"
        }));
    }

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

    fn collect_runtime_updates_until(
        sink: &RuntimeTurnUpdateSink,
        matches_target: impl Fn(&[RuntimeTurnUpdate]) -> bool,
    ) -> Vec<RuntimeTurnUpdate> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut collected = Vec::new();
        while Instant::now() < deadline {
            collected.extend(sink.drain());
            if matches_target(&collected) {
                return collected;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        collected
    }

    fn run_async_test<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should build")
            .block_on(future)
    }

    fn write_skill_md(path: &std::path::Path, name: &str, description: &str) {
        std::fs::create_dir_all(path.parent().expect("skill path should have parent")).unwrap();
        std::fs::write(
            path,
            format!("---\nname: {name}\ndescription: {description}\n---\nUse {name}.\n"),
        )
        .unwrap();
    }

    struct TestDir {
        path: std::path::PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dasclaw_app_server_{name}_{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
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
                    CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string(),
                ],
                model_provider: None,
            })
            .expect("initialize should accept the compatibility profile");
        let profile = response
            .compatibility_profiles
            .iter()
            .find(|profile| {
                profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID
            })
            .expect("initialize response should advertise the Codex v2 subset profile");
        let expected_profile = CompatibilityProfile::codex_app_server_v2_chat_session_subset();

        assert!(response.unavailable_requested_capabilities.is_empty());
        assert_eq!(profile.scope, expected_profile.scope);
        assert_eq!(
            profile.scope,
            dasclaw_app_server_protocol::CompatibilityProfileScope::ChatSessionSubset
        );
        assert_eq!(profile.methods, expected_profile.methods);
    }

    #[test]
    fn capabilities_list_reports_codex_v2_profile_as_chat_session_subset() {
        let mut server = AppServer::new();
        let response = server.capabilities();
        let profile = response
            .compatibility_profiles
            .iter()
            .find(|profile| {
                profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID
            })
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
        assert_eq!(server.drain_notifications().len(), 4);

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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let shutdown = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });
        assert_eq!(shutdown.lifecycle.state, LifecycleState::Stopped);

        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 3);
        assert_eq!(notifications[0].method, "item/completed");
        assert_eq!(notifications[0].params["turnId"], started.turn.id);
        assert_eq!(notifications[1].method, "turn/completed");
        assert_eq!(notifications[1].params["turn"]["status"], "interrupted");
        assert_eq!(notifications[2].method, "lifecycle/changed");
        assert_eq!(notifications[2].params["lifecycle"]["state"], "stopped");
    }

    #[test]
    fn shutdown_rejects_when_pending_turn_cancel_cannot_persist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("threads.json");
        let mut server = AppServer::with_runtime_bridge(Arc::new(NoopRuntimeBridge))
            .with_thread_snapshot_path(path.clone())
            .expect("persistent server");
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
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();
        replace_file_with_directory(&path);

        let shutdown = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::Test),
            timeout_ms: None,
        });

        assert!(!shutdown.accepted);
        assert_eq!(shutdown.lifecycle.state, LifecycleState::Degraded);
        let notifications = server.drain_notifications();
        assert_eq!(notifications[0].method, "error");
        assert_eq!(notifications[0].params["code"], "SERVICE_DEGRADED");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "degraded");
        assert_eq!(
            server
                .list_turns_for_test(TestTurnsSnapshotParams {
                    thread_id: thread.thread.id
                })
                .expect("turn list")
                .turns[0]
                .status,
            TurnStatus::Pending
        );
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
    fn json_rpc_client_response_without_method_is_not_treated_as_invalid_request() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge);

        let response = server.handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"approval_missing","result":{"decision":{"kind":"approve"}}}"#,
        );

        assert!(response.is_none());
        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["outcome"] == "failed"
            }),
            "unknown client response should emit failed serverRequest/resolved: {notifications:?}"
        );
    }

    #[test]
    fn protocol_schema_methods_are_all_routable_after_p3() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let server = initialized_server_with_bridge(bridge);
        assert_eq!(
            server.capabilities.approval.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            server.capabilities.tools.status,
            CapabilityStatus::Implemented
        );
        assert_eq!(
            server.capabilities.sandbox.status,
            CapabilityStatus::Implemented
        );
        let health = server.clone().health_check(HealthCheckParams {
            include_details: true,
        });
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Tools && service.status == ServiceStatus::Ready
        }));
        assert!(health.services.iter().any(|service| {
            service.service == ServiceName::Sandbox && service.status == ServiceStatus::Ready
        }));

        for method in server.protocol_schema().methods {
            assert!(
                supported_methods().contains(&method.method.as_str()),
                "protocol/schema advertised an unroutable method after P3: {}",
                method.method
            );
        }
    }

    #[test]
    fn approval_response_records_runtime_decision_and_emits_resolved_notification() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let turn = server
            .turn_start(TurnStartParams {
                thread_id,
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        assert_eq!(
            turn.turn.status,
            codex_status_from_turn_status(TurnStatus::Pending)
        );

        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let server_request = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .expect("approval server request should be emitted");
        let request_id = server_request["id"]
            .as_str()
            .expect("approval server request id should be a string")
            .to_string();
        assert_eq!(
            server_request["params"]["itemId"],
            serde_json::json!("turn_1:tool:tool_1")
        );

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"decision":{{"kind":"approve"}}}}}}"#
        ));

        assert!(response.is_none());
        assert_eq!(
            bridge.decisions(),
            vec![(
                "00000000-0000-0000-0000-000000000001".to_string(),
                dasclaw_runtime::ApprovalDecision::Approve
            )]
        );
        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "approved"
            }),
            "approval response should emit approved serverRequest/resolved: {notifications:?}"
        );
    }

    #[test]
    fn dynamic_tool_client_response_is_decoded_by_pending_kind() {
        let bridge = Arc::new(R1RuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        server.insert_pending_server_request_for_test(
            "tool_1",
            PendingServerRequestKind::DynamicToolCall,
            "runtime_tool_1",
            "thread_1",
            "turn_1",
        );

        let response = server.handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"tool_1","result":{"contentItems":[{"type":"inputText","text":"ok"}],"success":true}}"#,
        );

        assert!(response.is_none());
        assert_eq!(
            bridge.resolutions(),
            vec![RuntimeServerRequestResolution {
                request_id: "runtime_tool_1".to_string(),
                payload: RuntimeServerRequestResponse::DynamicTool(DynamicToolCallResponse {
                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                        text: "ok".to_string(),
                    }],
                    success: true,
                }),
            }]
        );
    }

    #[test]
    fn malformed_dynamic_tool_response_fails_turn_and_clears_pending_request() {
        let bridge = Arc::new(R1RuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        server.insert_pending_server_request_for_test(
            "tool_1",
            PendingServerRequestKind::DynamicToolCall,
            "runtime_tool_1",
            "thread_1",
            "turn_1",
        );

        let response =
            server.handle_json_rpc(r#"{"jsonrpc":"2.0","id":"tool_1","result":{"success":true}}"#);

        assert!(response.is_none());
        let resolutions = bridge.resolutions();
        assert_eq!(resolutions.len(), 1);
        assert_eq!(resolutions[0].request_id, "runtime_tool_1");
        assert!(matches!(
            &resolutions[0].payload,
            RuntimeServerRequestResponse::DynamicTool(response)
                if !response.success
                    && response.content_items.iter().any(|item| matches!(
                        item,
                        DynamicToolCallOutputContentItem::InputText { text }
                            if text.contains("missing field")
                    ))
        ));
        let notifications = server.drain_notifications();
        let resolved_notifications = notifications
            .iter()
            .filter(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == "tool_1"
            })
            .collect::<Vec<_>>();
        assert_eq!(
            resolved_notifications.len(),
            1,
            "malformed dynamic tool response should emit one failed serverRequest/resolved: {notifications:?}"
        );
        assert_eq!(resolved_notifications[0].params["outcome"], "failed");

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
        let later = server.drain_notifications();
        assert!(!later.iter().any(|notification| {
            notification.method == "serverRequest/resolved"
                && notification.params["requestId"] == "tool_1"
        }));
    }

    #[test]
    fn approval_reject_response_preserves_reason_in_resolved_notification() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let _turn = server
            .turn_start(TurnStartParams {
                thread_id,
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let request_id = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .and_then(|value| value["id"].as_str())
            .expect("approval server request should be emitted")
            .to_string();

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"decision":{{"kind":"reject","data":{{"reason":"not allowed"}}}}}}}}"#
        ));

        assert!(response.is_none());
        assert_eq!(
            bridge.decisions(),
            vec![(
                "00000000-0000-0000-0000-000000000001".to_string(),
                dasclaw_runtime::ApprovalDecision::Reject {
                    reason: Some("not allowed".to_string())
                }
            )]
        );
        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "rejected"
                    && notification.params["reason"] == "not allowed"
            }),
            "approval reject should preserve reason in serverRequest/resolved: {notifications:?}"
        );
    }

    #[test]
    fn expired_approval_request_rejects_fail_safe_and_late_response_is_unknown() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let request_id = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .and_then(|value| value["id"].as_str())
            .expect("approval server request should be emitted")
            .to_string();

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
        assert_turn_status_and_ready(&mut server, &thread_id, &turn.turn.id, TurnStatus::Failed);
        assert_eq!(
            bridge.decisions(),
            vec![(
                "00000000-0000-0000-0000-000000000001".to_string(),
                dasclaw_runtime::ApprovalDecision::Reject {
                    reason: Some("approval request timed out".to_string())
                }
            )]
        );

        let late = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"decision":{{"kind":"approve"}}}}}}"#
        ));

        assert!(late.is_none());
        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "timed_out"
            }),
            "expired request should emit timed_out serverRequest/resolved: {notifications:?}"
        );
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
            }),
            "late response should emit failed serverRequest/resolved: {notifications:?}"
        );
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "turn/completed"
                    && notification.params["threadId"] == thread_id
                    && notification.params["turn"]["id"] == turn.turn.id
                    && notification.params["turn"]["status"] == "failed"
            }),
            "timeout should fail the owning turn: {notifications:?}"
        );
    }

    #[test]
    fn approval_error_response_rejects_runtime_and_clears_pending_request() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let request_id = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .and_then(|value| value["id"].as_str())
            .expect("approval server request should be emitted")
            .to_string();

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"error":{{"code":-32000,"message":"client rejected response"}}}}"#
        ));

        assert!(response.is_none());
        assert_eq!(
            bridge.decisions(),
            vec![(
                "00000000-0000-0000-0000-000000000001".to_string(),
                dasclaw_runtime::ApprovalDecision::Reject {
                    reason: Some("approval response failed: client rejected response".to_string())
                }
            )]
        );
        let notifications = server.drain_notifications();
        assert_turn_status_and_ready(&mut server, &thread_id, &turn.turn.id, TurnStatus::Failed);
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
                    && notification.params["reason"] == "client rejected response"
            }),
            "client error response should resolve pending request as failed: {notifications:?}"
        );
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "turn/completed"
                    && notification.params["threadId"] == thread_id
                    && notification.params["turn"]["id"] == turn.turn.id
                    && notification.params["turn"]["status"] == "failed"
            }),
            "client error response should fail the owning turn: {notifications:?}"
        );

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
        let timeout_notifications = server.drain_notifications();
        assert!(
            !timeout_notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
            }),
            "cleared request must not resolve again on timeout: {timeout_notifications:?}"
        );
    }

    #[test]
    fn malformed_approval_response_rejects_runtime_and_clears_pending_request() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let request_id = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .and_then(|value| value["id"].as_str())
            .expect("approval server request should be emitted")
            .to_string();

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"unexpected":true}}}}"#
        ));

        assert!(response.is_none());
        assert!(matches!(
            bridge.decisions().as_slice(),
            [(
                id,
                dasclaw_runtime::ApprovalDecision::Reject {
                    reason: Some(reason)
                }
            )] if id == "00000000-0000-0000-0000-000000000001"
                && reason.starts_with("approval response failed:")
        ));
        let notifications = server.drain_notifications();
        assert_turn_status_and_ready(&mut server, &thread_id, &turn.turn.id, TurnStatus::Failed);
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
            }),
            "malformed response should resolve pending request as failed: {notifications:?}"
        );

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
        let timeout_notifications = server.drain_notifications();
        assert!(
            !timeout_notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
            }),
            "cleared malformed response request must not timeout later: {timeout_notifications:?}"
        );
    }

    #[test]
    fn malformed_permissions_response_rejects_runtime_and_clears_pending_request() {
        let bridge = Arc::new(R1RuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        server.insert_pending_server_request_for_test(
            "permissions_1",
            PendingServerRequestKind::PermissionsApproval,
            "runtime_permissions_1",
            "thread_1",
            "turn_1",
        );

        let response = server.handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"permissions_1","result":{"permissions":{}}}"#,
        );

        assert!(response.is_none());
        assert!(matches!(
            bridge.resolutions().as_slice(),
            [RuntimeServerRequestResolution {
                request_id,
                payload: RuntimeServerRequestResponse::Permissions(response),
            }] if request_id == "runtime_permissions_1"
                && response.decision == PermissionsApprovalDecision::Reject
        ));
        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == "permissions_1"
                    && notification.params["outcome"] == "failed"
            }),
            "malformed permissions response should resolve pending request as failed: {notifications:?}"
        );

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));
        let timeout_notifications = server.drain_notifications();
        assert!(
            !timeout_notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == "permissions_1"
            }),
            "cleared malformed permissions request must not timeout later: {timeout_notifications:?}"
        );
    }

    #[test]
    fn expired_permissions_request_rejects_runtime_fail_safe() {
        let bridge = Arc::new(R1RuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        server.insert_pending_server_request_for_test(
            "permissions_1",
            PendingServerRequestKind::PermissionsApproval,
            "runtime_permissions_1",
            "thread_1",
            "turn_1",
        );

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));

        assert!(matches!(
            bridge.resolutions().as_slice(),
            [RuntimeServerRequestResolution {
                request_id,
                payload: RuntimeServerRequestResponse::Permissions(response),
            }] if request_id == "runtime_permissions_1"
                && response.decision == PermissionsApprovalDecision::Reject
        ));
        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == "permissions_1"
                    && notification.params["outcome"] == "timed_out"
            }),
            "expired permissions request should emit timed_out serverRequest/resolved: {notifications:?}"
        );
    }

    #[test]
    fn interrupt_turn_resolves_pending_approval_server_request_as_failed() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let request_id = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .and_then(|value| value["id"].as_str())
            .expect("approval server request should be emitted")
            .to_string();

        server
            .interrupt_turn_for_test(TestTurnInterruptParams {
                thread_id: thread_id.clone(),
                turn_id: turn.turn.id.clone(),
            })
            .expect("turn cancel should succeed");

        let notifications = server.drain_notifications();
        assert_turn_status_and_ready(
            &mut server,
            &thread_id,
            &turn.turn.id,
            TurnStatus::Cancelled,
        );
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
                    && notification.params["reason"] == "turn cancelled"
            }),
            "turn cancel should resolve pending approval request: {notifications:?}"
        );
        let late = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"decision":{{"kind":"approve"}}}}}}"#
        ));
        assert!(late.is_none());
        let late_notifications = server.drain_notifications();
        assert!(
            late_notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
            }),
            "late response should be unknown after cancel cleanup: {late_notifications:?}"
        );
    }

    #[test]
    fn shutdown_resolves_pending_approval_server_request_as_failed() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;
        let _turn = server
            .turn_start(TurnStartParams {
                thread_id,
                input: text_input("run command".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let outputs = json_rpc_values(server.drain_json_rpc_notifications());
        let request_id = outputs
            .iter()
            .find(|value| value["method"] == "item/commandExecution/requestApproval")
            .and_then(|value| value["id"].as_str())
            .expect("approval server request should be emitted")
            .to_string();

        let _shutdown = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::ClientExit),
            timeout_ms: None,
        });

        let notifications = server.drain_notifications();
        assert!(
            notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
                    && notification.params["reason"] == "server shutdown"
            }),
            "shutdown should resolve pending approval request: {notifications:?}"
        );
        let late = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"decision":{{"kind":"approve"}}}}}}"#
        ));
        assert!(late.is_none());
        let late_notifications = server.drain_notifications();
        assert!(
            late_notifications.iter().any(|notification| {
                notification.method == "serverRequest/resolved"
                    && notification.params["requestId"] == request_id
                    && notification.params["outcome"] == "failed"
            }),
            "late response should be unknown after shutdown cleanup: {late_notifications:?}"
        );
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
        assert_eq!(notifications.len(), 4);
        assert_eq!(notifications[0].method, "lifecycle/changed");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[2].method, "capabilities/changed");
        assert_eq!(notifications[3].method, "notifications/initialized");
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
                    CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string(),
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
            CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID
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
    fn notification_bus_plain_drain_preserves_pending_server_requests_for_json_rpc_drain() {
        let mut bus = NotificationBus::new();
        bus.emit_server_request(
            JsonRpcServerRequest::new(
                "approval_1",
                server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
                serde_json::json!({"threadId":"thread_1"}),
            )
            .expect("server request should serialize"),
        );
        bus.emit_agent_message_delta(AgentMessageDeltaEvent {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1".to_string(),
            delta: "hello".to_string(),
        });

        let plain_drain = bus.drain_with_policy();
        assert!(!plain_drain.should_disconnect);
        assert_eq!(plain_drain.notifications.len(), 1);
        assert_eq!(
            plain_drain.notifications[0].method,
            "item/agentMessage/delta"
        );

        let json_rpc_drain = bus.drain_json_rpc_with_policy();
        assert!(!json_rpc_drain.should_disconnect);
        assert_eq!(json_rpc_drain.lines.len(), 1);
        let request: Value =
            serde_json::from_str(&json_rpc_drain.lines[0]).expect("server request should be JSON");
        assert_eq!(request["id"], "approval_1");
        assert_eq!(
            request["method"],
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
        );
    }

    #[test]
    fn notification_bus_signals_lag_disconnect_when_queue_is_full() {
        let mut bus = NotificationBus::new();
        for _ in 0..DEFAULT_MAX_PENDING_NOTIFICATIONS {
            bus.emit_capabilities_changed(CapabilitiesChangedEvent {
                capabilities: CapabilityMatrix::phase_one(),
                reason: CapabilitiesChangedReason::Initialize,
            });
        }
        bus.emit_capabilities_changed(CapabilitiesChangedEvent {
            capabilities: CapabilityMatrix::phase_one(),
            reason: CapabilitiesChangedReason::Initialize,
        });
        bus.emit_capabilities_changed(CapabilitiesChangedEvent {
            capabilities: CapabilityMatrix::phase_one(),
            reason: CapabilitiesChangedReason::Initialize,
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
            bus.emit_capabilities_changed(CapabilitiesChangedEvent {
                capabilities: CapabilityMatrix::phase_one(),
                reason: CapabilitiesChangedReason::Initialize,
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
            matrix.thread_lifecycle.methods,
            matrix.thread_goal.methods,
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
    fn json_rpc_config_requirements_read_returns_allowed_sandbox_modes() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"cfg","method":"{}"}}"#,
                method::CONFIG_REQUIREMENTS_READ
            ))
            .expect("route config requirements");
        let value: Value = serde_json::from_str(&response).expect("json response");

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

    #[test]
    fn json_rpc_config_requirements_read_requires_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"cfg","method":"{}"}}"#,
                method::CONFIG_REQUIREMENTS_READ
            ))
            .expect("config requirements should return a structured error");
        let value: Value = serde_json::from_str(&response).expect("json response");

        assert_eq!(value["id"], "cfg");
        assert_eq!(value["error"]["code"], -32003);
        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["capability"], "sandbox");
        assert!(value.get("result").is_none());
    }

    #[test]
    fn json_rpc_config_requirements_read_rejects_non_empty_params() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"cfg","method":"{}","params":{{"x":true}}}}"#,
                method::CONFIG_REQUIREMENTS_READ
            ))
            .expect("config requirements should return a structured error");
        let value: Value = serde_json::from_str(&response).expect("json response");

        assert_eq!(value["id"], "cfg");
        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert!(value.get("result").is_none());
    }

    #[test]
    fn json_rpc_thread_create_is_not_a_public_method_before_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#,
            )
            .expect("thread/create should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["error"]["code"], -32601);
        assert_eq!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
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
    fn json_rpc_model_list_returns_codex_shape_without_api_keys() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"models","method":"model/list","params":{}}"#)
            .expect("model/list should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("model/list response JSON");

        assert_eq!(value["id"], "models");
        assert_eq!(value["result"]["nextCursor"], serde_json::Value::Null);
        assert_eq!(value["result"]["data"][0]["id"], "gpt-test");
        assert_eq!(value["result"]["data"][0]["model"], "gpt-test");
        assert_eq!(value["result"]["data"][0]["isDefault"], true);
        assert_eq!(value["result"]["data"][1]["id"], "gpt-next");
        assert_eq!(value["result"]["data"][1]["isDefault"], false);
        assert!(!response.contains("test-api-key"));
        assert!(!response.contains(TEST_MODEL_API_BASE_URL));
        assert!(!response.contains("apiKey"));
        assert!(!response.contains("apiBaseUrl"));
    }

    #[test]
    fn json_rpc_model_list_without_params_returns_codex_shape() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"models","method":"model/list"}"#)
            .expect("model/list without params should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("model/list response JSON");

        assert_eq!(value["id"], "models");
        assert_eq!(value["result"]["nextCursor"], serde_json::Value::Null);
        assert_eq!(value["result"]["data"][0]["id"], "gpt-test");
        assert_eq!(value["result"]["data"][0]["isDefault"], true);
        assert_eq!(value["result"]["data"][1]["id"], "gpt-next");
        assert_eq!(value["result"]["data"][1]["isDefault"], false);
    }

    #[test]
    fn json_rpc_model_list_requires_initialize() {
        let mut server = AppServer::new();
        let response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"models","method":"model/list","params":{}}"#)
            .expect("model/list should return a structured error");
        let value: Value = serde_json::from_str(&response).expect("model/list error JSON");

        assert_eq!(value["id"], "models");
        assert_eq!(value["error"]["code"], -32003);
        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["capability"], "model_provider");
    }

    #[test]
    fn json_rpc_thread_start_returns_thread_view_after_initialize() {
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
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"/tmp/workspace"}}"#,
            )
            .expect("thread/start should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["result"]["thread"]["id"], "thread_1");
        assert_eq!(value["result"]["thread"]["cwd"], "/tmp/workspace");
        assert_eq!(value["result"]["thread"]["modelProvider"], "openai");
        assert!(value["result"].get("threadId").is_none());
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, "thread/started");
    }

    #[test]
    fn json_rpc_thread_start_requires_model_provider_for_truthful_metadata() {
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
            .expect("initialize should succeed without model provider");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"/tmp/workspace"}}"#,
            )
            .expect("thread/start should return a structured error");
        let value: Value = serde_json::from_str(&response).expect("response should be JSON");

        assert_eq!(value["id"], "thread");
        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "model_provider");
        assert!(server.drain_notifications().is_empty());
        assert!(
            server
                .thread_list(ThreadListParams {
                    cursor: None,
                    limit: None,
                    sort_direction: None,
                })
                .expect("thread/list should still work")
                .data
                .is_empty()
        );
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
            .thread_start(ThreadStartParams {
                cwd: Some("/tmp/workspace".to_string()),
                sandbox: None,
                permission_profile: None,
            })
            .expect("first thread should be created");
        let _second = server
            .thread_start(ThreadStartParams {
                cwd: Some("/tmp/second".to_string()),
                sandbox: None,
                permission_profile: None,
            })
            .expect("second thread should be created");

        let list = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"threads","method":"thread/list"}"#)
            .expect("thread/list should return a response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"thread","method":"thread/read","params":{{"threadId":"{}"}}}}"#,
                first.thread.id
            ))
            .expect("thread/read should return a response");
        let list_value: Value = serde_json::from_str(&list).expect("list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");

        assert_eq!(
            list_value["result"]["data"]
                .as_array()
                .expect("data should be an array")
                .len(),
            2
        );
        assert_eq!(read_value["result"]["thread"]["id"], "thread_1");
        assert_eq!(read_value["result"]["thread"]["cwd"], "/tmp/workspace");
    }

    #[test]
    fn app_server_thread_snapshot_reload_projects_thread_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("threads.json");
        let snapshot = serde_json::json!({
            "version": 1,
            "nextThreadId": 2,
            "nextTurnId": 1,
            "threads": [
                {
                    "threadId": "thread_1",
                    "title": "Loaded",
                    "workspaceRoot": "/tmp/workspace",
                    "sandbox": null,
                    "permissionProfile": null,
                    "forkedFromId": "thread_0",
                    "ephemeral": false,
                    "archived": false,
                    "subscribed": true,
                    "path": "/state/thread_1",
                    "createdAt": 11,
                    "updatedAt": 22,
                    "gitInfo": {
                        "sha": "abc123",
                        "branch": "main",
                        "originUrl": null
                    },
                    "goal": null,
                    "compactedTurnId": null,
                    "tokenUsage": null
                }
            ],
            "turns": []
        });
        fs::write(
            &path,
            serde_json::to_vec_pretty(&snapshot).expect("snapshot JSON"),
        )
        .expect("write snapshot");

        let mut server = AppServer::new()
            .with_thread_snapshot_path(path)
            .expect("load thread snapshot");
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

        let response = server
            .thread_read(ThreadReadParams {
                thread_id: "thread_1".to_string(),
            })
            .expect("thread/read should load persisted thread");
        let thread = response.thread;
        assert_eq!(thread.id, "thread_1");
        assert_eq!(thread.forked_from_id.as_deref(), Some("thread_0"));
        assert!(!thread.ephemeral);
        assert_eq!(thread.created_at, 11);
        assert_eq!(thread.updated_at, 22);
        assert_eq!(thread.path.as_deref(), Some("/state/thread_1"));
        assert_eq!(thread.cwd, "/tmp/workspace");
        assert_eq!(thread.name.as_deref(), Some("Loaded"));
        assert_eq!(
            thread.git_info.expect("git info").sha.as_deref(),
            Some("abc123")
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
    fn thread_archive_unarchive_and_name_emit_notifications() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server.drain_json_rpc_notifications();

        let name = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"name","method":"thread/name/set","params":{"threadId":"thread_1","name":"  Ship R4  "}}"#,
            )
            .expect("thread/name/set should return a response");
        let archive = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"archive","method":"thread/archive","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/archive should return a response");
        let unarchive = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"unarchive","method":"thread/unarchive","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/unarchive should return a response");

        let name_value: Value = serde_json::from_str(&name).expect("name response JSON");
        let archive_value: Value = serde_json::from_str(&archive).expect("archive response JSON");
        let unarchive_value: Value =
            serde_json::from_str(&unarchive).expect("unarchive response JSON");
        assert_eq!(name_value["result"], serde_json::json!({}));
        assert_eq!(archive_value["result"], serde_json::json!({}));
        assert_eq!(unarchive_value["result"]["thread"]["name"], "Ship R4");

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/name/updated"
                && line["params"]["threadId"] == "thread_1"
                && line["params"]["threadName"] == "Ship R4"
        }));
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/archived" && line["params"]["threadId"] == "thread_1"
        }));
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/unarchived" && line["params"]["threadId"] == "thread_1"
        }));
    }

    #[test]
    fn thread_resume_rejects_missing_history_and_path_mismatch_but_loads_existing_id() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server.drain_json_rpc_notifications();

        let missing = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"missing","method":"thread/resume","params":{"threadId":"missing"}}"#,
            )
            .expect("thread/resume missing should return a response");
        let history = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"history","method":"thread/resume","params":{"threadId":"thread_1","history":[{"role":"user"}]}}"#,
            )
            .expect("thread/resume history import should return a response");
        let path = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"path","method":"thread/resume","params":{"threadId":"thread_1","path":"/other/thread.json"}}"#,
            )
            .expect("thread/resume path mismatch should return a response");
        let success = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"resume","method":"thread/resume","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/resume should return a response");

        let missing_value: Value = serde_json::from_str(&missing).expect("missing response JSON");
        let history_value: Value = serde_json::from_str(&history).expect("history response JSON");
        let path_value: Value = serde_json::from_str(&path).expect("path response JSON");
        let success_value: Value = serde_json::from_str(&success).expect("success response JSON");
        assert_eq!(missing_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(
            history_value["error"]["message"],
            "history import is not supported by dasclaw app-server yet"
        );
        assert_eq!(
            path_value["error"]["message"],
            "history import is not supported by dasclaw app-server yet"
        );
        assert_eq!(success_value["result"]["thread"]["id"], "thread_1");
        assert_eq!(success_value["result"]["modelProvider"], "openai");
        assert_eq!(success_value["result"]["cwd"], "/workspace");

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/status/changed"
                && line["params"]["threadId"] == "thread_1"
                && line["params"]["status"]["type"] == "idle"
        }));
    }

    #[test]
    fn thread_fork_sets_forked_from_id_and_clones_turns_unless_excluded() {
        let mut server = initialized_server();
        let source = create_completed_thread_with_turns(&mut server, 2);
        server.drain_json_rpc_notifications();

        let cloned = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"fork","method":"thread/fork","params":{{"threadId":"{source}","ephemeral":true}}}}"#
            ))
            .expect("thread/fork should return a response");
        let excluded = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"fork-empty","method":"thread/fork","params":{{"threadId":"{source}","excludeTurns":true}}}}"#
            ))
            .expect("thread/fork exclude turns should return a response");

        let cloned_value: Value = serde_json::from_str(&cloned).expect("fork response JSON");
        let excluded_value: Value =
            serde_json::from_str(&excluded).expect("fork exclude response JSON");
        assert_eq!(cloned_value["result"]["thread"]["id"], "thread_2");
        assert_eq!(cloned_value["result"]["thread"]["forkedFromId"], source);
        assert_eq!(cloned_value["result"]["thread"]["ephemeral"], true);
        assert_eq!(
            cloned_value["result"]["thread"]["turns"]
                .as_array()
                .expect("turns array")
                .len(),
            2
        );
        assert_eq!(excluded_value["result"]["thread"]["id"], "thread_3");
        assert_eq!(excluded_value["result"]["thread"]["forkedFromId"], source);
        assert_eq!(
            excluded_value["result"]["thread"]["turns"]
                .as_array()
                .expect("turns array")
                .len(),
            0
        );

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/started" && line["params"]["thread"]["id"] == "thread_2"
        }));
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/started" && line["params"]["thread"]["id"] == "thread_3"
        }));
    }

    #[test]
    fn thread_fork_rejects_source_with_pending_turns() {
        let mut server = initialized_server();
        let source = create_completed_thread_with_turns(&mut server, 1);
        let pending_id = server.threads.next_turn_id();
        server
            .threads
            .record_started_turn(&source, pending_id)
            .expect("turn should be recorded");

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"fork","method":"thread/fork","params":{{"threadId":"{source}"}}}}"#
            ))
            .expect("thread/fork should return a response");
        let value: Value = serde_json::from_str(&response).expect("fork response JSON");

        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert!(
            value["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("pending turns")
        );
    }

    #[test]
    fn thread_unsubscribe_returns_unsubscribed_then_not_subscribed_and_not_loaded() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"resume","method":"thread/resume","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/resume should return a response");
        server.drain_json_rpc_notifications();

        let first = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"first","method":"thread/unsubscribe","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/unsubscribe should return a response");
        let second = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"second","method":"thread/unsubscribe","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/unsubscribe should return a response");
        let missing = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"missing","method":"thread/unsubscribe","params":{"threadId":"missing"}}"#,
            )
            .expect("thread/unsubscribe should return a response");

        let first_value: Value = serde_json::from_str(&first).expect("first response JSON");
        let second_value: Value = serde_json::from_str(&second).expect("second response JSON");
        let missing_value: Value = serde_json::from_str(&missing).expect("missing response JSON");
        assert_eq!(first_value["result"]["status"], "unsubscribed");
        assert_eq!(second_value["result"]["status"], "notSubscribed");
        assert_eq!(missing_value["result"]["status"], "notLoaded");
    }

    #[test]
    fn thread_rollback_rejects_pending_removes_terminal_turns_and_validates_count() {
        let mut server = initialized_server();
        let thread_id = create_completed_thread_with_turns(&mut server, 2);
        let pending_id = server.threads.next_turn_id();
        server
            .threads
            .record_started_turn(&thread_id, pending_id)
            .expect("record pending turn");

        let pending = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"pending","method":"thread/rollback","params":{{"threadId":"{thread_id}","numTurns":1}}}}"#
            ))
            .expect("thread/rollback pending should return a response");
        let pending_value: Value = serde_json::from_str(&pending).expect("pending response JSON");
        assert_eq!(pending_value["error"]["data"]["code"], "INVALID_PARAMS");

        let pending_id = server
            .threads
            .list_turns(&thread_id)
            .last()
            .expect("pending turn")
            .turn_id
            .clone();
        server
            .threads
            .cancel_turn(&thread_id, &pending_id)
            .expect("cancel pending turn");
        let zero = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"zero","method":"thread/rollback","params":{{"threadId":"{thread_id}","numTurns":0}}}}"#
            ))
            .expect("thread/rollback zero should return a response");
        let success = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"success","method":"thread/rollback","params":{{"threadId":"{thread_id}","numTurns":2}}}}"#
            ))
            .expect("thread/rollback should return a response");

        let zero_value: Value = serde_json::from_str(&zero).expect("zero response JSON");
        let success_value: Value = serde_json::from_str(&success).expect("success response JSON");
        assert_eq!(zero_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(
            success_value["result"]["thread"]["turns"]
                .as_array()
                .expect("turns array")
                .len(),
            1
        );

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/status/changed" && line["params"]["threadId"] == thread_id
        }));
    }

    #[test]
    fn thread_loaded_list_paginates_subscribed_non_archived_ids_in_creation_order() {
        let mut server = initialized_server();
        for cwd in ["/one", "/two", "/three"] {
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{{"cwd":"{cwd}"}}}}"#
                ))
                .expect("thread/start should return a response");
        }
        for thread_id in ["thread_1", "thread_2", "thread_3"] {
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"resume","method":"thread/resume","params":{{"threadId":"{thread_id}"}}}}"#
                ))
                .expect("thread/resume should return a response");
        }
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"archive","method":"thread/archive","params":{"threadId":"thread_2"}}"#,
            )
            .expect("thread/archive should return a response");

        let first = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"loaded","method":"thread/loaded/list","params":{"limit":1}}"#,
            )
            .expect("thread/loaded/list should return a response");
        let first_value: Value = serde_json::from_str(&first).expect("first response JSON");
        let cursor = first_value["result"]["nextCursor"]
            .as_str()
            .expect("next cursor");
        let second = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"loaded2","method":"thread/loaded/list","params":{{"cursor":"{cursor}","limit":5}}}}"#
            ))
            .expect("thread/loaded/list should return a response");
        let zero = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"zero","method":"thread/loaded/list","params":{"limit":0}}"#,
            )
            .expect("thread/loaded/list zero should return a response");

        let second_value: Value = serde_json::from_str(&second).expect("second response JSON");
        let zero_value: Value = serde_json::from_str(&zero).expect("zero response JSON");
        assert_eq!(
            first_value["result"]["data"],
            serde_json::json!(["thread_1"])
        );
        assert_eq!(
            second_value["result"]["data"],
            serde_json::json!(["thread_3"])
        );
        assert_eq!(
            second_value["result"]["nextCursor"],
            serde_json::Value::Null
        );
        assert_eq!(zero_value["error"]["data"]["code"], "INVALID_PARAMS");
    }

    #[test]
    fn thread_inject_items_rejects_missing_thread_and_appends_without_executing() {
        let mut server = initialized_server();
        let missing = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"missing","method":"thread/inject_items","params":{"threadId":"missing","items":[{"type":"agentMessage","text":"ghost"}]}}"#,
            )
            .expect("thread/inject_items missing should return a response");
        let missing_value: Value = serde_json::from_str(&missing).expect("missing response JSON");
        assert_eq!(missing_value["error"]["data"]["code"], "INVALID_PARAMS");

        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        let first = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"inject1","method":"thread/inject_items","params":{"threadId":"thread_1","items":[{"type":"agentMessage","text":"seed"}]}}"#,
            )
            .expect("thread/inject_items should return a response");
        let second = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"inject2","method":"thread/inject_items","params":{"threadId":"thread_1","items":[{"type":"reasoning","summary":["note"]}]}}"#,
            )
            .expect("thread/inject_items append should return a response");
        let read = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"read","method":"thread/read","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/read should return a response");

        let first_value: Value = serde_json::from_str(&first).expect("first response JSON");
        let second_value: Value = serde_json::from_str(&second).expect("second response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");
        assert_eq!(first_value["result"], serde_json::json!({}));
        assert_eq!(second_value["result"], serde_json::json!({}));
        let turns = read_value["result"]["thread"]["turns"]
            .as_array()
            .expect("turns array");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["items"][0]["type"], "agentMessage");
        assert_eq!(turns[0]["items"][0]["text"], "seed");
        assert_eq!(turns[0]["items"][1]["type"], "reasoning");
        assert_eq!(turns[0]["items"][1]["summary"], serde_json::json!(["note"]));
    }

    #[test]
    fn thread_lifecycle_inject_review_mode_items_requires_string_review() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");

        let valid = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"valid","method":"thread/inject_items","params":{"threadId":"thread_1","items":[{"type":"enteredReviewMode","review":"review started"},{"type":"exitedReviewMode","review":"review exited"}]}}"#,
            )
            .expect("valid review items should return a response");
        let read = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"read","method":"thread/read","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/read should return a response");

        let valid_value: Value = serde_json::from_str(&valid).expect("valid response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");
        assert_eq!(valid_value["result"], serde_json::json!({}));
        let items = read_value["result"]["thread"]["turns"][0]["items"]
            .as_array()
            .expect("items array");
        assert_eq!(items[0]["type"], "enteredReviewMode");
        assert_eq!(items[0]["review"], "review started");
        assert_eq!(items[1]["type"], "exitedReviewMode");
        assert_eq!(items[1]["review"], "review exited");

        for (id, item) in [
            ("missing", serde_json::json!({"type": "enteredReviewMode"})),
            (
                "wrong",
                serde_json::json!({"type": "exitedReviewMode", "review": 42}),
            ),
        ] {
            let response = server
                .handle_json_rpc(
                    &serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": "thread/inject_items",
                        "params": {
                            "threadId": "thread_1",
                            "items": [item]
                        }
                    })
                    .to_string(),
                )
                .expect("malformed review item should return a response");
            let value: Value = serde_json::from_str(&response).expect("error response JSON");
            assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
            assert_eq!(value["error"]["data"]["capability"], "thread_lifecycle");
            assert!(
                value["error"]["message"]
                    .as_str()
                    .expect("error message")
                    .contains("review must be a string")
            );
        }
    }

    #[test]
    fn thread_goal_set_get_and_clear_persist_and_emit_notifications() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server.drain_json_rpc_notifications();

        let set = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"set","method":"thread/goal/set","params":{"threadId":"thread_1","objective":" Ship R4 ","status":"budgetLimited","tokenBudget":5000}}"#,
            )
            .expect("thread/goal/set should return a response");
        let get = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"get","method":"thread/goal/get","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/goal/get should return a response");
        let clear = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"clear","method":"thread/goal/clear","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/goal/clear should return a response");
        let after_clear = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"after","method":"thread/goal/get","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/goal/get after clear should return a response");

        let set_value: Value = serde_json::from_str(&set).expect("set response JSON");
        let get_value: Value = serde_json::from_str(&get).expect("get response JSON");
        let clear_value: Value = serde_json::from_str(&clear).expect("clear response JSON");
        let after_clear_value: Value =
            serde_json::from_str(&after_clear).expect("after clear response JSON");
        assert_eq!(set_value["result"]["goal"]["objective"], "Ship R4");
        assert_eq!(set_value["result"]["goal"]["status"], "budgetLimited");
        assert_eq!(set_value["result"]["goal"]["tokenBudget"], 5000);
        assert_eq!(get_value["result"]["goal"], set_value["result"]["goal"]);
        assert_eq!(clear_value["result"], serde_json::json!({}));
        assert_eq!(after_clear_value["result"]["goal"], serde_json::Value::Null);

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/goal/updated"
                && line["params"]["threadId"] == "thread_1"
                && line["params"]["goal"]["objective"] == "Ship R4"
        }));
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/goal/cleared" && line["params"]["threadId"] == "thread_1"
        }));
    }

    #[test]
    fn thread_token_usage_update_accumulates_and_emits_nonzero_usage() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server.runtime_turn_updates.token_usage_updated(
            thread.thread_id.clone(),
            start.turn.id.clone(),
            TokenUsage {
                input_tokens: 6,
                output_tokens: 4,
                cache_creation_input_tokens: 1,
                cache_read_input_tokens: 2,
            },
        );
        server.runtime_turn_updates.token_usage_updated(
            thread.thread_id.clone(),
            start.turn.id.clone(),
            TokenUsage {
                input_tokens: 3,
                output_tokens: 2,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 1,
            },
        );

        let notifications = server.drain_notifications();
        let usage_events = notifications
            .iter()
            .filter(|notification| notification.method == "thread/tokenUsage/updated")
            .collect::<Vec<_>>();
        assert_eq!(usage_events.len(), 2);
        assert_eq!(
            usage_events[0].params["tokenUsage"]["last"]["inputTokens"],
            6
        );
        assert_eq!(
            usage_events[0].params["tokenUsage"]["last"]["cachedInputTokens"],
            3
        );
        assert_eq!(
            usage_events[0].params["tokenUsage"]["last"]["totalTokens"],
            10
        );
        assert_eq!(
            usage_events[1].params["tokenUsage"]["total"]["inputTokens"],
            9
        );
        assert_eq!(
            usage_events[1].params["tokenUsage"]["total"]["cachedInputTokens"],
            4
        );
        assert_eq!(
            usage_events[1].params["tokenUsage"]["total"]["outputTokens"],
            6
        );
    }

    #[test]
    fn review_start_inline_runs_review_turn_on_existing_thread() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let response = server
            .review_start(ReviewStartParams {
                thread_id: thread.thread_id.clone(),
                target: ReviewTarget::UncommittedChanges,
                delivery: None,
            })
            .expect("inline review should start");

        assert_eq!(response.review_thread_id, thread.thread_id);
        assert_eq!(response.turn.status, CodexTurnStatus::InProgress);
        assert_eq!(response.turn.id, "turn_1");

        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].thread_id, response.review_thread_id);
        assert_eq!(calls[0].turn_id, response.turn.id);
        assert!(
            calls[0]
                .prompt
                .contains("Review request: uncommitted changes")
        );
        assert!(calls[0].prompt.contains("Focus on correctness"));
        assert!(calls[0].prompt.contains("Return findings first"));
        drop(calls);
        interrupt_turn(&mut server, response.review_thread_id, response.turn.id);
    }

    #[test]
    fn review_start_detached_forks_review_thread() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let response = server
            .review_start(ReviewStartParams {
                thread_id: thread.thread_id.clone(),
                target: ReviewTarget::BaseBranch {
                    branch: "main".to_string(),
                },
                delivery: Some(ReviewDelivery::Detached),
            })
            .expect("detached review should start");

        assert_ne!(response.review_thread_id, thread.thread_id);
        assert_eq!(response.turn.status, CodexTurnStatus::InProgress);
        let review_thread = server
            .thread_read(ThreadReadParams {
                thread_id: response.review_thread_id.clone(),
            })
            .expect("review thread should be readable")
            .thread;
        assert_eq!(review_thread.forked_from_id, Some(thread.thread_id));
        assert!(review_thread.ephemeral);

        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].thread_id, response.review_thread_id);
        assert!(
            calls[0]
                .prompt
                .contains("Review request: changes against base branch main")
        );
        assert!(calls[0].prompt.contains("file and line references"));
        drop(calls);
        interrupt_turn(&mut server, response.review_thread_id, response.turn.id);
    }

    #[test]
    fn review_start_rejects_empty_custom_instructions() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let error = server
            .review_start(ReviewStartParams {
                thread_id: thread.thread_id,
                target: ReviewTarget::Custom {
                    instructions: " \n\t ".to_string(),
                },
                delivery: Some(ReviewDelivery::Inline),
            })
            .expect_err("empty custom instructions should fail");

        let AppServerError::Protocol { data } = error;
        assert_eq!(data.capability, Some("review".to_string()));
        assert!(bridge.calls.lock().expect("calls lock").is_empty());
    }

    #[test]
    fn review_start_json_rpc_returns_in_progress_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let response = server
            .handle_json_rpc(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "review",
                    "method": "review/start",
                    "params": {
                        "threadId": thread.thread_id,
                        "target": {"type": "uncommittedChanges"},
                        "delivery": "inline"
                    }
                })
                .to_string(),
            )
            .expect("review/start should return a response");

        let value: Value = serde_json::from_str(&response).expect("review response JSON");
        assert_eq!(value["result"]["reviewThreadId"], "thread_1");
        assert_eq!(value["result"]["turn"]["status"], "inProgress");
        assert_eq!(value["result"]["turn"]["id"], "turn_1");
        interrupt_turn(&mut server, "thread_1", "turn_1");
    }

    #[test]
    fn review_start_json_rpc_reports_review_capability_before_initialize() {
        let mut server = AppServer::new();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"review","method":"review/start","params":{"threadId":"thread_1","target":{"type":"uncommittedChanges"}}}"#,
            )
            .expect("review/start should return a response");

        let value: Value = serde_json::from_str(&response).expect("review error JSON");
        assert_eq!(value["error"]["data"]["code"], "NOT_INITIALIZED");
        assert_eq!(value["error"]["data"]["capability"], "review");
    }

    #[test]
    fn review_start_json_rpc_reports_review_capability_for_unknown_thread() {
        let mut server =
            initialized_server_with_bridge(Arc::new(RecordingRuntimeBridge::default()));

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"review","method":"review/start","params":{"threadId":"missing","target":{"type":"uncommittedChanges"}}}"#,
            )
            .expect("review/start should return a response");

        let value: Value = serde_json::from_str(&response).expect("review error JSON");
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "review");
    }

    #[test]
    fn review_start_commit_target_prompt_includes_sha_and_title() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        server
            .review_start(ReviewStartParams {
                thread_id: thread.thread_id.clone(),
                target: ReviewTarget::Commit {
                    sha: "abc123".to_string(),
                    title: Some("fix owner routing".to_string()),
                },
                delivery: Some(ReviewDelivery::Inline),
            })
            .expect("commit review should start");

        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0]
                .prompt
                .contains("Review request: commit abc123: fix owner routing")
        );
        drop(calls);
        interrupt_turn(&mut server, thread.thread_id, "turn_1");
    }

    #[test]
    fn review_start_detached_pending_turn_error_uses_review_capability() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let pending = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("leave pending".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let error = server
            .review_start(ReviewStartParams {
                thread_id: thread.thread_id.clone(),
                target: ReviewTarget::UncommittedChanges,
                delivery: Some(ReviewDelivery::Detached),
            })
            .expect_err("detached review should reject pending source thread");

        let AppServerError::Protocol { data } = error;
        assert_eq!(data.code, ErrorCode::InvalidParams);
        assert_eq!(data.capability, Some("review".to_string()));
        assert!(data.message.contains("pending turns"));
        assert!(!data.retryable);
        assert!(data.lifecycle.is_none());
        interrupt_turn(&mut server, thread.thread_id, pending.turn.id);
    }

    #[test]
    fn thread_compact_start_is_capability_unavailable_without_runtime_owner() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server.drain_json_rpc_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"compact","method":"thread/compact/start","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/compact/start should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("compact response JSON");

        assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
        assert_ne!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
        assert!(
            !json_rpc_values(server.drain_json_rpc_notifications())
                .iter()
                .any(|line| line["method"] == "thread/compacted")
        );
    }

    #[test]
    fn thread_compact_start_records_compacted_turn_when_runtime_supports_it() {
        let bridge = Arc::new(CompactRuntimeBridge);
        let mut server = initialized_server_with_bridge(bridge);
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"start","method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server.drain_json_rpc_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"compact","method":"thread/compact/start","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/compact/start should return a response");
        let value: Value = serde_json::from_str(&response).expect("compact response JSON");

        assert_eq!(value["result"], serde_json::json!({}));
        assert_eq!(
            server
                .threads
                .summary("thread_1")
                .expect("thread summary")
                .compacted_turn_id
                .as_deref(),
            Some("turn_compacted")
        );
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/compacted"
                && line["params"]["threadId"] == "thread_1"
                && line["params"]["turnId"] == "turn_compacted"
        }));
    }

    #[test]
    fn json_rpc_turn_start_requires_initialize() {
        let mut server = AppServer::new();
        let start = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#,
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
                r#"{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{"threadId":"missing","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#,
            )
            .expect("turn/start should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["error"]["code"], -32602);
        assert_eq!(start_value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(start_value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_and_interrupt_manage_pending_in_memory_turns() {
        let mut server = initialized_server();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: Some("Draft".to_string()),
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"hello","text_elements":[]}}]}}}}"#,
                thread.thread.id
            ))
            .expect("turn/start should return a structured response");
        let start_value: Value = serde_json::from_str(&start).expect("start response JSON");

        assert_eq!(start_value["result"]["turn"]["id"], "turn_1");
        assert_eq!(start_value["result"]["turn"]["status"], "inProgress");
        let started = server.drain_notifications();
        assert_eq!(started.len(), 3);
        assert_eq!(started[0].method, "lifecycle/changed");
        assert_eq!(started[0].params["lifecycle"]["state"], "running");
        assert_eq!(started[1].method, "turn/started");
        assert_eq!(started[2].method, "item/started");

        let interrupt = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-interrupt","method":"turn/interrupt","params":{{"threadId":"{}","turnId":"turn_1"}}}}"#,
                thread.thread.id
            ))
            .expect("turn/interrupt should return a structured response");
        let interrupt_value: Value =
            serde_json::from_str(&interrupt).expect("interrupt response JSON");

        assert_eq!(interrupt_value["result"], serde_json::json!({}));
        let cancelled = server.drain_notifications();
        assert_eq!(cancelled.len(), 3);
        assert_eq!(cancelled[0].method, "item/completed");
        assert_eq!(cancelled[1].method, "turn/completed");
        assert_eq!(cancelled[1].params["turn"]["status"], "interrupted");
        assert_eq!(cancelled[2].method, "lifecycle/changed");
        assert_eq!(cancelled[2].params["lifecycle"]["state"], "ready");
    }

    #[test]
    fn turn_steer_json_rpc_routes_and_forwards_active_turn_input() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_turn_steer());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello runtime"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"steer","method":"turn/steer","params":{{"threadId":"{}","expectedTurnId":"{}","input":[{{"type":"text","text":"extra context","text_elements":[]}}],"responsesapiClientMetadata":{{"source":"test"}}}}}}"#,
                thread.thread_id, start.turn.id
            ))
            .expect("turn/steer should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("steer response JSON");

        assert_eq!(value["result"]["turnId"], start.turn.id);
        let calls = bridge.steer_calls.lock().expect("steer calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].thread_id, thread.thread_id);
        assert_eq!(calls[0].turn_id, start.turn.id);
        assert_eq!(calls[0].prompt, "extra context");
        assert_eq!(calls[0].input, text_input("extra context"));
        assert_eq!(
            calls[0]
                .responsesapi_client_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("source"))
                .map(String::as_str),
            Some("test")
        );
    }

    #[test]
    fn turn_steer_rejects_feature_disabled_without_bridge_call() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello runtime"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let error = server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id,
                expected_turn_id: start.turn.id,
                input: text_input("extra context"),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should require runtime feature support");

        assert!(matches!(
            error,
            AppServerError::Protocol { data } if data.code == ErrorCode::CapabilityUnavailable
        ));
        assert!(
            bridge
                .steer_calls
                .lock()
                .expect("steer calls lock")
                .is_empty()
        );
    }

    #[test]
    fn turn_steer_rejects_mismatched_or_missing_active_turn_without_bridge_call() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_turn_steer());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello runtime"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let mismatch = server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id.clone(),
                expected_turn_id: "turn_missing".to_string(),
                input: text_input("extra context"),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should reject missing active turn");
        assert!(matches!(
            mismatch,
            AppServerError::Protocol { data } if data.code == ErrorCode::InvalidParams
        ));

        server
            .threads
            .apply_runtime_turn_update(RuntimeTurnUpdate {
                thread_id: thread.thread_id.clone(),
                turn_id: start.turn.id.clone(),
                outcome: RuntimeTurnOutcome::Completed {
                    output: "done".to_string(),
                },
            })
            .expect("turn should complete");
        let completed = server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id.clone(),
                expected_turn_id: start.turn.id.clone(),
                input: text_input("extra context"),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should reject terminal turn");
        assert!(matches!(
            completed,
            AppServerError::Protocol { data } if data.code == ErrorCode::InvalidParams
        ));

        let other_thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("other thread should be created");
        let wrong_thread = server
            .turn_steer(TurnSteerParams {
                thread_id: other_thread.thread_id,
                expected_turn_id: start.turn.id,
                input: text_input("extra context"),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should reject mismatched thread");
        assert!(matches!(
            wrong_thread,
            AppServerError::Protocol { data } if data.code == ErrorCode::InvalidParams
        ));
        assert!(
            bridge
                .steer_calls
                .lock()
                .expect("steer calls lock")
                .is_empty()
        );
    }

    #[test]
    fn turn_steer_rejects_empty_input_without_bridge_call() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_turn_steer());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello runtime"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let error = server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id.clone(),
                expected_turn_id: start.turn.id.clone(),
                input: Vec::new(),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should reject empty input");

        assert!(matches!(
            error,
            AppServerError::Protocol { data } if data.code == ErrorCode::InvalidParams
        ));
        assert!(
            bridge
                .steer_calls
                .lock()
                .expect("steer calls lock")
                .is_empty()
        );

        let whitespace = server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id.clone(),
                expected_turn_id: start.turn.id.clone(),
                input: text_input("   "),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should reject whitespace-only input");
        assert!(matches!(
            whitespace,
            AppServerError::Protocol { data } if data.code == ErrorCode::InvalidParams
        ));

        let image_only = server
            .turn_steer(TurnSteerParams {
                thread_id: thread.thread_id,
                expected_turn_id: start.turn.id,
                input: image_input("file:///tmp/image.png"),
                responsesapi_client_metadata: None,
            })
            .expect_err("turn/steer should reject image-only input");
        assert!(matches!(
            image_only,
            AppServerError::Protocol { data } if data.code == ErrorCode::InvalidParams
        ));
        assert!(
            bridge
                .steer_calls
                .lock()
                .expect("steer calls lock")
                .is_empty()
        );
    }

    #[test]
    fn turn_start_invokes_runtime_bridge_before_recording_pending_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello runtime".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start through runtime bridge");

        assert_eq!(start.turn.id, "turn_1");
        assert_eq!(
            start.turn.status,
            codex_status_from_turn_status(TurnStatus::Pending)
        );
        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].thread_id, thread.thread_id);
        assert_eq!(calls[0].turn_id, "turn_1");
        assert_eq!(calls[0].prompt, "hello runtime");
        assert_eq!(calls[0].reasoning_summary, ReasoningSummary::None);
        assert_eq!(calls[0].model_provider.model_id, "gpt-test");
        assert_eq!(
            calls[0].model_provider.api_base_url,
            TEST_MODEL_API_BASE_URL
        );
        assert_eq!(calls[0].model_provider.api_key, "test-api-key");

        let turns = server
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: calls[0].thread_id.clone(),
            })
            .expect("turn/list should still use session bookkeeping");
        assert_eq!(turns.turns.len(), 1);
        assert_eq!(turns.turns[0].status, TurnStatus::Pending);
    }

    #[test]
    fn turn_start_passes_reasoning_summary_to_runtime_request() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                input: text_input("hello runtime".to_string()),
                cwd: None,
                model: None,
                summary: Some("concise".to_string()),
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start through runtime bridge");

        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].reasoning_summary, ReasoningSummary::Concise);
    }

    #[test]
    fn turn_start_passes_thread_sandbox_context_to_runtime_bridge() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());

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

        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].sandbox_context.sandbox,
            Some(SandboxMode::WorkspaceWrite)
        );
        assert_eq!(calls[0].cwd, PathBuf::from("/tmp"));
    }

    #[test]
    fn turn_start_policy_overrides_thread_sandbox_context() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());

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

        let calls = bridge.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].sandbox_context.policy,
            Some(dasclaw_workspace_cap::policy::SandboxPolicy::new_read_only_policy())
        );
    }

    #[test]
    fn thread_start_rejects_danger_full_access_sandbox_context() {
        let mut server = initialized_server();

        let error = server
            .thread_start(ThreadStartParams {
                cwd: Some("/tmp".to_string()),
                sandbox: Some(SandboxMode::DangerFullAccess),
                permission_profile: None,
            })
            .expect_err("danger-full-access should be rejected for runtime context");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::InvalidParams);
                assert_eq!(data.capability.as_deref(), Some("thread/start"));
                assert!(
                    data.message.contains("danger-full-access"),
                    "{}",
                    data.message
                );
            }
        }
    }

    #[test]
    fn turn_start_rejects_danger_full_access_sandbox_policy() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let error = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                input: vec![UserInput::Text {
                    text: "hi".to_string(),
                    text_elements: Vec::new(),
                }],
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: Some(serde_json::json!({ "type": "danger-full-access" })),
                permission_profile: None,
            })
            .expect_err("danger-full-access should be rejected for runtime context");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::InvalidParams);
                assert_eq!(data.capability.as_deref(), Some("turn/start"));
                assert!(
                    data.message.contains("danger-full-access"),
                    "{}",
                    data.message
                );
            }
        }
        assert!(bridge.calls.lock().expect("calls lock").is_empty());
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created before model selection is needed");

        let error = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello runtime".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
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
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: thread.thread_id,
            })
            .expect("turn/list should still work after fail-safe rejection");
        assert!(turns.turns.is_empty());
    }

    #[test]
    fn turn_start_reports_pending_cancel_persist_failure_after_runtime_start_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("threads.json");
        let bridge = Arc::new(SnapshotBlockingRuntimeBridge {
            snapshot_path: path.clone(),
        });
        let mut server = AppServer::with_runtime_bridge(bridge)
            .with_thread_snapshot_path(path)
            .expect("persistent server");
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
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let error = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                input: text_input("hello runtime".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect_err("runtime and rollback persistence failures should be reported");

        match error {
            AppServerError::Protocol { data } => {
                assert_eq!(data.code, ErrorCode::ServiceDegraded);
                assert_eq!(data.capability.as_deref(), Some("thread_lifecycle"));
                assert!(data.message.contains("runtime start failed"));
                assert!(data.message.contains("failed to cancel pending turn"));
            }
        }
        let turns = server
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: thread.thread.id,
            })
            .expect("turn/list should still work");
        assert_eq!(turns.turns[0].status, TurnStatus::Pending);
    }

    #[test]
    fn model_provider_selection_applies_to_next_turn_snapshot_only() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let first = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("first".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
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
                input: text_input("second".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("second turn should use newly selected model");

        assert_eq!(response.selected_model_id, "gpt-next");
        assert_eq!(first.turn.id, "turn_1");
        assert_eq!(second.turn.id, "turn_2");
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-start","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"hello","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a structured runtime error");
        let value: Value = serde_json::from_str(&response).expect("start response JSON");

        assert_eq!(value["error"]["code"], -32005);
        assert_eq!(value["error"]["data"]["code"], "SERVICE_DEGRADED");
        assert_eq!(value["error"]["data"]["capability"], "runtime");
        assert_eq!(value["error"]["data"]["retryable"], true);
        let turns = server
            .list_turns_for_test(TestTurnsSnapshotParams {
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
                "capabilities/changed",
                "notifications/initialized"
            ]
        );

        let thread = json_rpc_value(
            server
                .handle_json_rpc(
                    r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#,
                )
                .expect("thread/start should return a JSON-RPC response"),
        );
        let thread_id = thread["result"]["thread"]["id"]
            .as_str()
            .expect("thread id should be returned")
            .to_string();
        let thread_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&thread_notifications),
            vec!["thread/started"]
        );

        let turn = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{thread_id}","input":[{{"type":"text","text":"hello","text_elements":[]}}]}}}}"#
                ))
                .expect("turn/start should return a JSON-RPC response"),
        );
        let turn_id = turn["result"]["turn"]["id"]
            .as_str()
            .expect("turn id should be returned")
            .to_string();
        assert_eq!(turn["result"]["turn"]["status"], "inProgress");

        let immediate_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&immediate_notifications),
            vec!["lifecycle/changed", "turn/started", "item/started"],
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
                "item/agentMessage/delta",
                "item/agentMessage/delta",
                "item/completed",
                "turn/completed",
                "lifecycle/changed"
            ]
        );
        assert_eq!(turn_notifications[0]["params"]["delta"], "Hel");
        assert_eq!(turn_notifications[1]["params"]["delta"], "lo");
        assert_eq!(turn_notifications[2]["params"]["item"]["text"], "Hello");
        assert_eq!(
            turn_notifications[3]["params"]["turn"]["status"],
            "completed"
        );
        assert_eq!(
            turn_notifications[4]["params"]["lifecycle"]["state"],
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
        assert_eq!(read["result"]["turn"]["items"][0]["text"], "Hello");
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
                    r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#,
                )
                .expect("thread/start should return a JSON-RPC response"),
        );
        let thread_id = thread["result"]["thread"]["id"]
            .as_str()
            .expect("thread id should be returned")
            .to_string();
        let _ = server.drain_json_rpc_notifications();

        let turn = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"hello","text_elements":[]}}]}}}}"#,
                    thread_id
                ))
                .expect("turn/start should return a JSON-RPC response"),
        );
        let turn_id = turn["result"]["turn"]["id"]
            .as_str()
            .expect("turn id should be returned")
            .to_string();

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&notifications),
            vec![
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "item/completed",
                "turn/completed",
                "error",
                "lifecycle/changed"
            ]
        );
        assert_eq!(notifications[0]["params"]["lifecycle"]["state"], "running");
        assert_eq!(notifications[4]["params"]["turn"]["status"], "failed");
        assert_eq!(
            notifications[4]["params"]["turn"]["error"]["message"],
            "runtime failed"
        );
        assert_eq!(notifications[5]["params"]["message"], "runtime failed");
        assert_eq!(notifications[6]["params"]["lifecycle"]["state"], "ready");

        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"read","method":"turn/read","params":{{"threadId":"{thread_id}","turnId":"{turn_id}"}}}}"#
            ))
            .expect("turn/read should return a JSON-RPC response");
        let read = json_rpc_value(read);
        assert_eq!(read["result"]["turn"]["status"], "failed");
        assert_eq!(read["result"]["turn"]["error"]["message"], "runtime failed");
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
                    r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#,
                )
                .expect("thread/start should return a JSON-RPC response"),
        );
        let thread_id = thread["result"]["thread"]["id"]
            .as_str()
            .expect("thread id should be returned")
            .to_string();
        let _ = server.drain_json_rpc_notifications();

        let turn = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"hello","text_elements":[]}}]}}}}"#,
                    thread_id
                ))
                .expect("turn/start should return a JSON-RPC response"),
        );
        let turn_id = turn["result"]["turn"]["id"]
            .as_str()
            .expect("turn id should be returned")
            .to_string();
        assert_eq!(
            methods_from_values(&json_rpc_values(server.drain_json_rpc_notifications())),
            vec!["lifecycle/changed", "turn/started", "item/started"]
        );

        let interrupt = json_rpc_value(
            server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":"interrupt","method":"turn/interrupt","params":{{"threadId":"{thread_id}","turnId":"{turn_id}"}}}}"#
                ))
                .expect("turn/interrupt should return a JSON-RPC response"),
        );
        assert_eq!(interrupt["result"], serde_json::json!({}));
        assert_eq!(
            methods_from_values(&json_rpc_values(server.drain_json_rpc_notifications())),
            vec!["item/completed", "turn/completed", "lifecycle/changed"]
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
        assert_eq!(read["result"]["turn"]["status"], "interrupted");
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 7);
        assert_eq!(notifications[0].method, "thread/started");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[2].method, "turn/started");
        assert_eq!(notifications[3].method, "item/started");
        assert_eq!(notifications[4].method, "item/completed");
        assert_eq!(
            notifications[4].params["item"]["text"],
            "hello from runtime"
        );
        assert_eq!(notifications[5].method, "turn/completed");
        assert_eq!(notifications[5].params["turn"]["status"], "completed");
        assert_eq!(notifications[6].method, "lifecycle/changed");
        assert_eq!(notifications[6].params["lifecycle"]["state"], "ready");

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: start.turn.id,
            })
            .expect("turn/read should apply completion update");

        assert_eq!(
            read.turn.status,
            codex_status_from_turn_status(TurnStatus::Completed)
        );
        assert_eq!(turn_text(&read.turn).as_deref(), Some("hello from runtime"));
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 8);
        assert_eq!(notifications[0].method, "thread/started");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[2].method, "turn/started");
        assert_eq!(notifications[3].method, "item/started");
        assert_eq!(notifications[4].method, "item/completed");
        assert_eq!(notifications[5].method, "turn/completed");
        assert_eq!(notifications[5].params["turn"]["status"], "failed");
        assert_eq!(
            notifications[5].params["turn"]["error"]["message"],
            "runtime failed"
        );
        assert_eq!(notifications[6].method, "error");
        assert_eq!(notifications[6].params["message"], "runtime failed");
        assert_eq!(notifications[7].method, "lifecycle/changed");
        assert_eq!(notifications[7].params["lifecycle"]["state"], "ready");

        let list = server
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: thread.thread_id,
            })
            .expect("turn/list should apply failure update");

        assert_eq!(list.turns.len(), 1);
        assert_eq!(list.turns[0].turn_id, start.turn.id);
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let first = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("first".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("first turn should start");
        let second = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("second".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("second turn should start");
        let _ = server.drain_notifications();

        server.runtime_turn_updates.complete(
            thread.thread_id.clone(),
            first.turn.id,
            "first done".to_string(),
        );
        let _after_first = server
            .list_turns_for_test(TestTurnsSnapshotParams {
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
            vec!["item/completed", "turn/completed"]
        );

        server.runtime_turn_updates.complete(
            thread.thread_id.clone(),
            second.turn.id,
            "second done".to_string(),
        );
        let _after_second = server
            .list_turns_for_test(TestTurnsSnapshotParams {
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
            vec!["item/completed", "turn/completed", "lifecycle/changed"]
        );
        assert_eq!(
            second_notifications[2].params["lifecycle"]["state"],
            "ready"
        );
    }

    #[test]
    fn lifecycle_status_applies_runtime_terminal_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server
            .runtime_turn_updates
            .complete(thread.thread_id, turn.turn.id, "done".to_string());
        let status = server.lifecycle_status();
        let notifications = server.drain_notifications();

        assert_eq!(status.lifecycle.state, LifecycleState::Ready);
        assert_eq!(
            notifications
                .iter()
                .map(|notification| notification.method.as_str())
                .collect::<Vec<_>>(),
            vec!["item/completed", "turn/completed", "lifecycle/changed"]
        );
    }

    #[test]
    fn health_check_applies_runtime_terminal_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server.runtime_turn_updates.fail(
            thread.thread_id,
            turn.turn.id,
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
            vec![
                "item/completed",
                "turn/completed",
                "error",
                "lifecycle/changed"
            ]
        );
    }

    #[test]
    fn capabilities_list_applies_runtime_terminal_update() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        server
            .runtime_turn_updates
            .complete(thread.thread_id, turn.turn.id, "done".to_string());
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
            vec!["item/completed", "turn/completed", "lifecycle/changed"]
        );
        assert_eq!(notifications[2].params["lifecycle"]["state"], "ready");
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        assert_eq!(notifications.len(), 5);
        assert_eq!(notifications[0].method, "thread/started");
        assert_eq!(notifications[1].method, "lifecycle/changed");
        assert_eq!(notifications[1].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[2].method, "turn/started");
        assert_eq!(notifications[3].method, "item/started");
        assert_eq!(notifications[4].method, "item/agentMessage/delta");
        assert_eq!(notifications[4].params["delta"], "hel");

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: start.turn.id,
            })
            .expect("turn/read should apply delta update");

        assert_eq!(
            read.turn.status,
            codex_status_from_turn_status(TurnStatus::Pending)
        );
        assert_eq!(turn_text(&read.turn), None);
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let lines = server.drain_json_rpc_notifications();
        assert_eq!(lines.len(), 7);
        let values = lines
            .iter()
            .map(|line| serde_json::from_str::<Value>(line).expect("notification JSON"))
            .collect::<Vec<_>>();
        assert_eq!(values[0]["method"], "thread/started");
        assert_eq!(values[1]["method"], "lifecycle/changed");
        assert_eq!(values[1]["params"]["lifecycle"]["state"], "running");
        assert_eq!(values[2]["method"], "turn/started");
        assert_eq!(values[3]["method"], "item/started");
        assert_eq!(values[4]["method"], "item/completed");
        assert_eq!(values[4]["params"]["item"]["text"], "json rpc output");
        assert_eq!(values[5]["method"], "turn/completed");
        assert_eq!(values[5]["params"]["turn"]["status"], "completed");
        assert_eq!(values[6]["method"], "lifecycle/changed");
        assert_eq!(values[6]["params"]["lifecycle"]["state"], "ready");
    }

    #[test]
    fn runtime_model_verification_update_emits_notification() {
        let mut server = initialized_server();
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("verify model".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_json_rpc_notifications();

        server.runtime_turn_updates.model_verification(
            thread.thread_id.clone(),
            turn.turn.id.clone(),
            vec![ModelVerification::TrustedAccessForCyber],
        );
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec!["model/verification"]
        );
        assert_eq!(notifications[0]["params"]["threadId"], thread.thread_id);
        assert_eq!(notifications[0]["params"]["turnId"], turn.turn.id);
        assert_eq!(
            notifications[0]["params"]["verifications"],
            serde_json::json!(["trustedAccessForCyber"])
        );
    }

    #[test]
    fn hook_service_events_drain_to_json_rpc_notifications() {
        let hook_service = hook_service::AppServerHookService::default();
        hook_service.push(AppServerHookNotification::Started(
            HookStartedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                run: sample_hook_run(HookRunStatus::Running),
            },
        ));
        hook_service.push(AppServerHookNotification::Completed(
            HookCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                run: sample_hook_run(HookRunStatus::Completed),
            },
        ));
        let services = app_services::AppServerServices {
            hooks: Arc::new(hook_service),
            ..app_services::AppServerServices::for_tests(
                app_services::TestLogService::ready(),
                app_services::TestJobService::ready(vec![]),
                app_services::TestSkillsService::ready(vec![]),
                app_services::TestMcpService::ready(vec![]),
                app_services::TestFsService::disabled(),
                app_services::TestCommandExecService::disabled(),
            )
        };
        let mut server = AppServer::new().with_app_services(services);

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec!["hook/started", "hook/completed"]
        );
        assert_eq!(notifications[0]["params"]["run"]["status"], "running");
        assert_eq!(notifications[0]["params"]["run"]["eventName"], "preToolUse");
        assert_eq!(notifications[1]["params"]["run"]["status"], "completed");
        assert_eq!(notifications[1]["params"]["run"]["eventName"], "preToolUse");
        assert!(server.drain_json_rpc_notifications().is_empty());
    }

    #[test]
    fn real_hook_registry_run_drains_to_json_rpc_notifications() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::passthrough(
                    "audit",
                    dasclaw_hooks::HookPoint::BeforeToolCall,
                )))
                .await;
            registry
                .run(&dasclaw_hooks::HookEvent::ToolCall {
                    tool_name: "shell.exec".to_string(),
                    parameters: serde_json::json!({"cmd": "pwd"}),
                    user_id: "user-1".to_string(),
                    thread_id: Some("thread-1".to_string()),
                    turn_id: Some("turn-1".to_string()),
                    context: "chat".to_string(),
                })
                .await
                .expect("hook run");
        });
        let mut server = AppServer::new().with_app_services(services);

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec!["hook/started", "hook/completed"]
        );
        assert_eq!(notifications[0]["params"]["threadId"], "thread-1");
        assert_eq!(notifications[0]["params"]["turnId"], "turn-1");
        assert_eq!(notifications[0]["params"]["run"]["eventName"], "preToolUse");
        assert_eq!(
            notifications[0]["params"]["run"]["sourcePath"],
            "dasclaw://hook/audit"
        );
        assert_eq!(notifications[0]["params"]["run"]["status"], "running");
        assert_eq!(notifications[1]["params"]["run"]["status"], "completed");
    }

    #[test]
    fn turn_start_runs_inbound_hook_and_emits_real_hook_notifications() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::passthrough(
                    "inbound-audit",
                    dasclaw_hooks::HookPoint::BeforeInbound,
                )))
                .await;
        });
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = AppServer::with_runtime_bridge(bridge.clone()).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_json_rpc_notifications();

        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                input: text_input("hello hooks"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start after inbound hook");
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(start.turn.id, "turn_1");
        assert_eq!(bridge.calls.lock().expect("calls lock").len(), 1);
        assert_eq!(
            methods_from_values(&notifications),
            vec![
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "hook/started",
                "hook/completed"
            ]
        );
        assert_eq!(
            notifications[3]["params"]["run"]["eventName"],
            "userPromptSubmit"
        );
        assert_eq!(
            notifications[4]["params"]["run"]["eventName"],
            "userPromptSubmit"
        );
        assert_eq!(notifications[4]["params"]["run"]["status"], "completed");
    }

    #[test]
    fn inbound_hook_modification_updates_runtime_prompt() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::modifying(
                    "inbound-transform",
                    dasclaw_hooks::HookPoint::BeforeInbound,
                    "[AUDIT] hello hooks",
                )))
                .await;
        });
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = AppServer::with_runtime_bridge(bridge.clone()).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_json_rpc_notifications();

        server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id,
                input: text_input("hello hooks"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start with modified inbound prompt");

        assert_eq!(
            bridge.calls.lock().expect("calls lock")[0].prompt,
            "[AUDIT] hello hooks"
        );
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(notifications[4]["params"]["run"]["status"], "completed");
    }

    #[test]
    fn inbound_hook_rejection_blocks_turn_start_fail_safe() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::rejecting(
                    "inbound-policy",
                    dasclaw_hooks::HookPoint::BeforeInbound,
                    "blocked inbound",
                )))
                .await;
        });
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = AppServer::with_runtime_bridge(bridge.clone()).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_json_rpc_notifications();

        let error = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                input: text_input("blocked prompt"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect_err("inbound rejection should block runtime start");
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        let turns = server
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: thread.thread.id,
            })
            .expect("turn/list should work after inbound rejection");

        assert!(error.public_message().contains("blocked inbound"));
        assert!(bridge.calls.lock().expect("calls lock").is_empty());
        assert_eq!(turns.turns.len(), 1);
        assert_eq!(turns.turns[0].status, TurnStatus::Cancelled);
        assert_eq!(
            methods_from_values(&notifications),
            vec!["hook/started", "hook/completed"]
        );
        assert_eq!(
            notifications[0]["params"]["run"]["eventName"],
            "userPromptSubmit"
        );
        assert_eq!(
            notifications[1]["params"]["run"]["eventName"],
            "userPromptSubmit"
        );
        assert_eq!(notifications[1]["params"]["run"]["status"], "blocked");
        assert_eq!(
            notifications[1]["params"]["run"]["statusMessage"],
            "blocked inbound"
        );
    }

    #[test]
    fn completed_turn_runs_outbound_hook_before_turn_completed() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::passthrough(
                    "outbound-audit",
                    dasclaw_hooks::HookPoint::BeforeOutbound,
                )))
                .await;
        });
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "runtime answer".to_string(),
            },
        ));
        let mut server = AppServer::with_runtime_bridge(bridge).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_json_rpc_notifications();

        server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id,
                input: text_input("complete with outbound hook"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec![
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "hook/started",
                "hook/completed",
                "item/completed",
                "turn/completed",
                "lifecycle/changed"
            ]
        );
        assert_eq!(
            notifications[3]["params"]["run"]["eventName"],
            "postToolUse"
        );
        assert_eq!(
            notifications[4]["params"]["run"]["eventName"],
            "postToolUse"
        );
        assert_eq!(notifications[4]["params"]["run"]["status"], "completed");
        assert_eq!(notifications[6]["params"]["turn"]["status"], "completed");
    }

    #[test]
    fn outbound_hook_modification_updates_completed_turn_output() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::modifying(
                    "outbound-transform",
                    dasclaw_hooks::HookPoint::BeforeOutbound,
                    "sanitized answer",
                )))
                .await;
        });
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "runtime answer".to_string(),
            },
        ));
        let mut server = AppServer::with_runtime_bridge(bridge).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_json_rpc_notifications();

        server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                input: text_input("complete with outbound transform"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec![
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "hook/started",
                "hook/completed",
                "item/completed",
                "turn/completed",
                "lifecycle/changed"
            ]
        );
        assert_eq!(
            notifications[5]["params"]["item"]["text"],
            "sanitized answer"
        );
        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread.id,
                turn_id: "turn_1".to_string(),
            })
            .expect("turn/read should expose modified outbound output");
        assert_eq!(turn_text(&read.turn), Some("sanitized answer".to_string()));
    }

    #[test]
    fn outbound_hook_rejection_fails_turn_without_successful_completion() {
        let runtime = test_tokio_runtime();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let registry = services
            .hook_registry
            .clone()
            .expect("real services should expose hook registry");
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::rejecting(
                    "outbound-policy",
                    dasclaw_hooks::HookPoint::BeforeOutbound,
                    "blocked outbound",
                )))
                .await;
        });
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "runtime answer".to_string(),
            },
        ));
        let mut server = AppServer::with_runtime_bridge(bridge).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let _ = server.drain_json_rpc_notifications();

        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id.clone(),
                input: text_input("complete with outbound rejection"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec![
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "hook/started",
                "hook/completed",
                "item/completed",
                "turn/completed",
                "error",
                "lifecycle/changed"
            ]
        );
        assert_eq!(
            notifications[3]["params"]["run"]["eventName"],
            "postToolUse"
        );
        assert_eq!(
            notifications[4]["params"]["run"]["eventName"],
            "postToolUse"
        );
        assert_eq!(notifications[4]["params"]["run"]["status"], "blocked");
        assert_eq!(
            notifications[4]["params"]["run"]["statusMessage"],
            "blocked outbound"
        );
        assert_eq!(notifications[6]["params"]["turn"]["status"], "failed");
        assert!(
            !notifications.iter().any(|notification| {
                notification["method"] == "turn/completed"
                    && notification["params"]["turn"]["status"] == "completed"
            }),
            "outbound rejection must not emit a successful turn/completed"
        );
        assert!(
            !notifications.iter().any(|notification| {
                notification["method"] == "item/completed"
                    && notification["params"]["item"]["text"] == "runtime answer"
            }),
            "rejected outbound output must not be emitted as a completed item"
        );

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread.id.clone(),
                turn_id: start.turn.id.clone(),
            })
            .expect("turn/read should expose failed terminal turn");
        assert_eq!(read.turn.status, CodexTurnStatus::Failed);
        let turns = server
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: thread.thread.id,
            })
            .expect("turn/list should expose failed terminal turn");
        assert_eq!(turns.turns.len(), 1);
        assert_eq!(turns.turns[0].status, TurnStatus::Failed);
    }

    #[test]
    fn runtime_client_tool_executor_blocks_rejected_hook_before_fallback() {
        let runtime = test_tokio_runtime();
        let hook_service = Arc::new(hook_service::AppServerHookService::wired());
        let observer: Arc<dyn dasclaw_hooks::HookRunObserver> = hook_service.clone();
        let registry = Arc::new(dasclaw_hooks::HookRegistry::new().with_observer(observer));
        runtime.block_on(async {
            registry
                .register(Arc::new(TestHook::rejecting(
                    "policy",
                    dasclaw_hooks::HookPoint::BeforeToolCall,
                    "blocked by policy",
                )))
                .await;
        });
        let fallback = Arc::new(CountingExecutor::new());
        let executor = RuntimeClientToolExecutor::new(
            RuntimeClientRequestContext {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                cwd: PathBuf::from("."),
                updates: RuntimeTurnUpdateSink::new(),
                pending_requests: Arc::new(Mutex::new(HashMap::new())),
                hook_registry: Some(registry),
            },
            Some(fallback.clone()),
        );
        let call = ToolCall {
            id: "call-1".to_string(),
            name: "shell.exec".to_string(),
            arguments: serde_json::json!({"cmd": "pwd"}),
            reasoning: None,
        };

        let result = runtime.block_on(async {
            dasclaw_runtime::ToolExecutor::execute(&executor, &call)
                .await
                .expect("tool executor should return hook rejection as tool result")
        });
        let notifications =
            app_services::HookNotificationService::drain_hook_notifications(&*hook_service);

        assert!(result.is_error);
        assert_eq!(result.content, "blocked by policy");
        assert_eq!(fallback.call_count_blocking(), 0);
        assert_eq!(notifications.len(), 2);
        let AppServerHookNotification::Completed(completed) = &notifications[1] else {
            panic!("second hook notification should be completed");
        };
        assert_eq!(completed.thread_id, "thread-1");
        assert_eq!(completed.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(completed.run.status, HookRunStatus::Blocked);
        assert_eq!(
            completed.run.status_message.as_deref(),
            Some("blocked by policy")
        );
        assert_eq!(completed.run.entries[0].kind, HookOutputEntryKind::Stop);
    }

    #[test]
    fn warning_service_events_drain_to_json_rpc_notifications() {
        let hook_service = hook_service::AppServerHookService::default();
        hook_service.push(AppServerHookNotification::Warning(WarningNotification {
            thread_id: Some("thread-1".to_string()),
            message: "general warning".to_string(),
        }));
        hook_service.push(AppServerHookNotification::ConfigWarning(
            ConfigWarningNotification {
                summary: "unsupported config key".to_string(),
                details: Some("experimental.foo is ignored".to_string()),
                path: Some("/tmp/config.json".to_string()),
                range: None,
            },
        ));
        let services = app_services::AppServerServices {
            hooks: Arc::new(hook_service),
            ..app_services::AppServerServices::for_tests(
                app_services::TestLogService::ready(),
                app_services::TestJobService::ready(vec![]),
                app_services::TestSkillsService::ready(vec![]),
                app_services::TestMcpService::ready(vec![]),
                app_services::TestFsService::disabled(),
                app_services::TestCommandExecService::disabled(),
            )
        };
        let mut server = AppServer::new().with_app_services(services);

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(
            methods_from_values(&notifications),
            vec!["warning", "configWarning"]
        );
        assert_eq!(notifications[0]["params"]["message"], "general warning");
        assert_eq!(
            notifications[1]["params"]["summary"],
            "unsupported config key"
        );
        assert_eq!(
            notifications[1]["params"]["details"],
            "experimental.foo is ignored"
        );
        assert_eq!(notifications[1]["params"]["path"], "/tmp/config.json");
        assert!(server.drain_json_rpc_notifications().is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn turn_start_emits_generic_warning_from_actual_sandbox_policy() {
        let _path = EnvVarGuard::set("PATH", tempfile::tempdir().expect("tempdir").path());
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;

        server
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: text_input("warn about sandbox".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: Some(serde_json::json!({ "type": "read-only" })),
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        let warning = notifications
            .iter()
            .find(|value| value["method"] == event::WARNING)
            .expect("generic warning from sandbox policy");
        assert_eq!(warning["params"]["threadId"], thread_id);
        assert!(
            warning["params"]["message"]
                .as_str()
                .expect("warning message")
                .contains("bubblewrap")
        );
        assert_eq!(
            bridge.calls.lock().expect("calls lock")[0]
                .sandbox_context
                .policy,
            Some(dasclaw_workspace_cap::policy::SandboxPolicy::new_read_only_policy())
        );
    }

    #[test]
    fn turn_start_external_sandbox_policy_does_not_emit_generic_warning() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge);
        let thread_id = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created")
            .thread_id;

        server
            .turn_start(TurnStartParams {
                thread_id,
                input: text_input("external sandbox".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: Some(serde_json::json!({
                    "type": "external-sandbox",
                    "network_access": "restricted"
                })),
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        let methods = methods_from_values(&notifications);
        assert!(!methods.contains(&event::WARNING));
    }

    #[test]
    fn turn_start_warning_conversion_error_does_not_leave_pending_turn() {
        let mut server =
            initialized_server_with_bridge(Arc::new(RecordingRuntimeBridge::default()));
        server
            .threads
            .push_thread_for_test(crate::thread_lifecycle::ThreadRecord {
                thread_id: "thread_bad_sandbox".to_string(),
                title: None,
                workspace_root: None,
                sandbox_context: RuntimeSandboxContext {
                    sandbox: None,
                    policy: Some(
                        dasclaw_workspace_cap::policy::SandboxPolicy::WorkspaceWrite {
                            writable_roots: vec![PathBuf::from("relative/root")],
                            network_access: false,
                            exclude_tmpdir_env_var: false,
                            exclude_slash_tmp: false,
                        },
                    ),
                },
                sandbox: None,
                permission_profile: None,
                forked_from_id: None,
                ephemeral: true,
                archived: false,
                subscribed: false,
                path: None,
                created_at: 1,
                updated_at: 1,
                git_info: None,
                goal: None,
                compacted_turn_id: None,
                token_usage: None,
            });

        let error = server
            .turn_start(TurnStartParams {
                thread_id: "thread_bad_sandbox".to_string(),
                input: text_input("bad sandbox"),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect_err("bad sandbox warning conversion should fail before recording turn");
        let turns = server
            .list_turns_for_test(TestTurnsSnapshotParams {
                thread_id: "thread_bad_sandbox".to_string(),
            })
            .expect("turn/list should remain usable");

        assert!(
            error
                .public_message()
                .contains("non-absolute writable root")
        );
        assert!(turns.turns.is_empty());
    }

    #[test]
    fn manually_queued_warning_delivery_does_not_flip_producer_readiness() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let hook_service = Arc::new(hook_service::AppServerHookService::wired());
        let mut services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        services.hooks = hook_service.clone();
        let mut server =
            AppServer::with_runtime_bridge(Arc::new(NoopRuntimeBridge)).with_app_services(services);
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
        let _ = server.drain_json_rpc_notifications();

        hook_service.push(AppServerHookNotification::Warning(WarningNotification {
            thread_id: Some("thread-1".to_string()),
            message: "general warning".to_string(),
        }));
        hook_service.push(AppServerHookNotification::GuardianWarning(
            GuardianWarningNotification {
                thread_id: "thread-1".to_string(),
                message: "guardian warning".to_string(),
            },
        ));

        let capabilities = server.capabilities().capabilities;
        assert_eq!(capabilities.warnings.events, expected_warning_events(false));

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert_eq!(
            methods_from_values(&notifications),
            vec!["warning", "guardianWarning"]
        );
    }

    #[test]
    fn initialize_emits_config_warning_and_deprecation_for_startup_config() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        write_app_server_config(
            tempdir.path(),
            serde_json::json!({
                "model": "gpt-test",
                "unknown_key": true,
                "experimental_instructions_file": "legacy.md"
            }),
        );
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let mut server =
            AppServer::with_runtime_bridge(Arc::new(NoopRuntimeBridge)).with_app_services(services);

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
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        let methods = methods_from_values(&notifications);
        assert!(methods.contains(&"configWarning"));
        assert!(methods.contains(&"deprecationNotice"));
        let config_warning = notifications
            .iter()
            .find(|notification| notification["method"] == "configWarning")
            .expect("config warning notification");
        assert_eq!(
            config_warning["params"]["summary"],
            "unsupported config key"
        );
        assert!(
            config_warning["params"]["details"]
                .as_str()
                .expect("details")
                .contains("unknown_key")
        );
        let deprecation = notifications
            .iter()
            .find(|notification| notification["method"] == "deprecationNotice")
            .expect("deprecation notice notification");
        assert_eq!(
            deprecation["params"]["summary"],
            "experimental_instructions_file"
        );
    }

    #[test]
    fn config_service_collects_warning_for_unknown_config_key() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        write_app_server_config(tempdir.path(), serde_json::json!({"unknown_key": true}));
        let service =
            crate::config_service::AppServerConfigService::new(tempdir.path().to_path_buf());

        app_services::ConfigService::read(
            &service,
            ConfigReadParams {
                include_layers: false,
                cwd: None,
            },
        )
        .expect("config read should succeed");
        let warnings = service.drain_config_warnings();

        assert_eq!(warnings.len(), 1);
        let AppServerHookNotification::ConfigWarning(warning) = &warnings[0] else {
            panic!("expected config warning");
        };
        assert_eq!(warning.summary, "unsupported config key");
        assert!(
            warning
                .details
                .as_deref()
                .expect("details")
                .contains("unknown_key")
        );
    }

    #[test]
    fn capabilities_advertise_ready_r6_warning_producers_only() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut server = initialized_server_with_root(tempdir.path());

        let capabilities = server.capabilities().capabilities;

        assert_eq!(capabilities.hooks.status, CapabilityStatus::Implemented);
        assert_eq!(capabilities.warnings.status, CapabilityStatus::Implemented);
        assert_eq!(capabilities.warnings.events, expected_warning_events(false));
        assert!(
            !capabilities
                .warnings
                .events
                .contains(&event::GUARDIAN_WARNING.to_string())
        );
        assert!(
            !capabilities
                .model_provider
                .events
                .contains(&event::MODEL_REROUTED.to_string())
        );
        assert!(
            !capabilities
                .model_provider
                .events
                .contains(&event::MODEL_VERIFICATION.to_string())
        );
    }

    #[test]
    fn provider_runtime_bridge_does_not_advertise_model_events_without_trusted_reasons() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(tempdir.path().to_path_buf());
        let mut server = AppServer::with_runtime_bridge(Arc::new(
            DasclawAgentRuntimeBridge::from_model_provider_snapshot(),
        ))
        .with_app_services(services);
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

        let capabilities = server.capabilities().capabilities;

        assert!(
            !capabilities
                .model_provider
                .events
                .contains(&event::MODEL_REROUTED.to_string())
        );
        assert!(
            !capabilities
                .model_provider
                .events
                .contains(&event::MODEL_VERIFICATION.to_string())
        );
    }

    #[test]
    fn real_r6_model_delivery_features_do_not_advertise_producer_readiness() {
        let bridge = Arc::new(ModelDeliveryRuntimeBridge);
        let mut server = initialized_server_with_bridge(bridge);
        let capabilities = server.capabilities().capabilities;

        assert!(capabilities.model_provider.events.is_empty());
    }

    #[test]
    fn runtime_model_reroute_update_emits_notification() {
        let mut server = initialized_server();
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("reroute model".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_json_rpc_notifications();

        server.runtime_turn_updates.model_rerouted(
            thread.thread_id.clone(),
            turn.turn.id.clone(),
            "gpt-test".to_string(),
            "gpt-next".to_string(),
            ModelRerouteReason::HighRiskCyberActivity,
        );
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());

        assert_eq!(methods_from_values(&notifications), vec!["model/rerouted"]);
        assert_eq!(notifications[0]["params"]["threadId"], thread.thread_id);
        assert_eq!(notifications[0]["params"]["turnId"], turn.turn.id);
        assert_eq!(notifications[0]["params"]["fromModel"], "gpt-test");
        assert_eq!(notifications[0]["params"]["toModel"], "gpt-next");
        assert_eq!(
            notifications[0]["params"]["reason"],
            "highRiskCyberActivity"
        );
    }

    #[test]
    fn runtime_model_update_ignores_stale_turn() {
        let mut server = initialized_server();
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("complete before stale update".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_json_rpc_notifications();
        server.runtime_turn_updates.complete(
            thread.thread_id.clone(),
            turn.turn.id.clone(),
            "done".to_string(),
        );
        let _ = server.drain_json_rpc_notifications();

        server.runtime_turn_updates.model_rerouted(
            thread.thread_id,
            turn.turn.id,
            "gpt-test".to_string(),
            "gpt-next".to_string(),
            ModelRerouteReason::HighRiskCyberActivity,
        );

        assert!(server.drain_json_rpc_notifications().is_empty());
    }

    #[test]
    fn interrupt_turn_invokes_runtime_bridge_before_recording_interrupted_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::default());
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let cancelled = server
            .interrupt_turn_for_test(TestTurnInterruptParams {
                thread_id: thread.thread_id.clone(),
                turn_id: start.turn.id.clone(),
            })
            .expect("runtime cancellation should call runtime bridge first");

        assert_eq!(cancelled.status, TurnStatus::Cancelled);
        let cancel_calls = bridge.cancel_calls.lock().expect("cancel calls lock");
        assert_eq!(cancel_calls.len(), 1);
        assert_eq!(cancel_calls[0].thread_id, thread.thread_id);
        assert_eq!(cancel_calls[0].turn_id, start.turn.id);
    }

    #[test]
    fn interrupt_turn_runtime_bridge_error_does_not_record_interrupted_turn() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_cancel_result(Err(
            RuntimeBridgeError::retryable("cancel unavailable"),
        )));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let error = server
            .interrupt_turn_for_test(TestTurnInterruptParams {
                thread_id: thread.thread_id.clone(),
                turn_id: start.turn.id.clone(),
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
                turn_id: start.turn.id,
            })
            .expect("turn/read should still find the pending turn");
        assert_eq!(
            read.turn.status,
            codex_status_from_turn_status(TurnStatus::Pending)
        );
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
                    cwd: PathBuf::from("."),
                    model_provider: test_runtime_model_snapshot(),
                    reasoning_summary: ReasoningSummary::None,
                    sandbox_context: RuntimeSandboxContext::empty(),
                    updates: RuntimeTurnUpdateSink::new(),
                    hook_registry: None,
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
    fn dasclaw_runtime_bridge_rejects_unenforced_sandbox_context() {
        let factory_called = Arc::new(Mutex::new(false));
        let factory_called_clone = Arc::clone(&factory_called);
        let bridge = DasclawAgentRuntimeBridge::new(move |_token| {
            *factory_called_clone.lock().expect("factory called lock") = true;
            Err(RuntimeBridgeError::fatal("factory should not be called"))
        });

        let error = bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "hello".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext {
                    sandbox: Some(SandboxMode::WorkspaceWrite),
                    policy: Some(
                        dasclaw_workspace_cap::policy::SandboxPolicy::new_workspace_write_policy(),
                    ),
                },
                updates: RuntimeTurnUpdateSink::new(),
                hook_registry: None,
            })
            .expect_err("real runtime bridge must reject unenforced sandbox context");

        assert_eq!(
            error.message,
            "runtime bridge does not support sandbox context enforcement"
        );
        assert!(!*factory_called.lock().expect("factory called lock"));
    }

    #[test]
    fn dasclaw_runtime_bridge_passes_model_snapshot_to_agent_factory() {
        let captured_snapshots = Arc::new(Mutex::new(Vec::new()));
        let factory_snapshots = Arc::clone(&captured_snapshots);
        let captured_reasoning = Arc::new(Mutex::new(Vec::new()));
        let factory_reasoning = Arc::clone(&captured_reasoning);
        let bridge = DasclawAgentRuntimeBridge::new_with_model_provider(
            move |_token, snapshot, reasoning_summary| {
                factory_snapshots
                    .lock()
                    .expect("factory snapshots lock")
                    .push(snapshot);
                factory_reasoning
                    .lock()
                    .expect("factory reasoning lock")
                    .push(reasoning_summary);
                Err(RuntimeBridgeError::fatal("test factory stops before run"))
            },
        );

        let error = bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "hello".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::Concise,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: RuntimeTurnUpdateSink::new(),
                hook_registry: None,
            })
            .expect_err("test factory should stop before spawning");

        assert_eq!(error.message, "test factory stops before run");
        let snapshots = captured_snapshots.lock().expect("captured snapshots lock");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].model_id, "gpt-test");
        assert_eq!(snapshots[0].api_key, "test-api-key");
        let reasoning = captured_reasoning.lock().expect("captured reasoning lock");
        assert_eq!(reasoning.as_slice(), &[ReasoningSummary::Concise]);
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
                cwd: PathBuf::from("."),
                model_provider: snapshot,
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let updates = wait_for_runtime_updates(&updates);
        assert_eq!(
            updates.len(),
            3,
            "expected delta + usage + completion: {updates:?}"
        );
        assert!(matches!(
            &updates[0].outcome,
            RuntimeTurnOutcome::Delta { delta } if delta == "invoke bridge path"
        ));
        assert!(matches!(
            &updates[1].outcome,
            RuntimeTurnOutcome::TokenUsageUpdated { usage } if *usage == TokenUsage::default()
        ));
        assert!(matches!(
            &updates[2].outcome,
            RuntimeTurnOutcome::Completed { output } if output == "invoke bridge path"
        ));
    }

    #[test]
    fn dasclaw_runtime_bridge_forwards_nonzero_completed_usage_before_completion() {
        let bridge =
            DasclawAgentRuntimeBridge::from_responder(Arc::new(ScriptedResponder::new(vec![
                text_output_with_usage(
                    "usage bridge path",
                    TokenUsage {
                        input_tokens: 8,
                        output_tokens: 5,
                        cache_creation_input_tokens: 2,
                        cache_read_input_tokens: 3,
                    },
                ),
            ])));
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "hello".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let updates = wait_for_runtime_updates(&updates);
        assert!(matches!(
            &updates[0].outcome,
            RuntimeTurnOutcome::Delta { delta } if delta == "usage bridge path"
        ));
        assert!(matches!(
            &updates[1].outcome,
            RuntimeTurnOutcome::TokenUsageUpdated { usage }
                if usage.input_tokens == 8
                    && usage.output_tokens == 5
                    && usage.cache_creation_input_tokens == 2
                    && usage.cache_read_input_tokens == 3
        ));
        assert!(matches!(
            &updates[2].outcome,
            RuntimeTurnOutcome::Completed { output } if output == "usage bridge path"
        ));
    }

    #[test]
    fn runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata() {
        let bridge =
            DasclawAgentRuntimeBridge::from_responder(Arc::new(ScriptedResponder::new(vec![
                text_output_with_metadata(
                    "actual model differs",
                    ResponseMetadata {
                        actual_model: Some("gpt-next".to_string()),
                        ..ResponseMetadata::default()
                    },
                ),
            ])));
        let mut server = initialized_server_with_bridge(Arc::new(bridge));
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            notifications
                .iter()
                .all(|notification| notification["method"] != "model/rerouted"),
            "actual_model alone should not emit high-risk model/rerouted: {notifications:?}"
        );
    }

    #[test]
    fn runtime_completed_event_emits_model_verification_from_response_metadata() {
        let bridge =
            DasclawAgentRuntimeBridge::from_responder(Arc::new(ScriptedResponder::new(vec![
                text_output_with_metadata(
                    "verified",
                    ResponseMetadata {
                        actual_model: Some("gpt-test".to_string()),
                        model_verifications: vec![ResponseModelVerification::TrustedAccessForCyber],
                        ..ResponseMetadata::default()
                    },
                ),
            ])));
        let mut server = initialized_server_with_bridge(Arc::new(bridge));
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let turn = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        let verification = notifications
            .iter()
            .find(|notification| notification["method"] == "model/verification")
            .expect("model/verification notification");

        assert_eq!(verification["params"]["threadId"], thread.thread_id);
        assert_eq!(verification["params"]["turnId"], turn.turn.id);
        assert_eq!(
            verification["params"]["verifications"],
            serde_json::json!(["trustedAccessForCyber"])
        );
    }

    #[test]
    fn runtime_same_model_metadata_does_not_emit_reroute() {
        let bridge =
            DasclawAgentRuntimeBridge::from_responder(Arc::new(ScriptedResponder::new(vec![
                text_output_with_metadata(
                    "same model",
                    ResponseMetadata {
                        actual_model: Some("gpt-test".to_string()),
                        ..ResponseMetadata::default()
                    },
                ),
            ])));
        let mut server = initialized_server_with_bridge(Arc::new(bridge));
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));

        assert!(
            notifications
                .iter()
                .all(|notification| notification["method"] != "model/rerouted"),
            "same-model metadata should not emit model/rerouted: {notifications:?}"
        );
    }

    #[test]
    fn agent_runtime_bridge_maps_approval_needed_and_tool_result_to_runtime_updates() {
        let executor = Arc::new(CountingExecutor::new());
        let factory_executor = Arc::clone(&executor);
        let bridge = DasclawAgentRuntimeBridge::new_with_model_provider_and_features(
            move |token, _snapshot, _reasoning_summary| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        tool_call_output("bash", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(
                        Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>
                    )
                    .tools(vec![dummy_tool("bash")])
                    .approval_policy(Arc::new(AlwaysApprovePolicy))
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                ..RuntimeBridgeFeatures::default()
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "run tool".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_approval = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(update.outcome, RuntimeTurnOutcome::ApprovalRequested { .. })
            })
        });
        let approval_id = before_approval
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::ApprovalRequested { request } => {
                    assert_eq!(request.tool_name, "bash");
                    assert_eq!(request.tool_call_id, "call_1");
                    Some(request.request_id.clone())
                }
                _ => None,
            })
            .expect("approval request update should be emitted");
        let output_delta = before_approval
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::CommandOutputDelta { update } => Some(update),
                _ => None,
            })
            .expect("command output delta should be emitted before approval");
        assert_eq!(output_delta.item_id, "turn_1:tool:call_1");
        assert_eq!(output_delta.delta, "tool call started: bash");
        assert!(!output_delta.delta.contains("secret-token"));
        assert!(!output_delta.delta.contains("/tmp/x"));
        assert!(!output_delta.delta.contains("tool_call_id"));

        bridge
            .resolve_approval(RuntimeApprovalDecision {
                request_id: approval_id,
                decision: dasclaw_runtime::ApprovalDecision::Approve,
            })
            .expect("approval should resume the runtime turn");

        let after_approval = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::Completed { .. }))
        });
        let tool_result = after_approval
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::ToolResult { update } => Some(update),
                _ => None,
            })
            .expect("tool result update should be emitted after approval");
        assert_eq!(tool_result.item_id, "turn_1:tool:call_1");
        assert!(tool_result.content.contains("ran bash"));
        assert!(!tool_result.is_error);
        assert!(
            after_approval.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::Completed { output } if output == "done"
                )
            }),
            "runtime should complete after approved tool execution: {after_approval:?}"
        );
        assert_eq!(executor.call_count_blocking(), 1);
    }

    #[test]
    fn agent_runtime_bridge_does_not_emit_dynamic_request_from_tool_start() {
        let executor = Arc::new(CountingExecutor::new());
        let factory_executor = Arc::clone(&executor);
        let bridge = DasclawAgentRuntimeBridge::new_with_model_provider_and_features(
            move |token, _snapshot, _reasoning_summary| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(
                        Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>
                    )
                    .tools(vec![dummy_tool("open_url")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                dynamic_tool_call: true,
                ..RuntimeBridgeFeatures::default()
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "open the docs".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let drained = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::ToolResult { .. }))
        });
        assert!(
            !drained.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::DynamicToolCallRequested { .. }
                )
            }),
            "tool start must not emit a duplicate dynamic tool request: {drained:?}"
        );
        assert!(
            drained.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::CommandOutputDelta { update }
                        if update.delta == "tool call started: open_url"
                )
            }),
            "plain tool start should remain an output delta: {drained:?}"
        );
        assert!(
            drained.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::ToolResult { update }
                        if update.item_id == "turn_1:tool:call_1"
                            && update.content == "ran open_url"
                            && !update.is_error
                )
            }),
            "tool result should still be emitted: {drained:?}"
        );
        assert_eq!(executor.call_count_blocking(), 1);
    }

    #[test]
    fn stale_runtime_request_response_is_rejected() {
        let bridge = DasclawAgentRuntimeBridge::new(|_| {
            Err(RuntimeBridgeError::fatal("factory should not be used"))
        });

        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: "call_1".to_string(),
                payload: RuntimeServerRequestResponse::DynamicTool(DynamicToolCallResponse {
                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                        text: "ok".to_string(),
                    }],
                    success: true,
                }),
            })
            .expect_err("stale dynamic tool response should be rejected");
    }

    #[test]
    fn agent_runtime_bridge_maps_non_command_approval_to_permissions_request() {
        let executor = Arc::new(CountingExecutor::new());
        let factory_executor = Arc::clone(&executor);
        let bridge = DasclawAgentRuntimeBridge::new_with_model_provider_and_features(
            move |token, _snapshot, _reasoning_summary| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(
                        Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>
                    )
                    .tools(vec![dummy_tool("open_url")])
                    .approval_policy(Arc::new(AlwaysApprovePolicy))
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                dynamic_tool_call: true,
                permissions_approval: true,
                ..RuntimeBridgeFeatures::default()
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "open the docs".to_string(),
                cwd: PathBuf::from("/turn/local"),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_approval = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::PermissionsApprovalRequested { .. }
                )
            })
        });
        let permissions_request = before_approval
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::PermissionsApprovalRequested { request } => {
                    Some(request.clone())
                }
                _ => None,
            })
            .expect("permissions request should be emitted");
        assert_eq!(permissions_request.item_id, "turn_1:tool:call_1");
        assert_eq!(permissions_request.cwd, "/turn/local");
        assert_eq!(
            permissions_request.reason.as_deref(),
            Some("approve call to open_url")
        );
        assert_eq!(
            permissions_request.permissions["url"],
            "https://example.test"
        );

        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: permissions_request.request_id,
                payload: RuntimeServerRequestResponse::Permissions(
                    PermissionsRequestApprovalResponse {
                        decision: PermissionsApprovalDecision::Approve,
                        permissions: serde_json::json!({"url": "https://example.test"}),
                        scope: dasclaw_app_server_protocol::PermissionGrantScope::Turn,
                        strict_auto_review: None,
                    },
                ),
            })
            .expect("permissions response should approve and resume the runtime turn");

        let after_approval = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::Completed { .. }))
        });

        assert!(
            !before_approval.iter().any(|update| {
                matches!(update.outcome, RuntimeTurnOutcome::ApprovalRequested { .. })
            }),
            "non-command approval should not use command approval surface: {before_approval:?}"
        );
        assert!(
            after_approval.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::ToolResult { update }
                        if update.item_id == "turn_1:tool:call_1"
                            && update.content == "ran open_url"
                            && !update.is_error
                )
            }),
            "permissions approval should allow tool result: {after_approval:?}"
        );
        assert!(
            after_approval.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::Completed { output } if output == "done"
                )
            }),
            "permissions approval should let the turn complete: {after_approval:?}"
        );
        assert_eq!(executor.call_count_blocking(), 1);
    }

    #[test]
    fn non_command_permissions_denial_rejects_legacy_approval_waiter() {
        let executor = Arc::new(CountingExecutor::new());
        let factory_executor = Arc::clone(&executor);
        let bridge = DasclawAgentRuntimeBridge::new_with_model_provider_and_features(
            move |token, _snapshot, _reasoning_summary| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(
                        Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>
                    )
                    .tools(vec![dummy_tool("open_url")])
                    .approval_policy(Arc::new(AlwaysApprovePolicy))
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                dynamic_tool_call: true,
                permissions_approval: true,
                ..RuntimeBridgeFeatures::default()
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "open the docs".to_string(),
                cwd: PathBuf::from("/turn/local"),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_denial = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::PermissionsApprovalRequested { .. }
                )
            })
        });
        let permissions_request = before_denial
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::PermissionsApprovalRequested { request } => {
                    Some(request.clone())
                }
                _ => None,
            })
            .expect("permissions request should be emitted");

        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: permissions_request.request_id,
                payload: RuntimeServerRequestResponse::Permissions(denied_permissions_response()),
            })
            .expect("permissions denial should be delivered as rejection");

        let after_denial = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::Failed { .. } | RuntimeTurnOutcome::ToolResult { .. }
                )
            })
        });
        assert_eq!(
            executor.call_count_blocking(),
            0,
            "permissions denial must not execute the gated tool"
        );
        assert!(
            after_denial.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::Failed { error } if error.contains("denied")
                        || error.contains("reject")
                )
            }) || !after_denial
                .iter()
                .any(|update| { matches!(&update.outcome, RuntimeTurnOutcome::Completed { .. }) }),
            "permissions denial must not be converted to approval: {after_denial:?}"
        );
        bridge.shutdown();
    }

    #[test]
    fn dynamic_tool_request_waits_for_renderer_response_and_cleans_pending() {
        let bridge = DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("open_url")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "open docs".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_response = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::DynamicToolCallRequested { .. }
                )
            })
        });
        let request = before_response
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::DynamicToolCallRequested { request } => Some(request.clone()),
                _ => None,
            })
            .expect("dynamic tool request should be emitted");
        assert_eq!(request.request_id, "call_1");
        assert_eq!(request.tool, "open_url");
        assert!(
            !before_response
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::ToolResult { .. })),
            "tool result must wait for renderer response: {before_response:?}"
        );

        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: request.request_id,
                payload: RuntimeServerRequestResponse::DynamicTool(DynamicToolCallResponse {
                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                        text: "opened".to_string(),
                    }],
                    success: true,
                }),
            })
            .expect("dynamic tool response should resume tool execution");

        let after_response = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::Completed { .. }))
        });
        assert!(
            after_response.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::ToolResult { update }
                        if update.item_id == "turn_1:tool:call_1"
                            && update.content.contains("opened")
                            && !update.is_error
                )
            }),
            "renderer response should become tool result: {after_response:?}"
        );
        assert!(
            after_response.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::Completed { output } if output == "done"
                )
            }),
            "turn should complete after renderer response: {after_response:?}"
        );
        bridge.shutdown();
    }

    #[test]
    fn dynamic_tool_json_rpc_response_resumes_runtime_waiter_once() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("open_url")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"open docs","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        assert_eq!(
            json_rpc_value(start)["result"]["turn"]["status"],
            "inProgress"
        );

        let before_response = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_TOOL_CALL,
            Duration::from_secs(2),
        );
        let tool_requests = before_response
            .iter()
            .filter(|value| value["method"] == server_request::ITEM_TOOL_CALL)
            .collect::<Vec<_>>();
        assert_eq!(
            tool_requests.len(),
            1,
            "one runtime-owned dynamic tool request should be emitted: {before_response:?}"
        );
        let request_id = tool_requests[0]["id"]
            .as_str()
            .expect("server request id should be a string")
            .to_string();
        assert_eq!(request_id, "call_1");
        assert_eq!(tool_requests[0]["params"]["tool"], "open_url");
        assert_eq!(
            tool_requests[0]["params"]["arguments"]["url"],
            "https://example.test"
        );

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"contentItems":[{{"type":"inputText","text":"opened"}}],"success":true}}}}"#
        ));
        assert!(response.is_none());

        let after_response =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            after_response.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "approved"
            }),
            "renderer response should resolve the runtime-owned server request: {after_response:?}"
        );
        assert!(
            after_response.iter().any(|value| {
                value["method"] == event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION
                    && value["params"]["itemId"] == "turn_1:tool:call_1"
                    && value["params"]["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("opened"))
                    && value["params"]["isError"] == false
            }),
            "runtime waiter result should be emitted as tool output: {after_response:?}"
        );

        let duplicate = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"contentItems":[{{"type":"inputText","text":"duplicate"}}],"success":true}}}}"#
        ));
        assert!(duplicate.is_none());
        let duplicate_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(
            duplicate_notifications.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "failed"
            }),
            "duplicate response should be rejected after the waiter is consumed: {duplicate_notifications:?}"
        );
    }

    #[test]
    fn shutdown_cancels_pending_dynamic_tool_json_rpc_request() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("open_url")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"open docs","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let before_shutdown = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_TOOL_CALL,
            Duration::from_secs(2),
        );
        let request_id = before_shutdown
            .iter()
            .find(|value| value["method"] == server_request::ITEM_TOOL_CALL)
            .and_then(|value| value["id"].as_str())
            .expect("dynamic tool request should be emitted")
            .to_string();

        let shutdown = server.shutdown(ShutdownParams {
            reason: Some(ShutdownReason::ClientExit),
            timeout_ms: None,
        });
        assert_eq!(shutdown.lifecycle.state, LifecycleState::Stopped);

        let shutdown_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(
            shutdown_notifications.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "failed"
                    && value["params"]["reason"] == "server shutdown"
            }),
            "shutdown should fail the pending runtime-owned request: {shutdown_notifications:?}"
        );

        let late = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"contentItems":[{{"type":"inputText","text":"late"}}],"success":true}}}}"#
        ));
        assert!(late.is_none());
        let late_notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(
            late_notifications.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "failed"
            }),
            "late response should be unknown after shutdown cleanup: {late_notifications:?}"
        );
    }

    #[test]
    fn malformed_dynamic_tool_json_rpc_response_unblocks_runtime_waiter_fail_safe() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("open_url")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"open docs","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let before_response = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_TOOL_CALL,
            Duration::from_secs(2),
        );
        let request_id = before_response
            .iter()
            .find(|value| value["method"] == server_request::ITEM_TOOL_CALL)
            .and_then(|value| value["id"].as_str())
            .expect("dynamic tool request should be emitted")
            .to_string();

        let malformed = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"success":true}}}}"#
        ));
        assert!(malformed.is_none());

        let after_response =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            after_response.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "failed"
            }),
            "malformed response should fail the server request: {after_response:?}"
        );
        assert!(
            after_response.iter().any(|value| {
                value["method"] == "turn/completed" && value["params"]["turn"]["status"] == "failed"
            }),
            "malformed response should fail the turn after unblocking runtime waiter: {after_response:?}"
        );
    }

    #[test]
    fn timed_out_dynamic_tool_json_rpc_request_unblocks_runtime_waiter_fail_safe() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        client_tool_call_output("open_url", "call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("open_url")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("open docs".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let before_timeout = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_TOOL_CALL,
            Duration::from_secs(2),
        );
        let request_id = before_timeout
            .iter()
            .find(|value| value["method"] == server_request::ITEM_TOOL_CALL)
            .and_then(|value| value["id"].as_str())
            .expect("dynamic tool request should be emitted")
            .to_string();

        server.expire_pending_server_requests_for_tests(Duration::from_secs(301));

        let timeout_notifications =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            timeout_notifications.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "timed_out"
            }),
            "timeout should resolve the server request as timed_out: {timeout_notifications:?}"
        );
        assert_turn_status_and_ready(
            &mut server,
            &thread.thread_id,
            &started.turn.id,
            TurnStatus::Failed,
        );
    }

    #[test]
    fn user_input_json_rpc_response_resumes_runtime_waiter() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        user_input_tool_call_output("call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("request_user_input")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"ask user","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let before_response = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            Duration::from_secs(2),
        );
        let request_id = before_response
            .iter()
            .find(|value| value["method"] == server_request::ITEM_TOOL_REQUEST_USER_INPUT)
            .and_then(|value| value["id"].as_str())
            .expect("user-input request should be emitted")
            .to_string();
        assert_eq!(request_id, "call_1");

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"answers":{{"choice":{{"answers":["continue"]}}}}}}}}"#
        ));
        assert!(response.is_none());

        let after_response =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            after_response.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "approved"
            }),
            "user-input JSON-RPC response should resolve the request: {after_response:?}"
        );
        assert!(
            after_response.iter().any(|value| {
                value["method"] == event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION
                    && value["params"]["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("continue"))
                    && value["params"]["isError"] == false
            }),
            "user-input JSON-RPC response should become non-error tool output: {after_response:?}"
        );
    }

    #[test]
    fn permissions_json_rpc_denial_resumes_runtime_waiter_fail_safe() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        permissions_tool_call_output("call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("request_permissions")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"ask permissions","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let before_response = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
            Duration::from_secs(2),
        );
        let request_id = before_response
            .iter()
            .find(|value| value["method"] == server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL)
            .and_then(|value| value["id"].as_str())
            .expect("permissions request should be emitted")
            .to_string();
        assert_eq!(request_id, "call_1");

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"permissions":null,"scope":"turn"}}}}"#
        ));
        assert!(response.is_none());

        let after_response =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            after_response.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "rejected"
            }),
            "permissions denial should resolve the request as rejected: {after_response:?}"
        );
        assert!(
            after_response.iter().any(|value| {
                value["method"] == event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION
                    && value["params"]["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("null"))
                    && value["params"]["isError"] == true
            }),
            "permissions JSON-RPC denial should become error tool output: {after_response:?}"
        );
    }

    #[test]
    fn file_change_json_rpc_denial_resumes_runtime_waiter_without_executor() {
        let executor = Arc::new(CountingExecutor::new());
        let factory_executor = Arc::clone(&executor);
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new_with_runtime_context(
            move |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        file_change_tool_call_output("call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(
                        context,
                        Some(
                            Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>
                        ),
                    ))
                    .tools(vec![dummy_tool("apply_patch")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _start = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"change file","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let before_response = drain_json_rpc_until_method(
            &mut server,
            server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
            Duration::from_secs(2),
        );
        let request_id = before_response
            .iter()
            .find(|value| value["method"] == server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL)
            .and_then(|value| value["id"].as_str())
            .expect("file-change request should be emitted")
            .to_string();
        assert_eq!(request_id, "call_1");

        let response = server.handle_json_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":{request_id:?},"result":{{"decision":"decline"}}}}"#
        ));
        assert!(response.is_none());

        let after_response =
            drain_json_rpc_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            after_response.iter().any(|value| {
                value["method"] == "serverRequest/resolved"
                    && value["params"]["requestId"] == request_id
                    && value["params"]["outcome"] == "rejected"
            }),
            "file-change denial should resolve the request as rejected: {after_response:?}"
        );
        assert!(
            after_response.iter().any(|value| {
                value["method"] == event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION
                    && value["params"]["message"] == "file change denied by user"
                    && value["params"]["isError"] == true
            }),
            "file-change JSON-RPC denial should become error tool output: {after_response:?}"
        );
        assert_eq!(executor.call_count_blocking(), 0);
    }

    #[test]
    fn user_input_request_returns_renderer_answers() {
        let bridge = DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        user_input_tool_call_output("call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("request_user_input")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "ask user".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_response = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::ToolUserInputRequested { .. }
                )
            })
        });
        let request = before_response
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::ToolUserInputRequested { request } => Some(request.clone()),
                _ => None,
            })
            .expect("user input request should be emitted");
        assert_eq!(request.item_id, "turn_1:tool:call_1");
        assert_eq!(request.questions[0].id, "choice");

        let mut answers = HashMap::new();
        answers.insert(
            "choice".to_string(),
            dasclaw_app_server_protocol::ToolRequestUserInputAnswer {
                answers: vec!["continue".to_string()],
            },
        );
        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: request.request_id,
                payload: RuntimeServerRequestResponse::ToolUserInput(
                    ToolRequestUserInputResponse { answers },
                ),
            })
            .expect("user input response should resume tool execution");

        let after_response = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::Completed { .. }))
        });
        assert!(
            after_response.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::ToolResult { update }
                        if update.item_id == "turn_1:tool:call_1"
                            && update.content.contains("continue")
                            && !update.is_error
                )
            }),
            "renderer answers should become tool result: {after_response:?}"
        );
        bridge.shutdown();
    }

    #[test]
    fn permissions_denial_is_fail_safe() {
        let bridge = DasclawAgentRuntimeBridge::new_with_runtime_context(
            |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        permissions_tool_call_output("call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(context, None))
                    .tools(vec![dummy_tool("request_permissions")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "ask permissions".to_string(),
                cwd: PathBuf::from("/turn/local"),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_response = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::PermissionsApprovalRequested { .. }
                )
            })
        });
        let request = before_response
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::PermissionsApprovalRequested { request } => {
                    Some(request.clone())
                }
                _ => None,
            })
            .expect("permissions request should be emitted");
        assert_eq!(request.cwd, "/turn/local");

        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: request.request_id,
                payload: RuntimeServerRequestResponse::Permissions(denied_permissions_response()),
            })
            .expect("permissions denial should resume with fail-safe output");

        let after_response = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::Completed { .. }))
        });
        assert!(
            after_response.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::ToolResult { update }
                        if update.item_id == "turn_1:tool:call_1"
                            && update.is_error
                            && update.content.contains("null")
                )
            }),
            "permissions denial should be surfaced as error tool result: {after_response:?}"
        );
        bridge.shutdown();
    }

    #[test]
    fn file_change_denial_prevents_inner_executor() {
        let executor = Arc::new(CountingExecutor::new());
        let factory_executor = Arc::clone(&executor);
        let bridge = DasclawAgentRuntimeBridge::new_with_runtime_context(
            move |token, _snapshot, _reasoning_summary, context| {
                dasclaw_runtime::Agent::builder()
                    .responder(ScriptedResponder::new(vec![
                        file_change_tool_call_output("call_1"),
                        text_output("done"),
                    ]))
                    .tool_executor(RuntimeClientToolExecutor::new(
                        context,
                        Some(
                            Arc::clone(&factory_executor) as Arc<dyn dasclaw_runtime::ToolExecutor>
                        ),
                    ))
                    .tools(vec![dummy_tool("apply_patch")])
                    .cancellation_token(token)
                    .build()
                    .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
            },
        );
        let updates = RuntimeTurnUpdateSink::new();

        bridge
            .start_turn(RuntimeTurnStartRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                prompt: "change file".to_string(),
                cwd: PathBuf::from("."),
                model_provider: test_runtime_model_snapshot(),
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: updates.clone(),
                hook_registry: None,
            })
            .expect("turn should start");

        let before_response = collect_runtime_updates_until(&updates, |updates| {
            updates.iter().any(|update| {
                matches!(
                    update.outcome,
                    RuntimeTurnOutcome::FileChangeApprovalRequested { .. }
                )
            })
        });
        let request = before_response
            .iter()
            .find_map(|update| match &update.outcome {
                RuntimeTurnOutcome::FileChangeApprovalRequested { request } => {
                    Some(request.clone())
                }
                _ => None,
            })
            .expect("file-change approval request should be emitted");
        assert_eq!(request.item_id, "turn_1:tool:call_1");

        bridge
            .resolve_server_request(RuntimeServerRequestResolution {
                request_id: request.request_id,
                payload: RuntimeServerRequestResponse::FileChange(
                    FileChangeApprovalDecision::Decline,
                ),
            })
            .expect("file-change denial should resume with denied tool result");

        let after_response = collect_runtime_updates_until(&updates, |updates| {
            updates
                .iter()
                .any(|update| matches!(update.outcome, RuntimeTurnOutcome::Completed { .. }))
        });
        assert!(
            after_response.iter().any(|update| {
                matches!(
                    &update.outcome,
                    RuntimeTurnOutcome::ToolResult { update }
                        if update.item_id == "turn_1:tool:call_1"
                            && update.is_error
                            && update.content == "file change denied by user"
                )
            }),
            "denial should surface as error tool result: {after_response:?}"
        );
        assert_eq!(executor.call_count_blocking(), 0);
        bridge.shutdown();
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
                cwd: PathBuf::from("."),
                model_provider: snapshot,
                reasoning_summary: ReasoningSummary::None,
                sandbox_context: RuntimeSandboxContext::empty(),
                updates: RuntimeTurnUpdateSink::new(),
                hook_registry: None,
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        let methods = notifications
            .iter()
            .map(|notification| notification.method.as_str())
            .collect::<Vec<_>>();
        assert!(methods.contains(&"thread/started"));
        assert!(methods.contains(&"turn/started"));
        assert!(methods.contains(&"item/agentMessage/delta"));
        assert!(methods.contains(&"turn/completed"));
        let deltas = notifications
            .iter()
            .filter(|notification| notification.method == "item/agentMessage/delta")
            .map(|notification| notification.params["delta"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(deltas, vec!["Hel", "lo"]);

        let completed = notifications
            .iter()
            .find(|notification| notification.method == "turn/completed")
            .expect("completion notification should be emitted");
        assert_eq!(completed.params["turn"]["status"], "completed");
        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn.id,
            })
            .expect("turn/read should observe completed runtime result");
        assert_eq!(
            read.turn.status,
            codex_status_from_turn_status(TurnStatus::Completed)
        );
        assert_eq!(turn_text(&read.turn).as_deref(), Some("Hello"));
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        let agent_deltas = notifications
            .iter()
            .filter(|notification| notification.method == "item/agentMessage/delta")
            .map(|notification| notification.params["delta"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(agent_deltas, vec!["final answer"]);
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
                turn_id: started.turn.id,
            })
            .expect("turn/read should observe completed runtime result");
        assert_eq!(turn_text(&read.turn).as_deref(), Some("final answer"));
    }

    #[test]
    fn dasclaw_runtime_bridge_forwards_r5_agent_events_to_json_rpc_notifications() {
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(|token| {
            dasclaw_runtime::Agent::builder()
                .responder(ScriptedRuntimeResponder::new(
                    [
                        dasclaw_runtime::AgentEvent::ReasoningSummaryPartAdded {
                            item_id: Some("reasoning_item_1".to_string()),
                            summary_index: 2,
                        },
                        dasclaw_runtime::AgentEvent::ReasoningRawTextChunk {
                            item_id: Some("reasoning_item_1".to_string()),
                            content_index: 3,
                            delta: "raw reasoning".to_string(),
                        },
                        dasclaw_runtime::AgentEvent::PlanDelta {
                            item_id: Some("plan_item_1".to_string()),
                            delta: "add verification".to_string(),
                        },
                        dasclaw_runtime::AgentEvent::TurnPlanUpdated {
                            explanation: Some("Adjusting plan".to_string()),
                            plan: vec![AgentPlanStep {
                                step: "inspect current producer".to_string(),
                                status: AgentPlanStepStatus::InProgress,
                            }],
                        },
                        dasclaw_runtime::AgentEvent::TurnDiffUpdated {
                            diff: "--- a/file\n+++ b/file\n".to_string(),
                        },
                        dasclaw_runtime::AgentEvent::RawResponseItemCompleted {
                            item: serde_json::json!({
                                "id": "raw_item_1",
                                "type": "message",
                                "content": "raw provider item",
                            }),
                        },
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        let values = notifications
            .iter()
            .map(|notification| {
                serde_json::json!({
                    "method": notification.method,
                    "params": notification.params,
                })
            })
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == event::ITEM_REASONING_SUMMARY_PART_ADDED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["itemId"] == "reasoning_item_1"
                && value["params"]["summaryIndex"] == 2
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::ITEM_REASONING_TEXT_DELTA
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["itemId"] == "reasoning_item_1"
                && value["params"]["contentIndex"] == 3
                && value["params"]["delta"] == "raw reasoning"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::ITEM_PLAN_DELTA
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["itemId"] == "plan_item_1"
                && value["params"]["delta"] == "add verification"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::TURN_PLAN_UPDATED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["explanation"] == "Adjusting plan"
                && value["params"]["plan"][0]["step"] == "inspect current producer"
                && value["params"]["plan"][0]["status"] == "inProgress"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::TURN_DIFF_UPDATED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["diff"] == "--- a/file\n+++ b/file\n"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::RAW_RESPONSE_ITEM_COMPLETED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["item"]["id"] == "raw_item_1"
                && value["params"]["item"]["content"] == "raw provider item"
        }));
    }

    #[test]
    fn provider_backed_raw_response_item_reaches_app_server_notification() {
        let (base_url, fixture_done) = spawn_codex_chatgpt_responses_fixture_server();
        let bridge = Arc::new(DasclawAgentRuntimeBridge::new(move |token| {
            let provider = Arc::new(
                dasclaw_llm_provider::provider::CodexChatGptProvider::with_lazy_model(
                    &base_url,
                    SecretString::from("test-api-key".to_string()),
                    "gpt-4o",
                    None,
                    None,
                    5,
                ),
            );
            dasclaw_runtime::Agent::builder()
                .responder(
                    LlmProviderResponder::new(provider)
                        .with_reasoning_summary(ReasoningSummary::Auto),
                )
                .cancellation_token(token)
                .build()
                .map_err(|error| RuntimeBridgeError::fatal(error.to_string()))
        }));
        let mut server = initialized_codex_v2_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(5));
        fixture_done
            .recv_timeout(Duration::from_secs(1))
            .expect("fixture server should serve model list and Responses SSE");
        let values = notifications
            .iter()
            .map(|notification| {
                serde_json::json!({
                    "method": notification.method,
                    "params": notification.params,
                })
            })
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == event::ITEM_REASONING_TEXT_DELTA
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["itemId"] == "reasoning_item_1"
                && value["params"]["contentIndex"] == 3
                && value["params"]["delta"] == "raw reasoning"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::TURN_PLAN_UPDATED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["explanation"] == "provider-backed plan"
                && value["params"]["plan"][0]["step"] == "inspect current producer"
                && value["params"]["plan"][0]["status"] == "inProgress"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::TURN_DIFF_UPDATED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["diff"] == "--- a/file\n+++ b/file\n"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == event::RAW_RESPONSE_ITEM_COMPLETED
                && value["params"]["threadId"] == thread.thread_id
                && value["params"]["turnId"] == started.turn.id
                && value["params"]["item"]["id"] == "raw_item_1"
                && value["params"]["item"]["content"] == "raw provider item"
        }));
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start through runtime responder");

        let notifications =
            drain_until_method(&mut server, "turn/completed", Duration::from_secs(2));
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "item/agentMessage/delta"),
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
                turn_id: started.turn.id,
            })
            .expect("turn/read should observe runtime responder result");
        assert_eq!(
            read.turn.status,
            codex_status_from_turn_status(TurnStatus::Completed)
        );
        assert_eq!(turn_text(&read.turn).as_deref(), Some("responder"));
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let _ = server.drain_notifications();

        let cancelled = server
            .interrupt_turn_for_test(TestTurnInterruptParams {
                thread_id: thread.thread_id.clone(),
                turn_id: started.turn.id.clone(),
            })
            .expect("runtime cancellation should cancel runtime token");
        assert!(cancelled.accepted);
        assert_eq!(cancelled.status, TurnStatus::Cancelled);

        let notifications = drain_for(&mut server, Duration::from_millis(150));
        assert!(
            notifications
                .iter()
                .any(|notification| notification.method == "turn/completed"
                    && notification.params["turn"]["status"] == "interrupted"),
            "interrupted completion notification should be emitted: {notifications:?}"
        );
        assert!(
            notifications
                .iter()
                .all(|notification| notification.method != "error"),
            "runtime stopped update must not override cancellation: {notifications:?}"
        );

        let read = server
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn.id,
            })
            .expect("turn/read should keep cancelled status");
        assert_eq!(
            read.turn.status,
            codex_status_from_turn_status(TurnStatus::Cancelled)
        );
        assert_eq!(turn_text(&read.turn), None);
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
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
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
                .any(|notification| notification.method == "turn/completed"
                    && notification.params["turn"]["status"] == "interrupted"),
            "shutdown should interrupt pending runtime turn: {notifications:?}"
        );
        assert!(
            notifications
                .iter()
                .all(|notification| notification.method != "error"),
            "runtime stopped update must not emit failure after shutdown: {notifications:?}"
        );

        let read = server
            .threads
            .turn_summary(&thread.thread_id, &started.turn.id);
        assert_eq!(
            read.expect("turn should still be recorded").status,
            TurnStatus::Cancelled
        );
    }

    #[test]
    fn json_rpc_turn_list_and_read_return_in_memory_turn_status() {
        let mut server = initialized_server();
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let start = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should be started");
        let _ = server.interrupt_turn_for_test(TestTurnInterruptParams {
            thread_id: thread.thread_id.clone(),
            turn_id: start.turn.id.clone(),
        });

        let list = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turns","method":"thread/turns/list","params":{{"threadId":"{}"}}}}"#,
                thread.thread_id
            ))
            .expect("thread/turns/list should return a response");
        let read = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/read","params":{{"threadId":"{}","turnId":"{}"}}}}"#,
                thread.thread_id, start.turn.id
            ))
            .expect("turn/read should return a response");
        let list_value: Value = serde_json::from_str(&list).expect("list response JSON");
        let read_value: Value = serde_json::from_str(&read).expect("read response JSON");

        assert_eq!(
            list_value["result"]["data"]
                .as_array()
                .expect("data should be an array")
                .len(),
            1
        );
        assert_eq!(read_value["result"]["turn"]["id"], "turn_1");
        assert_eq!(read_value["result"]["turn"]["status"], "interrupted");
    }

    #[test]
    fn json_rpc_turn_read_rejects_unknown_turn() {
        let mut server = initialized_server();
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
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
    fn json_rpc_turn_interrupt_rejects_unknown_turn_after_thread_exists() {
        let mut server = initialized_server();
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn-interrupt","method":"turn/interrupt","params":{{"threadId":"{}","turnId":"missing"}}}}"#,
                thread.thread_id
            ))
            .expect("turn/interrupt should return a structured response");
        let value: Value = serde_json::from_str(&response).expect("interrupt response JSON");

        assert_eq!(value["error"]["code"], -32602);
        assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
        assert_eq!(value["error"]["data"]["capability"], "session");
    }

    #[test]
    fn json_rpc_turn_start_accepts_codex_v2_input_items() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge.clone());
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");

        let response = server
            .handle_json_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{{"threadId":"{}","input":[{{"type":"text","text":"hello input","text_elements":[]}}]}}}}"#,
                thread.thread_id
            ))
            .expect("turn/start should return a JSON-RPC response");
        let value: Value = serde_json::from_str(&response).expect("turn response JSON");
        let calls = bridge.calls.lock().expect("calls lock");

        assert_eq!(value["result"]["turn"]["id"], "turn_1");
        assert_eq!(calls[0].prompt, "hello input");
    }

    #[test]
    fn json_rpc_accepts_codex_v2_thread_start_and_turn_interrupt() {
        let mut server = initialized_server();
        let thread_response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#,
            )
            .expect("thread/start should return a JSON-RPC response");
        let thread: Value = serde_json::from_str(&thread_response).expect("thread response JSON");
        let thread_id = thread["result"]["thread"]["id"]
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
        let turn_id = turn["result"]["turn"]["id"]
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
    fn codex_v2_requested_profile_emits_item_notifications() {
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
                    CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string(),
                ],
                model_provider: Some(test_model_provider_config()),
            })
            .expect("initialize should succeed");
        let initialize_notifications = server.drain_notifications();
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: Some("Draft".to_string()),
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let thread_notifications = server.drain_notifications();

        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id,
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let notifications = server.drain_notifications();
        let methods = notifications
            .iter()
            .map(|notification| notification.method.as_str())
            .collect::<Vec<_>>();

        assert_eq!(thread_notifications.len(), 1);
        assert_eq!(thread_notifications[0].method, "thread/started");
        assert_eq!(started.turn.id, "turn_1");
        assert_eq!(
            methods,
            [
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "item/agentMessage/delta",
                "item/completed",
                "turn/completed",
                "lifecycle/changed",
            ]
        );
        assert_eq!(notifications[0].params["lifecycle"]["state"], "running");
        assert_eq!(notifications[6].params["lifecycle"]["state"], "ready");

        let profile = CompatibilityProfile::codex_app_server_v2_chat_session_subset();
        let emitted_methods = initialize_notifications
            .iter()
            .chain(thread_notifications.iter())
            .chain(notifications.iter())
            .map(|notification| notification.method.as_str())
            .collect::<Vec<_>>();
        for method in emitted_methods {
            assert!(
                profile.events.iter().any(|event| event == method),
                "chat-session subset profile did not declare emitted event: {method}"
            );
        }
    }

    #[test]
    fn codex_v2_thread_start_response_includes_thread_view_without_legacy_aliases() {
        let mut server = initialized_codex_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#,
            )
            .expect("thread/start should return a response");
        let value: Value = serde_json::from_str(&response).expect("thread/start response JSON");

        assert_eq!(value["result"]["thread"]["id"], "thread_1");
        assert_eq!(value["result"]["thread"]["cwd"], "Draft");
        assert_eq!(value["result"]["thread"]["modelProvider"], "openai");
        assert_eq!(value["result"]["thread"]["source"], "appServer");
        assert_eq!(value["result"]["thread"]["status"]["type"], "idle");
        assert!(value["result"].get("threadId").is_none());
    }

    #[test]
    fn thread_start_json_rpc_returns_v2_shaped_thread_without_legacy_aliases() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("response should be valid JSON");

        assert_eq!(value["result"]["thread"]["id"], "thread_1");
        assert_eq!(value["result"]["thread"]["cwd"], "/workspace");
        assert_eq!(value["result"]["thread"]["modelProvider"], "openai");
        assert_eq!(value["result"]["modelProvider"], "openai");
        assert!(value["result"].get("threadId").is_none());
        assert!(value["result"].get("lifecycle").is_none());

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "thread/started" && line["params"]["thread"]["id"] == "thread_1"
        }));
        assert!(
            !notifications
                .iter()
                .any(|line| line["method"] == "thread/created")
        );
    }

    #[test]
    fn thread_start_json_rpc_rejects_legacy_thread_create_fields() {
        for params in [
            r#"{"title":"legacy title"}"#,
            r#"{"workspaceRoot":"/legacy/workspace"}"#,
        ] {
            let mut server = initialized_server();
            let response = server
                .handle_json_rpc(&format!(
                    r#"{{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{params}}}"#
                ))
                .expect("thread/start should return a structured error response");
            let value: serde_json::Value =
                serde_json::from_str(&response).expect("response should be valid JSON");

            assert_eq!(value["error"]["code"], -32602);
            assert_eq!(value["error"]["data"]["code"], "INVALID_PARAMS");
            assert!(server.drain_notifications().is_empty());
        }
    }

    #[test]
    fn thread_create_json_rpc_is_not_a_public_method() {
        let mut server = initialized_server();
        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":1,"method":"thread/create","params":{"title":"hello"}}"#,
            )
            .expect("unknown method should return an error response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("response should be valid JSON");

        assert_eq!(value["error"]["code"], -32601);
        assert_eq!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
    }

    #[test]
    fn turn_start_json_rpc_returns_v2_shaped_turn_and_events() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server.drain_json_rpc_notifications();

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":2,"method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hi"}]}}"#,
            )
            .expect("turn/start should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("response should be valid JSON");

        assert_eq!(value["result"]["turn"]["id"], "turn_1");
        assert_eq!(value["result"]["turn"]["status"], "inProgress");
        assert!(value["result"].get("turnId").is_none());
        assert!(value["result"].get("status").is_none());
        assert!(value["result"].get("lifecycle").is_none());

        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "turn/started"
                && line["params"]["threadId"] == "thread_1"
                && line["params"]["turn"]["id"] == "turn_1"
        }));
        assert!(notifications.iter().any(|line| {
            line["method"] == "item/started"
                && line["params"]["item"]["type"] == "agentMessage"
                && line["params"]["item"]["id"] == "turn_1"
        }));
        assert!(
            !notifications
                .iter()
                .any(|line| line["method"] == "turn/delta")
        );
    }

    #[test]
    fn list_json_rpc_methods_return_codex_paged_data_shapes() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":2,"method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hi"}]}}"#,
            )
            .expect("turn/start should return a response");

        let thread_list_response = server
            .handle_json_rpc(r#"{"jsonrpc":"2.0","id":3,"method":"thread/list","params":{}}"#)
            .expect("thread/list should return a response");
        let thread_list_value: serde_json::Value =
            serde_json::from_str(&thread_list_response).expect("response should be valid JSON");
        assert_eq!(thread_list_value["result"]["data"][0]["id"], "thread_1");
        assert!(thread_list_value["result"].get("threads").is_none());
        assert!(thread_list_value["result"]["nextCursor"].is_null());
        assert!(thread_list_value["result"]["backwardsCursor"].is_null());

        let turn_list_response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":4,"method":"thread/turns/list","params":{"threadId":"thread_1"}}"#,
            )
            .expect("thread/turns/list should return a response");
        let turn_list_value: serde_json::Value =
            serde_json::from_str(&turn_list_response).expect("response should be valid JSON");
        assert_eq!(turn_list_value["result"]["data"][0]["id"], "turn_1");
        assert!(turn_list_value["result"].get("turns").is_none());
        assert!(turn_list_value["result"]["nextCursor"].is_null());
        assert!(turn_list_value["result"]["backwardsCursor"].is_null());

        let legacy_turn_list_response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":5,"method":"turn/list","params":{"threadId":"thread_1"}}"#,
            )
            .expect("unknown legacy method should return an error");
        let legacy_turn_list_value: serde_json::Value =
            serde_json::from_str(&legacy_turn_list_response)
                .expect("response should be valid JSON");
        assert_eq!(legacy_turn_list_value["error"]["code"], -32601);
        assert_eq!(
            legacy_turn_list_value["error"]["data"]["code"],
            "UNKNOWN_METHOD"
        );
    }

    #[test]
    fn turn_interrupt_replaces_turn_cancel_as_public_cancel_method() {
        let mut server = initialized_server();
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":1,"method":"thread/start","params":{"cwd":"/workspace"}}"#,
            )
            .expect("thread/start should return a response");
        server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":2,"method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hi"}]}}"#,
            )
            .expect("turn/start should return a response");

        let legacy_response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":3,"method":"turn/cancel","params":{"threadId":"thread_1","turnId":"turn_1"}}"#,
            )
            .expect("unknown legacy method should return an error");
        let legacy_value: serde_json::Value =
            serde_json::from_str(&legacy_response).expect("response should be valid JSON");
        assert_eq!(legacy_value["error"]["code"], -32601);
        assert_eq!(legacy_value["error"]["data"]["code"], "UNKNOWN_METHOD");

        let response = server
            .handle_json_rpc(
                r#"{"jsonrpc":"2.0","id":4,"method":"turn/interrupt","params":{"threadId":"thread_1","turnId":"turn_1"}}"#,
            )
            .expect("turn/interrupt should return a response");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("response should be valid JSON");
        assert_eq!(value["result"], serde_json::json!({}));
        let notifications = json_rpc_values(server.drain_json_rpc_notifications());
        assert!(notifications.iter().any(|line| {
            line["method"] == "turn/completed"
                && line["params"]["threadId"] == "thread_1"
                && line["params"]["turn"]["id"] == "turn_1"
                && line["params"]["turn"]["status"] == "interrupted"
        }));
    }

    #[test]
    fn codex_v2_turn_start_and_completed_notifications_include_turn_view() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([
            RuntimeTurnOutcome::Completed {
                output: "hello".to_string(),
            },
        ]));
        let mut server = initialized_codex_server_with_bridge(bridge);
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread/start should succeed");
        let _ = server.drain_notifications();

        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id,
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn/start should succeed");
        let notifications = server.drain_notifications();

        assert_eq!(started.turn.id, "turn_1");
        assert_eq!(started.turn.status, CodexTurnStatus::InProgress);
        let started_notification = notifications
            .iter()
            .find(|notification| notification.method == "turn/started")
            .expect("turn/started should be emitted");
        assert_eq!(started_notification.params["turn"]["id"], "turn_1");
        assert_eq!(started_notification.params["turn"]["status"], "inProgress");
        let completed = notifications
            .iter()
            .find(|notification| notification.method == "turn/completed")
            .expect("turn/completed should be emitted");
        assert_eq!(completed.params["turn"]["id"], "turn_1");
        assert_eq!(completed.params["turn"]["status"], "completed");
    }

    #[test]
    fn codex_v2_turn_failed_notification_includes_failed_turn_view() {
        let bridge = Arc::new(SequencedRuntimeBridge::new([RuntimeTurnOutcome::Failed {
            error: "runtime failed".to_string(),
        }]));
        let mut server = initialized_codex_server_with_bridge(bridge);
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: None,
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread/start should succeed");
        let _ = server.drain_notifications();

        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread.id,
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn/start should succeed");
        let notifications = server.drain_notifications();

        assert_eq!(started.turn.id, "turn_1");
        let failed = notifications
            .iter()
            .find(|notification| notification.method == "turn/completed")
            .expect("turn/completed should be emitted");
        assert_eq!(failed.params["turn"]["id"], "turn_1");
        assert_eq!(failed.params["turn"]["status"], "failed");
        assert_eq!(failed.params["turn"]["error"]["message"], "runtime failed");
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
                    "requestedCapabilities": [CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID],
                    "modelProvider": test_model_provider_config()
                }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "thread",
                "method": "thread/start",
                "params": {"cwd": "Draft"}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "turn",
                "method": "turn/start",
                "params": {"threadId": "thread_1", "input": [{"type": "text", "text": "hello", "text_elements": []}]}
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
                profile["id"].as_str()
                    == Some(CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID)
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
        assert!(emitted_methods.contains(&"thread/started"));
        assert!(emitted_methods.contains(&"lifecycle/changed"));
        assert!(emitted_methods.contains(&"item/agentMessage/delta"));
        assert!(advertised_events.contains("item/reasoning/summaryPartAdded"));
        assert!(advertised_events.contains("item/reasoning/textDelta"));
        for method in emitted_methods {
            assert!(
                advertised_events.contains(method),
                "stdio emitted notification not declared by advertised chat-session subset profile: {method}"
            );
        }
    }

    #[test]
    fn p3_stdio_e2e_approval_approve_unblocks_tool_execution() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let server = AppServer::with_runtime_bridge(bridge.clone());
        let input = [
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "initialize",
                "method": "initialize",
                "params": {
                    "client": {"name": "codex", "version": "2.0.0", "transport": "stdio"},
                    "protocolVersion": ProtocolVersion::current(),
                    "requestedCapabilities": [
                        CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID,
                        "approval",
                        "tools",
                        "sandbox"
                    ],
                    "modelProvider": test_model_provider_config()
                }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "thread",
                "method": "thread/start",
                "params": {"cwd": "P3 approval"}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "turn",
                "method": "turn/start",
                "params": {"threadId": "thread_1", "input": [{"type": "text", "text": "run echo", "text_elements": []}]}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "approval_00000000-0000-0000-0000-000000000001",
                "result": {"decision": {"kind": "approve"}}
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
        .expect("stdio loop should complete");

        let values = String::from_utf8(stdout)
            .expect("stdio output should be UTF-8")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("stdio line should be JSON"))
            .collect::<Vec<_>>();
        let emitted_methods = values
            .iter()
            .filter_map(|value| value.get("method").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert!(emitted_methods.contains(&"item/commandExecution/requestApproval"));
        assert!(emitted_methods.contains(&"serverRequest/resolved"));
        assert!(emitted_methods.contains(&"item/commandExecution/terminalInteraction"));
        assert!(values.iter().any(|value| {
            value["method"] == "serverRequest/resolved" && value["params"]["outcome"] == "approved"
        }));
        assert_eq!(
            bridge.decisions(),
            vec![(
                "00000000-0000-0000-0000-000000000001".to_string(),
                dasclaw_runtime::ApprovalDecision::Approve
            )]
        );
    }

    #[test]
    fn p3_stdio_e2e_approval_reject_fails_safe_without_tool_output() {
        let bridge = Arc::new(ManualApprovalBridge::default());
        let server = AppServer::with_runtime_bridge(bridge.clone());
        let input = [
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "initialize",
                "method": "initialize",
                "params": {
                    "client": {"name": "codex", "version": "2.0.0", "transport": "stdio"},
                    "protocolVersion": ProtocolVersion::current(),
                    "requestedCapabilities": [
                        CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID,
                        "approval",
                        "tools",
                        "sandbox"
                    ],
                    "modelProvider": test_model_provider_config()
                }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "thread",
                "method": "thread/start",
                "params": {"cwd": "P3 rejection"}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "turn",
                "method": "turn/start",
                "params": {"threadId": "thread_1", "input": [{"type": "text", "text": "run echo", "text_elements": []}]}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "approval_00000000-0000-0000-0000-000000000001",
                "result": {"decision": {"kind": "reject", "data": {"reason": "not allowed"}}}
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
        .expect("stdio loop should complete");

        let values = String::from_utf8(stdout)
            .expect("stdio output should be UTF-8")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("stdio line should be JSON"))
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == "serverRequest/resolved"
                && value["params"]["outcome"] == "rejected"
                && value["params"]["reason"] == "not allowed"
        }));
        assert!(
            !values.iter().any(|value| {
                value["method"] == "item/commandExecution/terminalInteraction"
                    && value["params"]["isError"] == false
            }),
            "reject path must not emit successful tool output"
        );
        assert!(matches!(
            bridge.decisions().as_slice(),
            [(
                id,
                dasclaw_runtime::ApprovalDecision::Reject {
                    reason: Some(reason)
                }
            )] if id == "00000000-0000-0000-0000-000000000001" && reason == "not allowed"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn stdio_streaming_command_exec_accepts_write_before_final_exec_response() {
        use base64::Engine as _;

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
                "command": ["sh", "-c", "printf ready; IFS= read -r line; printf 'got:%s' \"$line\""],
                "processId": "stdio_proc",
                "tty": true,
                "streamStdin": true,
                "streamStdoutStderr": true,
                "timeoutMs": 500
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
        run_stdio_server_with_app_server(
            server,
            std::io::BufReader::new(Cursor::new(input)),
            &mut output,
        )
        .expect("stdio server should complete");

        let text = String::from_utf8(output).expect("stdio output utf8");
        let values = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json line"))
            .collect::<Vec<_>>();

        assert!(
            values.iter().any(|value| {
                value["method"] == event::COMMAND_EXEC_OUTPUT_DELTA
                    && value["params"]["processId"] == "stdio_proc"
            }),
            "expected outputDelta notification in stdio output: {text}"
        );
        assert!(
            values
                .iter()
                .any(|value| value["id"] == "write" && value["result"].is_object()),
            "expected successful write response in stdio output: {text}"
        );
        assert!(
            values.iter().any(|value| {
                value["id"] == "cmd"
                    && value["result"]["exitCode"] == 0
                    && value["result"]["stdout"] == ""
            }),
            "expected final successful command response in stdio output: {text}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn stdio_streaming_command_exec_notification_eof_does_not_wait_forever() {
        let temp = TestDir::new("stdio_streaming_command_exec_notification_eof");
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
        let exec_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "command/exec",
            "params": {
                "command": ["sh", "-c", "printf note-done"],
                "processId": "stdio_notification_proc",
                "tty": true,
                "streamStdoutStderr": true,
                "timeoutMs": 1_000
            }
        });

        let input = format!("{initialize}\n{exec_notification}\n");
        let mut output = Vec::new();
        run_stdio_server_with_app_server(
            server,
            std::io::BufReader::new(Cursor::new(input)),
            &mut output,
        )
        .expect("stdio server should complete after notification EOF");

        let text = String::from_utf8(output).expect("stdio output utf8");
        let values = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json line"))
            .collect::<Vec<_>>();

        assert!(
            values.iter().any(|value| {
                value["method"] == event::COMMAND_EXEC_OUTPUT_DELTA
                    && value["params"]["processId"] == "stdio_notification_proc"
            }),
            "expected outputDelta notification in stdio output: {text}"
        );
        assert!(
            !values
                .iter()
                .any(|value| value.get("id").is_some_and(|id| id == "cmd")),
            "command/exec notification must not emit a response: {text}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn stdio_streaming_command_exec_invalid_jsonrpc_version_does_not_spawn() {
        let temp = TestDir::new("stdio_streaming_command_exec_bad_version");
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
            "jsonrpc": "1.0",
            "id": "bad-stream",
            "method": "command/exec",
            "params": {
                "command": ["sh", "-c", "printf should-not-run"],
                "processId": "stdio_bad_version_proc",
                "tty": true,
                "streamStdoutStderr": true
            }
        });

        let input = format!("{initialize}\n{exec}\n");
        let mut output = Vec::new();
        run_stdio_server_with_app_server(
            server,
            std::io::BufReader::new(Cursor::new(input)),
            &mut output,
        )
        .expect("stdio server should complete");

        let text = String::from_utf8(output).expect("stdio output utf8");
        let values = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json line"))
            .collect::<Vec<_>>();

        let response = values
            .iter()
            .find(|value| value["id"] == "bad-stream")
            .expect("invalid JSON-RPC version should return a response");
        assert_eq!(response["error"]["data"]["code"], "INVALID_PARAMS");
        assert!(
            !values
                .iter()
                .any(|value| value["method"] == event::COMMAND_EXEC_OUTPUT_DELTA),
            "invalid JSON-RPC version must not spawn command exec: {text}"
        );
    }

    #[test]
    fn default_profile_emits_v2_item_notifications() {
        let bridge = Arc::new(RecordingRuntimeBridge::with_completion(
            RuntimeTurnOutcome::Completed {
                output: "hello from runtime".to_string(),
            },
        ));
        let mut server = initialized_server_with_bridge(bridge);
        let thread = server
            .create_thread_for_test(TestThreadParams { cwd: None })
            .expect("thread should be created");
        let _ = server.drain_notifications();

        let started = server
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                input: text_input("hello".to_string()),
                cwd: None,
                model: None,
                summary: None,
                sandbox_policy: None,
                permission_profile: None,
            })
            .expect("turn should start");
        let methods = server
            .drain_notifications()
            .into_iter()
            .map(|notification| notification.method)
            .collect::<Vec<_>>();

        assert_eq!(started.turn.id, "turn_1");
        assert_eq!(
            methods,
            vec![
                "lifecycle/changed",
                "turn/started",
                "item/started",
                "item/completed",
                "turn/completed",
                "lifecycle/changed"
            ]
        );
    }

    fn initialized_server() -> AppServer {
        initialized_server_with_bridge(Arc::new(NoopRuntimeBridge))
    }

    fn initialized_server_with_root(root: &std::path::Path) -> AppServer {
        let services =
            app_services::AppServerServices::real_with_root_for_tests(root.to_path_buf());
        let mut server =
            AppServer::with_runtime_bridge(Arc::new(NoopRuntimeBridge)).with_app_services(services);
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
        server
    }

    fn initialized_server_with_thread_snapshot(
        root: &std::path::Path,
        snapshot_path: &std::path::Path,
        snapshot: serde_json::Value,
    ) -> AppServer {
        fs::write(
            snapshot_path,
            serde_json::to_vec_pretty(&snapshot).expect("snapshot json"),
        )
        .expect("write thread snapshot");
        let services =
            app_services::AppServerServices::real_with_root_for_tests(root.to_path_buf());
        let mut server = AppServer::with_runtime_bridge(Arc::new(NoopRuntimeBridge))
            .with_app_services(services)
            .with_thread_snapshot_path(snapshot_path)
            .expect("thread snapshot should load");
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
        server
    }

    fn write_app_server_config(root: &Path, value: serde_json::Value) {
        let dir = root.join(".dasclaw");
        fs::create_dir_all(&dir).expect("config dir");
        fs::write(
            dir.join("app-server-config.json"),
            serde_json::to_vec_pretty(&value).expect("config json"),
        )
        .expect("write config");
    }

    fn create_completed_thread_with_turns(server: &mut AppServer, count: usize) -> String {
        let thread = server
            .thread_start(ThreadStartParams {
                cwd: Some("/workspace".to_string()),
                sandbox: None,
                permission_profile: None,
            })
            .expect("thread should be created");
        let thread_id = thread.thread.id;
        for index in 0..count {
            let turn_id = server.threads.next_turn_id();
            server
                .threads
                .record_started_turn(&thread_id, turn_id.clone())
                .expect("turn should be recorded");
            server
                .threads
                .apply_runtime_turn_update(RuntimeTurnUpdate {
                    thread_id: thread_id.clone(),
                    turn_id,
                    outcome: RuntimeTurnOutcome::Completed {
                        output: format!("turn {index}"),
                    },
                })
                .expect("turn should complete");
        }
        thread_id
    }

    fn initialized_server_with_bridge(bridge: Arc<dyn RuntimeBridge>) -> AppServer {
        initialized_server_with_bridge_and_capabilities(bridge, Vec::new())
    }

    fn initialized_codex_server() -> AppServer {
        initialized_codex_server_with_bridge(Arc::new(NoopRuntimeBridge))
    }

    fn initialized_codex_server_with_bridge(bridge: Arc<dyn RuntimeBridge>) -> AppServer {
        let mut server = AppServer::with_runtime_bridge(bridge);
        server
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: Some(WorkspaceInfo {
                    root: Some("/tmp/workspace".to_string()),
                    trust: WorkspaceTrust::Unknown,
                }),
                requested_capabilities: vec![
                    CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string(),
                ],
                model_provider: Some(test_model_provider_config()),
            })
            .expect("codex server should initialize");
        let _ = server.drain_notifications();
        server
    }

    fn initialized_codex_v2_server_with_bridge(bridge: Arc<dyn RuntimeBridge>) -> AppServer {
        initialized_server_with_bridge_and_capabilities(
            bridge,
            vec![CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string()],
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
            api_base_url: Some(TEST_MODEL_API_BASE_URL.to_string()),
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

    fn spawn_codex_chatgpt_responses_fixture_server() -> (String, std::sync::mpsc::Receiver<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("fixture server should bind localhost");
        listener
            .set_nonblocking(true)
            .expect("fixture server should support nonblocking accept");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        thread::spawn(move || {
            let sse = r#"event: response.reasoning_text.delta
data: {"item_id":"reasoning_item_1","content_index":3,"delta":"raw reasoning"}

event: response.output_text.delta
data: {"delta":"final answer"}

event: response.output_item.done
data: {"item":{"id":"raw_item_1","type":"message","content":"raw provider item"}}

event: turn.plan.updated
data: {"explanation":"provider-backed plan","plan":[{"step":"inspect current producer","status":"inProgress"}]}

event: turn.diff.updated
data: {"diff":"--- a/file\n+++ b/file\n"}

event: response.completed
data: {"response":{"usage":{"input_tokens":3,"output_tokens":2}}}

"#;
            let responses = [
                (
                    "GET /models",
                    "application/json",
                    r#"{"models":[{"slug":"gpt-4o"}]}"#.to_string(),
                ),
                ("POST /responses", "text/event-stream", sse.to_string()),
            ];
            let deadline = Instant::now() + Duration::from_secs(5);

            for (expected_prefix, content_type, body) in responses {
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(accepted) => break accepted,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "fixture server timed out waiting for {expected_prefix}"
                            );
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("fixture server accept failed: {error}"),
                    }
                };

                let request = read_fixture_http_request(&mut stream);
                let request_line = request.lines().next().unwrap_or_default();
                assert!(
                    request_line.starts_with(expected_prefix),
                    "expected request line prefix {expected_prefix:?}, got {request_line:?}"
                );
                write_fixture_http_response(&mut stream, content_type, &body);
            }

            let _ = done_tx.send(());
        });

        (base_url, done_rx)
    }

    fn read_fixture_http_request(stream: &mut std::net::TcpStream) -> String {
        use std::io::Read as _;

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("fixture request stream should support read timeout");
        let mut buffer = Vec::new();
        let mut scratch = [0_u8; 1024];
        let mut expected_len = None;

        loop {
            let read = stream
                .read(&mut scratch)
                .expect("fixture server should read request bytes");
            assert!(read > 0, "fixture client closed request before headers");
            buffer.extend_from_slice(&scratch[..read]);

            if expected_len.is_none()
                && let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
            {
                let header_text = String::from_utf8_lossy(&buffer[..header_end]);
                let content_len = header_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                expected_len = Some(header_end + 4 + content_len);
            }

            if let Some(expected_len) = expected_len
                && buffer.len() >= expected_len
            {
                return String::from_utf8_lossy(&buffer).into_owned();
            }
        }
    }

    fn sample_hook_run(status: HookRunStatus) -> HookRunSummary {
        HookRunSummary {
            id: "hook-run-1".to_string(),
            event_name: HookEventName::PreToolUse,
            handler_type: HookHandlerType::Command,
            execution_mode: HookExecutionMode::Sync,
            scope: HookScope::Turn,
            source_path: "/tmp/hook.sh".to_string(),
            source: HookSource::Project,
            display_order: 1,
            status,
            status_message: None,
            started_at: 1,
            completed_at: Some(2),
            duration_ms: Some(1),
            entries: vec![HookOutputEntry {
                kind: HookOutputEntryKind::Warning,
                text: "check this".to_string(),
            }],
        }
    }

    fn test_tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime")
    }

    struct TestHook {
        name: String,
        points: Vec<dasclaw_hooks::HookPoint>,
        rejection: Option<String>,
        modification: Option<String>,
    }

    impl TestHook {
        fn passthrough(name: &str, point: dasclaw_hooks::HookPoint) -> Self {
            Self::new(name, point, None, None)
        }

        fn rejecting(name: &str, point: dasclaw_hooks::HookPoint, reason: &str) -> Self {
            Self::new(name, point, Some(reason), None)
        }

        fn modifying(name: &str, point: dasclaw_hooks::HookPoint, modified: &str) -> Self {
            Self::new(name, point, None, Some(modified))
        }

        fn new(
            name: &str,
            point: dasclaw_hooks::HookPoint,
            rejection: Option<&str>,
            modification: Option<&str>,
        ) -> Self {
            Self {
                name: name.to_string(),
                points: vec![point],
                rejection: rejection.map(ToString::to_string),
                modification: modification.map(ToString::to_string),
            }
        }
    }

    fn write_fixture_http_response(
        stream: &mut std::net::TcpStream,
        content_type: &str,
        body: &str,
    ) {
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("fixture server should write response");
    }

    #[async_trait::async_trait]
    impl dasclaw_hooks::Hook for TestHook {
        fn name(&self) -> &str {
            &self.name
        }

        fn hook_points(&self) -> &[dasclaw_hooks::HookPoint] {
            &self.points
        }

        async fn execute(
            &self,
            _event: &dasclaw_hooks::HookEvent,
            _ctx: &dasclaw_hooks::HookContext,
        ) -> Result<dasclaw_hooks::HookOutcome, dasclaw_hooks::HookError> {
            if let Some(reason) = &self.rejection {
                return Ok(dasclaw_hooks::HookOutcome::reject(reason.clone()));
            }
            if let Some(modified) = &self.modification {
                return Ok(dasclaw_hooks::HookOutcome::modify(modified.clone()));
            }
            Ok(dasclaw_hooks::HookOutcome::ok())
        }
    }

    fn replace_file_with_directory(path: &Path) {
        if path.is_file() {
            fs::remove_file(path).expect("remove snapshot file");
        } else if path.is_dir() {
            fs::remove_dir_all(path).expect("remove snapshot directory");
        }
        fs::create_dir(path).expect("create blocking snapshot directory");
    }

    fn text_input(text: impl Into<String>) -> Vec<dasclaw_app_server_protocol::UserInput> {
        vec![dasclaw_app_server_protocol::UserInput::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }]
    }

    fn image_input(url: impl Into<String>) -> Vec<dasclaw_app_server_protocol::UserInput> {
        vec![dasclaw_app_server_protocol::UserInput::Image { url: url.into() }]
    }

    fn interrupt_turn(
        server: &mut AppServer,
        thread_id: impl Into<String>,
        turn_id: impl Into<String>,
    ) {
        server
            .turn_interrupt(TurnInterruptParams {
                thread_id: thread_id.into(),
                turn_id: turn_id.into(),
            })
            .expect("turn should interrupt");
    }

    fn assert_turn_status_and_ready(
        server: &mut AppServer,
        thread_id: &str,
        turn_id: &str,
        expected_status: TurnStatus,
    ) {
        let turn = server
            .turn_read(TurnReadParams {
                thread_id: thread_id.to_string(),
                turn_id: turn_id.to_string(),
            })
            .expect("turn should be readable after terminal approval path");
        assert_eq!(
            turn.turn.status,
            codex_status_from_turn_status(expected_status)
        );
        assert_eq!(
            server.lifecycle_status().lifecycle.state,
            LifecycleState::Ready
        );
    }

    fn codex_status_from_turn_status(status: TurnStatus) -> CodexTurnStatus {
        match status {
            TurnStatus::Pending => CodexTurnStatus::InProgress,
            TurnStatus::Completed => CodexTurnStatus::Completed,
            TurnStatus::Failed => CodexTurnStatus::Failed,
            TurnStatus::Cancelled => CodexTurnStatus::Interrupted,
        }
    }

    fn turn_text(turn: &CodexTurn) -> Option<String> {
        turn.items.iter().find_map(|item| match item {
            CodexThreadItem::AgentMessage { text, .. } if !text.is_empty() => Some(text.clone()),
            CodexThreadItem::AgentMessage { .. } => None,
            CodexThreadItem::Reasoning { .. } => None,
            CodexThreadItem::EnteredReviewMode { .. } => None,
            CodexThreadItem::ExitedReviewMode { .. } => None,
        })
    }

    fn methods_from_values(values: &[Value]) -> Vec<&str> {
        values
            .iter()
            .filter_map(|value| value.get("method").and_then(Value::as_str))
            .collect()
    }

    fn expected_warning_events(guardian_warnings: bool) -> Vec<String> {
        let mut events = Vec::new();
        if generic_warning_producer_available() {
            events.push(event::WARNING.to_string());
        }
        if guardian_warnings {
            events.push(event::GUARDIAN_WARNING.to_string());
        }
        events.push(event::CONFIG_WARNING.to_string());
        events.push(event::DEPRECATION_NOTICE.to_string());
        events
    }

    fn auto_approval_review_update(
        status: &str,
        risk_level: Option<&str>,
        rationale: Option<&str>,
    ) -> RuntimeAutoApprovalReviewUpdate {
        RuntimeAutoApprovalReviewUpdate {
            review_id: "review_1".to_string(),
            target_item_id: Some("turn_1:file:patch".to_string()),
            review: GuardianApprovalReview {
                status: status.to_string(),
                risk_level: risk_level.map(ToString::to_string),
                user_authorization: None,
                rationale: rationale.map(ToString::to_string),
            },
            action: "apply_patch".to_string(),
        }
    }

    #[cfg(target_os = "linux")]
    struct EnvVarGuard {
        key: &'static str,
        old_value: Option<std::ffi::OsString>,
    }

    #[cfg(target_os = "linux")]
    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let old_value = std::env::var_os(key);
            // SAFETY: This test guard is used by single-test nextest processes
            // and restores the original variable on drop before returning.
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, old_value }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: The guard owns this temporary mutation and restores the
            // process environment before the test exits.
            unsafe {
                match &self.old_value {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
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
        text_output_with_usage(text, TokenUsage::default())
    }

    fn text_output_with_usage(text: &str, usage: TokenUsage) -> RespondOutput {
        RespondOutput {
            result: RespondResult::Text(text.to_string()),
            usage,
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        }
    }

    fn text_output_with_metadata(text: &str, metadata: ResponseMetadata) -> RespondOutput {
        RespondOutput {
            result: RespondResult::Text(text.to_string()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata,
        }
    }

    struct ScriptedResponder {
        script: tokio::sync::Mutex<Vec<RespondOutput>>,
    }

    impl ScriptedResponder {
        fn new(script: Vec<RespondOutput>) -> Self {
            Self {
                script: tokio::sync::Mutex::new(script),
            }
        }
    }

    #[async_trait::async_trait]
    impl dasclaw_runtime::AgentResponder for ScriptedResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            let mut script = self.script.lock().await;
            if script.is_empty() {
                return Err("script exhausted".into());
            }
            Ok(script.remove(0))
        }
    }

    struct CountingExecutor {
        calls: tokio::sync::Mutex<Vec<ToolCall>>,
    }

    impl CountingExecutor {
        fn new() -> Self {
            Self {
                calls: tokio::sync::Mutex::new(Vec::new()),
            }
        }

        fn call_count_blocking(&self) -> usize {
            self.calls.blocking_lock().len()
        }
    }

    #[async_trait::async_trait]
    impl dasclaw_runtime::ToolExecutor for CountingExecutor {
        async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
            self.calls.lock().await.push(call.clone());
            Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("ran {}", call.name),
                is_error: false,
            })
        }
    }

    struct AlwaysApprovePolicy;

    #[async_trait::async_trait]
    impl dasclaw_runtime::ApprovalPolicy for AlwaysApprovePolicy {
        async fn evaluate(&self, call: &ToolCall) -> Option<dasclaw_runtime::ApprovalRequest> {
            Some(dasclaw_runtime::ApprovalRequest {
                description: format!("approve call to {}", call.name),
                display_parameters: call.arguments.clone(),
                allow_always: true,
            })
        }
    }

    fn tool_call_output(name: &str, id: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: name.into(),
                    arguments: serde_json::json!({
                        "path": "/tmp/x",
                        "tool_call_id": id,
                        "api_key": "secret-token",
                    }),
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    fn client_tool_call_output(name: &str, id: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: name.into(),
                    arguments: serde_json::json!({
                        "tool_call_id": id,
                        "url": "https://example.test",
                    }),
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    fn user_input_tool_call_output(id: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: "request_user_input".into(),
                    arguments: serde_json::json!({
                        "tool_call_id": id,
                        "questions": [{
                            "id": "choice",
                            "header": "Choice",
                            "question": "Continue?",
                            "options": [{
                                "label": "Continue",
                                "description": "Proceed with the request"
                            }]
                        }]
                    }),
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    fn permissions_tool_call_output(id: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: "request_permissions".into(),
                    arguments: serde_json::json!({
                        "tool_call_id": id,
                        "reason": "need network",
                        "permissions": { "network": true }
                    }),
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    fn file_change_tool_call_output(id: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: "apply_patch".into(),
                    arguments: serde_json::json!({
                        "tool_call_id": id,
                        "path": "/tmp/r2.txt",
                        "reason": "apply patch"
                    }),
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    fn dummy_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.into(),
            description: "test tool".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    #[derive(Debug, Default)]
    struct R1RuntimeBridge {
        resolutions: Mutex<Vec<RuntimeServerRequestResolution>>,
    }

    impl R1RuntimeBridge {
        fn resolutions(&self) -> Vec<RuntimeServerRequestResolution> {
            self.resolutions.lock().expect("resolutions lock").clone()
        }
    }

    impl RuntimeBridge for R1RuntimeBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                dynamic_tool_call: true,
                tool_user_input: true,
                permissions_approval: true,
                file_change_approval: true,
                file_change_events: true,
                auto_approval_review: true,
                thread_compact: false,
                turn_steer: false,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn cancel_turn(
            &self,
            _request: RuntimeTurnCancelRequest,
        ) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn resolve_server_request(
            &self,
            resolution: RuntimeServerRequestResolution,
        ) -> Result<(), RuntimeBridgeError> {
            self.resolutions
                .lock()
                .expect("resolutions lock")
                .push(resolution);
            Ok(())
        }

        fn shutdown(&self) {}
    }

    #[derive(Debug)]
    struct R1ProducingBridge;

    impl RuntimeBridge for R1ProducingBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                dynamic_tool_call: true,
                tool_user_input: true,
                permissions_approval: true,
                file_change_approval: true,
                file_change_events: true,
                auto_approval_review: true,
                thread_compact: false,
                turn_steer: false,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            let thread_id = request.thread_id.clone();
            let turn_id = request.turn_id.clone();
            request.updates.dynamic_tool_call_requested(
                thread_id.clone(),
                turn_id.clone(),
                RuntimeDynamicToolCallRequest {
                    request_id: "dynamic_1".to_string(),
                    call_id: "call_1".to_string(),
                    namespace: Some("browser".to_string()),
                    tool: "open_url".to_string(),
                    arguments: serde_json::json!({"url": "https://example.test"}),
                },
            );
            request.updates.tool_user_input_requested(
                thread_id.clone(),
                turn_id.clone(),
                RuntimeToolUserInputRequest {
                    request_id: "user_input_1".to_string(),
                    item_id: format!("{turn_id}:tool:ask"),
                    questions: vec![ToolRequestUserInputQuestion {
                        id: "choice".to_string(),
                        header: "Mode".to_string(),
                        question: "Pick a mode".to_string(),
                        is_other: false,
                        is_secret: false,
                        options: None,
                    }],
                },
            );
            request.updates.file_change_approval_requested(
                thread_id.clone(),
                turn_id.clone(),
                RuntimeFileChangeApprovalRequest {
                    request_id: "file_change_1".to_string(),
                    item_id: format!("{turn_id}:file:patch"),
                    reason: Some("apply patch".to_string()),
                    grant_root: Some("/workspace".to_string()),
                },
            );
            request.updates.permissions_approval_requested(
                thread_id.clone(),
                turn_id.clone(),
                RuntimePermissionsApprovalRequest {
                    request_id: "permissions_1".to_string(),
                    item_id: format!("{turn_id}:permissions:network"),
                    cwd: "/workspace".to_string(),
                    reason: Some("network access required".to_string()),
                    permissions: serde_json::json!({"network": {"allow": ["example.test"]}}),
                },
            );
            request.updates.file_change_output_delta(
                thread_id.clone(),
                turn_id.clone(),
                RuntimeFileChangeOutputDeltaUpdate {
                    item_id: format!("{turn_id}:file:patch"),
                    delta: "@@ -1 +1 @@\n".to_string(),
                },
            );
            request.updates.file_change_patch_updated(
                thread_id.clone(),
                turn_id.clone(),
                RuntimeFileChangePatchUpdatedUpdate {
                    item_id: format!("{turn_id}:file:patch"),
                    changes: vec![FileUpdateChange {
                        path: "src/lib.rs".to_string(),
                        kind: dasclaw_app_server_protocol::FileUpdateKind::Update,
                        unified_diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
                    }],
                },
            );
            request.updates.auto_approval_review_started(
                thread_id.clone(),
                turn_id.clone(),
                RuntimeAutoApprovalReviewUpdate {
                    review_id: "review_1".to_string(),
                    target_item_id: Some(format!("{turn_id}:file:patch")),
                    review: GuardianApprovalReview {
                        status: "running".to_string(),
                        risk_level: None,
                        user_authorization: None,
                        rationale: Some("reviewing patch".to_string()),
                    },
                    action: "review".to_string(),
                },
            );
            request.updates.auto_approval_review_completed(
                thread_id,
                turn_id.clone(),
                RuntimeAutoApprovalReviewUpdate {
                    review_id: "review_1".to_string(),
                    target_item_id: Some(format!("{turn_id}:file:patch")),
                    review: GuardianApprovalReview {
                        status: "approved".to_string(),
                        risk_level: Some("low".to_string()),
                        user_authorization: Some("not_required".to_string()),
                        rationale: Some("patch is within policy".to_string()),
                    },
                    action: "approve".to_string(),
                },
            );
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
    struct AutoApprovalReviewFeatureBridge;

    impl RuntimeBridge for AutoApprovalReviewFeatureBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                auto_approval_review: true,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
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
    struct ModelDeliveryRuntimeBridge;

    impl RuntimeBridge for ModelDeliveryRuntimeBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                model_reroutes: true,
                model_verifications: true,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
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

    #[derive(Debug, Default)]
    struct RecordingRuntimeBridge {
        calls: Mutex<Vec<RuntimeTurnStartRequest>>,
        cancel_calls: Mutex<Vec<RuntimeTurnCancelRequest>>,
        steer_calls: Mutex<Vec<RuntimeTurnSteerRequest>>,
        turn_steer: bool,
        result: Mutex<Option<Result<(), RuntimeBridgeError>>>,
        cancel_result: Mutex<Option<Result<(), RuntimeBridgeError>>>,
        completion: Mutex<Option<RuntimeTurnOutcome>>,
    }

    impl RecordingRuntimeBridge {
        fn with_result(result: Result<(), RuntimeBridgeError>) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }

        fn with_completion(completion: RuntimeTurnOutcome) -> Self {
            Self {
                completion: Mutex::new(Some(completion)),
                ..Self::default()
            }
        }

        fn with_cancel_result(result: Result<(), RuntimeBridgeError>) -> Self {
            Self {
                cancel_result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }

        fn with_turn_steer() -> Self {
            Self {
                turn_steer: true,
                ..Self::default()
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
                push_test_runtime_outcome(&request, outcome.clone());
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
                    push_test_runtime_outcome(&request, outcome);
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

    #[derive(Debug)]
    struct SnapshotBlockingRuntimeBridge {
        snapshot_path: PathBuf,
    }

    #[derive(Debug)]
    struct CompactRuntimeBridge;

    impl RuntimeBridge for CompactRuntimeBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                thread_compact: true,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn cancel_turn(
            &self,
            _request: RuntimeTurnCancelRequest,
        ) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn compact_thread(
            &self,
            _request: RuntimeThreadCompactRequest,
        ) -> Result<RuntimeThreadCompactResult, RuntimeBridgeError> {
            Ok(RuntimeThreadCompactResult {
                turn_id: "turn_compacted".to_string(),
            })
        }

        fn shutdown(&self) {}
    }

    impl RuntimeBridge for SnapshotBlockingRuntimeBridge {
        fn start_turn(&self, _request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            replace_file_with_directory(&self.snapshot_path);
            Err(RuntimeBridgeError::fatal("runtime start failed"))
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
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                turn_steer: self.turn_steer,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            if let Some(completion) = self.completion.lock().expect("completion lock").clone() {
                push_test_runtime_outcome(&request, completion);
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

        fn steer_turn(&self, request: RuntimeTurnSteerRequest) -> Result<(), RuntimeBridgeError> {
            self.steer_calls
                .lock()
                .expect("steer calls lock")
                .push(request);
            Ok(())
        }

        fn shutdown(&self) {}
    }

    fn push_test_runtime_outcome(request: &RuntimeTurnStartRequest, outcome: RuntimeTurnOutcome) {
        match outcome {
            RuntimeTurnOutcome::Delta { delta } => {
                request
                    .updates
                    .delta(request.thread_id.clone(), request.turn_id.clone(), delta);
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
            RuntimeTurnOutcome::ReasoningSummaryPartAdded { update } => {
                request.updates.reasoning_summary_part_added(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::ReasoningTextDelta { update } => {
                request.updates.reasoning_text_delta(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::PlanUpdated { update } => {
                request.updates.turn_plan_updated(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::DiffUpdated { update } => {
                request.updates.turn_diff_updated(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::RawResponseItemCompleted { update } => {
                request.updates.raw_response_item_completed(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::PlanDelta { update } => {
                request.updates.plan_delta(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::ApprovalRequested { request: approval } => {
                request.updates.approval_requested(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    approval,
                );
            }
            RuntimeTurnOutcome::ToolResult { update } => {
                request.updates.tool_result(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::CommandOutputDelta { update } => {
                request.updates.command_output_delta(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::DynamicToolCallRequested { request: dynamic } => {
                request.updates.dynamic_tool_call_requested(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    dynamic,
                );
            }
            RuntimeTurnOutcome::ToolUserInputRequested {
                request: user_input,
            } => {
                request.updates.tool_user_input_requested(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    user_input,
                );
            }
            RuntimeTurnOutcome::FileChangeApprovalRequested {
                request: file_change,
            } => {
                request.updates.file_change_approval_requested(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    file_change,
                );
            }
            RuntimeTurnOutcome::PermissionsApprovalRequested {
                request: permissions,
            } => {
                request.updates.permissions_approval_requested(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    permissions,
                );
            }
            RuntimeTurnOutcome::FileChangeOutputDelta { update } => {
                request.updates.file_change_output_delta(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::FileChangePatchUpdated { update } => {
                request.updates.file_change_patch_updated(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::AutoApprovalReviewStarted { update } => {
                request.updates.auto_approval_review_started(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::AutoApprovalReviewCompleted { update } => {
                request.updates.auto_approval_review_completed(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    update,
                );
            }
            RuntimeTurnOutcome::TokenUsageUpdated { usage } => {
                request.updates.token_usage_updated(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    usage,
                );
            }
            RuntimeTurnOutcome::ModelRerouted {
                from_model,
                to_model,
                reason,
            } => {
                request.updates.model_rerouted(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    from_model,
                    to_model,
                    reason,
                );
            }
            RuntimeTurnOutcome::ModelVerification { verifications } => {
                request.updates.model_verification(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    verifications,
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
                request
                    .updates
                    .fail(request.thread_id.clone(), request.turn_id.clone(), error);
            }
        }
    }

    #[derive(Debug, Default)]
    struct ManualApprovalBridge {
        decisions: Mutex<Vec<(String, dasclaw_runtime::ApprovalDecision)>>,
        active: Mutex<Option<ManualActiveTurn>>,
    }

    #[derive(Debug, Clone)]
    struct ManualActiveTurn {
        thread_id: String,
        turn_id: String,
        updates: RuntimeTurnUpdateSink,
    }

    impl ManualApprovalBridge {
        fn decisions(&self) -> Vec<(String, dasclaw_runtime::ApprovalDecision)> {
            self.decisions.lock().expect("decisions lock").clone()
        }
    }

    impl RuntimeBridge for ManualApprovalBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            let active = ManualActiveTurn {
                thread_id: request.thread_id.clone(),
                turn_id: request.turn_id.clone(),
                updates: request.updates.clone(),
            };
            *self.active.lock().expect("active lock") = Some(active);
            request.updates.approval_requested(
                request.thread_id,
                request.turn_id,
                RuntimeApprovalRequest {
                    request_id: "00000000-0000-0000-0000-000000000001".to_string(),
                    tool_call_id: "tool_1".to_string(),
                    tool_name: "shell".to_string(),
                    command: Some("echo ok".to_string()),
                    description: "Run shell command".to_string(),
                    display_parameters: serde_json::json!({"cmd":"echo ok"}),
                    allow_always: true,
                },
            );
            Ok(())
        }

        fn cancel_turn(
            &self,
            _request: RuntimeTurnCancelRequest,
        ) -> Result<(), RuntimeBridgeError> {
            Ok(())
        }

        fn resolve_approval(
            &self,
            decision: RuntimeApprovalDecision,
        ) -> Result<(), RuntimeBridgeError> {
            let request_id = decision.request_id.clone();
            let runtime_decision = decision.decision.clone();
            self.decisions
                .lock()
                .expect("decisions lock")
                .push((request_id, runtime_decision.clone()));
            let active = self.active.lock().expect("active lock").clone();
            if let Some(active) = active {
                match runtime_decision {
                    dasclaw_runtime::ApprovalDecision::Approve
                    | dasclaw_runtime::ApprovalDecision::ApproveAlways => {
                        active.updates.tool_result(
                            active.thread_id.clone(),
                            active.turn_id.clone(),
                            RuntimeToolResultUpdate {
                                item_id: format!("{}:tool:tool_1", active.turn_id),
                                content: "hello from sandbox".to_string(),
                                is_error: false,
                            },
                        );
                        active.updates.complete(
                            active.thread_id,
                            active.turn_id,
                            "done".to_string(),
                        );
                    }
                    dasclaw_runtime::ApprovalDecision::Reject { reason } => {
                        active.updates.fail(
                            active.thread_id,
                            active.turn_id,
                            reason.unwrap_or_else(|| "approval rejected".to_string()),
                        );
                    }
                }
            }
            Ok(())
        }

        fn shutdown(&self) {
            *self.active.lock().expect("active lock") = None;
        }
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
