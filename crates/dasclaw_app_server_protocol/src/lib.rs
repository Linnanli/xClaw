//! Protocol types for the dasclaw local app-server.
//!
//! This crate intentionally models the GUI control-plane boundary only. It
//! does not define agent-loop internals or tool-execution traits; those stay
//! in `dasclaw_runtime`.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_MAJOR: u16 = 0;
pub const PROTOCOL_MINOR: u16 = 1;
pub const PROTOCOL_PATCH: u16 = 0;
pub const JSON_RPC_VERSION: &str = "2.0";

pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const PROTOCOL_SCHEMA: &str = "protocol/schema";
    pub const HEALTH_CHECK: &str = "health/check";
    pub const CAPABILITIES_LIST: &str = "capabilities/list";
    pub const LIFECYCLE_STATUS: &str = "lifecycle/status";
    pub const SHUTDOWN: &str = "shutdown";
    pub const THREAD_START: &str = "thread/start";
    pub const THREAD_LIST: &str = "thread/list";
    pub const THREAD_READ: &str = "thread/read";
    pub const THREAD_TURNS_LIST: &str = "thread/turns/list";
    pub const TURN_START: &str = "turn/start";
    pub const TURN_INTERRUPT: &str = "turn/interrupt";
    pub const TURN_READ: &str = "turn/read";
    pub const MODEL_LIST: &str = "model/list";
    pub const MODEL_PROVIDER_SELECT_FOR_NEXT_TURN: &str = "modelProvider/selectForNextTurn";
    pub const APPROVAL_RESPOND: &str = "approval/respond";
}

pub mod server_request {
    pub const ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: &str =
        "item/commandExecution/requestApproval";
    pub const ITEM_FILE_CHANGE_REQUEST_APPROVAL: &str = "item/fileChange/requestApproval";
    pub const ITEM_PERMISSIONS_REQUEST_APPROVAL: &str = "item/permissions/requestApproval";
    pub const ITEM_TOOL_REQUEST_USER_INPUT: &str = "item/tool/requestUserInput";
    pub const ITEM_TOOL_CALL: &str = "item/tool/call";
}

pub mod event {
    pub const NOTIFICATIONS_INITIALIZED: &str = "notifications/initialized";
    pub const LIFECYCLE_CHANGED: &str = "lifecycle/changed";
    pub const HEALTH_CHANGED: &str = "health/changed";
    pub const CAPABILITIES_CHANGED: &str = "capabilities/changed";
    pub const LOG_ENTRY: &str = "log/entry";
    pub const THREAD_STARTED: &str = "thread/started";
    pub const TURN_STARTED: &str = "turn/started";
    pub const TURN_COMPLETED: &str = "turn/completed";
    pub const ITEM_STARTED: &str = "item/started";
    pub const ITEM_AGENT_MESSAGE_DELTA: &str = "item/agentMessage/delta";
    pub const ITEM_REASONING_SUMMARY_TEXT_DELTA: &str = "item/reasoning/summaryTextDelta";
    pub const ITEM_REASONING_SUMMARY_PART_ADDED: &str = "item/reasoning/summaryPartAdded";
    pub const ITEM_REASONING_TEXT_DELTA: &str = "item/reasoning/textDelta";
    pub const ITEM_COMPLETED: &str = "item/completed";
    pub const ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: &str =
        "item/commandExecution/requestApproval";
    pub const ITEM_COMMAND_EXECUTION_APPROVAL_SUBMITTED: &str =
        "item/commandExecution/approvalSubmitted";
    pub const SERVER_REQUEST_RESOLVED: &str = "serverRequest/resolved";
    pub const ITEM_AUTO_APPROVAL_REVIEW_STARTED: &str = "item/autoApprovalReview/started";
    pub const ITEM_AUTO_APPROVAL_REVIEW_COMPLETED: &str = "item/autoApprovalReview/completed";
    pub const ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: &str = "item/commandExecution/outputDelta";
    pub const ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION: &str =
        "item/commandExecution/terminalInteraction";
    pub const ERROR: &str = "error";
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    #[must_use]
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn ok(
        id: Option<serde_json::Value>,
        result: impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    #[must_use]
    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcClientResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcClientResponse {
    pub fn ok(id: serde_json::Value, result: impl Serialize) -> Result<Self, serde_json::Error> {
        Ok(Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcServerRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcServerRequest {
    pub fn new(
        id: impl Serialize,
        method: impl Into<String>,
        params: impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: serde_json::to_value(id)?,
            method: method.into(),
            params: Some(serde_json::to_value(params)?),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcIncoming {
    Request(JsonRpcRequest),
    ClientResponse(JsonRpcClientResponse),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ErrorData>,
}

impl JsonRpcError {
    #[must_use]
    pub fn new(code: i64, message: impl Into<String>, data: Option<ErrorData>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ProtocolVersion {
    #[must_use]
    pub const fn current() -> Self {
        Self {
            major: PROTOCOL_MAJOR,
            minor: PROTOCOL_MINOR,
            patch: PROTOCOL_PATCH,
        }
    }

    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
    pub transport: TransportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Stdio,
    UnixSocket,
    NamedPipe,
    HttpWs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: ProtocolVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client: ClientInfo,
    pub protocol_version: ProtocolVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requested_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<ModelProviderInitializeConfig>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientModelConfig {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_call_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

impl fmt::Debug for ClientModelConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientModelConfig")
            .field("model_id", &self.model_id)
            .field("display_name", &self.display_name)
            .field("provider", &self.provider)
            .field("api_base_url", &self.api_base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("api_format", &self.api_format)
            .field("model_call_mode", &self.model_call_mode)
            .field("source", &self.source)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderInitializeConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ClientModelConfig>,
    pub selected_model: ClientModelConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderSelectForNextTurnParams {
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderSelectForNextTurnResponse {
    pub selected_model_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_hidden: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListResponse {
    pub data: Vec<CodexModel>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModel {
    pub id: String,
    pub model: String,
    pub upgrade: Option<String>,
    pub upgrade_info: Option<ModelUpgradeInfo>,
    pub availability_nux: Option<ModelAvailabilityNux>,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub supported_reasoning_efforts: Vec<CodexReasoningEffortOption>,
    pub default_reasoning_effort: CodexReasoningEffort,
    pub input_modalities: Vec<CodexInputModality>,
    pub supports_personality: bool,
    pub additional_speed_tiers: Vec<String>,
    pub is_default: bool,
}

impl CodexModel {
    #[must_use]
    pub fn from_client_model(model: &ClientModelConfig, is_default: bool) -> Self {
        let display_name = model
            .display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| model.model_id.clone());
        Self {
            id: model.model_id.clone(),
            model: model.model_id.clone(),
            upgrade: None,
            upgrade_info: None,
            availability_nux: None,
            display_name,
            description: String::new(),
            hidden: false,
            supported_reasoning_efforts: vec![CodexReasoningEffortOption {
                reasoning_effort: CodexReasoningEffort::None,
                description: "No reasoning effort override".to_string(),
            }],
            default_reasoning_effort: CodexReasoningEffort::None,
            input_modalities: vec![CodexInputModality::Text],
            supports_personality: false,
            additional_speed_tiers: Vec::new(),
            is_default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexInputModality {
    Text,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexReasoningEffortOption {
    pub reasoning_effort: CodexReasoningEffort,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAvailabilityNux {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUpgradeInfo {
    pub model: String,
    pub upgrade_copy: Option<String>,
    pub model_link: Option<String>,
    pub migration_markdown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    pub trust: WorkspaceTrust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceTrust {
    Trusted,
    Untrusted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub server: ServerInfo,
    pub lifecycle: LifecycleSnapshot,
    pub capabilities: CapabilityMatrix,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility_profiles: Vec<CompatibilityProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable_requested_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Starting,
    Initializing,
    Ready,
    Running,
    AwaitingApproval,
    Degraded,
    Failed,
    Restarting,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleReason {
    ProcessStarted,
    InitializeRequested,
    RuntimeReady,
    RequestInProgress,
    ApprovalPending,
    PolicyDegraded,
    DlpDegraded,
    ProviderDegraded,
    VersionMismatch,
    ShutdownRequested,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleSnapshot {
    pub state: LifecycleState,
    pub reason: LifecycleReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub since: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_services: Vec<ServiceHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Implemented,
    Declared,
    Disabled,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub id: String,
    pub status: CapabilityStatus,
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Capability {
    #[must_use]
    pub fn implemented(id: impl Into<String>, methods: &[&str], events: &[&str]) -> Self {
        Self::new(id, CapabilityStatus::Implemented, methods, events, None)
    }

    #[must_use]
    pub fn declared(id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            id,
            CapabilityStatus::Declared,
            &[],
            &[],
            Some(reason.into()),
        )
    }

    #[must_use]
    pub fn declared_contract(
        id: impl Into<String>,
        methods: &[&str],
        events: &[&str],
        reason: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            CapabilityStatus::Declared,
            methods,
            events,
            Some(reason.into()),
        )
    }

    fn new(
        id: impl Into<String>,
        status: CapabilityStatus,
        methods: &[&str],
        events: &[&str],
        reason: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            status,
            version: "0.1.0".to_string(),
            methods: methods.iter().map(|method| (*method).to_string()).collect(),
            events: events.iter().map(|event| (*event).to_string()).collect(),
            reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
}

impl CapabilityMatrix {
    #[must_use]
    pub fn phase_one() -> Self {
        Self {
            protocol: Capability::implemented(
                "protocol",
                &[method::INITIALIZE, method::PROTOCOL_SCHEMA],
                &[event::NOTIFICATIONS_INITIALIZED],
            ),
            lifecycle: Capability::implemented(
                "lifecycle",
                &[method::LIFECYCLE_STATUS, method::SHUTDOWN],
                &[event::LIFECYCLE_CHANGED],
            ),
            health: Capability::implemented(
                "health",
                &[method::HEALTH_CHECK, method::CAPABILITIES_LIST],
                &[event::HEALTH_CHANGED, event::CAPABILITIES_CHANGED],
            ),
            logs: declared_future_capability("logs"),
            session: Capability::implemented(
                "session",
                &[
                    method::THREAD_START,
                    method::THREAD_LIST,
                    method::THREAD_READ,
                    method::THREAD_TURNS_LIST,
                    method::TURN_START,
                    method::TURN_INTERRUPT,
                    method::TURN_READ,
                ],
                &[
                    event::THREAD_STARTED,
                    event::TURN_STARTED,
                    event::TURN_COMPLETED,
                    event::ITEM_STARTED,
                    event::ITEM_AGENT_MESSAGE_DELTA,
                    event::ITEM_REASONING_SUMMARY_TEXT_DELTA,
                    event::ITEM_REASONING_SUMMARY_PART_ADDED,
                    event::ITEM_REASONING_TEXT_DELTA,
                    event::ITEM_COMPLETED,
                    event::ERROR,
                ],
            ),
            approval: declared_future_capability("approval"),
            dlp_policy: declared_future_capability("dlp_policy"),
            model_provider: Capability::implemented(
                "model_provider",
                &[
                    method::MODEL_LIST,
                    method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN,
                ],
                &[],
            ),
            tools: declared_future_capability("tools"),
            jobs: declared_future_capability("jobs"),
            skills: declared_future_capability("skills"),
            mcp: declared_future_capability("mcp"),
            sandbox: declared_future_capability("sandbox"),
        }
    }

    #[must_use]
    pub fn with_p3_approval_tool_sandbox(mut self) -> Self {
        self.approval = Capability::implemented(
            "approval",
            &[method::APPROVAL_RESPOND],
            &[
                server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
                event::SERVER_REQUEST_RESOLVED,
            ],
        );
        self.tools = Capability::implemented(
            "tools",
            &[],
            &[
                event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
                event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
            ],
        );
        self.sandbox = Capability::new(
            "sandbox",
            CapabilityStatus::Implemented,
            &[],
            &[],
            Some("runtime bridge reports sandbox-ready execution".to_string()),
        );
        self
    }
}

fn declared_future_capability(id: &'static str) -> Capability {
    Capability::declared(
        id,
        "declared for protocol compatibility; not implemented in Phase 1",
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolSchemaResponse {
    pub protocol_version: ProtocolVersion,
    pub methods: Vec<MethodSchema>,
    pub events: Vec<EventSchema>,
    pub capabilities: CapabilityMatrix,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility_profiles: Vec<CompatibilityProfile>,
}

impl ProtocolSchemaResponse {
    #[must_use]
    pub fn phase_one(capabilities: CapabilityMatrix) -> Self {
        Self {
            protocol_version: ProtocolVersion::current(),
            methods: phase_one_methods(),
            events: phase_one_events(),
            capabilities,
            compatibility_profiles: vec![
                CompatibilityProfile::codex_app_server_v2_chat_session_subset(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityProfile {
    pub id: String,
    pub version: String,
    pub scope: CompatibilityProfileScope,
    pub description: String,
    pub methods: Vec<String>,
    pub events: Vec<String>,
    pub aliases: Vec<CompatibilityAlias>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_opt_outs: Vec<CapabilityOptOut>,
    pub event_queue: NotificationQueuePolicy,
}

impl CompatibilityProfile {
    pub const CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID: &'static str =
        "codex_app_server_v2_chat_session_subset";

    #[must_use]
    pub fn codex_app_server_v2_chat_session_subset() -> Self {
        Self {
            id: Self::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID.to_string(),
            version: "2.0.0-chat-session-subset".to_string(),
            scope: CompatibilityProfileScope::ChatSessionSubset,
            description: "Dasclaw native app-server protocol shaped as a truthful Codex app-server v2 chat-session subset".to_string(),
            methods: vec![
                method::INITIALIZE.to_string(),
                method::THREAD_START.to_string(),
                method::THREAD_READ.to_string(),
                method::THREAD_LIST.to_string(),
                method::THREAD_TURNS_LIST.to_string(),
                method::TURN_START.to_string(),
                method::TURN_INTERRUPT.to_string(),
                method::TURN_READ.to_string(),
                method::MODEL_LIST.to_string(),
            ],
            events: CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS
                .iter()
                .map(|event| (*event).to_string())
                .collect(),
            aliases: Vec::new(),
            capability_opt_outs: vec![
                CapabilityOptOut::phase_one("codex.rich_input"),
                CapabilityOptOut::phase_one("codex.tool_calls"),
                CapabilityOptOut::phase_one("codex.diff"),
                CapabilityOptOut::phase_one("codex.plan"),
                CapabilityOptOut::phase_one("tools"),
                CapabilityOptOut::phase_one("mcp"),
                CapabilityOptOut::phase_one("skills"),
                CapabilityOptOut::phase_one("dlp_policy"),
                CapabilityOptOut::phase_one("jobs"),
                CapabilityOptOut::phase_one("sandbox"),
                CapabilityOptOut::phase_one("thread.fork"),
                CapabilityOptOut::phase_one("thread.archive"),
                CapabilityOptOut::phase_one("thread.resume"),
                CapabilityOptOut::phase_one("thread.compact"),
                CapabilityOptOut::phase_one("thread.rollback"),
            ],
            event_queue: NotificationQueuePolicy::bounded_lag_disconnect(),
        }
    }
}

const CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS: &[&str] = &[
    event::NOTIFICATIONS_INITIALIZED,
    event::LIFECYCLE_CHANGED,
    event::CAPABILITIES_CHANGED,
    event::THREAD_STARTED,
    event::TURN_STARTED,
    event::TURN_COMPLETED,
    event::ITEM_STARTED,
    event::ITEM_AGENT_MESSAGE_DELTA,
    event::ITEM_REASONING_SUMMARY_TEXT_DELTA,
    event::ITEM_REASONING_SUMMARY_PART_ADDED,
    event::ITEM_REASONING_TEXT_DELTA,
    event::ITEM_COMPLETED,
    event::ERROR,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityProfileScope {
    ChatSessionSubset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityAlias {
    pub legacy: String,
    pub compatible: String,
    pub kind: CompatibilityAliasKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityAliasKind {
    Alias,
    DeprecatedHelper,
    LegacySmokeSurface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityOptOut {
    pub capability: String,
    pub reason: String,
}

impl CapabilityOptOut {
    fn phase_one(capability: &'static str) -> Self {
        Self {
            capability: capability.to_string(),
            reason: "phase_1_chat_session_subset".to_string(),
        }
    }
}

pub const DEFAULT_MAX_PENDING_NOTIFICATIONS: u16 = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationQueuePolicy {
    pub max_pending_notifications: u16,
    pub overflow: NotificationQueueOverflowPolicy,
}

impl NotificationQueuePolicy {
    #[must_use]
    pub fn bounded_lag_disconnect() -> Self {
        Self {
            max_pending_notifications: DEFAULT_MAX_PENDING_NOTIFICATIONS,
            overflow: NotificationQueueOverflowPolicy::LagDisconnect,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationQueueOverflowPolicy {
    LagDisconnect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodSchema {
    pub method: String,
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_type: Option<String>,
    pub result_type: String,
    pub requires_initialize: bool,
}

impl MethodSchema {
    fn new(
        method: &'static str,
        capability: &'static str,
        params_type: Option<&'static str>,
        result_type: &'static str,
        requires_initialize: bool,
    ) -> Self {
        Self {
            method: method.to_string(),
            capability: capability.to_string(),
            params_type: params_type.map(str::to_string),
            result_type: result_type.to_string(),
            requires_initialize,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSchema {
    pub event: String,
    pub capability: String,
    pub payload_type: String,
}

impl EventSchema {
    fn new(event: &'static str, capability: &'static str, payload_type: &'static str) -> Self {
        Self {
            event: event.to_string(),
            capability: capability.to_string(),
            payload_type: payload_type.to_string(),
        }
    }
}

fn phase_one_methods() -> Vec<MethodSchema> {
    vec![
        MethodSchema::new(
            method::INITIALIZE,
            "protocol",
            Some("InitializeParams"),
            "InitializeResponse",
            false,
        ),
        MethodSchema::new(
            method::PROTOCOL_SCHEMA,
            "protocol",
            None,
            "ProtocolSchemaResponse",
            false,
        ),
        MethodSchema::new(
            method::HEALTH_CHECK,
            "health",
            Some("HealthCheckParams"),
            "HealthCheckResponse",
            false,
        ),
        MethodSchema::new(
            method::CAPABILITIES_LIST,
            "health",
            None,
            "CapabilitiesListResponse",
            false,
        ),
        MethodSchema::new(
            method::LIFECYCLE_STATUS,
            "lifecycle",
            None,
            "LifecycleStatusResponse",
            false,
        ),
        MethodSchema::new(
            method::SHUTDOWN,
            "lifecycle",
            Some("ShutdownParams"),
            "ShutdownResponse",
            false,
        ),
        MethodSchema::new(
            method::THREAD_START,
            "session",
            Some("ThreadStartParams"),
            "ThreadStartResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_LIST,
            "session",
            Some("ThreadListParams"),
            "ThreadListResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_READ,
            "session",
            Some("ThreadReadParams"),
            "ThreadReadResponse",
            true,
        ),
        MethodSchema::new(
            method::TURN_START,
            "session",
            Some("TurnStartParams"),
            "TurnStartResponse",
            true,
        ),
        MethodSchema::new(
            method::TURN_INTERRUPT,
            "session",
            Some("TurnInterruptParams"),
            "TurnInterruptResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_TURNS_LIST,
            "session",
            Some("ThreadTurnsListParams"),
            "ThreadTurnsListResponse",
            true,
        ),
        MethodSchema::new(
            method::TURN_READ,
            "session",
            Some("TurnReadParams"),
            "TurnReadResponse",
            true,
        ),
        MethodSchema::new(
            method::MODEL_LIST,
            "model_provider",
            Some("ModelListParams"),
            "ModelListResponse",
            true,
        ),
        MethodSchema::new(
            method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN,
            "model_provider",
            Some("ModelProviderSelectForNextTurnParams"),
            "ModelProviderSelectForNextTurnResponse",
            true,
        ),
        MethodSchema::new(
            method::APPROVAL_RESPOND,
            "approval",
            Some("ApprovalResponsePayload"),
            "ServerRequestResolvedEvent",
            true,
        ),
    ]
}

fn phase_one_events() -> Vec<EventSchema> {
    vec![
        EventSchema::new(
            event::NOTIFICATIONS_INITIALIZED,
            "protocol",
            "NotificationsInitializedEvent",
        ),
        EventSchema::new(
            event::LIFECYCLE_CHANGED,
            "lifecycle",
            "LifecycleChangedEvent",
        ),
        EventSchema::new(event::HEALTH_CHANGED, "health", "HealthChangedEvent"),
        EventSchema::new(
            event::CAPABILITIES_CHANGED,
            "health",
            "CapabilitiesChangedEvent",
        ),
        EventSchema::new(event::LOG_ENTRY, "logs", "LogEntryEvent"),
        EventSchema::new(event::THREAD_STARTED, "session", "ThreadStartedEvent"),
        EventSchema::new(event::TURN_STARTED, "session", "TurnStartedEvent"),
        EventSchema::new(event::TURN_COMPLETED, "session", "TurnCompletedEvent"),
        EventSchema::new(event::ITEM_STARTED, "session", "ItemStartedEvent"),
        EventSchema::new(
            event::ITEM_AGENT_MESSAGE_DELTA,
            "session",
            "AgentMessageDeltaEvent",
        ),
        EventSchema::new(
            event::ITEM_REASONING_SUMMARY_TEXT_DELTA,
            "session",
            "ReasoningSummaryTextDeltaEvent",
        ),
        EventSchema::new(
            event::ITEM_REASONING_SUMMARY_PART_ADDED,
            "session",
            "ReasoningSummaryPartAddedEvent",
        ),
        EventSchema::new(
            event::ITEM_REASONING_TEXT_DELTA,
            "session",
            "ReasoningTextDeltaEvent",
        ),
        EventSchema::new(event::ITEM_COMPLETED, "session", "ItemCompletedEvent"),
        EventSchema::new(
            event::SERVER_REQUEST_RESOLVED,
            "approval",
            "ServerRequestResolvedEvent",
        ),
        EventSchema::new(
            event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
            "tools",
            "CommandExecutionOutputDeltaEvent",
        ),
        EventSchema::new(
            event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
            "tools",
            "CommandExecutionTerminalInteractionEvent",
        ),
        EventSchema::new(event::ERROR, "session", "ErrorEvent"),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceName {
    Protocol,
    Lifecycle,
    Session,
    Runtime,
    DlpPolicy,
    ModelProvider,
    Tools,
    Sandbox,
    Jobs,
    Skills,
    Mcp,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Ready,
    Degraded,
    Disabled,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {
    pub service: ServiceName,
    pub status: ServiceStatus,
    pub fail_safe: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ServiceHealth {
    #[must_use]
    pub fn ready(service: ServiceName) -> Self {
        Self {
            service,
            status: ServiceStatus::Ready,
            fail_safe: false,
            message: None,
        }
    }

    #[must_use]
    pub fn disabled(service: ServiceName, message: impl Into<String>) -> Self {
        Self {
            service,
            status: ServiceStatus::Disabled,
            fail_safe: false,
            message: Some(message.into()),
        }
    }

    #[must_use]
    pub fn degraded(service: ServiceName, message: impl Into<String>) -> Self {
        Self {
            service,
            status: ServiceStatus::Degraded,
            fail_safe: false,
            message: Some(message.into()),
        }
    }

    #[must_use]
    pub fn unavailable_fail_safe(service: ServiceName, message: impl Into<String>) -> Self {
        Self {
            service,
            status: ServiceStatus::Unavailable,
            fail_safe: true,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckParams {
    #[serde(default)]
    pub include_details: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    pub ok: bool,
    pub lifecycle: LifecycleSnapshot,
    pub services: Vec<ServiceHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesListResponse {
    pub capabilities: CapabilityMatrix,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility_profiles: Vec<CompatibilityProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadStartParams {
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartResponse {
    pub thread: CodexThread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_direction: Option<SortDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResponse {
    pub data: Vec<CodexThread>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadResponse {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserInput {
    Text {
        text: String,
        #[serde(default)]
        text_elements: Vec<serde_json::Value>,
    },
    Image {
        url: String,
    },
}

impl TurnStartParams {
    #[must_use]
    pub fn prompt_text(&self) -> String {
        self.input
            .iter()
            .filter_map(|input| match input {
                UserInput::Text { text, .. } => Some(text.as_str()),
                UserInput::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Pending,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartResponse {
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurnsListParams {
    pub thread_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_direction: Option<SortDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurnsListResponse {
    pub data: Vec<CodexTurn>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnReadParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnReadResponse {
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThread {
    pub id: String,
    pub forked_from_id: Option<String>,
    pub preview: String,
    pub ephemeral: bool,
    pub model_provider: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: CodexThreadStatus,
    pub path: Option<String>,
    pub cwd: String,
    pub cli_version: String,
    pub source: CodexSessionSource,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub git_info: Option<CodexGitInfo>,
    pub name: Option<String>,
    pub turns: Vec<CodexTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurn {
    pub id: String,
    pub items: Vec<CodexThreadItem>,
    pub status: CodexTurnStatus,
    pub error: Option<CodexTurnError>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
}

impl CodexTurn {
    #[must_use]
    pub fn in_progress(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            status: CodexTurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexThreadItem {
    #[serde(rename_all = "camelCase")]
    AgentMessage { id: String, text: String },
    #[serde(rename_all = "camelCase")]
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<String>,
        #[serde(default)]
        content: Vec<String>,
    },
}

impl CodexThreadItem {
    #[must_use]
    pub fn started_agent_message(id: impl Into<String>) -> Self {
        Self::AgentMessage {
            id: id.into(),
            text: String::new(),
        }
    }

    #[must_use]
    pub fn completed_agent_message(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::AgentMessage {
            id: id.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTurnError {
    pub message: String,
    pub codex_error_info: Option<serde_json::Value>,
    pub additional_details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CodexThreadStatus {
    Idle,
    Active { active_flags: Vec<String> },
    SystemError,
    NotLoaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexTurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexSessionSource {
    AppServer,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexGitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub description: String,
    pub display_parameters: serde_json::Value,
    pub allow_always: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub description: String,
    pub display_parameters: serde_json::Value,
    pub allow_always: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AppServerApprovalDecision {
    Approve,
    Reject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    ApproveAlways,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalResponsePayload {
    pub decision: AppServerApprovalDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerRequestResolutionOutcome {
    Approved,
    Rejected,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRequestResolvedEvent {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub outcome: ServerRequestResolutionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionOutputDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionTerminalInteractionEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub message: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartedEvent {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartedEvent {
    pub thread_id: String,
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedEvent {
    pub thread_id: String,
    pub turn: CodexTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item: CodexThreadItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningSummaryTextDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub summary_index: i64,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningSummaryPartAddedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub summary_index: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningTextDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub content_index: i64,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item: CodexThreadItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleStatusResponse {
    pub lifecycle: LifecycleSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownReason {
    ClientExit,
    Restart,
    UserRequested,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ShutdownReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownResponse {
    pub accepted: bool,
    pub lifecycle: LifecycleSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleChangedEvent {
    pub lifecycle: LifecycleSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state: Option<LifecycleState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthChangedEvent {
    pub ok: bool,
    pub services: Vec<ServiceHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitiesChangedReason {
    Initialize,
    ConfigChanged,
    ServiceDegraded,
    ServiceRecovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesChangedEvent {
    pub capabilities: CapabilityMatrix,
    pub reason: CapabilitiesChangedReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsInitializedEvent {
    pub lifecycle: LifecycleSnapshot,
    pub compatibility_profiles: Vec<CompatibilityProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable_requested_capabilities: Vec<String>,
    pub event_queue: NotificationQueuePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

impl ServerNotification {
    pub fn notifications_initialized(
        event: NotificationsInitializedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::NOTIFICATIONS_INITIALIZED, event)
    }

    pub fn lifecycle_changed(event: LifecycleChangedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::LIFECYCLE_CHANGED, event)
    }

    pub fn health_changed(event: HealthChangedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::HEALTH_CHANGED, event)
    }

    pub fn capabilities_changed(
        event: CapabilitiesChangedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::CAPABILITIES_CHANGED, event)
    }

    pub fn log_entry(event: LogEntryEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::LOG_ENTRY, event)
    }

    pub fn thread_started(event: ThreadStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_STARTED, event)
    }

    pub fn turn_started(event: TurnStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_STARTED, event)
    }

    pub fn turn_completed(event: TurnCompletedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_COMPLETED, event)
    }

    pub fn item_started(event: ItemStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_STARTED, event)
    }

    pub fn agent_message_delta(event: AgentMessageDeltaEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_AGENT_MESSAGE_DELTA, event)
    }

    pub fn reasoning_summary_text_delta(
        event: ReasoningSummaryTextDeltaEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_REASONING_SUMMARY_TEXT_DELTA, event)
    }

    pub fn reasoning_summary_part_added(
        event: ReasoningSummaryPartAddedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_REASONING_SUMMARY_PART_ADDED, event)
    }

    pub fn reasoning_text_delta(event: ReasoningTextDeltaEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_REASONING_TEXT_DELTA, event)
    }

    pub fn item_completed(event: ItemCompletedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_COMPLETED, event)
    }

    pub fn server_request_resolved(
        event: ServerRequestResolvedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::SERVER_REQUEST_RESOLVED, event)
    }

    pub fn command_execution_output_delta(
        event: CommandExecutionOutputDeltaEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA, event)
    }

    pub fn command_execution_terminal_interaction(
        event: CommandExecutionTerminalInteractionEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION, event)
    }

    pub fn error(event: ErrorEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ERROR, event)
    }

    fn new(method: impl Into<String>, params: impl Serialize) -> Result<Self, serde_json::Error> {
        Ok(Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            method: method.into(),
            params: serde_json::to_value(params)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntryEvent {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub time: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    #[error("version mismatch")]
    VersionMismatch,
    #[error("unknown method")]
    UnknownMethod,
    #[error("capability unavailable")]
    CapabilityUnavailable,
    #[error("invalid params")]
    InvalidParams,
    #[error("not initialized")]
    NotInitialized,
    #[error("operation in progress")]
    OperationInProgress,
    #[error("service degraded")]
    ServiceDegraded,
    #[error("notification queue overflow")]
    NotificationQueueOverflow,
    #[error("internal error")]
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_versions_match_by_major() {
        assert!(
            ProtocolVersion::current().is_compatible_with(ProtocolVersion {
                major: 0,
                minor: 99,
                patch: 99,
            })
        );
        assert!(
            !ProtocolVersion::current().is_compatible_with(ProtocolVersion {
                major: 1,
                minor: 0,
                patch: 0,
            })
        );
    }

    fn codex_thread_fixture() -> CodexThread {
        CodexThread {
            id: "thread_1".to_string(),
            forked_from_id: None,
            preview: "hello".to_string(),
            ephemeral: false,
            model_provider: "openai".to_string(),
            created_at: 1,
            updated_at: 2,
            status: CodexThreadStatus::Idle,
            path: None,
            cwd: "/workspace".to_string(),
            cli_version: "0.0.0".to_string(),
            source: CodexSessionSource::AppServer,
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: None,
            turns: vec![CodexTurn::in_progress("turn_1")],
        }
    }

    #[test]
    fn thread_and_turn_responses_are_v2_shaped_without_legacy_id_aliases() {
        let thread_response = ThreadStartResponse {
            thread: codex_thread_fixture(),
            model: "gpt-5".to_string(),
            model_provider: "openai".to_string(),
            cwd: "/workspace".to_string(),
        };
        let turn_response = TurnStartResponse {
            turn: CodexTurn::in_progress("turn_1"),
        };

        let thread_value =
            serde_json::to_value(thread_response).expect("thread/start response should serialize");
        let turn_value =
            serde_json::to_value(turn_response).expect("turn/start response should serialize");

        assert_eq!(thread_value["thread"]["id"], "thread_1");
        assert_eq!(thread_value["thread"]["status"]["type"], "idle");
        assert_eq!(thread_value["thread"]["source"], "appServer");
        assert_eq!(
            serde_json::to_value(CodexThreadStatus::Active {
                active_flags: Vec::new(),
            })
            .expect("active thread status should serialize"),
            serde_json::json!({
                "type": "active",
                "activeFlags": [],
            })
        );
        assert_eq!(thread_value["model"], "gpt-5");
        assert_eq!(thread_value["modelProvider"], "openai");
        assert_eq!(thread_value["cwd"], "/workspace");
        assert!(thread_value.get("threadId").is_none());
        assert!(thread_value.get("lifecycle").is_none());

        assert_eq!(turn_value["turn"]["id"], "turn_1");
        assert_eq!(turn_value["turn"]["status"], "inProgress");
        assert!(turn_value.get("turnId").is_none());
        assert!(turn_value.get("status").is_none());
        assert!(turn_value.get("lifecycle").is_none());
    }

    #[test]
    fn phase_one_matrix_only_implements_minimal_surface() {
        let matrix = CapabilityMatrix::phase_one();

        assert_eq!(matrix.protocol.status, CapabilityStatus::Implemented);
        assert!(
            matrix
                .protocol
                .methods
                .contains(&method::PROTOCOL_SCHEMA.to_string())
        );
        assert_eq!(matrix.lifecycle.status, CapabilityStatus::Implemented);
        assert_eq!(matrix.health.status, CapabilityStatus::Implemented);
        assert_eq!(matrix.logs.status, CapabilityStatus::Declared);
        assert_eq!(matrix.session.status, CapabilityStatus::Implemented);
        assert!(
            matrix
                .session
                .methods
                .contains(&method::THREAD_START.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::THREAD_LIST.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::THREAD_READ.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::THREAD_TURNS_LIST.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::TURN_START.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::TURN_INTERRUPT.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::TURN_READ.to_string())
        );
        assert!(
            matrix
                .session
                .events
                .contains(&event::TURN_COMPLETED.to_string())
        );
        assert_eq!(matrix.approval.status, CapabilityStatus::Declared);
        assert_eq!(matrix.dlp_policy.status, CapabilityStatus::Declared);
        assert_eq!(matrix.model_provider.status, CapabilityStatus::Implemented);
        assert!(
            matrix
                .model_provider
                .methods
                .contains(&method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN.to_string())
        );
    }

    #[test]
    fn phase_one_schema_describes_routable_protocol_surface() {
        let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
        let method_names = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>();
        let event_names = schema
            .events
            .iter()
            .map(|event| event.event.as_str())
            .collect::<Vec<_>>();

        assert_eq!(schema.protocol_version, ProtocolVersion::current());
        assert!(method_names.contains(&method::INITIALIZE));
        assert!(method_names.contains(&method::PROTOCOL_SCHEMA));
        assert!(method_names.contains(&method::HEALTH_CHECK));
        assert!(method_names.contains(&method::THREAD_START));
        assert!(method_names.contains(&method::THREAD_LIST));
        assert!(method_names.contains(&method::THREAD_READ));
        assert!(method_names.contains(&method::THREAD_TURNS_LIST));
        assert!(method_names.contains(&method::TURN_START));
        assert!(method_names.contains(&method::TURN_INTERRUPT));
        assert!(method_names.contains(&method::TURN_READ));
        assert!(method_names.contains(&method::MODEL_LIST));
        assert!(method_names.contains(&method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN));

        assert!(!method_names.contains(&"thread/create"));
        assert!(!method_names.contains(&"turn/cancel"));
        assert!(!method_names.contains(&"turns/list"));
        assert!(!method_names.contains(&"turn/list"));

        assert!(event_names.contains(&event::LIFECYCLE_CHANGED));
        assert!(event_names.contains(&event::CAPABILITIES_CHANGED));
        assert!(event_names.contains(&event::THREAD_STARTED));
        assert!(event_names.contains(&event::TURN_STARTED));
        assert!(event_names.contains(&event::TURN_COMPLETED));
        assert!(event_names.contains(&event::ITEM_STARTED));
        assert!(event_names.contains(&event::ITEM_AGENT_MESSAGE_DELTA));
        assert!(event_names.contains(&event::ITEM_COMPLETED));

        assert!(!event_names.contains(&"thread/created"));
        assert!(!event_names.contains(&"turn/delta"));
        assert!(!event_names.contains(&"turn/failed"));
        assert!(!event_names.contains(&"turn/cancelled"));
        assert_eq!(schema.capabilities.logs.status, CapabilityStatus::Declared);
    }

    #[test]
    fn compatibility_profile_is_descriptive_subset_without_aliases() {
        let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
        let profile = schema
            .compatibility_profiles
            .iter()
            .find(|profile| {
                profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID
            })
            .expect("schema should describe the Codex v2-shaped chat-session subset");

        assert_eq!(profile.scope, CompatibilityProfileScope::ChatSessionSubset);
        assert!(profile.aliases.is_empty());
        assert!(profile.methods.contains(&method::THREAD_START.to_string()));
        assert!(profile.methods.contains(&method::THREAD_LIST.to_string()));
        assert!(
            profile
                .methods
                .contains(&method::THREAD_TURNS_LIST.to_string())
        );
        assert!(profile.methods.contains(&method::TURN_START.to_string()));
        assert!(
            profile
                .methods
                .contains(&method::TURN_INTERRUPT.to_string())
        );
        assert!(!profile.methods.contains(&"thread/create".to_string()));
        assert!(!profile.methods.contains(&"turn/cancel".to_string()));
        assert!(!profile.methods.contains(&"turns/list".to_string()));
        assert!(!profile.methods.contains(&"turn/list".to_string()));
        assert!(profile.events.contains(&event::THREAD_STARTED.to_string()));
        assert!(profile.events.contains(&event::TURN_COMPLETED.to_string()));
        assert!(!profile.events.contains(&"turn/delta".to_string()));
    }

    #[test]
    fn interrupt_and_list_responses_match_codex_v2_shapes() {
        let interrupt_response = TurnInterruptResponse {};
        let thread_list_response = ThreadListResponse {
            data: vec![codex_thread_fixture()],
            next_cursor: None,
            backwards_cursor: None,
        };
        let turn_list_response = ThreadTurnsListResponse {
            data: vec![CodexTurn::in_progress("turn_1")],
            next_cursor: None,
            backwards_cursor: None,
        };

        let interrupt_value = serde_json::to_value(interrupt_response)
            .expect("turn/interrupt response should serialize");
        let thread_list_value = serde_json::to_value(thread_list_response)
            .expect("thread/list response should serialize");
        let turn_list_value = serde_json::to_value(turn_list_response)
            .expect("thread/turns/list response should serialize");

        assert_eq!(interrupt_value, serde_json::json!({}));
        assert!(interrupt_value.get("turn").is_none());
        assert_eq!(thread_list_value["data"][0]["id"], "thread_1");
        assert!(thread_list_value["nextCursor"].is_null());
        assert!(thread_list_value["backwardsCursor"].is_null());
        assert!(thread_list_value.get("threads").is_none());
        assert_eq!(turn_list_value["data"][0]["id"], "turn_1");
        assert!(turn_list_value["nextCursor"].is_null());
        assert!(turn_list_value["backwardsCursor"].is_null());
        assert!(turn_list_value.get("turns").is_none());
    }

    #[test]
    fn phase_one_schema_methods_match_implemented_capability_methods() {
        let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
        let schema_methods = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>();
        let implemented_methods = [
            schema.capabilities.protocol.methods,
            schema.capabilities.lifecycle.methods,
            schema.capabilities.health.methods,
            schema.capabilities.session.methods,
            schema.capabilities.model_provider.methods,
        ]
        .concat();

        for method in implemented_methods {
            assert!(
                schema_methods.contains(&method.as_str()),
                "implemented capability method must be described by protocol/schema: {method}"
            );
        }
    }

    #[test]
    fn phase_one_schema_method_and_event_names_are_unique() {
        let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
        let mut methods = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>();
        let mut events = schema
            .events
            .iter()
            .map(|event| event.event.as_str())
            .collect::<Vec<_>>();

        methods.sort_unstable();
        events.sort_unstable();

        assert!(
            methods.windows(2).all(|pair| pair[0] != pair[1]),
            "protocol/schema method names must be unique"
        );
        assert!(
            events.windows(2).all(|pair| pair[0] != pair[1]),
            "protocol/schema event names must be unique"
        );
    }

    #[test]
    fn lifecycle_notification_uses_json_rpc_notification_shape() {
        let notification = ServerNotification::lifecycle_changed(LifecycleChangedEvent {
            lifecycle: LifecycleSnapshot {
                state: LifecycleState::Ready,
                reason: LifecycleReason::RuntimeReady,
                message: None,
                since: "0".to_string(),
                degraded_services: Vec::new(),
            },
            previous_state: Some(LifecycleState::Initializing),
        })
        .expect("test notification should serialize");

        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, "lifecycle/changed");
        assert_eq!(notification.params["lifecycle"]["state"], "ready");
    }

    #[test]
    fn json_rpc_request_without_id_is_notification() {
        let request: JsonRpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"shutdown"}"#)
                .expect("request should parse");

        assert!(request.is_notification());
    }

    #[test]
    fn json_rpc_response_serializes_standard_shape() {
        let response = JsonRpcResponse::error(
            Some(serde_json::json!("req_1")),
            JsonRpcError::new(
                -32601,
                "Method not found",
                Some(ErrorData {
                    code: ErrorCode::UnknownMethod,
                    message: "unknown method".to_string(),
                    lifecycle: None,
                    capability: None,
                    retryable: false,
                }),
            ),
        );
        let value = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(value["jsonrpc"], JSON_RPC_VERSION);
        assert_eq!(value["id"], "req_1");
        assert_eq!(value["error"]["data"]["code"], "UNKNOWN_METHOD");
    }

    #[test]
    fn json_rpc_incoming_distinguishes_requests_from_client_responses() {
        let request: JsonRpcIncoming = serde_json::from_value(serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": "x",
            "method": "health/check"
        }))
        .expect("request should deserialize as incoming JSON-RPC message");
        let response: JsonRpcIncoming = serde_json::from_value(serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": "approval_x",
            "result": {
                "decision": {"kind": "approve"}
            }
        }))
        .expect("client response should deserialize as incoming JSON-RPC message");

        match request {
            JsonRpcIncoming::Request(request) => {
                assert_eq!(request.method, "health/check");
            }
            JsonRpcIncoming::ClientResponse(_) => {
                panic!("method-bearing incoming message should be a request");
            }
        }

        match response {
            JsonRpcIncoming::ClientResponse(response) => {
                assert_eq!(response.id, serde_json::json!("approval_x"));
                assert!(response.result.is_some());
            }
            JsonRpcIncoming::Request(_) => {
                panic!("result-bearing incoming message should be a client response");
            }
        }
    }

    #[test]
    fn codex_v2_profile_declares_chat_session_subset_and_security_opt_outs() {
        fn contains_forbidden_p0_p2_domain(name: &str) -> bool {
            [
                "approval",
                "tool/",
                "command/",
                "fs/",
                "mcpServer/",
                "plugin/",
                "marketplace/",
            ]
            .iter()
            .any(|fragment| name.contains(fragment))
        }

        let profile = CompatibilityProfile::codex_app_server_v2_chat_session_subset();

        assert_eq!(
            profile.id,
            CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID
        );
        assert_eq!(profile.scope, CompatibilityProfileScope::ChatSessionSubset);
        assert!(profile.description.contains("chat-session subset"));
        assert_eq!(
            profile.methods,
            vec![
                method::INITIALIZE,
                method::THREAD_START,
                method::THREAD_READ,
                method::THREAD_LIST,
                method::THREAD_TURNS_LIST,
                method::TURN_START,
                method::TURN_INTERRUPT,
                method::TURN_READ,
                method::MODEL_LIST,
            ]
        );
        assert!(profile.capability_opt_outs.iter().any(|opt_out| {
            opt_out.capability == "codex.tool_calls"
                && opt_out.reason == "phase_1_chat_session_subset"
        }));
        assert!(profile.capability_opt_outs.iter().any(|opt_out| {
            opt_out.capability == "mcp" && opt_out.reason == "phase_1_chat_session_subset"
        }));
        assert!(profile.aliases.is_empty());
        assert!(
            !profile
                .methods
                .iter()
                .any(|method| contains_forbidden_p0_p2_domain(method))
        );
        assert!(
            !profile
                .events
                .iter()
                .any(|event| contains_forbidden_p0_p2_domain(event))
        );
    }

    #[test]
    fn model_provider_capability_advertises_codex_model_list_and_native_selection() {
        let matrix = CapabilityMatrix::phase_one();

        assert_eq!(matrix.model_provider.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.model_provider.methods,
            vec![
                method::MODEL_LIST.to_string(),
                method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN.to_string(),
            ]
        );
    }

    #[test]
    fn codex_model_list_response_serializes_without_provider_secret_fields() {
        let response = ModelListResponse {
            data: vec![CodexModel::from_client_model(
                &ClientModelConfig {
                    model_id: "gpt-test".to_string(),
                    display_name: Some("GPT Test".to_string()),
                    provider: Some("openai".to_string()),
                    api_base_url: Some("https://api.test/v1".to_string()),
                    api_key: Some("secret-key".to_string()),
                    api_format: Some("openai".to_string()),
                    model_call_mode: Some("stream".to_string()),
                    source: Some("test".to_string()),
                    capabilities: vec!["chat".to_string()],
                },
                true,
            )],
            next_cursor: None,
        };

        let value = serde_json::to_value(response).expect("model/list response should serialize");

        assert_eq!(value["data"][0]["id"], "gpt-test");
        assert_eq!(value["data"][0]["model"], "gpt-test");
        assert_eq!(value["data"][0]["displayName"], "GPT Test");
        assert_eq!(value["data"][0]["defaultReasoningEffort"], "none");
        assert_eq!(
            value["data"][0]["inputModalities"],
            serde_json::json!(["text"])
        );
        assert_eq!(value["data"][0]["isDefault"], true);
        assert!(!value.to_string().contains("secret-key"));
        assert!(value["data"][0].get("apiKey").is_none());
        assert!(value["data"][0].get("apiBaseUrl").is_none());
    }

    #[test]
    fn capabilities_response_can_advertise_compatibility_profiles() {
        let response = CapabilitiesListResponse {
            capabilities: CapabilityMatrix::phase_one(),
            compatibility_profiles: vec![
                CompatibilityProfile::codex_app_server_v2_chat_session_subset(),
            ],
        };
        let value = serde_json::to_value(response).expect("capabilities should serialize");

        assert_eq!(
            value["compatibilityProfiles"][0]["id"],
            CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID
        );
        assert_eq!(
            value["compatibilityProfiles"][0]["scope"],
            "chat_session_subset"
        );
    }

    #[test]
    fn turn_start_params_use_codex_v2_input_shape() {
        let params: TurnStartParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "input": [{"type": "text", "text": "hello", "text_elements": []}],
            "cwd": "/workspace",
            "model": "gpt-5",
            "summary": "concise"
        }))
        .expect("turn/start should accept Codex v2 input arrays");

        assert_eq!(params.prompt_text(), "hello");
        assert_eq!(params.cwd.as_deref(), Some("/workspace"));
        assert_eq!(params.model.as_deref(), Some("gpt-5"));
        assert_eq!(params.summary.as_deref(), Some("concise"));
        assert_eq!(
            serde_json::to_value(params).expect("turn/start params should serialize"),
            serde_json::json!({
                "threadId": "thread_1",
                "input": [{"type": "text", "text": "hello", "text_elements": []}],
                "cwd": "/workspace",
                "model": "gpt-5",
                "summary": "concise"
            })
        );
    }

    #[test]
    fn turn_start_params_accept_codex_v2_user_input_text_array() {
        let from_array: TurnStartParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "input": [
                {"type": "text", "text": "hello", "text_elements": []},
                {"type": "text", "text": "world", "text_elements": []}
            ]
        }))
        .expect("turn/start should accept Codex v2 UserInput text arrays");

        assert_eq!(from_array.prompt_text(), "hello\nworld");
    }

    #[test]
    fn turn_start_prompt_text_skips_codex_v2_image_inputs() {
        let mixed = serde_json::from_value::<TurnStartParams>(serde_json::json!({
            "threadId": "thread_1",
            "input": [
                {"type": "text", "text": "hello", "text_elements": []},
                {"type": "image", "url": "file:///tmp/image.png"},
                {"type": "text", "text": "world", "text_elements": []}
            ]
        }))
        .expect("turn/start should deserialize text and image inputs");

        assert_eq!(mixed.prompt_text(), "hello\nworld");
    }

    #[test]
    fn turn_start_prompt_text_is_empty_without_text_inputs() {
        let image_only = serde_json::from_value::<TurnStartParams>(serde_json::json!({
            "threadId": "thread_1",
            "input": [
                {"type": "image", "url": "file:///tmp/image.png"}
            ]
        }))
        .expect("turn/start should deserialize the declared UserInput subset");

        assert_eq!(image_only.prompt_text(), "");
    }

    #[test]
    fn codex_v2_contract_fixtures_use_json_rpc_and_item_event_shapes() {
        let requests = vec![
            JsonRpcRequest {
                jsonrpc: JSON_RPC_VERSION.to_string(),
                id: Some(serde_json::json!("init")),
                method: method::INITIALIZE.to_string(),
                params: Some(serde_json::json!({
                    "client": {"name": "codex", "version": "2.0.0", "transport": "stdio"},
                    "protocolVersion": {"major": 0, "minor": 1, "patch": 0},
                    "requestedCapabilities": [CompatibilityProfile::CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_ID]
                })),
            },
            JsonRpcRequest {
                jsonrpc: JSON_RPC_VERSION.to_string(),
                id: Some(serde_json::json!("thread")),
                method: method::THREAD_START.to_string(),
                params: Some(serde_json::json!({"cwd": "/workspace"})),
            },
            JsonRpcRequest {
                jsonrpc: JSON_RPC_VERSION.to_string(),
                id: Some(serde_json::json!("read")),
                method: method::THREAD_READ.to_string(),
                params: Some(serde_json::json!({"threadId": "thread_1"})),
            },
            JsonRpcRequest {
                jsonrpc: JSON_RPC_VERSION.to_string(),
                id: Some(serde_json::json!("turn")),
                method: method::TURN_START.to_string(),
                params: Some(serde_json::json!({
                    "threadId": "thread_1",
                    "input": [{"type": "text", "text": "hello", "text_elements": []}]
                })),
            },
            JsonRpcRequest {
                jsonrpc: JSON_RPC_VERSION.to_string(),
                id: Some(serde_json::json!("interrupt")),
                method: method::TURN_INTERRUPT.to_string(),
                params: Some(serde_json::json!({"threadId": "thread_1", "turnId": "turn_1"})),
            },
        ];
        let methods = requests
            .iter()
            .map(|request| request.method.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            methods,
            vec![
                method::INITIALIZE,
                method::THREAD_START,
                method::THREAD_READ,
                method::TURN_START,
                method::TURN_INTERRUPT,
            ]
        );
        for request in requests {
            let value = serde_json::to_value(request).expect("fixture should serialize");
            assert_eq!(value["jsonrpc"], JSON_RPC_VERSION);
            assert!(value.get("id").is_some());
        }

        let events = [
            ServerNotification::notifications_initialized(NotificationsInitializedEvent {
                lifecycle: LifecycleSnapshot {
                    state: LifecycleState::Ready,
                    reason: LifecycleReason::RuntimeReady,
                    message: None,
                    since: "0".to_string(),
                    degraded_services: Vec::new(),
                },
                compatibility_profiles: vec![
                    CompatibilityProfile::codex_app_server_v2_chat_session_subset(),
                ],
                unavailable_requested_capabilities: Vec::new(),
                event_queue: NotificationQueuePolicy::bounded_lag_disconnect(),
            })
            .expect("notifications/initialized fixture should serialize"),
            ServerNotification::lifecycle_changed(LifecycleChangedEvent {
                lifecycle: LifecycleSnapshot {
                    state: LifecycleState::Running,
                    reason: LifecycleReason::RequestInProgress,
                    message: None,
                    since: "1".to_string(),
                    degraded_services: Vec::new(),
                },
                previous_state: Some(LifecycleState::Ready),
            })
            .expect("lifecycle/changed fixture should serialize"),
            ServerNotification::capabilities_changed(CapabilitiesChangedEvent {
                capabilities: CapabilityMatrix::phase_one(),
                reason: CapabilitiesChangedReason::Initialize,
            })
            .expect("capabilities/changed fixture should serialize"),
            ServerNotification::thread_started(ThreadStartedEvent {
                thread: codex_thread_fixture(),
            })
            .expect("thread/started fixture should serialize"),
            ServerNotification::turn_started(TurnStartedEvent {
                thread_id: "thread_1".to_string(),
                turn: CodexTurn::in_progress("turn_1"),
            })
            .expect("turn/started fixture should serialize"),
            ServerNotification::turn_completed(TurnCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn: CodexTurn {
                    id: "turn_1".to_string(),
                    items: vec![CodexThreadItem::completed_agent_message("turn_1", "hello")],
                    status: CodexTurnStatus::Completed,
                    error: None,
                    started_at: None,
                    completed_at: Some(2),
                    duration_ms: Some(1),
                },
            })
            .expect("turn/completed fixture should serialize"),
            ServerNotification::item_started(ItemStartedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item: CodexThreadItem::started_agent_message("turn_1"),
            })
            .expect("item/started fixture should serialize"),
            ServerNotification::agent_message_delta(AgentMessageDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1".to_string(),
                delta: "hel".to_string(),
            })
            .expect("item delta fixture should serialize"),
            ServerNotification::reasoning_summary_text_delta(ReasoningSummaryTextDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:reasoning".to_string(),
                summary_index: 0,
                delta: "scratch".to_string(),
            })
            .expect("item/reasoning fixture should serialize"),
            ServerNotification::reasoning_summary_part_added(ReasoningSummaryPartAddedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:reasoning".to_string(),
                summary_index: 1,
            })
            .expect("item/reasoning summary part fixture should serialize"),
            ServerNotification::reasoning_text_delta(ReasoningTextDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:reasoning".to_string(),
                content_index: 0,
                delta: "raw scratch".to_string(),
            })
            .expect("item/reasoning text fixture should serialize"),
            ServerNotification::item_completed(ItemCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item: CodexThreadItem::completed_agent_message("turn_1", "hello"),
            })
            .expect("item/completed fixture should serialize"),
            ServerNotification::error(ErrorEvent {
                code: ErrorCode::ServiceDegraded,
                message: "runtime failed".to_string(),
                thread_id: Some("thread_1".to_string()),
                turn_id: Some("turn_1".to_string()),
                retryable: true,
            })
            .expect("error fixture should serialize"),
        ];
        let event_names = events
            .iter()
            .map(|event| event.method.as_str())
            .collect::<Vec<_>>();

        assert_eq!(event_names, CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS);
        assert_eq!(events[0].params["eventQueue"]["overflow"], "lag_disconnect");
        assert_eq!(events[3].params["thread"]["id"], "thread_1");
        assert_eq!(events[4].params["turn"]["id"], "turn_1");
        assert_eq!(events[5].params["turn"]["status"], "completed");
        assert_eq!(events[6].params["item"]["type"], "agentMessage");
        assert_eq!(events[7].params["delta"], "hel");
        assert_eq!(events[8].params["delta"], "scratch");
        assert_eq!(events[9].method, event::ITEM_REASONING_SUMMARY_PART_ADDED);
        assert_eq!(events[9].params["summaryIndex"], 1);
        assert_eq!(events[10].method, event::ITEM_REASONING_TEXT_DELTA);
        assert_eq!(events[10].params["contentIndex"], 0);
        assert_eq!(events[10].params["delta"], "raw scratch");
    }

    #[test]
    fn codex_v2_contract_exposes_structured_reasoning_delta() {
        let notification =
            ServerNotification::reasoning_summary_text_delta(ReasoningSummaryTextDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:reasoning".to_string(),
                summary_index: 0,
                delta: "private scratch".to_string(),
            })
            .expect("reasoning delta fixture should serialize");

        assert_eq!(
            notification.method,
            event::ITEM_REASONING_SUMMARY_TEXT_DELTA
        );
        assert_eq!(notification.params["threadId"], "thread_1");
        assert_eq!(notification.params["turnId"], "turn_1");
        assert_eq!(notification.params["itemId"], "turn_1:reasoning");
        assert_eq!(notification.params["summaryIndex"], 0);
        assert_eq!(notification.params["delta"], "private scratch");
        assert!(
            !notification.params["delta"]
                .as_str()
                .expect("delta should be text")
                .contains("<think>")
        );
    }

    #[test]
    fn p3_capability_helper_advertises_approval_tools_and_sandbox_contracts() {
        let matrix = CapabilityMatrix::phase_one().with_p3_approval_tool_sandbox();
        let schema = ProtocolSchemaResponse::phase_one(matrix.clone());
        let supported_methods = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>();
        let supported_notifications = schema
            .events
            .iter()
            .map(|event| event.event.as_str())
            .collect::<Vec<_>>();

        assert_eq!(matrix.approval.status, CapabilityStatus::Implemented);
        assert_eq!(matrix.tools.status, CapabilityStatus::Implemented);
        assert_eq!(matrix.sandbox.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.approval.methods,
            vec![method::APPROVAL_RESPOND.to_string()]
        );
        assert_eq!(
            matrix.approval.events,
            vec![
                server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL.to_string(),
                event::SERVER_REQUEST_RESOLVED.to_string(),
            ]
        );
        assert_eq!(matrix.tools.methods, Vec::<String>::new());
        assert_eq!(
            matrix.tools.events,
            vec![
                event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA.to_string(),
                event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION.to_string(),
            ]
        );
        assert!(
            !matrix
                .approval
                .events
                .contains(&server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL.to_string())
        );
        assert!(
            !matrix
                .approval
                .events
                .contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED.to_string())
        );
        assert!(
            !matrix
                .tools
                .events
                .contains(&server_request::ITEM_TOOL_CALL.to_string())
        );
        assert!(
            !matrix
                .tools
                .events
                .contains(&server_request::ITEM_TOOL_REQUEST_USER_INPUT.to_string())
        );
        assert!(
            matrix
                .sandbox
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("runtime bridge reports sandbox-ready execution")
        );
        assert!(supported_methods.contains(&method::APPROVAL_RESPOND));
        assert!(supported_notifications.contains(&event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA));
        assert!(
            supported_notifications.contains(&event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION)
        );
        assert!(supported_notifications.contains(&event::SERVER_REQUEST_RESOLVED));
    }

    #[test]
    fn server_initiated_request_serializes_as_json_rpc_request_with_id() {
        let request = JsonRpcServerRequest::new(
            "approval_00000000-0000-0000-0000-000000000001",
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
            CommandExecutionApprovalRequest {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:tool:bash".to_string(),
                tool_call_id: "call_1".to_string(),
                tool_name: "bash".to_string(),
                command: Some("echo hello".to_string()),
                description: "approve call to bash".to_string(),
                display_parameters: serde_json::json!({"cmd": "echo hello"}),
                allow_always: true,
            },
        )
        .expect("server approval request should serialize");
        let value =
            serde_json::to_value(&request).expect("server approval request should serialize");
        let round_trip: JsonRpcServerRequest =
            serde_json::from_value(value.clone()).expect("server request should deserialize");

        assert_eq!(value["jsonrpc"], JSON_RPC_VERSION);
        assert_eq!(value["id"], "approval_00000000-0000-0000-0000-000000000001");
        assert_eq!(
            value["method"],
            server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
        );
        assert_eq!(value["params"]["threadId"], "thread_1");
        assert_eq!(value["params"]["displayParameters"]["cmd"], "echo hello");
        assert!(value["params"].get("rawArguments").is_none());
        assert!(
            value.get("id").is_some(),
            "server request must not be a notification"
        );
        assert_eq!(
            round_trip.id,
            serde_json::json!("approval_00000000-0000-0000-0000-000000000001")
        );
    }

    #[test]
    fn client_response_deserializes_approval_decision_payload() {
        let approve_round_trip: JsonRpcClientResponse = serde_json::from_value(serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": "approval_approve",
            "result": {
                "decision": {"kind": "approve"}
            }
        }))
        .expect("approve response should deserialize from JSON wire shape");
        let reject_round_trip: JsonRpcClientResponse = serde_json::from_value(serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": "approval_reject",
            "result": {
                "decision": {
                    "kind": "reject",
                    "data": {"reason": "user denied command"}
                }
            }
        }))
        .expect("reject response should deserialize from JSON wire shape");
        let failure_round_trip: JsonRpcClientResponse = serde_json::from_value(serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": "approval_error",
            "error": {
                "code": -32000,
                "message": "approval failed"
            }
        }))
        .expect("error response should deserialize from JSON wire shape");
        let approve: ApprovalResponsePayload = serde_json::from_value(
            approve_round_trip
                .result
                .expect("approve response should contain result"),
        )
        .expect("approve payload should deserialize");
        let reject: ApprovalResponsePayload = serde_json::from_value(
            reject_round_trip
                .result
                .expect("reject response should contain result"),
        )
        .expect("reject payload should deserialize");

        assert_eq!(approve_round_trip.id, serde_json::json!("approval_approve"));
        assert_eq!(approve.decision, AppServerApprovalDecision::Approve);
        assert_eq!(reject_round_trip.id, serde_json::json!("approval_reject"));
        assert_eq!(
            reject.decision,
            AppServerApprovalDecision::Reject {
                reason: Some("user denied command".to_string()),
            }
        );
        assert_eq!(failure_round_trip.id, serde_json::json!("approval_error"));
        assert_eq!(
            failure_round_trip
                .error
                .expect("error response should contain error")
                .message,
            "approval failed"
        );
    }

    #[test]
    fn server_request_resolved_notification_hides_decision_reason_when_absent() {
        let output_delta =
            ServerNotification::command_execution_output_delta(CommandExecutionOutputDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:tool:bash".to_string(),
                delta: "hello\n".to_string(),
            })
            .expect("command output delta notification should serialize");
        let terminal_interaction = ServerNotification::command_execution_terminal_interaction(
            CommandExecutionTerminalInteractionEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:tool:bash".to_string(),
                message: "press enter to continue".to_string(),
                is_error: false,
            },
        )
        .expect("terminal interaction notification should serialize");
        let resolved = ServerNotification::server_request_resolved(ServerRequestResolvedEvent {
            request_id: "approval_00000000-0000-0000-0000-000000000001".to_string(),
            thread_id: Some("thread_1".to_string()),
            turn_id: Some("turn_1".to_string()),
            outcome: ServerRequestResolutionOutcome::Approved,
            reason: None,
        })
        .expect("server request resolved notification should serialize");

        assert_eq!(
            output_delta.method,
            event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA
        );
        assert_eq!(output_delta.params["itemId"], "turn_1:tool:bash");
        assert_eq!(output_delta.params["delta"], "hello\n");
        assert_eq!(
            terminal_interaction.method,
            event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION
        );
        assert_eq!(
            terminal_interaction.params["message"],
            "press enter to continue"
        );
        assert_eq!(terminal_interaction.params["isError"], false);
        assert_eq!(resolved.method, event::SERVER_REQUEST_RESOLVED);
        assert_eq!(
            resolved.params["requestId"],
            "approval_00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(resolved.params["outcome"], "approved");
        assert!(resolved.params.get("reason").is_none());
    }

    #[test]
    fn tool_lifecycle_server_requests_are_stable() {
        let tool_call = JsonRpcServerRequest::new(
            "tool_call_00000000-0000-0000-0000-000000000001",
            server_request::ITEM_TOOL_CALL,
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "itemId": "turn_1:tool:bash",
                "toolCallId": "call_1",
                "toolName": "bash",
                "displayParameters": {"cmd": "echo hello"}
            }),
        )
        .expect("tool call server request should serialize");
        let user_input = JsonRpcServerRequest::new(
            "tool_input_00000000-0000-0000-0000-000000000001",
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "itemId": "turn_1:tool:prompt",
                "toolCallId": "call_2",
                "toolName": "prompt",
                "prompt": "continue?"
            }),
        )
        .expect("tool user input server request should serialize");
        let tool_call_params = tool_call
            .params
            .expect("tool call server request should include params");
        let user_input_params = user_input
            .params
            .expect("tool user input server request should include params");

        assert_eq!(tool_call.method, server_request::ITEM_TOOL_CALL);
        assert_eq!(tool_call_params["toolName"], "bash");
        assert_eq!(tool_call_params["displayParameters"]["cmd"], "echo hello");
        assert_eq!(
            user_input.method,
            server_request::ITEM_TOOL_REQUEST_USER_INPUT
        );
        assert_eq!(user_input_params["prompt"], "continue?");
    }
}
