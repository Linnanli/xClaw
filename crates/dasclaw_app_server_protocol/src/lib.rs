//! Protocol types for the dasclaw local app-server.
//!
//! This crate intentionally models the GUI control-plane boundary only. It
//! does not define agent-loop internals or tool-execution traits; those stay
//! in `dasclaw_runtime`.

use std::{collections::HashMap, fmt};

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
    pub const JOBS_LIST: &str = "jobs/list";
    pub const JOBS_READ: &str = "jobs/read";
    pub const SKILLS_LIST: &str = "skills/list";
    pub const SKILLS_CONFIG_WRITE: &str = "skills/config/write";
    pub const MCP_SERVER_OAUTH_LOGIN: &str = "mcpServer/oauth/login";
    pub const CONFIG_REQUIREMENTS_READ: &str = "configRequirements/read";
    pub const CONFIG_MCP_SERVER_RELOAD: &str = "config/mcpServer/reload";
    pub const MCP_SERVER_STATUS_LIST: &str = "mcpServerStatus/list";
    pub const MCP_SERVER_RESOURCE_READ: &str = "mcpServer/resource/read";
    pub const MCP_SERVER_TOOL_CALL: &str = "mcpServer/tool/call";
    pub const FS_READ_FILE: &str = "fs/readFile";
    pub const FS_WRITE_FILE: &str = "fs/writeFile";
    pub const FS_CREATE_DIRECTORY: &str = "fs/createDirectory";
    pub const FS_GET_METADATA: &str = "fs/getMetadata";
    pub const FS_READ_DIRECTORY: &str = "fs/readDirectory";
    pub const FS_REMOVE: &str = "fs/remove";
    pub const FS_COPY: &str = "fs/copy";
    pub const FS_WATCH: &str = "fs/watch";
    pub const FS_UNWATCH: &str = "fs/unwatch";
    pub const COMMAND_EXEC: &str = "command/exec";
    pub const COMMAND_EXEC_WRITE: &str = "command/exec/write";
    pub const COMMAND_EXEC_TERMINATE: &str = "command/exec/terminate";
    pub const COMMAND_EXEC_RESIZE: &str = "command/exec/resize";
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
    pub const ITEM_FILE_CHANGE_OUTPUT_DELTA: &str = "item/fileChange/outputDelta";
    pub const ITEM_FILE_CHANGE_PATCH_UPDATED: &str = "item/fileChange/patchUpdated";
    pub const SKILLS_CHANGED: &str = "skills/changed";
    pub const ITEM_MCP_TOOL_CALL_PROGRESS: &str = "item/mcpToolCall/progress";
    pub const MCP_SERVER_OAUTH_LOGIN_COMPLETED: &str = "mcpServer/oauthLogin/completed";
    pub const MCP_SERVER_STARTUP_STATUS_UPDATED: &str = "mcpServer/startupStatus/updated";
    pub const FS_CHANGED: &str = "fs/changed";
    pub const COMMAND_EXEC_OUTPUT_DELTA: &str = "command/exec/outputDelta";
    pub const ERROR: &str = "error";
}

const FILESYSTEM_METHODS: &[&str] = &[
    method::FS_READ_FILE,
    method::FS_WRITE_FILE,
    method::FS_CREATE_DIRECTORY,
    method::FS_GET_METADATA,
    method::FS_READ_DIRECTORY,
    method::FS_REMOVE,
    method::FS_COPY,
    method::FS_WATCH,
    method::FS_UNWATCH,
];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
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
    pub filesystem: Capability,
    pub command_exec: Capability,
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
            filesystem: declared_future_capability("filesystem"),
            command_exec: declared_future_capability("command_exec"),
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

    #[must_use]
    pub fn with_app_services(mut self, availability: AppServerServiceAvailability) -> Self {
        if availability.logs {
            self.logs = Capability::implemented("logs", &[], &[event::LOG_ENTRY]);
        }
        if availability.jobs {
            self.jobs =
                Capability::implemented("jobs", &[method::JOBS_LIST, method::JOBS_READ], &[]);
        }
        if availability.skills {
            self.skills = Capability::implemented(
                "skills",
                &[method::SKILLS_LIST, method::SKILLS_CONFIG_WRITE],
                &[event::SKILLS_CHANGED],
            );
        }
        let mcp_methods = availability.mcp.methods();
        let mcp_events = availability.mcp.events();
        if !mcp_methods.is_empty() || !mcp_events.is_empty() {
            self.mcp = Capability::implemented("mcp", &mcp_methods, &mcp_events);
        }
        self.with_p5_filesystem_command(availability.p5)
    }

    #[must_use]
    pub fn with_p5_filesystem_command(mut self, availability: AppServerP5Availability) -> Self {
        self.filesystem = declared_future_capability("filesystem");
        self.command_exec = declared_future_capability("command_exec");

        if availability.filesystem {
            self.filesystem =
                Capability::implemented("filesystem", FILESYSTEM_METHODS, &[event::FS_CHANGED]);
        }

        let command_methods = availability.command.methods();
        let command_events = availability.command.events();
        if !command_methods.is_empty() || !command_events.is_empty() {
            self.command_exec =
                Capability::implemented("command_exec", &command_methods, &command_events);
        }
        self
    }
}

fn declared_future_capability(id: &'static str) -> Capability {
    Capability::declared(
        id,
        "declared for protocol compatibility; not implemented in Phase 1",
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerServiceAvailability {
    pub logs: bool,
    pub jobs: bool,
    pub skills: bool,
    pub mcp: McpServiceAvailability,
    pub p5: AppServerP5Availability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerP5Availability {
    pub filesystem: bool,
    pub command: CommandExecAvailability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandExecAvailability {
    pub exec: bool,
    pub output_delta_events: bool,
    pub terminate: bool,
    pub write: bool,
    pub resize: bool,
}

impl CommandExecAvailability {
    #[must_use]
    pub fn methods(&self) -> Vec<&'static str> {
        let mut methods = Vec::new();
        if self.exec {
            methods.push(method::COMMAND_EXEC);
        }
        if self.write {
            methods.push(method::COMMAND_EXEC_WRITE);
        }
        if self.terminate {
            methods.push(method::COMMAND_EXEC_TERMINATE);
        }
        if self.resize {
            methods.push(method::COMMAND_EXEC_RESIZE);
        }
        methods
    }

    #[must_use]
    pub fn events(&self) -> Vec<&'static str> {
        if self.output_delta_events {
            vec![event::COMMAND_EXEC_OUTPUT_DELTA]
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct McpServiceAvailability {
    pub status_list: bool,
    pub reload: bool,
    pub tool_call: bool,
    pub resource_read: bool,
    pub tool_call_progress_events: bool,
    pub startup_status_events: bool,
}

impl McpServiceAvailability {
    #[must_use]
    pub fn methods(&self) -> Vec<&'static str> {
        let mut methods = Vec::new();
        if self.reload {
            methods.push(method::CONFIG_MCP_SERVER_RELOAD);
        }
        if self.status_list {
            methods.push(method::MCP_SERVER_STATUS_LIST);
        }
        if self.resource_read {
            methods.push(method::MCP_SERVER_RESOURCE_READ);
        }
        if self.tool_call {
            methods.push(method::MCP_SERVER_TOOL_CALL);
        }
        methods
    }

    #[must_use]
    pub fn events(&self) -> Vec<&'static str> {
        let mut events = Vec::new();
        if self.tool_call_progress_events {
            events.push(event::ITEM_MCP_TOOL_CALL_PROGRESS);
        }
        if self.startup_status_events {
            events.push(event::MCP_SERVER_STARTUP_STATUS_UPDATED);
        }
        events
    }
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
                CapabilityOptOut::phase_one("filesystem"),
                CapabilityOptOut::phase_one("command_exec"),
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
            method::CONFIG_REQUIREMENTS_READ,
            "sandbox",
            None,
            "ConfigRequirementsReadResponse",
            true,
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
        MethodSchema::new(
            method::JOBS_LIST,
            "jobs",
            Some("JobListParams"),
            "JobListResponse",
            true,
        ),
        MethodSchema::new(
            method::JOBS_READ,
            "jobs",
            Some("JobReadParams"),
            "JobReadResponse",
            true,
        ),
        MethodSchema::new(
            method::SKILLS_LIST,
            "skills",
            Some("SkillsListParams"),
            "SkillsListResponse",
            true,
        ),
        MethodSchema::new(
            method::SKILLS_CONFIG_WRITE,
            "skills",
            Some("SkillsConfigWriteParams"),
            "SkillsConfigWriteResponse",
            true,
        ),
        MethodSchema::new(
            method::MCP_SERVER_OAUTH_LOGIN,
            "mcp",
            Some("McpServerOauthLoginParams"),
            "McpServerOauthLoginResponse",
            true,
        ),
        MethodSchema::new(
            method::CONFIG_MCP_SERVER_RELOAD,
            "mcp",
            Some("McpServerReloadParams"),
            "McpServerReloadResponse",
            true,
        ),
        MethodSchema::new(
            method::MCP_SERVER_STATUS_LIST,
            "mcp",
            Some("ListMcpServerStatusParams"),
            "ListMcpServerStatusResponse",
            true,
        ),
        MethodSchema::new(
            method::MCP_SERVER_RESOURCE_READ,
            "mcp",
            Some("McpResourceReadParams"),
            "McpResourceReadResponse",
            true,
        ),
        MethodSchema::new(
            method::MCP_SERVER_TOOL_CALL,
            "mcp",
            Some("McpServerToolCallParams"),
            "McpServerToolCallResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_READ_FILE,
            "filesystem",
            Some("FsReadFileParams"),
            "FsReadFileResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_WRITE_FILE,
            "filesystem",
            Some("FsWriteFileParams"),
            "FsWriteFileResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_CREATE_DIRECTORY,
            "filesystem",
            Some("FsCreateDirectoryParams"),
            "FsCreateDirectoryResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_GET_METADATA,
            "filesystem",
            Some("FsGetMetadataParams"),
            "FsGetMetadataResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_READ_DIRECTORY,
            "filesystem",
            Some("FsReadDirectoryParams"),
            "FsReadDirectoryResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_REMOVE,
            "filesystem",
            Some("FsRemoveParams"),
            "FsRemoveResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_COPY,
            "filesystem",
            Some("FsCopyParams"),
            "FsCopyResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_WATCH,
            "filesystem",
            Some("FsWatchParams"),
            "FsWatchResponse",
            true,
        ),
        MethodSchema::new(
            method::FS_UNWATCH,
            "filesystem",
            Some("FsUnwatchParams"),
            "FsUnwatchResponse",
            true,
        ),
        MethodSchema::new(
            method::COMMAND_EXEC,
            "command_exec",
            Some("CommandExecParams"),
            "CommandExecResponse",
            true,
        ),
        MethodSchema::new(
            method::COMMAND_EXEC_WRITE,
            "command_exec",
            Some("CommandExecWriteParams"),
            "CommandExecWriteResponse",
            true,
        ),
        MethodSchema::new(
            method::COMMAND_EXEC_TERMINATE,
            "command_exec",
            Some("CommandExecTerminateParams"),
            "CommandExecTerminateResponse",
            true,
        ),
        MethodSchema::new(
            method::COMMAND_EXEC_RESIZE,
            "command_exec",
            Some("CommandExecResizeParams"),
            "CommandExecResizeResponse",
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
        EventSchema::new(event::SKILLS_CHANGED, "skills", "SkillsChangedNotification"),
        EventSchema::new(
            event::ITEM_MCP_TOOL_CALL_PROGRESS,
            "mcp",
            "McpToolCallProgressNotification",
        ),
        EventSchema::new(
            event::MCP_SERVER_OAUTH_LOGIN_COMPLETED,
            "mcp",
            "McpServerOauthLoginCompletedNotification",
        ),
        EventSchema::new(
            event::MCP_SERVER_STARTUP_STATUS_UPDATED,
            "mcp",
            "McpServerStatusUpdatedNotification",
        ),
        EventSchema::new(event::FS_CHANGED, "filesystem", "FsChangedNotification"),
        EventSchema::new(
            event::COMMAND_EXEC_OUTPUT_DELTA,
            "command_exec",
            "CommandExecOutputDeltaNotification",
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
    Filesystem,
    CommandExec,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequirementsReadResponse {
    pub allowed_sandbox_modes: Vec<SandboxMode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadStartParams {
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolCallOutputContentItem {
    #[serde(rename_all = "camelCase")]
    InputText { text: String },
    #[serde(rename_all = "camelCase")]
    InputImage { image_url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub is_other: bool,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<ToolRequestUserInputOption>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputResponse {
    pub answers: HashMap<String, ToolRequestUserInputAnswer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_root: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChangeApprovalDecision {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalResponse {
    pub decision: FileChangeApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionGrantScope {
    #[default]
    Turn,
    Session,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalResponse {
    pub permissions: serde_json::Value,
    #[serde(default)]
    pub scope: PermissionGrantScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_auto_review: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileUpdateKind {
    Add,
    Delete,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: FileUpdateKind,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeOutputDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangePatchUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<FileUpdateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianApprovalReview {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_authorization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprovalReviewStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprovalReviewCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_item_id: Option<String>,
    pub review: GuardianApprovalReview,
    pub action: String,
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
pub struct FsReadFileParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadFileResponse {
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteFileParams {
    pub path: String,
    pub data_base64: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<FsWriteMode>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FsWriteMode {
    Create,
    #[default]
    Overwrite,
    Append,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteFileResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCreateDirectoryParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCreateDirectoryResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGetMetadataParams {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGetMetadataResponse {
    pub is_file: bool,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub created_at_ms: u64,
    pub modified_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryParams {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryEntry {
    pub file_name: String,
    pub is_file: bool,
    pub is_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryResponse {
    pub entries: Vec<FsReadDirectoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRemoveParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRemoveResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCopyParams {
    pub source_path: String,
    pub destination_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCopyResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWatchParams {
    pub path: String,
    pub watch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWatchResponse {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsUnwatchParams {
    pub watch_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsUnwatchResponse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FsChangedKind {
    Created,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsChangedNotification {
    pub watch_id: String,
    pub path: String,
    pub kind: FsChangedKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandExecOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecTerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecParams {
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_timeout: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_bytes_cap: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_output_cap: Option<bool>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<CommandExecTerminalSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_stdin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_stdout_stderr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecOutputDeltaNotification {
    pub process_id: String,
    pub stream: CommandExecOutputStream,
    pub delta_base64: String,
    pub cap_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecWriteParams {
    pub process_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_stdin: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecWriteResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecTerminateParams {
    pub process_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecTerminateResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResizeParams {
    pub process_id: String,
    pub size: CommandExecTerminalSize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResizeResponse {}

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

    pub fn file_change_output_delta(
        event: FileChangeOutputDeltaEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_FILE_CHANGE_OUTPUT_DELTA, event)
    }

    pub fn file_change_patch_updated(
        event: FileChangePatchUpdatedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_FILE_CHANGE_PATCH_UPDATED, event)
    }

    pub fn auto_approval_review_started(
        event: AutoApprovalReviewStartedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_AUTO_APPROVAL_REVIEW_STARTED, event)
    }

    pub fn auto_approval_review_completed(
        event: AutoApprovalReviewCompletedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED, event)
    }

    pub fn skills_changed(event: SkillsChangedNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::SKILLS_CHANGED, event)
    }

    pub fn mcp_tool_call_progress(
        event: McpToolCallProgressNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_MCP_TOOL_CALL_PROGRESS, event)
    }

    pub fn mcp_server_oauth_login_completed(
        event: McpServerOauthLoginCompletedNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::MCP_SERVER_OAUTH_LOGIN_COMPLETED, event)
    }

    pub fn mcp_server_startup_status_updated(
        event: McpServerStatusUpdatedNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::MCP_SERVER_STARTUP_STATUS_UPDATED, event)
    }

    pub fn fs_changed(event: FsChangedNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::FS_CHANGED, event)
    }

    pub fn command_exec_output_delta(
        event: CommandExecOutputDeltaNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::COMMAND_EXEC_OUTPUT_DELTA, event)
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListParams {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cwds: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_reload: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_cwd_extra_user_roots: Option<Vec<SkillsListExtraRootsForCwd>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListExtraRootsForCwd {
    pub cwd: String,
    pub extra_user_roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListResponse {
    pub data: Vec<SkillsListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListEntry {
    pub cwd: String,
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillErrorInfo {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<SkillInterface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<SkillDependencies>,
    pub path: String,
    pub scope: SkillScope,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInterface {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_small: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_large: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_prompt: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDependencies {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<SkillToolDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillToolDependency {
    #[serde(rename = "type")]
    pub dependency_type: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillScope {
    User,
    Repo,
    System,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfigWriteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfigWriteResponse {
    pub effective_enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsChangedNotification {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReadParams {
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListResponse {
    pub data: Vec<JobSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReadResponse {
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub job_id: String,
    pub title: String,
    pub description: String,
    pub state: JobSnapshotState,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobSnapshotState {
    Queued,
    Pending,
    InProgress,
    Completed,
    Submitted,
    Accepted,
    Failed,
    Stuck,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServerStatusParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<McpServerStatusDetail>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpServerStatusDetail {
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "toolsAndAuthOnly")]
    ToolsAndAuthOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServerStatusResponse {
    pub data: Vec<McpServerStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatus {
    pub name: String,
    #[serde(default)]
    pub tools: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<Resource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_templates: Vec<ResourceTemplate>,
    pub auth_status: McpAuthStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    pub uri_template: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpAuthStatus {
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "notLoggedIn")]
    NotLoggedIn,
    #[serde(rename = "bearerToken")]
    BearerToken,
    #[serde(rename = "oAuth")]
    OAuth,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerReloadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerReloadResponse {
    pub reloaded: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerToolCallParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub server: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolProgressIdentity {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
}

impl McpServerToolCallParams {
    #[must_use]
    pub fn progress_identity(&self) -> Option<McpToolProgressIdentity> {
        Some(McpToolProgressIdentity {
            thread_id: self.thread_id.clone(),
            turn_id: self.turn_id.clone()?,
            item_id: self.item_id.clone()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerToolCallResponse {
    pub content: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceReadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub server: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceReadResponse {
    pub contents: Vec<McpResourceContent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginResponse {
    pub authorization_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginCompletedNotification {
    pub name: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpServerStartupState {
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatusUpdatedNotification {
    pub name: String,
    pub status: McpServerStartupState,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallProgressNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub message: String,
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
        assert!(profile.capability_opt_outs.iter().any(|opt_out| {
            opt_out.capability == "filesystem" && opt_out.reason == "phase_1_chat_session_subset"
        }));
        assert!(profile.capability_opt_outs.iter().any(|opt_out| {
            opt_out.capability == "command_exec" && opt_out.reason == "phase_1_chat_session_subset"
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

    #[test]
    fn r1_server_requests_serialize_codex_v2_shapes() {
        let dynamic_tool = JsonRpcServerRequest::new(
            "tool_1",
            server_request::ITEM_TOOL_CALL,
            DynamicToolCallParams {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                call_id: "call_1".to_string(),
                namespace: Some("client".to_string()),
                tool: "open_url".to_string(),
                arguments: serde_json::json!({"url":"https://example.test"}),
            },
        )
        .expect("dynamic tool server request should serialize");
        let user_input = JsonRpcServerRequest::new(
            "input_1",
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            ToolRequestUserInputParams {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:tool:ask".to_string(),
                questions: vec![ToolRequestUserInputQuestion {
                    id: "choice".to_string(),
                    header: "Mode".to_string(),
                    question: "Pick a mode".to_string(),
                    is_other: false,
                    is_secret: false,
                    options: Some(vec![ToolRequestUserInputOption {
                        label: "Safe".to_string(),
                        description: "Continue with read-only work".to_string(),
                    }]),
                }],
            },
        )
        .expect("tool user input server request should serialize");
        let file_change = JsonRpcServerRequest::new(
            "file_1",
            server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
            FileChangeRequestApprovalParams {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:file:patch".to_string(),
                reason: Some("apply generated patch".to_string()),
                grant_root: Some("/workspace".to_string()),
            },
        )
        .expect("file change approval server request should serialize");
        let permissions = JsonRpcServerRequest::new(
            "perm_1",
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
            PermissionsRequestApprovalParams {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:permission:network".to_string(),
                cwd: "/workspace".to_string(),
                reason: Some("network access required".to_string()),
                permissions: serde_json::json!({"network":{"allow":["example.test"]}}),
            },
        )
        .expect("permissions approval server request should serialize");

        let user_input_params = user_input
            .params
            .expect("tool user input server request should include params");
        let file_change_params = file_change
            .params
            .expect("file change approval server request should include params");
        let permissions_params = permissions
            .params
            .expect("permissions approval server request should include params");

        assert_eq!(
            serde_json::to_value(&dynamic_tool)
                .expect("dynamic tool server request should serialize to JSON"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "tool_1",
                "method": "item/tool/call",
                "params": {
                    "threadId": "thread_1",
                    "turnId": "turn_1",
                    "callId": "call_1",
                    "namespace": "client",
                    "tool": "open_url",
                    "arguments": {"url":"https://example.test"}
                }
            })
        );

        assert_eq!(
            user_input_params,
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "itemId": "turn_1:tool:ask",
                "questions": [{
                    "id": "choice",
                    "header": "Mode",
                    "question": "Pick a mode",
                    "isOther": false,
                    "isSecret": false,
                    "options": [{
                        "label": "Safe",
                        "description": "Continue with read-only work"
                    }]
                }]
            })
        );

        assert_eq!(
            file_change_params,
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "itemId": "turn_1:file:patch",
                "reason": "apply generated patch",
                "grantRoot": "/workspace"
            })
        );

        assert_eq!(
            permissions_params,
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "itemId": "turn_1:permission:network",
                "cwd": "/workspace",
                "reason": "network access required",
                "permissions": {"network":{"allow":["example.test"]}}
            })
        );

        assert_eq!(
            serde_json::to_value(DynamicToolCallParams {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                call_id: "call_2".to_string(),
                namespace: None,
                tool: "fetch".to_string(),
                arguments: serde_json::json!({}),
            })
            .expect("dynamic tool params without namespace should serialize"),
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "callId": "call_2",
                "tool": "fetch",
                "arguments": {}
            })
        );

        assert_eq!(
            serde_json::to_value(FileChangeRequestApprovalParams {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:file:patch".to_string(),
                reason: None,
                grant_root: None,
            })
            .expect("file change params without optional fields should serialize"),
            serde_json::json!({
                "threadId": "thread_1",
                "turnId": "turn_1",
                "itemId": "turn_1:file:patch"
            })
        );
    }

    #[test]
    fn r1_response_payloads_deserialize_by_kind() {
        let dynamic: DynamicToolCallResponse = serde_json::from_value(serde_json::json!({
            "contentItems": [{"type": "inputText", "text": "opened"}],
            "success": true
        }))
        .expect("dynamic tool response should deserialize");
        let user_input: ToolRequestUserInputResponse = serde_json::from_value(serde_json::json!({
            "answers": {
                "choice": {"answers": ["Safe"]}
            }
        }))
        .expect("tool user input response should deserialize");
        let file_change: FileChangeRequestApprovalResponse =
            serde_json::from_value(serde_json::json!({"decision": "acceptForSession"}))
                .expect("file change response should deserialize");
        let permissions: PermissionsRequestApprovalResponse =
            serde_json::from_value(serde_json::json!({
                "permissions": {"network":{"allow":["example.test"]}},
                "scope": "session",
                "strictAutoReview": true
            }))
            .expect("permissions response should deserialize");
        let default_permissions: PermissionsRequestApprovalResponse =
            serde_json::from_value(serde_json::json!({
                "permissions": {}
            }))
            .expect("permissions response should default missing optional fields");

        assert_eq!(
            dynamic.content_items,
            vec![DynamicToolCallOutputContentItem::InputText {
                text: "opened".to_string(),
            }]
        );
        assert_eq!(
            user_input.answers["choice"].answers,
            vec!["Safe".to_string()]
        );
        assert_eq!(
            file_change.decision,
            FileChangeApprovalDecision::AcceptForSession
        );
        assert_eq!(permissions.scope, PermissionGrantScope::Session);
        assert_eq!(permissions.strict_auto_review, Some(true));
        assert_eq!(default_permissions.scope, PermissionGrantScope::Turn);
        assert_eq!(default_permissions.strict_auto_review, None);
        assert_eq!(
            serde_json::to_value(default_permissions)
                .expect("default permissions response should serialize"),
            serde_json::json!({
                "permissions": {},
                "scope": "turn"
            })
        );
    }

    #[test]
    fn r1_file_change_and_auto_review_notifications_serialize() {
        let output = ServerNotification::file_change_output_delta(FileChangeOutputDeltaEvent {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:file:patch".to_string(),
            delta: "patched src/lib.rs".to_string(),
        })
        .expect("file change output delta should serialize");
        let patch = ServerNotification::file_change_patch_updated(FileChangePatchUpdatedEvent {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            item_id: "turn_1:file:patch".to_string(),
            changes: vec![FileUpdateChange {
                path: "src/lib.rs".to_string(),
                kind: FileUpdateKind::Update,
                unified_diff: "@@ -1 +1 @@".to_string(),
            }],
        })
        .expect("file change patch update should serialize");
        let started =
            ServerNotification::auto_approval_review_started(AutoApprovalReviewStartedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                review_id: "review_1".to_string(),
                target_item_id: Some("turn_1:tool:shell".to_string()),
                review: GuardianApprovalReview {
                    status: "inProgress".to_string(),
                    risk_level: Some("low".to_string()),
                    user_authorization: None,
                    rationale: Some("read-only command".to_string()),
                },
                action: "review".to_string(),
            })
            .expect("auto approval review started should serialize");
        let completed =
            ServerNotification::auto_approval_review_completed(AutoApprovalReviewCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                review_id: "review_1".to_string(),
                target_item_id: Some("turn_1:tool:shell".to_string()),
                review: GuardianApprovalReview {
                    status: "approved".to_string(),
                    risk_level: Some("low".to_string()),
                    user_authorization: Some("allowed".to_string()),
                    rationale: Some("command is read-only".to_string()),
                },
                action: "review".to_string(),
            })
            .expect("auto approval review completed should serialize");

        assert_eq!(
            serde_json::to_value(output).expect("file change output delta should serialize"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "item/fileChange/outputDelta",
                "params": {
                    "threadId": "thread_1",
                    "turnId": "turn_1",
                    "itemId": "turn_1:file:patch",
                    "delta": "patched src/lib.rs"
                }
            })
        );
        assert_eq!(
            serde_json::to_value(patch).expect("file change patch update should serialize"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "item/fileChange/patchUpdated",
                "params": {
                    "threadId": "thread_1",
                    "turnId": "turn_1",
                    "itemId": "turn_1:file:patch",
                    "changes": [{
                        "path": "src/lib.rs",
                        "kind": "update",
                        "unifiedDiff": "@@ -1 +1 @@"
                    }]
                }
            })
        );
        assert_eq!(
            serde_json::to_value(started).expect("auto approval review started should serialize"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "item/autoApprovalReview/started",
                "params": {
                    "threadId": "thread_1",
                    "turnId": "turn_1",
                    "reviewId": "review_1",
                    "targetItemId": "turn_1:tool:shell",
                    "review": {
                        "status": "inProgress",
                        "riskLevel": "low",
                        "rationale": "read-only command"
                    },
                    "action": "review"
                }
            })
        );
        assert_eq!(
            serde_json::to_value(completed)
                .expect("auto approval review completed should serialize"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "item/autoApprovalReview/completed",
                "params": {
                    "threadId": "thread_1",
                    "turnId": "turn_1",
                    "reviewId": "review_1",
                    "targetItemId": "turn_1:tool:shell",
                    "review": {
                        "status": "approved",
                        "riskLevel": "low",
                        "userAuthorization": "allowed",
                        "rationale": "command is read-only"
                    },
                    "action": "review"
                }
            })
        );
    }

    #[test]
    fn app_server_phase_one_keeps_services_declared_until_owner_is_wired() {
        let matrix = CapabilityMatrix::phase_one();

        assert_eq!(matrix.logs.status, CapabilityStatus::Declared);
        assert_eq!(matrix.jobs.status, CapabilityStatus::Declared);
        assert_eq!(matrix.skills.status, CapabilityStatus::Declared);
        assert_eq!(matrix.mcp.status, CapabilityStatus::Declared);
    }

    #[test]
    fn app_server_capability_helper_advertises_only_ready_services() {
        let matrix =
            CapabilityMatrix::phase_one().with_app_services(AppServerServiceAvailability {
                logs: true,
                jobs: true,
                skills: false,
                mcp: McpServiceAvailability {
                    status_list: true,
                    reload: true,
                    tool_call: true,
                    resource_read: true,
                    tool_call_progress_events: true,
                    startup_status_events: true,
                },
                p5: AppServerP5Availability::default(),
            });

        assert_eq!(matrix.logs.status, CapabilityStatus::Implemented);
        assert_eq!(matrix.logs.events, vec![event::LOG_ENTRY.to_string()]);
        assert_eq!(matrix.jobs.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.jobs.methods,
            vec![method::JOBS_LIST.to_string(), method::JOBS_READ.to_string()]
        );
        assert_eq!(matrix.skills.status, CapabilityStatus::Declared);
        assert_eq!(matrix.mcp.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.mcp.methods,
            vec![
                method::CONFIG_MCP_SERVER_RELOAD.to_string(),
                method::MCP_SERVER_STATUS_LIST.to_string(),
                method::MCP_SERVER_RESOURCE_READ.to_string(),
                method::MCP_SERVER_TOOL_CALL.to_string(),
            ]
        );
        assert_eq!(
            matrix.mcp.events,
            vec![
                event::ITEM_MCP_TOOL_CALL_PROGRESS.to_string(),
                event::MCP_SERVER_STARTUP_STATUS_UPDATED.to_string(),
            ]
        );
    }

    #[test]
    fn app_server_capability_helper_does_not_advertise_unwired_mcp_effectful_routes() {
        let matrix =
            CapabilityMatrix::phase_one().with_app_services(AppServerServiceAvailability {
                mcp: McpServiceAvailability {
                    status_list: true,
                    reload: true,
                    startup_status_events: true,
                    ..McpServiceAvailability::default()
                },
                ..AppServerServiceAvailability::default()
            });

        assert_eq!(matrix.mcp.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.mcp.methods,
            vec![
                method::CONFIG_MCP_SERVER_RELOAD.to_string(),
                method::MCP_SERVER_STATUS_LIST.to_string(),
            ]
        );
        assert_eq!(
            matrix.mcp.events,
            vec![event::MCP_SERVER_STARTUP_STATUS_UPDATED.to_string()]
        );
        assert!(
            !matrix
                .mcp
                .methods
                .contains(&method::MCP_SERVER_TOOL_CALL.to_string())
        );
        assert!(
            !matrix
                .mcp
                .methods
                .contains(&method::MCP_SERVER_RESOURCE_READ.to_string())
        );
        assert!(
            !matrix
                .mcp
                .methods
                .contains(&method::MCP_SERVER_OAUTH_LOGIN.to_string())
        );
    }

    #[test]
    fn skills_list_response_serializes_codex_shape() {
        let params = SkillsListParams {
            cwds: vec!["/repo".into()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: "/repo".into(),
                extra_user_roots: vec!["/repo/.codex/skills".into()],
            }]),
        };
        let response = SkillsListResponse {
            data: vec![SkillsListEntry {
                cwd: "/repo".into(),
                skills: vec![SkillMetadata {
                    name: "review".into(),
                    description: "Review local code".into(),
                    short_description: Some("Code review".into()),
                    interface: Some(SkillInterface {
                        display_name: Some("Review".into()),
                        short_description: Some("Code review".into()),
                        icon_small: None,
                        icon_large: None,
                        brand_color: None,
                        default_prompt: None,
                    }),
                    dependencies: Some(SkillDependencies {
                        tools: vec![SkillToolDependency {
                            dependency_type: "mcp".into(),
                            value: "github".into(),
                            description: Some("GitHub MCP".into()),
                            transport: Some("stdio".into()),
                            command: Some("github-mcp".into()),
                            url: None,
                        }],
                    }),
                    path: "/repo/.codex/skills/review/SKILL.json".into(),
                    scope: SkillScope::Repo,
                    enabled: true,
                }],
                errors: vec![SkillErrorInfo {
                    path: "/repo/.codex/skills/bad/SKILL.md".into(),
                    message: "invalid frontmatter".into(),
                }],
            }],
        };
        let write_params = SkillsConfigWriteParams {
            path: Some("/repo/.codex/skills/review/SKILL.json".into()),
            name: None,
            enabled: false,
        };
        let write_response = SkillsConfigWriteResponse {
            effective_enabled: false,
        };
        let changed = SkillsChangedNotification {};

        let params_value = serde_json::to_value(params).expect("skills/list params serialize");
        let value = serde_json::to_value(response).expect("skills/list response serializes");
        let write_params_value =
            serde_json::to_value(write_params).expect("skills/config/write params serialize");
        let write_response_value =
            serde_json::to_value(write_response).expect("skills/config/write response serialize");
        let changed_value =
            serde_json::to_value(changed).expect("skills/changed notification serializes");

        assert_eq!(params_value["cwds"][0], "/repo");
        assert_eq!(params_value["forceReload"], true);
        assert_eq!(
            params_value["perCwdExtraUserRoots"][0]["extraUserRoots"][0],
            "/repo/.codex/skills"
        );
        assert_eq!(value["data"][0]["cwd"], "/repo");
        assert_eq!(value["data"][0]["skills"][0]["name"], "review");
        assert_eq!(
            value["data"][0]["skills"][0]["shortDescription"],
            "Code review"
        );
        assert_eq!(value["data"][0]["skills"][0]["scope"], "repo");
        assert_eq!(value["data"][0]["skills"][0]["enabled"], true);
        assert_eq!(
            value["data"][0]["skills"][0]["dependencies"]["tools"][0]["type"],
            "mcp"
        );
        assert_eq!(
            value["data"][0]["errors"][0]["message"],
            "invalid frontmatter"
        );
        assert_eq!(
            write_params_value["path"],
            "/repo/.codex/skills/review/SKILL.json"
        );
        assert!(write_params_value["name"].is_null());
        assert_eq!(write_params_value["enabled"], false);
        assert_eq!(write_response_value["effectiveEnabled"], false);
        assert_eq!(changed_value, serde_json::json!({}));
    }

    #[test]
    fn mcp_status_response_serializes_codex_shape() {
        let response = ListMcpServerStatusResponse {
            data: vec![McpServerStatus {
                name: "github".into(),
                tools: serde_json::json!({
                    "list_issues": {
                        "name": "list_issues",
                        "description": "List issues",
                        "inputSchema": {"type": "object"}
                    }
                }),
                resources: vec![],
                resource_templates: vec![],
                auth_status: McpAuthStatus::NotLoggedIn,
            }],
            next_cursor: None,
        };

        let value = serde_json::to_value(response).expect("mcp status response serializes");
        assert_eq!(value["data"][0]["name"], "github");
        assert_eq!(value["data"][0]["authStatus"], "notLoggedIn");
        assert_eq!(
            value["data"][0]["tools"]["list_issues"]["name"],
            "list_issues"
        );
        assert!(value["nextCursor"].is_null());
    }

    #[test]
    fn mcp_tool_oauth_and_resource_shapes_match_codex_schema() {
        let tool_params = McpServerToolCallParams {
            thread_id: "thread_1".into(),
            turn_id: Some("turn_1".into()),
            item_id: Some("item_1".into()),
            server: "github".into(),
            tool: "list_issues".into(),
            arguments: Some(serde_json::json!({"owner": "openai"})),
            meta: Some(serde_json::json!({"request": "abc"})),
        };
        let tool_response = McpServerToolCallResponse {
            content: vec![serde_json::json!({"type": "text", "text": "ok"})],
            structured_content: Some(serde_json::json!({"issues": []})),
            is_error: Some(false),
            meta: Some(serde_json::json!({"server": "github"})),
        };
        let oauth_params = McpServerOauthLoginParams {
            name: "github".into(),
            scopes: Some(vec!["repo".into()]),
            timeout_secs: Some(60),
        };
        let oauth_response = McpServerOauthLoginResponse {
            authorization_url: "https://auth.example.test/oauth".into(),
        };
        let read_params = McpResourceReadParams {
            thread_id: Some("thread_1".into()),
            server: "github".into(),
            uri: "repo://openai/codex".into(),
        };
        let read_response = McpResourceReadResponse {
            contents: vec![McpResourceContent {
                uri: "repo://openai/codex".into(),
                mime_type: Some("text/plain".into()),
                text: Some("content".into()),
                blob: None,
                meta: Some(serde_json::json!({"etag": "1"})),
            }],
        };
        let oauth_completed = McpServerOauthLoginCompletedNotification {
            name: "github".into(),
            success: true,
            error: None,
        };
        let status_updated = McpServerStatusUpdatedNotification {
            name: "github".into(),
            status: McpServerStartupState::Ready,
            error: None,
        };
        let progress = McpToolCallProgressNotification {
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            item_id: "item_1".into(),
            message: "running list_issues".into(),
        };

        let tool_params_value =
            serde_json::to_value(tool_params).expect("mcp tool params serialize");
        let tool_response_value =
            serde_json::to_value(tool_response).expect("mcp tool response serialize");
        let oauth_params_value =
            serde_json::to_value(oauth_params).expect("mcp oauth params serialize");
        let oauth_response_value =
            serde_json::to_value(oauth_response).expect("mcp oauth response serialize");
        let read_params_value =
            serde_json::to_value(read_params).expect("mcp resource params serialize");
        let read_response_value =
            serde_json::to_value(read_response).expect("mcp resource response serialize");
        let oauth_completed_value =
            serde_json::to_value(oauth_completed).expect("mcp oauth notification serialize");
        let status_updated_value =
            serde_json::to_value(status_updated).expect("mcp status notification serialize");
        let progress_value =
            serde_json::to_value(progress).expect("mcp progress notification serialize");

        assert_eq!(tool_params_value["threadId"], "thread_1");
        assert_eq!(tool_params_value["turnId"], "turn_1");
        assert_eq!(tool_params_value["itemId"], "item_1");
        assert_eq!(tool_params_value["server"], "github");
        assert_eq!(tool_params_value["tool"], "list_issues");
        assert_eq!(tool_params_value["_meta"]["request"], "abc");
        assert_eq!(tool_response_value["content"][0]["text"], "ok");
        assert_eq!(
            tool_response_value["structuredContent"]["issues"],
            serde_json::json!([])
        );
        assert_eq!(tool_response_value["isError"], false);
        assert_eq!(tool_response_value["_meta"]["server"], "github");
        assert_eq!(oauth_params_value["name"], "github");
        assert_eq!(oauth_params_value["scopes"][0], "repo");
        assert_eq!(oauth_params_value["timeoutSecs"], 60);
        assert_eq!(
            oauth_response_value["authorizationUrl"],
            "https://auth.example.test/oauth"
        );
        assert_eq!(read_params_value["threadId"], "thread_1");
        assert_eq!(read_params_value["server"], "github");
        assert_eq!(read_response_value["contents"][0]["text"], "content");
        assert_eq!(read_response_value["contents"][0]["_meta"]["etag"], "1");
        assert_eq!(oauth_completed_value["name"], "github");
        assert_eq!(oauth_completed_value["success"], true);
        assert!(oauth_completed_value["error"].is_null());
        assert_eq!(status_updated_value["name"], "github");
        assert_eq!(status_updated_value["status"], "ready");
        assert!(status_updated_value["error"].is_null());
        assert_eq!(progress_value["threadId"], "thread_1");
        assert_eq!(progress_value["turnId"], "turn_1");
        assert_eq!(progress_value["itemId"], "item_1");
        assert_eq!(progress_value["message"], "running list_issues");
    }

    #[test]
    fn mcp_tool_call_params_deserialize_legacy_and_progress_identity_requires_full_ids() {
        let legacy: McpServerToolCallParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "server": "github",
            "tool": "list_issues",
            "arguments": {"owner": "openai"}
        }))
        .expect("legacy tool call params should deserialize");

        assert_eq!(legacy.thread_id, "thread_1");
        assert_eq!(legacy.server, "github");
        assert_eq!(legacy.tool, "list_issues");
        assert_eq!(
            legacy.arguments,
            Some(serde_json::json!({"owner": "openai"}))
        );
        assert!(legacy.turn_id.is_none());
        assert!(legacy.item_id.is_none());
        assert!(legacy.progress_identity().is_none());

        let missing_item: McpServerToolCallParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "turnId": "turn_1",
            "server": "github",
            "tool": "list_issues"
        }))
        .expect("tool call params with only turnId should deserialize");
        assert!(missing_item.progress_identity().is_none());

        let current: McpServerToolCallParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "turnId": "turn_1",
            "itemId": "item_1",
            "server": "github",
            "tool": "list_issues",
            "_meta": {"request": "abc"}
        }))
        .expect("current tool call params should deserialize");

        assert_eq!(
            current.progress_identity(),
            Some(McpToolProgressIdentity {
                thread_id: "thread_1".into(),
                turn_id: "turn_1".into(),
                item_id: "item_1".into(),
            })
        );
    }

    #[test]
    fn native_job_list_response_serializes_snake_case_states() {
        let response = JobListResponse {
            data: vec![JobSnapshot {
                job_id: "job_1".into(),
                title: "Run audit".into(),
                description: "Audit repository".into(),
                state: JobSnapshotState::InProgress,
                created_at: "2026-06-19T00:00:00Z".into(),
                updated_at: Some("2026-06-19T00:01:00Z".into()),
                thread_id: Some("thread_1".into()),
            }],
            next_cursor: None,
        };

        let value = serde_json::to_value(response).expect("job list response serializes");
        assert_eq!(value["data"][0]["jobId"], "job_1");
        assert_eq!(value["data"][0]["state"], "in_progress");
    }

    #[test]
    fn native_job_snapshot_states_cover_dasclaw_runtime_state_machine() {
        let states = [
            (JobSnapshotState::Pending, "pending"),
            (JobSnapshotState::InProgress, "in_progress"),
            (JobSnapshotState::Completed, "completed"),
            (JobSnapshotState::Submitted, "submitted"),
            (JobSnapshotState::Accepted, "accepted"),
            (JobSnapshotState::Failed, "failed"),
            (JobSnapshotState::Stuck, "stuck"),
            (JobSnapshotState::Cancelled, "cancelled"),
        ];

        for (state, expected) in states {
            let value = serde_json::to_value(state).expect("job state serializes");
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn p5_protocol_declares_fs_and_command_methods() {
        let matrix =
            CapabilityMatrix::phase_one().with_p5_filesystem_command(AppServerP5Availability {
                filesystem: true,
                command: CommandExecAvailability {
                    exec: true,
                    output_delta_events: true,
                    terminate: false,
                    write: false,
                    resize: false,
                },
            });

        assert_eq!(matrix.filesystem.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.filesystem.methods,
            vec![
                method::FS_READ_FILE.to_string(),
                method::FS_WRITE_FILE.to_string(),
                method::FS_CREATE_DIRECTORY.to_string(),
                method::FS_GET_METADATA.to_string(),
                method::FS_READ_DIRECTORY.to_string(),
                method::FS_REMOVE.to_string(),
                method::FS_COPY.to_string(),
                method::FS_WATCH.to_string(),
                method::FS_UNWATCH.to_string(),
            ]
        );
        assert_eq!(
            matrix.filesystem.events,
            vec![event::FS_CHANGED.to_string()]
        );

        assert_eq!(matrix.command_exec.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.command_exec.methods,
            vec![method::COMMAND_EXEC.to_string()]
        );
        assert_eq!(
            matrix.command_exec.events,
            vec![event::COMMAND_EXEC_OUTPUT_DELTA.to_string()]
        );
        assert!(
            !matrix
                .command_exec
                .methods
                .contains(&method::COMMAND_EXEC_WRITE.to_string())
        );
        assert!(
            !matrix
                .command_exec
                .methods
                .contains(&method::COMMAND_EXEC_TERMINATE.to_string())
        );
        assert!(
            !matrix
                .command_exec
                .methods
                .contains(&method::COMMAND_EXEC_RESIZE.to_string())
        );
    }

    #[test]
    fn p5_protocol_default_availability_keeps_capabilities_declared() {
        let matrix = CapabilityMatrix::phase_one()
            .with_p5_filesystem_command(AppServerP5Availability {
                filesystem: true,
                command: CommandExecAvailability {
                    exec: true,
                    output_delta_events: true,
                    terminate: true,
                    write: true,
                    resize: true,
                },
            })
            .with_p5_filesystem_command(AppServerP5Availability::default());

        assert_eq!(matrix.filesystem.status, CapabilityStatus::Declared);
        assert!(matrix.filesystem.methods.is_empty());
        assert!(matrix.filesystem.events.is_empty());
        assert_eq!(matrix.command_exec.status, CapabilityStatus::Declared);
        assert!(matrix.command_exec.methods.is_empty());
        assert!(matrix.command_exec.events.is_empty());
    }

    #[test]
    fn p5_protocol_declares_all_command_methods_when_enabled() {
        let matrix =
            CapabilityMatrix::phase_one().with_p5_filesystem_command(AppServerP5Availability {
                filesystem: false,
                command: CommandExecAvailability {
                    exec: true,
                    output_delta_events: true,
                    terminate: true,
                    write: true,
                    resize: true,
                },
            });

        assert_eq!(matrix.filesystem.status, CapabilityStatus::Declared);
        assert_eq!(matrix.command_exec.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.command_exec.methods,
            vec![
                method::COMMAND_EXEC.to_string(),
                method::COMMAND_EXEC_WRITE.to_string(),
                method::COMMAND_EXEC_TERMINATE.to_string(),
                method::COMMAND_EXEC_RESIZE.to_string(),
            ]
        );
        assert_eq!(
            matrix.command_exec.events,
            vec![event::COMMAND_EXEC_OUTPUT_DELTA.to_string()]
        );
    }

    #[test]
    fn sandbox_protocol_params_match_codex_wire_shape() {
        let thread: ThreadStartParams = serde_json::from_value(serde_json::json!({
            "cwd": "/repo",
            "sandbox": "workspace-write",
            "permissionProfile": {
                "type": "managed",
                "file_system": {
                    "type": "restricted",
                    "entries": []
                },
                "network": "restricted"
            }
        }))
        .expect("thread/start should accept sandbox fields");

        assert_eq!(thread.sandbox, Some(SandboxMode::WorkspaceWrite));
        assert!(thread.permission_profile.is_some());

        let turn: TurnStartParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "input": [{ "type": "text", "text": "hi" }],
            "sandboxPolicy": { "type": "read-only", "network_access": false },
            "permissionProfile": { "type": "disabled" }
        }))
        .expect("turn/start should accept sandbox fields");

        assert_eq!(turn.sandbox_policy.as_ref().unwrap()["type"], "read-only");
        assert_eq!(
            turn.permission_profile.as_ref().unwrap()["type"],
            "disabled"
        );

        let command: CommandExecParams = serde_json::from_value(serde_json::json!({
            "command": ["sh", "-c", "printf hi"],
            "sandboxPolicy": { "type": "workspace-write" },
            "permissionProfile": { "type": "disabled" }
        }))
        .expect("command/exec should accept permissionProfile");

        assert!(command.sandbox_policy.is_some());
        assert!(command.permission_profile.is_some());
    }

    #[test]
    fn config_requirements_read_is_advertised_in_schema() {
        let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
        let method = schema
            .methods
            .iter()
            .find(|method| method.method == method::CONFIG_REQUIREMENTS_READ)
            .expect("protocol/schema must include configRequirements/read");
        assert_eq!(method.capability, "sandbox");
    }

    #[test]
    fn p5_protocol_serializes_fs_and_command_payloads() {
        let read_params = FsReadFileParams {
            path: "/tmp/example.txt".to_string(),
            offset: Some(2),
            length: Some(4),
        };
        assert_eq!(
            serde_json::to_value(read_params).expect("serialize read params"),
            serde_json::json!({
                "path": "/tmp/example.txt",
                "offset": 2,
                "length": 4
            })
        );

        let read = FsReadFileResponse {
            data_base64: "aGVsbG8=".to_string(),
        };
        assert_eq!(
            serde_json::to_value(read).expect("serialize read response"),
            serde_json::json!({"dataBase64": "aGVsbG8="})
        );

        let write = FsWriteFileParams {
            path: "/tmp/example.txt".to_string(),
            data_base64: "aGVsbG8=".to_string(),
            mode: Some(FsWriteMode::Append),
        };
        assert_eq!(
            serde_json::to_value(write).expect("serialize write params"),
            serde_json::json!({
                "path": "/tmp/example.txt",
                "dataBase64": "aGVsbG8=",
                "mode": "append"
            })
        );

        let exec = CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf hello".to_string(),
            ],
            cwd: Some("/tmp".to_string()),
            timeout_ms: Some(5_000),
            disable_timeout: None,
            output_bytes_cap: Some(1024),
            disable_output_cap: Some(true),
            env: Default::default(),
            process_id: Some("proc_1".to_string()),
            sandbox_policy: None,
            permission_profile: None,
            size: None,
            stream_stdin: None,
            stream_stdout_stderr: Some(true),
            tty: None,
        };
        assert_eq!(
            serde_json::to_value(exec).expect("serialize exec params"),
            serde_json::json!({
                "command": ["sh", "-c", "printf hello"],
                "cwd": "/tmp",
                "timeoutMs": 5000,
                "outputBytesCap": 1024,
                "disableOutputCap": true,
                "processId": "proc_1",
                "streamStdoutStderr": true
            })
        );

        let changed = ServerNotification::fs_changed(FsChangedNotification {
            watch_id: "watch_1".to_string(),
            path: "/tmp/example.txt".to_string(),
            kind: FsChangedKind::Modified,
        })
        .expect("fs changed notification should serialize");
        assert_eq!(changed.method, event::FS_CHANGED);
    }
}
