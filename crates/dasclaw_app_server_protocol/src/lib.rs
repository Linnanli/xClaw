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
    pub const THREAD_RESUME: &str = "thread/resume";
    pub const THREAD_FORK: &str = "thread/fork";
    pub const THREAD_ARCHIVE: &str = "thread/archive";
    pub const THREAD_UNARCHIVE: &str = "thread/unarchive";
    pub const THREAD_UNSUBSCRIBE: &str = "thread/unsubscribe";
    pub const THREAD_NAME_SET: &str = "thread/name/set";
    pub const THREAD_METADATA_UPDATE: &str = "thread/metadata/update";
    pub const THREAD_ROLLBACK: &str = "thread/rollback";
    pub const THREAD_LOADED_LIST: &str = "thread/loaded/list";
    pub const THREAD_INJECT_ITEMS: &str = "thread/inject_items";
    pub const THREAD_SHELL_COMMAND: &str = "thread/shellCommand";
    pub const THREAD_APPROVE_GUARDIAN_DENIED_ACTION: &str = "thread/approveGuardianDeniedAction";
    pub const THREAD_GOAL_SET: &str = "thread/goal/set";
    pub const THREAD_GOAL_GET: &str = "thread/goal/get";
    pub const THREAD_GOAL_CLEAR: &str = "thread/goal/clear";
    pub const THREAD_COMPACT_START: &str = "thread/compact/start";
    pub const THREAD_LIST: &str = "thread/list";
    pub const THREAD_READ: &str = "thread/read";
    pub const THREAD_TURNS_LIST: &str = "thread/turns/list";
    pub const TURN_START: &str = "turn/start";
    pub const TURN_STEER: &str = "turn/steer";
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
    pub const CONFIG_READ: &str = "config/read";
    pub const CONFIG_VALUE_WRITE: &str = "config/value/write";
    pub const CONFIG_BATCH_WRITE: &str = "config/batchWrite";
    pub const CONFIG_MCP_SERVER_RELOAD: &str = "config/mcpServer/reload";
    pub const MCP_SERVER_STATUS_LIST: &str = "mcpServerStatus/list";
    pub const MCP_SERVER_RESOURCE_READ: &str = "mcpServer/resource/read";
    pub const MCP_SERVER_TOOL_CALL: &str = "mcpServer/tool/call";
    pub const GIT_DIFF_TO_REMOTE: &str = "gitDiffToRemote";
    pub const FUZZY_FILE_SEARCH: &str = "fuzzyFileSearch";
    pub const GET_CONVERSATION_SUMMARY: &str = "getConversationSummary";
    pub const REVIEW_START: &str = "review/start";
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
    pub const THREAD_STATUS_CHANGED: &str = "thread/status/changed";
    pub const THREAD_ARCHIVED: &str = "thread/archived";
    pub const THREAD_UNARCHIVED: &str = "thread/unarchived";
    pub const THREAD_NAME_UPDATED: &str = "thread/name/updated";
    pub const THREAD_GOAL_UPDATED: &str = "thread/goal/updated";
    pub const THREAD_GOAL_CLEARED: &str = "thread/goal/cleared";
    pub const THREAD_TOKEN_USAGE_UPDATED: &str = "thread/tokenUsage/updated";
    pub const THREAD_COMPACTED: &str = "thread/compacted";
    pub const TURN_STARTED: &str = "turn/started";
    pub const TURN_PLAN_UPDATED: &str = "turn/plan/updated";
    pub const TURN_DIFF_UPDATED: &str = "turn/diff/updated";
    pub const TURN_COMPLETED: &str = "turn/completed";
    pub const ITEM_STARTED: &str = "item/started";
    pub const ITEM_AGENT_MESSAGE_DELTA: &str = "item/agentMessage/delta";
    pub const ITEM_REASONING_SUMMARY_TEXT_DELTA: &str = "item/reasoning/summaryTextDelta";
    pub const ITEM_REASONING_SUMMARY_PART_ADDED: &str = "item/reasoning/summaryPartAdded";
    pub const ITEM_REASONING_TEXT_DELTA: &str = "item/reasoning/textDelta";
    pub const ITEM_PLAN_DELTA: &str = "item/plan/delta";
    pub const ITEM_COMPLETED: &str = "item/completed";
    pub const RAW_RESPONSE_ITEM_COMPLETED: &str = "rawResponseItem/completed";
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
    pub const FUZZY_FILE_SEARCH_SESSION_UPDATED: &str = "fuzzyFileSearch/sessionUpdated";
    pub const FUZZY_FILE_SEARCH_SESSION_COMPLETED: &str = "fuzzyFileSearch/sessionCompleted";
    pub const MODEL_REROUTED: &str = "model/rerouted";
    pub const MODEL_VERIFICATION: &str = "model/verification";
    pub const HOOK_STARTED: &str = "hook/started";
    pub const HOOK_COMPLETED: &str = "hook/completed";
    pub const WARNING: &str = "warning";
    pub const GUARDIAN_WARNING: &str = "guardianWarning";
    pub const CONFIG_WARNING: &str = "configWarning";
    pub const DEPRECATION_NOTICE: &str = "deprecationNotice";
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
    pub thread_lifecycle: Capability,
    pub thread_goal: Capability,
    pub thread_compact: Capability,
    pub config: Capability,
    pub repo: Capability,
    pub search: Capability,
    pub review: Capability,
    pub hooks: Capability,
    pub warnings: Capability,
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
                    event::TURN_PLAN_UPDATED,
                    event::TURN_DIFF_UPDATED,
                    event::ITEM_STARTED,
                    event::ITEM_AGENT_MESSAGE_DELTA,
                    event::ITEM_REASONING_SUMMARY_TEXT_DELTA,
                    event::ITEM_REASONING_SUMMARY_PART_ADDED,
                    event::ITEM_REASONING_TEXT_DELTA,
                    event::ITEM_PLAN_DELTA,
                    event::RAW_RESPONSE_ITEM_COMPLETED,
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
            config: declared_future_capability("config"),
            repo: declared_future_capability("repo"),
            search: declared_future_capability("search"),
            review: declared_future_capability("review"),
            hooks: declared_future_capability("hooks"),
            warnings: declared_future_capability("warnings"),
            thread_lifecycle: Capability::implemented(
                "thread_lifecycle",
                &[
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
                ],
                &[
                    event::THREAD_STATUS_CHANGED,
                    event::THREAD_ARCHIVED,
                    event::THREAD_UNARCHIVED,
                    event::THREAD_NAME_UPDATED,
                    event::THREAD_TOKEN_USAGE_UPDATED,
                ],
            ),
            thread_goal: Capability::implemented(
                "thread_goal",
                &[
                    method::THREAD_GOAL_SET,
                    method::THREAD_GOAL_GET,
                    method::THREAD_GOAL_CLEAR,
                ],
                &[event::THREAD_GOAL_UPDATED, event::THREAD_GOAL_CLEARED],
            ),
            thread_compact: declared_future_capability("thread_compact"),
        }
    }

    #[must_use]
    pub fn with_p3_approval_tool_sandbox(mut self) -> Self {
        self = self.with_runtime_tool_approval(RuntimeToolApprovalAvailability {
            command_approval: true,
            dynamic_tool_call: false,
            tool_user_input: false,
            permissions_approval: false,
            file_change_approval: false,
            file_change_events: false,
            auto_approval_review: false,
        });
        self = self.with_runtime_command_tool_events_ready();
        self.with_runtime_sandbox_ready()
    }

    #[must_use]
    pub fn with_runtime_command_tool_events_ready(mut self) -> Self {
        self.tools = Capability::implemented(
            "tools",
            &[],
            &[
                event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
                event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
            ],
        );
        self
    }

    #[must_use]
    pub fn with_runtime_tool_approval(
        mut self,
        availability: RuntimeToolApprovalAvailability,
    ) -> Self {
        if availability.command_approval {
            let mut approval_events = vec![
                server_request::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL,
                event::SERVER_REQUEST_RESOLVED,
            ];
            if availability.dynamic_tool_call {
                approval_events.push(server_request::ITEM_TOOL_CALL);
            }
            if availability.tool_user_input {
                approval_events.push(server_request::ITEM_TOOL_REQUEST_USER_INPUT);
            }
            if availability.permissions_approval {
                approval_events.push(server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL);
            }
            if availability.file_change_approval {
                approval_events.push(server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL);
            }
            self.approval =
                Capability::implemented("approval", &[method::APPROVAL_RESPOND], &approval_events);
        }

        if availability.file_change_events || availability.auto_approval_review {
            let mut tool_events = vec![
                event::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA,
                event::ITEM_COMMAND_EXECUTION_TERMINAL_INTERACTION,
            ];
            if availability.file_change_events {
                tool_events.push(event::ITEM_FILE_CHANGE_OUTPUT_DELTA);
                tool_events.push(event::ITEM_FILE_CHANGE_PATCH_UPDATED);
            }
            if availability.auto_approval_review {
                tool_events.push(event::ITEM_AUTO_APPROVAL_REVIEW_STARTED);
                tool_events.push(event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED);
            }
            self.tools = Capability::implemented("tools", &[], &tool_events);
        }

        self
    }

    #[must_use]
    pub fn with_runtime_sandbox_ready(mut self) -> Self {
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
    pub fn with_runtime_turn_steer_ready(mut self) -> Self {
        let method = method::TURN_STEER.to_string();
        if !self.session.methods.contains(&method) {
            self.session.methods.push(method);
        }
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
        if availability.r6.config {
            self.config = Capability::implemented(
                "config",
                &[
                    method::CONFIG_READ,
                    method::CONFIG_VALUE_WRITE,
                    method::CONFIG_BATCH_WRITE,
                ],
                &[],
            );
        }
        if availability.r6.repo {
            self.repo = Capability::implemented("repo", &[method::GIT_DIFF_TO_REMOTE], &[]);
        }
        if availability.r6.search {
            self.search = Capability::implemented(
                "search",
                &[method::FUZZY_FILE_SEARCH],
                &[
                    event::FUZZY_FILE_SEARCH_SESSION_UPDATED,
                    event::FUZZY_FILE_SEARCH_SESSION_COMPLETED,
                ],
            );
        }
        if availability.r6.review {
            self.review = Capability::implemented(
                "review",
                &[method::GET_CONVERSATION_SUMMARY, method::REVIEW_START],
                &[],
            );
        }
        let model_provider_event_names = model_provider_events(availability.r6);
        if !model_provider_event_names.is_empty() {
            self.model_provider = Capability::implemented(
                "model_provider",
                &[
                    method::MODEL_LIST,
                    method::MODEL_PROVIDER_SELECT_FOR_NEXT_TURN,
                ],
                &model_provider_event_names,
            );
        }
        if availability.r6.hooks {
            self.hooks = Capability::implemented(
                "hooks",
                &[],
                &[event::HOOK_STARTED, event::HOOK_COMPLETED],
            );
        }
        let warning_event_names = warning_events(availability.r6);
        if !warning_event_names.is_empty() {
            self.warnings = Capability::implemented("warnings", &[], &warning_event_names);
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

    #[must_use]
    pub fn with_thread_compact_ready(mut self) -> Self {
        self.thread_compact = Capability::implemented(
            "thread_compact",
            &[method::THREAD_COMPACT_START],
            &[event::THREAD_COMPACTED],
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

fn model_provider_events(availability: AppServerR6Availability) -> Vec<&'static str> {
    availability.model_producer_events()
}

fn warning_events(availability: AppServerR6Availability) -> Vec<&'static str> {
    availability.warning_producer_events()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerServiceAvailability {
    pub logs: bool,
    pub jobs: bool,
    pub skills: bool,
    pub mcp: McpServiceAvailability,
    pub p5: AppServerP5Availability,
    pub r6: AppServerR6Availability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerP5Availability {
    pub filesystem: bool,
    pub command: CommandExecAvailability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppServerR6Availability {
    pub config: bool,
    pub repo: bool,
    pub search: bool,
    pub review: bool,
    pub model_events: bool,
    pub model_reroutes: bool,
    pub model_verifications: bool,
    pub hooks: bool,
    pub warnings: bool,
    pub config_warnings: bool,
    pub deprecation_notices: bool,
    pub guardian_warnings: bool,
}

impl AppServerR6Availability {
    #[must_use]
    fn model_producer_events(self) -> Vec<&'static str> {
        let mut events = Vec::new();
        if self.model_events || self.model_reroutes {
            events.push(event::MODEL_REROUTED);
        }
        if self.model_events || self.model_verifications {
            events.push(event::MODEL_VERIFICATION);
        }
        events
    }

    #[must_use]
    fn warning_producer_events(self) -> Vec<&'static str> {
        let mut events = Vec::new();
        if self.warnings {
            events.push(event::WARNING);
        }
        if self.guardian_warnings {
            events.push(event::GUARDIAN_WARNING);
        }
        if self.config_warnings {
            events.push(event::CONFIG_WARNING);
        }
        if self.deprecation_notices {
            events.push(event::DEPRECATION_NOTICE);
        }
        events
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeToolApprovalAvailability {
    pub command_approval: bool,
    pub dynamic_tool_call: bool,
    pub tool_user_input: bool,
    pub permissions_approval: bool,
    pub file_change_approval: bool,
    pub file_change_events: bool,
    pub auto_approval_review: bool,
}

impl RuntimeToolApprovalAvailability {
    #[must_use]
    pub fn all_ready(self) -> bool {
        self.command_approval
            && self.dynamic_tool_call
            && self.tool_user_input
            && self.permissions_approval
            && self.file_change_approval
            && self.file_change_events
            && self.auto_approval_review
    }
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
    pub oauth_login: bool,
    pub tool_call_progress_events: bool,
    pub oauth_login_completed_events: bool,
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
        if self.oauth_login {
            methods.push(method::MCP_SERVER_OAUTH_LOGIN);
        }
        methods
    }

    #[must_use]
    pub fn events(&self) -> Vec<&'static str> {
        let mut events = Vec::new();
        if self.tool_call_progress_events {
            events.push(event::ITEM_MCP_TOOL_CALL_PROGRESS);
        }
        if self.oauth_login_completed_events {
            events.push(event::MCP_SERVER_OAUTH_LOGIN_COMPLETED);
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
    pub fn current() -> Self {
        Self::phase_one(CapabilityMatrix::phase_one())
    }

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
                method::THREAD_RESUME.to_string(),
                method::THREAD_FORK.to_string(),
                method::THREAD_ARCHIVE.to_string(),
                method::THREAD_UNARCHIVE.to_string(),
                method::THREAD_UNSUBSCRIBE.to_string(),
                method::THREAD_NAME_SET.to_string(),
                method::THREAD_METADATA_UPDATE.to_string(),
                method::THREAD_ROLLBACK.to_string(),
                method::THREAD_LOADED_LIST.to_string(),
                method::THREAD_INJECT_ITEMS.to_string(),
                method::THREAD_SHELL_COMMAND.to_string(),
                method::THREAD_APPROVE_GUARDIAN_DENIED_ACTION.to_string(),
                method::THREAD_GOAL_SET.to_string(),
                method::THREAD_GOAL_GET.to_string(),
                method::THREAD_GOAL_CLEAR.to_string(),
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
                CapabilityOptOut::phase_one("codex.tool_calls"),
                CapabilityOptOut::phase_one("tools"),
                CapabilityOptOut::phase_one("mcp"),
                CapabilityOptOut::phase_one("skills"),
                CapabilityOptOut::phase_one("filesystem"),
                CapabilityOptOut::phase_one("command_exec"),
                CapabilityOptOut::phase_one("dlp_policy"),
                CapabilityOptOut::phase_one("jobs"),
                CapabilityOptOut::phase_one("sandbox"),
                CapabilityOptOut::phase_one("thread.compact"),
                CapabilityOptOut::phase_one("codex.rich_input"),
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
    event::TURN_PLAN_UPDATED,
    event::TURN_DIFF_UPDATED,
    event::ITEM_STARTED,
    event::ITEM_AGENT_MESSAGE_DELTA,
    event::ITEM_REASONING_SUMMARY_TEXT_DELTA,
    event::ITEM_REASONING_SUMMARY_PART_ADDED,
    event::ITEM_REASONING_TEXT_DELTA,
    event::ITEM_PLAN_DELTA,
    event::RAW_RESPONSE_ITEM_COMPLETED,
    event::ITEM_COMPLETED,
    event::THREAD_STATUS_CHANGED,
    event::THREAD_ARCHIVED,
    event::THREAD_UNARCHIVED,
    event::THREAD_NAME_UPDATED,
    event::THREAD_GOAL_UPDATED,
    event::THREAD_GOAL_CLEARED,
    event::THREAD_TOKEN_USAGE_UPDATED,
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
            method::CONFIG_READ,
            "config",
            Some("ConfigReadParams"),
            "ConfigReadResponse",
            true,
        ),
        MethodSchema::new(
            method::CONFIG_VALUE_WRITE,
            "config",
            Some("ConfigValueWriteParams"),
            "ConfigWriteResponse",
            true,
        ),
        MethodSchema::new(
            method::CONFIG_BATCH_WRITE,
            "config",
            Some("ConfigBatchWriteParams"),
            "ConfigWriteResponse",
            true,
        ),
        MethodSchema::new(
            method::GIT_DIFF_TO_REMOTE,
            "repo",
            Some("GitDiffToRemoteParams"),
            "GitDiffToRemoteResponse",
            true,
        ),
        MethodSchema::new(
            method::FUZZY_FILE_SEARCH,
            "search",
            Some("FuzzyFileSearchParams"),
            "FuzzyFileSearchResponse",
            true,
        ),
        MethodSchema::new(
            method::GET_CONVERSATION_SUMMARY,
            "review",
            Some("GetConversationSummaryParams"),
            "GetConversationSummaryResponse",
            true,
        ),
        MethodSchema::new(
            method::REVIEW_START,
            "review",
            Some("ReviewStartParams"),
            "ReviewStartResponse",
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
            method::THREAD_RESUME,
            "thread_lifecycle",
            Some("ThreadResumeParams"),
            "ThreadResumeResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_FORK,
            "thread_lifecycle",
            Some("ThreadForkParams"),
            "ThreadForkResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_ARCHIVE,
            "thread_lifecycle",
            Some("ThreadArchiveParams"),
            "ThreadArchiveResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_UNARCHIVE,
            "thread_lifecycle",
            Some("ThreadUnarchiveParams"),
            "ThreadUnarchiveResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_UNSUBSCRIBE,
            "thread_lifecycle",
            Some("ThreadUnsubscribeParams"),
            "ThreadUnsubscribeResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_NAME_SET,
            "thread_lifecycle",
            Some("ThreadSetNameParams"),
            "ThreadSetNameResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_METADATA_UPDATE,
            "thread_lifecycle",
            Some("ThreadMetadataUpdateParams"),
            "ThreadMetadataUpdateResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_ROLLBACK,
            "thread_lifecycle",
            Some("ThreadRollbackParams"),
            "ThreadRollbackResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_LOADED_LIST,
            "thread_lifecycle",
            Some("ThreadLoadedListParams"),
            "ThreadLoadedListResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_INJECT_ITEMS,
            "thread_lifecycle",
            Some("ThreadInjectItemsParams"),
            "ThreadInjectItemsResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_SHELL_COMMAND,
            "session",
            Some("ThreadShellCommandParams"),
            "EmptyResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_APPROVE_GUARDIAN_DENIED_ACTION,
            "session",
            Some("ThreadApproveGuardianDeniedActionParams"),
            "EmptyResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_GOAL_SET,
            "thread_goal",
            Some("ThreadGoalSetParams"),
            "ThreadGoalSetResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_GOAL_GET,
            "thread_goal",
            Some("ThreadGoalGetParams"),
            "ThreadGoalGetResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_GOAL_CLEAR,
            "thread_goal",
            Some("ThreadGoalClearParams"),
            "ThreadGoalClearResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_COMPACT_START,
            "thread_compact",
            Some("ThreadCompactStartParams"),
            "ThreadCompactStartResponse",
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
            method::TURN_STEER,
            "session",
            Some("TurnSteerParams"),
            "TurnSteerResponse",
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
        EventSchema::new(
            event::THREAD_STATUS_CHANGED,
            "thread_lifecycle",
            "ThreadStatusChangedEvent",
        ),
        EventSchema::new(
            event::THREAD_ARCHIVED,
            "thread_lifecycle",
            "ThreadArchivedEvent",
        ),
        EventSchema::new(
            event::THREAD_UNARCHIVED,
            "thread_lifecycle",
            "ThreadUnarchivedEvent",
        ),
        EventSchema::new(
            event::THREAD_NAME_UPDATED,
            "thread_lifecycle",
            "ThreadNameUpdatedEvent",
        ),
        EventSchema::new(
            event::THREAD_GOAL_UPDATED,
            "thread_goal",
            "ThreadGoalUpdatedEvent",
        ),
        EventSchema::new(
            event::THREAD_GOAL_CLEARED,
            "thread_goal",
            "ThreadGoalClearedEvent",
        ),
        EventSchema::new(
            event::THREAD_TOKEN_USAGE_UPDATED,
            "thread_lifecycle",
            "ThreadTokenUsageUpdatedEvent",
        ),
        EventSchema::new(
            event::THREAD_COMPACTED,
            "thread_compact",
            "ThreadCompactedEvent",
        ),
        EventSchema::new(event::TURN_STARTED, "session", "TurnStartedEvent"),
        EventSchema::new(event::TURN_COMPLETED, "session", "TurnCompletedEvent"),
        EventSchema::new(event::TURN_PLAN_UPDATED, "session", "TurnPlanUpdatedEvent"),
        EventSchema::new(event::TURN_DIFF_UPDATED, "session", "TurnDiffUpdatedEvent"),
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
        EventSchema::new(event::ITEM_PLAN_DELTA, "session", "PlanDeltaEvent"),
        EventSchema::new(
            event::RAW_RESPONSE_ITEM_COMPLETED,
            "session",
            "RawResponseItemCompletedEvent",
        ),
        EventSchema::new(event::ITEM_COMPLETED, "session", "ItemCompletedEvent"),
        EventSchema::new(
            event::SERVER_REQUEST_RESOLVED,
            "approval",
            "ServerRequestResolvedEvent",
        ),
        EventSchema::new(
            server_request::ITEM_TOOL_CALL,
            "approval",
            "DynamicToolCallParams",
        ),
        EventSchema::new(
            server_request::ITEM_TOOL_REQUEST_USER_INPUT,
            "approval",
            "ToolRequestUserInputParams",
        ),
        EventSchema::new(
            server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
            "approval",
            "PermissionsRequestApprovalParams",
        ),
        EventSchema::new(
            server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
            "approval",
            "FileChangeRequestApprovalParams",
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
        EventSchema::new(
            event::ITEM_FILE_CHANGE_OUTPUT_DELTA,
            "tools",
            "FileChangeOutputDeltaEvent",
        ),
        EventSchema::new(
            event::ITEM_FILE_CHANGE_PATCH_UPDATED,
            "tools",
            "FileChangePatchUpdatedEvent",
        ),
        EventSchema::new(
            event::ITEM_AUTO_APPROVAL_REVIEW_STARTED,
            "tools",
            "AutoApprovalReviewStartedEvent",
        ),
        EventSchema::new(
            event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED,
            "tools",
            "AutoApprovalReviewCompletedEvent",
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
        EventSchema::new(
            event::FUZZY_FILE_SEARCH_SESSION_UPDATED,
            "search",
            "FuzzyFileSearchSessionUpdatedNotification",
        ),
        EventSchema::new(
            event::FUZZY_FILE_SEARCH_SESSION_COMPLETED,
            "search",
            "FuzzyFileSearchSessionCompletedNotification",
        ),
        EventSchema::new(
            event::MODEL_REROUTED,
            "model_provider",
            "ModelReroutedNotification",
        ),
        EventSchema::new(
            event::MODEL_VERIFICATION,
            "model_provider",
            "ModelVerificationNotification",
        ),
        EventSchema::new(event::HOOK_STARTED, "hooks", "HookStartedNotification"),
        EventSchema::new(event::HOOK_COMPLETED, "hooks", "HookCompletedNotification"),
        EventSchema::new(event::WARNING, "warnings", "WarningNotification"),
        EventSchema::new(
            event::GUARDIAN_WARNING,
            "warnings",
            "GuardianWarningNotification",
        ),
        EventSchema::new(
            event::CONFIG_WARNING,
            "warnings",
            "ConfigWarningNotification",
        ),
        EventSchema::new(
            event::DEPRECATION_NOTICE,
            "warnings",
            "DeprecationNoticeNotification",
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
    Config,
    Repo,
    Search,
    Hooks,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigReadParams {
    pub include_layers: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigReadResponse {
    pub config: serde_json::Value,
    pub origins: std::collections::BTreeMap<String, ConfigLayerMetadata>,
    pub layers: Option<Vec<ConfigLayer>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLayerMetadata {
    pub name: ConfigLayerSource,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLayer {
    pub name: ConfigLayerSource,
    pub version: String,
    pub config: serde_json::Value,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConfigLayerSource {
    System { file: String },
    User { file: String },
    Project { dot_dasclaw_folder: String },
    SessionFlags,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValueWriteParams {
    pub key_path: String,
    pub value: serde_json::Value,
    pub merge_strategy: MergeStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    Replace,
    Upsert,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBatchWriteParams {
    pub edits: Vec<ConfigEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload_user_config: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEdit {
    pub key_path: String,
    pub value: serde_json::Value,
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigWriteResponse {
    pub config: serde_json::Value,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteParams {
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteResponse {
    pub sha: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchParams {
    pub query: String,
    pub roots: Vec<String>,
    pub cancellation_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchResponse {
    pub files: Vec<FuzzyFileSearchResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchResult {
    pub root: String,
    pub path: String,
    pub match_type: FuzzyFileSearchMatchType,
    pub file_name: String,
    pub score: f64,
    pub indices: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuzzyFileSearchMatchType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionUpdatedNotification {
    pub session_id: String,
    pub query: String,
    pub files: Vec<FuzzyFileSearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionCompletedNotification {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum GetConversationSummaryParams {
    RolloutPath { rollout_path: String },
    ConversationId { conversation_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConversationSummaryResponse {
    pub summary: ConversationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub path: String,
    pub preview: String,
    pub timestamp: Option<String>,
    pub updated_at: Option<String>,
    pub model_provider: String,
    pub cwd: String,
    pub cli_version: String,
    pub source: CodexSessionSource,
    pub git_info: Option<CodexGitInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartParams {
    pub thread_id: String,
    pub target: ReviewTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<ReviewDelivery>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ReviewTarget {
    UncommittedChanges,
    BaseBranch { branch: String },
    Commit { sha: String, title: Option<String> },
    Custom { instructions: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartResponse {
    pub turn: CodexTurn,
    pub review_thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelReroutedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub from_model: String,
    pub to_model: String,
    pub reason: ModelRerouteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelRerouteReason {
    HighRiskCyberActivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVerificationNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub verifications: Vec<ModelVerification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelVerification {
    TrustedAccessForCyber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStartedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookCompletedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub run: HookRunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookRunSummary {
    pub id: String,
    pub event_name: HookEventName,
    pub handler_type: HookHandlerType,
    pub execution_mode: HookExecutionMode,
    pub scope: HookScope,
    pub source_path: String,
    pub source: HookSource,
    pub display_order: u64,
    pub status: HookRunStatus,
    pub status_message: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub entries: Vec<HookOutputEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookEventName {
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    SessionStart,
    UserPromptSubmit,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookExecutionMode {
    Sync,
    Async,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookHandlerType {
    Command,
    Prompt,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookScope {
    Thread,
    Turn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookSource {
    System,
    User,
    Project,
    Mdm,
    SessionFlags,
    LegacyManagedConfigFile,
    LegacyManagedConfigMdm,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookRunStatus {
    Running,
    Completed,
    Failed,
    Blocked,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutputEntry {
    pub kind: HookOutputEntryKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookOutputEntryKind {
    Warning,
    Stop,
    Feedback,
    Context,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningNotification {
    pub thread_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianWarningNotification {
    pub thread_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigWarningNotification {
    pub summary: String,
    pub details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeprecationNoticeNotification {
    pub summary: String,
    pub details: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub exclude_turns: bool,
    #[serde(default)]
    pub persist_extended_history: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeResponse {
    pub thread: CodexThread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkParams {
    pub thread_id: String,
    #[serde(default)]
    pub ephemeral: bool,
    #[serde(default)]
    pub exclude_turns: bool,
    #[serde(default)]
    pub persist_extended_history: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkResponse {
    pub thread: CodexThread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnarchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnarchiveResponse {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnsubscribeParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadUnsubscribeStatus {
    NotLoaded,
    NotSubscribed,
    Unsubscribed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnsubscribeResponse {
    pub status: ThreadUnsubscribeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSetNameParams {
    pub thread_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSetNameResponse {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMetadataUpdateParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_info: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMetadataUpdateResponse {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackParams {
    pub thread_id: String,
    pub num_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackResponse {
    pub thread: CodexThread,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadLoadedListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadLoadedListResponse {
    pub data: Vec<String>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadInjectItemsParams {
    pub thread_id: String,
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadInjectItemsResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadShellCommandParams {
    pub thread_id: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadApproveGuardianDeniedActionParams {
    pub thread_id: String,
    pub event: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadGoalStatus {
    Active,
    Paused,
    BudgetLimited,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoal {
    pub thread_id: String,
    pub objective: String,
    pub status: ThreadGoalStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<i64>,
    pub tokens_used: i64,
    pub time_used_seconds: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalSetParams {
    pub thread_id: String,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ThreadGoalStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalSetResponse {
    pub goal: ThreadGoal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalGetParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalGetResponse {
    pub goal: Option<ThreadGoal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalClearParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalClearResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCompactStartParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCompactStartResponse {}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteerParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
    pub expected_turn_id: String,
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
pub struct TurnSteerResponse {
    pub turn_id: String,
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
    #[serde(rename_all = "camelCase")]
    EnteredReviewMode { id: String, review: String },
    #[serde(rename_all = "camelCase")]
    ExitedReviewMode { id: String, review: String },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionsApprovalDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalResponse {
    pub decision: PermissionsApprovalDecision,
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
pub struct ThreadStatusChangedEvent {
    pub thread_id: String,
    pub status: CodexThreadStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchivedEvent {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnarchivedEvent {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadNameUpdatedEvent {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalUpdatedEvent {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub goal: ThreadGoal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalClearedEvent {
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageBreakdown {
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsage {
    pub total: TokenUsageBreakdown,
    pub last: TokenUsageBreakdown,
    pub model_context_window: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsageUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub token_usage: ThreadTokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCompactedEvent {
    pub thread_id: String,
    pub turn_id: String,
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
pub struct TurnPlanUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub explanation: Option<String>,
    pub plan: Vec<TurnPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanStep {
    pub step: String,
    pub status: TurnPlanStepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDiffUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub diff: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawResponseItemCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
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

    pub fn thread_status_changed(
        event: ThreadStatusChangedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_STATUS_CHANGED, event)
    }

    pub fn thread_archived(event: ThreadArchivedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_ARCHIVED, event)
    }

    pub fn thread_unarchived(event: ThreadUnarchivedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_UNARCHIVED, event)
    }

    pub fn thread_name_updated(event: ThreadNameUpdatedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_NAME_UPDATED, event)
    }

    pub fn thread_goal_updated(event: ThreadGoalUpdatedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_GOAL_UPDATED, event)
    }

    pub fn thread_goal_cleared(event: ThreadGoalClearedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_GOAL_CLEARED, event)
    }

    pub fn thread_token_usage_updated(
        event: ThreadTokenUsageUpdatedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_TOKEN_USAGE_UPDATED, event)
    }

    pub fn thread_compacted(event: ThreadCompactedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_COMPACTED, event)
    }

    pub fn turn_started(event: TurnStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_STARTED, event)
    }

    pub fn turn_completed(event: TurnCompletedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_COMPLETED, event)
    }

    pub fn turn_plan_updated(event: TurnPlanUpdatedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_PLAN_UPDATED, event)
    }

    pub fn turn_diff_updated(event: TurnDiffUpdatedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_DIFF_UPDATED, event)
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

    pub fn raw_response_item_completed(
        event: RawResponseItemCompletedEvent,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::RAW_RESPONSE_ITEM_COMPLETED, event)
    }

    pub fn plan_delta(event: PlanDeltaEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_PLAN_DELTA, event)
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

    pub fn fuzzy_file_search_session_updated(
        event: FuzzyFileSearchSessionUpdatedNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::FUZZY_FILE_SEARCH_SESSION_UPDATED, event)
    }

    pub fn fuzzy_file_search_session_completed(
        event: FuzzyFileSearchSessionCompletedNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::FUZZY_FILE_SEARCH_SESSION_COMPLETED, event)
    }

    pub fn model_rerouted(event: ModelReroutedNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::MODEL_REROUTED, event)
    }

    pub fn model_verification(
        event: ModelVerificationNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::MODEL_VERIFICATION, event)
    }

    pub fn hook_started(event: HookStartedNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::HOOK_STARTED, event)
    }

    pub fn hook_completed(event: HookCompletedNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::HOOK_COMPLETED, event)
    }

    pub fn warning(event: WarningNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::WARNING, event)
    }

    pub fn guardian_warning(event: GuardianWarningNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::GUARDIAN_WARNING, event)
    }

    pub fn config_warning(event: ConfigWarningNotification) -> Result<Self, serde_json::Error> {
        Self::new(event::CONFIG_WARNING, event)
    }

    pub fn deprecation_notice(
        event: DeprecationNoticeNotification,
    ) -> Result<Self, serde_json::Error> {
        Self::new(event::DEPRECATION_NOTICE, event)
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
    pub callback_url: String,
    pub state: String,
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

    fn protocol_schema() -> ProtocolSchemaResponse {
        ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one())
    }

    #[test]
    fn r6_protocol_schema_lists_config_repo_search_review_and_warning_contracts() {
        let schema = ProtocolSchemaResponse::current();
        let methods: std::collections::BTreeSet<_> = schema
            .methods
            .iter()
            .map(|entry| entry.method.as_str())
            .collect();
        for method in [
            method::CONFIG_READ,
            method::CONFIG_VALUE_WRITE,
            method::CONFIG_BATCH_WRITE,
            method::GIT_DIFF_TO_REMOTE,
            method::FUZZY_FILE_SEARCH,
            method::GET_CONVERSATION_SUMMARY,
            method::REVIEW_START,
        ] {
            assert!(methods.contains(method), "missing R6 method {method}");
        }

        let events: std::collections::BTreeSet<_> = schema
            .events
            .iter()
            .map(|entry| entry.event.as_str())
            .collect();
        for event in [
            event::FUZZY_FILE_SEARCH_SESSION_UPDATED,
            event::FUZZY_FILE_SEARCH_SESSION_COMPLETED,
            event::MODEL_REROUTED,
            event::MODEL_VERIFICATION,
            event::HOOK_STARTED,
            event::HOOK_COMPLETED,
            event::WARNING,
            event::GUARDIAN_WARNING,
            event::CONFIG_WARNING,
            event::DEPRECATION_NOTICE,
        ] {
            assert!(events.contains(event), "missing R6 event {event}");
        }
    }

    #[test]
    fn r6_core_dtos_serialize_with_codex_wire_shapes() {
        let config_response = ConfigReadResponse {
            config: serde_json::json!({"model": "gpt-5.5"}),
            origins: std::collections::BTreeMap::from([(
                "model".to_string(),
                ConfigLayerMetadata {
                    name: ConfigLayerSource::User {
                        file: "/home/user/.dasclaw/config.json".to_string(),
                    },
                    version: "v1".to_string(),
                },
            )]),
            layers: Some(vec![ConfigLayer {
                name: ConfigLayerSource::Project {
                    dot_dasclaw_folder: "/repo/.dasclaw".to_string(),
                },
                version: "v2".to_string(),
                config: serde_json::json!({"sandbox": "workspace-write"}),
                disabled_reason: None,
            }]),
        };
        assert_eq!(
            serde_json::to_value(config_response).expect("config response should serialize"),
            serde_json::json!({
                "config": {"model": "gpt-5.5"},
                "origins": {
                    "model": {
                        "name": {"type": "user", "file": "/home/user/.dasclaw/config.json"},
                        "version": "v1"
                    }
                },
                "layers": [{
                    "name": {
                        "type": "project",
                        "dotDasclawFolder": "/repo/.dasclaw"
                    },
                    "version": "v2",
                    "config": {"sandbox": "workspace-write"},
                    "disabledReason": null
                }]
            })
        );

        assert_eq!(
            serde_json::to_value(GitDiffToRemoteParams {
                cwd: "/repo".to_string(),
            })
            .expect("git diff params should serialize"),
            serde_json::json!({"cwd": "/repo"})
        );
        assert_eq!(
            serde_json::to_value(GitDiffToRemoteResponse {
                sha: "abc123".to_string(),
                diff: "diff --git a/file b/file".to_string(),
            })
            .expect("git diff response should serialize"),
            serde_json::json!({"sha": "abc123", "diff": "diff --git a/file b/file"})
        );

        let search_response = FuzzyFileSearchResponse {
            files: vec![FuzzyFileSearchResult {
                root: "/repo".to_string(),
                path: "src/lib.rs".to_string(),
                match_type: FuzzyFileSearchMatchType::File,
                file_name: "lib.rs".to_string(),
                score: 0.92,
                indices: Some(vec![0, 4]),
            }],
        };
        assert_eq!(
            serde_json::to_value(FuzzyFileSearchParams {
                query: "lib".to_string(),
                roots: vec!["/repo".to_string()],
                cancellation_token: Some("cancel-1".to_string()),
            })
            .expect("search params should serialize"),
            serde_json::json!({
                "query": "lib",
                "roots": ["/repo"],
                "cancellationToken": "cancel-1"
            })
        );
        assert_eq!(
            serde_json::to_value(search_response).expect("search response should serialize"),
            serde_json::json!({
                "files": [{
                    "root": "/repo",
                    "path": "src/lib.rs",
                    "matchType": "file",
                    "fileName": "lib.rs",
                    "score": 0.92,
                    "indices": [0, 4]
                }]
            })
        );

        assert_eq!(
            serde_json::to_value(GetConversationSummaryParams::RolloutPath {
                rollout_path: "/tmp/rollout.jsonl".to_string(),
            })
            .expect("summary params should serialize"),
            serde_json::json!({"rolloutPath": "/tmp/rollout.jsonl"})
        );
        assert_eq!(
            serde_json::to_value(GetConversationSummaryParams::ConversationId {
                conversation_id: "conv-1".to_string(),
            })
            .expect("summary params should serialize"),
            serde_json::json!({"conversationId": "conv-1"})
        );

        let review_turn = CodexTurn::in_progress("turn-review");
        assert_eq!(
            serde_json::to_value(ReviewStartParams {
                thread_id: "thread-1".to_string(),
                target: ReviewTarget::BaseBranch {
                    branch: "main".to_string(),
                },
                delivery: Some(ReviewDelivery::Inline),
            })
            .expect("review params should serialize"),
            serde_json::json!({
                "threadId": "thread-1",
                "target": {"type": "baseBranch", "branch": "main"},
                "delivery": "inline"
            })
        );
        assert_eq!(
            serde_json::to_value(ReviewStartResponse {
                turn: review_turn,
                review_thread_id: "review-thread-1".to_string(),
            })
            .expect("review response should serialize"),
            serde_json::json!({
                "turn": {
                    "id": "turn-review",
                    "items": [],
                    "status": "inProgress",
                    "error": null,
                    "startedAt": null,
                    "completedAt": null,
                    "durationMs": null
                },
                "reviewThreadId": "review-thread-1"
            })
        );
    }

    #[test]
    fn codex_thread_item_review_mode_items_roundtrip() {
        let entered = CodexThreadItem::EnteredReviewMode {
            id: "item-enter".to_string(),
            review: "review local changes".to_string(),
        };
        let exited = CodexThreadItem::ExitedReviewMode {
            id: "item-exit".to_string(),
            review: "review complete".to_string(),
        };

        let entered_json = serde_json::to_value(&entered).expect("entered item serializes");
        let exited_json = serde_json::to_value(&exited).expect("exited item serializes");

        assert_eq!(
            entered_json,
            serde_json::json!({
                "type": "enteredReviewMode",
                "id": "item-enter",
                "review": "review local changes"
            })
        );
        assert_eq!(
            exited_json,
            serde_json::json!({
                "type": "exitedReviewMode",
                "id": "item-exit",
                "review": "review complete"
            })
        );
        assert_eq!(
            serde_json::from_value::<CodexThreadItem>(entered_json).expect("entered roundtrip"),
            entered
        );
        assert_eq!(
            serde_json::from_value::<CodexThreadItem>(exited_json).expect("exited roundtrip"),
            exited
        );
    }

    #[test]
    fn r6_model_hook_and_warning_dtos_serialize_with_codex_wire_shapes() {
        assert_eq!(
            serde_json::to_value(ModelVerificationNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                verifications: vec![ModelVerification::TrustedAccessForCyber],
            })
            .expect("model verification should serialize"),
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "verifications": ["trustedAccessForCyber"]
            })
        );

        let hook = HookStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            run: HookRunSummary {
                id: "hook-run-1".to_string(),
                event_name: HookEventName::PreToolUse,
                handler_type: HookHandlerType::Command,
                execution_mode: HookExecutionMode::Sync,
                scope: HookScope::Turn,
                source_path: "/tmp/hook.sh".to_string(),
                source: HookSource::Project,
                display_order: 1,
                status: HookRunStatus::Running,
                status_message: None,
                started_at: 1,
                completed_at: Some(2),
                duration_ms: Some(1),
                entries: vec![HookOutputEntry {
                    kind: HookOutputEntryKind::Warning,
                    text: "check this".to_string(),
                }],
            },
        };
        assert_eq!(
            serde_json::to_value(hook).expect("hook started should serialize"),
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "run": {
                    "id": "hook-run-1",
                    "eventName": "preToolUse",
                    "handlerType": "command",
                    "executionMode": "sync",
                    "scope": "turn",
                    "sourcePath": "/tmp/hook.sh",
                    "source": "project",
                    "displayOrder": 1,
                    "status": "running",
                    "statusMessage": null,
                    "startedAt": 1,
                    "completedAt": 2,
                    "durationMs": 1,
                    "entries": [{"kind": "warning", "text": "check this"}]
                }
            })
        );

        assert_eq!(
            serde_json::to_value(WarningNotification {
                thread_id: None,
                message: "config file ignored".to_string(),
            })
            .expect("warning should serialize"),
            serde_json::json!({"threadId": null, "message": "config file ignored"})
        );
        assert_eq!(
            serde_json::to_value(ConfigWarningNotification {
                summary: "unsupported config key".to_string(),
                details: Some("experimental.foo is ignored".to_string()),
                path: Some("/tmp/config.json".to_string()),
                range: Some(TextRange {
                    start_line: 1,
                    start_column: 2,
                    end_line: 1,
                    end_column: 9,
                }),
            })
            .expect("config warning should serialize"),
            serde_json::json!({
                "summary": "unsupported config key",
                "details": "experimental.foo is ignored",
                "path": "/tmp/config.json",
                "range": {
                    "startLine": 1,
                    "startColumn": 2,
                    "endLine": 1,
                    "endColumn": 9
                }
            })
        );
        assert_eq!(
            serde_json::to_value(DeprecationNoticeNotification {
                summary: "old key is deprecated".to_string(),
                details: Some("use newKey".to_string()),
            })
            .expect("deprecation notice should serialize"),
            serde_json::json!({
                "summary": "old key is deprecated",
                "details": "use newKey"
            })
        );
    }

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

    #[test]
    fn r4_thread_lifecycle_methods_and_events_are_registered() {
        assert_r4_thread_lifecycle_methods_and_events_are_registered();
    }

    #[test]
    fn protocol_declares_thread_shell_command_and_guardian_replay_methods() {
        assert_eq!(method::THREAD_SHELL_COMMAND, "thread/shellCommand");
        assert_eq!(
            method::THREAD_APPROVE_GUARDIAN_DENIED_ACTION,
            "thread/approveGuardianDeniedAction"
        );

        let methods = phase_one_methods()
            .into_iter()
            .map(|method| method.method)
            .collect::<std::collections::BTreeSet<_>>();

        assert!(methods.contains(method::THREAD_SHELL_COMMAND));
        assert!(methods.contains("thread/shellCommand"));
        assert!(methods.contains(method::THREAD_APPROVE_GUARDIAN_DENIED_ACTION));
        assert!(methods.contains("thread/approveGuardianDeniedAction"));
    }

    #[test]
    fn thread_shell_command_params_serialize_in_codex_shape() {
        let params = ThreadShellCommandParams {
            thread_id: "thread_1".to_string(),
            command: "cargo check -p dasclaw_app_server --tests".to_string(),
        };

        let value = serde_json::to_value(params).expect("thread shell command params serialize");

        assert_eq!(value["threadId"], "thread_1");
        assert_eq!(
            value["command"],
            "cargo check -p dasclaw_app_server --tests"
        );
        assert!(value.get("thread_id").is_none());
    }

    #[test]
    fn guardian_replay_params_accept_serialized_guardian_event() {
        let params: ThreadApproveGuardianDeniedActionParams =
            serde_json::from_value(serde_json::json!({
                "threadId": "thread_1",
                "event": {
                    "id": "guardian_1",
                    "turn_id": "turn_1",
                    "status": "denied",
                    "risk_level": "high",
                    "user_authorization": "low",
                    "rationale": "command denied by guardian",
                    "decision_source": "agent",
                    "action": {
                        "type": "command",
                        "source": "shell",
                        "command": "rm -rf target",
                        "cwd": "/tmp/workspace"
                    }
                }
            }))
            .expect("guardian replay params deserialize");

        let value =
            serde_json::to_value(params).expect("guardian replay params serialize back to JSON");

        assert_eq!(
            value,
            serde_json::json!({
                "threadId": "thread_1",
                "event": {
                    "id": "guardian_1",
                    "turn_id": "turn_1",
                    "status": "denied",
                    "risk_level": "high",
                    "user_authorization": "low",
                    "rationale": "command denied by guardian",
                    "decision_source": "agent",
                    "action": {
                        "type": "command",
                        "source": "shell",
                        "command": "rm -rf target",
                        "cwd": "/tmp/workspace"
                    }
                }
            })
        );
        assert!(value.get("thread_id").is_none());
    }

    #[test]
    fn r4_thread_lifecycle_payloads_use_camel_case_wire_names() {
        assert_r4_thread_lifecycle_payloads_use_camel_case_wire_names();
    }

    #[test]
    fn phase_one_schema_capabilities_reference_declared_matrix_ids() {
        assert_phase_one_schema_capabilities_reference_declared_matrix_ids();
    }

    #[test]
    fn r1_capability_helper_advertises_runtime_tool_approval_surface_only_when_ready() {
        let base = CapabilityMatrix::phase_one();
        assert_eq!(base.approval.status, CapabilityStatus::Declared);
        assert_eq!(base.tools.status, CapabilityStatus::Declared);

        let ready = CapabilityMatrix::phase_one().with_runtime_tool_approval(
            RuntimeToolApprovalAvailability {
                command_approval: true,
                dynamic_tool_call: true,
                tool_user_input: true,
                permissions_approval: true,
                file_change_approval: true,
                file_change_events: true,
                auto_approval_review: true,
            },
        );

        assert_eq!(ready.approval.status, CapabilityStatus::Implemented);
        assert_eq!(ready.tools.status, CapabilityStatus::Implemented);
        assert!(
            ready
                .approval
                .methods
                .contains(&method::APPROVAL_RESPOND.to_string())
        );
        assert!(
            ready
                .approval
                .events
                .contains(&server_request::ITEM_TOOL_CALL.to_string())
        );
        assert!(
            ready
                .approval
                .events
                .contains(&server_request::ITEM_TOOL_REQUEST_USER_INPUT.to_string())
        );
        assert!(
            ready
                .approval
                .events
                .contains(&server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL.to_string())
        );
        assert!(
            ready
                .approval
                .events
                .contains(&server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL.to_string())
        );
        assert!(
            ready
                .tools
                .events
                .contains(&event::ITEM_FILE_CHANGE_OUTPUT_DELTA.to_string())
        );
        assert!(
            ready
                .tools
                .events
                .contains(&event::ITEM_FILE_CHANGE_PATCH_UPDATED.to_string())
        );
        assert!(
            ready
                .tools
                .events
                .contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED.to_string())
        );
        assert!(
            ready
                .tools
                .events
                .contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED.to_string())
        );

        let partial = CapabilityMatrix::phase_one().with_runtime_tool_approval(
            RuntimeToolApprovalAvailability {
                command_approval: true,
                dynamic_tool_call: false,
                tool_user_input: false,
                permissions_approval: false,
                file_change_approval: false,
                file_change_events: false,
                auto_approval_review: false,
            },
        );

        assert_eq!(partial.approval.status, CapabilityStatus::Implemented);
        assert_eq!(partial.tools.status, CapabilityStatus::Declared);
        assert!(
            [
                server_request::ITEM_TOOL_CALL,
                server_request::ITEM_TOOL_REQUEST_USER_INPUT,
                server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL,
                server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL,
            ]
            .into_iter()
            .all(|event| partial
                .approval
                .events
                .iter()
                .all(|advertised| advertised != event))
        );
        assert!(
            [
                event::ITEM_FILE_CHANGE_OUTPUT_DELTA,
                event::ITEM_FILE_CHANGE_PATCH_UPDATED,
                event::ITEM_AUTO_APPROVAL_REVIEW_STARTED,
                event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED,
            ]
            .into_iter()
            .all(|event| partial
                .tools
                .events
                .iter()
                .all(|advertised| advertised != event))
        );
    }

    #[test]
    fn phase_one_schema_includes_r1_server_requests_and_events() {
        let schema = ProtocolSchemaResponse::phase_one(
            CapabilityMatrix::phase_one().with_runtime_tool_approval(
                RuntimeToolApprovalAvailability {
                    command_approval: true,
                    dynamic_tool_call: true,
                    tool_user_input: true,
                    permissions_approval: true,
                    file_change_approval: true,
                    file_change_events: true,
                    auto_approval_review: true,
                },
            ),
        );
        let event_names = schema
            .events
            .iter()
            .map(|event| event.event.as_str())
            .collect::<Vec<_>>();

        assert!(event_names.contains(&server_request::ITEM_TOOL_CALL));
        assert!(event_names.contains(&server_request::ITEM_TOOL_REQUEST_USER_INPUT));
        assert!(event_names.contains(&server_request::ITEM_PERMISSIONS_REQUEST_APPROVAL));
        assert!(event_names.contains(&server_request::ITEM_FILE_CHANGE_REQUEST_APPROVAL));
        assert!(event_names.contains(&event::ITEM_FILE_CHANGE_OUTPUT_DELTA));
        assert!(event_names.contains(&event::ITEM_FILE_CHANGE_PATCH_UPDATED));
        assert!(event_names.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_STARTED));
        assert!(event_names.contains(&event::ITEM_AUTO_APPROVAL_REVIEW_COMPLETED));
    }

    fn assert_r4_thread_lifecycle_methods_and_events_are_registered() {
        let schema = protocol_schema();
        let methods: std::collections::BTreeSet<_> = schema
            .methods
            .iter()
            .map(|method| method.method.as_str())
            .collect();
        let events: std::collections::BTreeSet<_> = schema
            .events
            .iter()
            .map(|event| event.event.as_str())
            .collect();

        for method in [
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
        ] {
            assert!(methods.contains(method), "{method} missing from schema");
        }

        for event in [
            event::THREAD_STATUS_CHANGED,
            event::THREAD_ARCHIVED,
            event::THREAD_UNARCHIVED,
            event::THREAD_NAME_UPDATED,
            event::THREAD_GOAL_UPDATED,
            event::THREAD_GOAL_CLEARED,
            event::THREAD_TOKEN_USAGE_UPDATED,
            event::THREAD_COMPACTED,
        ] {
            assert!(events.contains(event), "{event} missing from schema");
        }
    }

    fn assert_r4_thread_lifecycle_payloads_use_camel_case_wire_names() {
        let unsubscribe = serde_json::to_value(ThreadUnsubscribeResponse {
            status: ThreadUnsubscribeStatus::NotSubscribed,
        })
        .expect("unsubscribe response serializes");
        assert_eq!(unsubscribe["status"], "notSubscribed");

        let metadata: ThreadMetadataUpdateParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "gitInfo": {
                "sha": null,
                "branch": "main",
                "originUrl": "https://example.test/repo.git"
            }
        }))
        .expect("metadata update params deserialize");
        assert_eq!(metadata.thread_id, "thread_1");
        assert_eq!(
            metadata.git_info.expect("git info patch present")["sha"],
            serde_json::Value::Null
        );

        let usage = ThreadTokenUsageUpdatedEvent {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            token_usage: ThreadTokenUsage {
                total: TokenUsageBreakdown {
                    total_tokens: 10,
                    input_tokens: 6,
                    cached_input_tokens: 2,
                    output_tokens: 4,
                    reasoning_output_tokens: 0,
                },
                last: TokenUsageBreakdown {
                    total_tokens: 10,
                    input_tokens: 6,
                    cached_input_tokens: 2,
                    output_tokens: 4,
                    reasoning_output_tokens: 0,
                },
                model_context_window: None,
            },
        };
        let value = serde_json::to_value(usage).expect("usage event serializes");
        assert_eq!(value["threadId"], "thread_1");
        assert_eq!(value["tokenUsage"]["last"]["cachedInputTokens"], 2);

        let goal_status =
            serde_json::to_value(ThreadGoalStatus::BudgetLimited).expect("goal status serializes");
        assert_eq!(goal_status, "budgetLimited");

        let goal = ThreadGoal {
            thread_id: "thread_1".to_string(),
            objective: "ship R4".to_string(),
            status: ThreadGoalStatus::BudgetLimited,
            token_budget: Some(1000),
            tokens_used: 250,
            time_used_seconds: 60,
            created_at: 1,
            updated_at: 2,
        };
        let goal_value = serde_json::to_value(goal).expect("goal serializes");
        assert_eq!(goal_value["threadId"], "thread_1");
        assert_eq!(goal_value["objective"], "ship R4");
        assert_eq!(goal_value["tokenBudget"], 1000);

        let rollback = serde_json::to_value(ThreadRollbackParams {
            thread_id: "thread_1".to_string(),
            num_turns: 2,
        })
        .expect("rollback params serialize");
        assert_eq!(rollback["threadId"], "thread_1");
        assert_eq!(rollback["numTurns"], 2);

        let compacted = serde_json::to_value(ThreadCompactedEvent {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
        })
        .expect("compacted event serializes");
        assert_eq!(compacted["threadId"], "thread_1");
        assert_eq!(compacted["turnId"], "turn_1");

        let loaded_list_params = serde_json::to_value(ThreadLoadedListParams {
            cursor: Some("cursor_1".to_string()),
            limit: Some(25),
        })
        .expect("loaded list params serialize");
        assert_eq!(loaded_list_params["cursor"], "cursor_1");
        assert_eq!(loaded_list_params["limit"], 25);

        let loaded_list_response = serde_json::to_value(ThreadLoadedListResponse {
            data: vec!["thread_1".to_string()],
            next_cursor: Some("cursor_2".to_string()),
        })
        .expect("loaded list response serializes");
        assert_eq!(loaded_list_response["data"][0], "thread_1");
        assert_eq!(loaded_list_response["nextCursor"], "cursor_2");

        let goal_updated = serde_json::to_value(ThreadGoalUpdatedEvent {
            thread_id: "thread_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            goal: ThreadGoal {
                thread_id: "thread_1".to_string(),
                objective: "ship R4".to_string(),
                status: ThreadGoalStatus::Active,
                token_budget: None,
                tokens_used: 0,
                time_used_seconds: 0,
                created_at: 1,
                updated_at: 2,
            },
        })
        .expect("goal updated event serializes");
        assert_eq!(goal_updated["threadId"], "thread_1");
        assert_eq!(goal_updated["turnId"], "turn_1");
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
            !matrix
                .session
                .methods
                .contains(&method::TURN_STEER.to_string()),
            "turn/steer is routable but only advertised after runtime bridge feature readiness"
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
        for event in [
            event::TURN_PLAN_UPDATED,
            event::TURN_DIFF_UPDATED,
            event::ITEM_PLAN_DELTA,
            event::RAW_RESPONSE_ITEM_COMPLETED,
            event::ITEM_REASONING_SUMMARY_PART_ADDED,
            event::ITEM_REASONING_TEXT_DELTA,
        ] {
            assert!(
                matrix.session.events.contains(&event.to_string()),
                "R5 implemented session event should be advertised: {event}"
            );
        }
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
        assert!(method_names.contains(&method::TURN_STEER));
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
        assert!(event_names.contains(&event::TURN_PLAN_UPDATED));
        assert!(event_names.contains(&event::TURN_DIFF_UPDATED));
        assert!(event_names.contains(&event::ITEM_STARTED));
        assert!(event_names.contains(&event::ITEM_AGENT_MESSAGE_DELTA));
        assert!(event_names.contains(&event::ITEM_PLAN_DELTA));
        assert!(event_names.contains(&event::RAW_RESPONSE_ITEM_COMPLETED));
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
        for method in [
            method::THREAD_RESUME,
            method::THREAD_FORK,
            method::THREAD_ARCHIVE,
            method::THREAD_ROLLBACK,
            method::THREAD_GOAL_SET,
            method::THREAD_GOAL_GET,
            method::THREAD_GOAL_CLEAR,
        ] {
            assert!(
                profile.methods.contains(&method.to_string()),
                "implemented R4 method should be in compatibility profile: {method}"
            );
        }
        for event in [
            event::THREAD_ARCHIVED,
            event::THREAD_STATUS_CHANGED,
            event::THREAD_GOAL_UPDATED,
            event::THREAD_TOKEN_USAGE_UPDATED,
        ] {
            assert!(
                profile.events.contains(&event.to_string()),
                "implemented R4 event should be in compatibility profile: {event}"
            );
        }
        assert!(
            !profile
                .methods
                .contains(&method::THREAD_COMPACT_START.to_string())
        );
        assert!(
            !profile
                .events
                .contains(&event::THREAD_COMPACTED.to_string())
        );
        assert!(
            profile
                .capability_opt_outs
                .iter()
                .any(|opt_out| opt_out.capability == "thread.compact")
        );
        for capability in [
            "thread.fork",
            "thread.archive",
            "thread.resume",
            "thread.rollback",
        ] {
            assert!(
                !profile
                    .capability_opt_outs
                    .iter()
                    .any(|opt_out| opt_out.capability == capability),
                "implemented R4 capability should not remain opted out: {capability}"
            );
        }
    }

    #[test]
    fn runtime_turn_steer_capability_is_feature_gated() {
        let base = CapabilityMatrix::phase_one();
        assert!(
            !base
                .session
                .methods
                .contains(&method::TURN_STEER.to_string())
        );

        let ready = base.with_runtime_turn_steer_ready();
        assert!(
            ready
                .session
                .methods
                .contains(&method::TURN_STEER.to_string())
        );
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
            schema.capabilities.thread_lifecycle.methods,
            schema.capabilities.thread_goal.methods,
        ]
        .concat();

        for method in implemented_methods {
            assert!(
                schema_methods.contains(&method.as_str()),
                "implemented capability method must be described by protocol/schema: {method}"
            );
        }
    }

    fn assert_phase_one_schema_capabilities_reference_declared_matrix_ids() {
        let schema = ProtocolSchemaResponse::phase_one(CapabilityMatrix::phase_one());
        let capability_ids = [
            schema.capabilities.protocol.id.as_str(),
            schema.capabilities.lifecycle.id.as_str(),
            schema.capabilities.health.id.as_str(),
            schema.capabilities.session.id.as_str(),
            schema.capabilities.approval.id.as_str(),
            schema.capabilities.dlp_policy.id.as_str(),
            schema.capabilities.model_provider.id.as_str(),
            schema.capabilities.tools.id.as_str(),
            schema.capabilities.jobs.id.as_str(),
            schema.capabilities.skills.id.as_str(),
            schema.capabilities.mcp.id.as_str(),
            schema.capabilities.sandbox.id.as_str(),
            schema.capabilities.logs.id.as_str(),
            schema.capabilities.filesystem.id.as_str(),
            schema.capabilities.command_exec.id.as_str(),
            schema.capabilities.thread_lifecycle.id.as_str(),
            schema.capabilities.thread_goal.id.as_str(),
            schema.capabilities.thread_compact.id.as_str(),
            schema.capabilities.config.id.as_str(),
            schema.capabilities.repo.id.as_str(),
            schema.capabilities.search.id.as_str(),
            schema.capabilities.review.id.as_str(),
            schema.capabilities.hooks.id.as_str(),
            schema.capabilities.warnings.id.as_str(),
        ];

        for method in &schema.methods {
            assert!(
                capability_ids.contains(&method.capability.as_str()),
                "method {} references undeclared capability {}",
                method.method,
                method.capability
            );
        }
        for event in &schema.events {
            assert!(
                capability_ids.contains(&event.capability.as_str()),
                "event {} references undeclared capability {}",
                event.event,
                event.capability
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
        for method in [
            method::INITIALIZE,
            method::THREAD_START,
            method::THREAD_READ,
            method::THREAD_LIST,
            method::THREAD_TURNS_LIST,
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
            method::TURN_START,
            method::TURN_INTERRUPT,
            method::TURN_READ,
            method::MODEL_LIST,
        ] {
            assert!(
                profile.methods.contains(&method.to_string()),
                "implemented chat-session method should be in compatibility profile: {method}"
            );
        }
        for event in [
            event::TURN_PLAN_UPDATED,
            event::TURN_DIFF_UPDATED,
            event::RAW_RESPONSE_ITEM_COMPLETED,
            event::ITEM_PLAN_DELTA,
            event::ITEM_REASONING_SUMMARY_PART_ADDED,
            event::ITEM_REASONING_TEXT_DELTA,
        ] {
            assert!(
                profile.events.contains(&event.to_string()),
                "implemented R5 event should be in compatibility profile: {event}"
            );
        }
        for capability in ["codex.diff", "codex.plan"] {
            assert!(
                !profile
                    .capability_opt_outs
                    .iter()
                    .any(|opt_out| opt_out.capability == capability),
                "implemented R5/profile capability should not remain opted out: {capability}"
            );
        }
        assert!(
            !profile.methods.contains(&method::TURN_STEER.to_string()),
            "turn/steer remains feature-gated and is not in the default chat-session profile"
        );
        assert!(
            profile
                .capability_opt_outs
                .iter()
                .any(|opt_out| opt_out.capability == "codex.rich_input"),
            "full rich input remains opted out while runtime prompt injection requires text"
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
                id: Some(serde_json::json!("steer")),
                method: method::TURN_STEER.to_string(),
                params: Some(serde_json::json!({
                    "threadId": "thread_1",
                    "expectedTurnId": "turn_1",
                    "input": [{"type": "text", "text": "continue", "text_elements": []}]
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
                method::TURN_STEER,
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
            ServerNotification::turn_plan_updated(TurnPlanUpdatedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                explanation: Some("Adjusting plan".to_string()),
                plan: vec![TurnPlanStep {
                    step: "inspect current producer".to_string(),
                    status: TurnPlanStepStatus::InProgress,
                }],
            })
            .expect("turn/plan/updated fixture should serialize"),
            ServerNotification::turn_diff_updated(TurnDiffUpdatedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                diff: "--- a/file\n+++ b/file\n".to_string(),
            })
            .expect("turn/diff/updated fixture should serialize"),
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
            ServerNotification::plan_delta(PlanDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1:plan".to_string(),
                delta: "- inspect\n".to_string(),
            })
            .expect("item/plan/delta fixture should serialize"),
            ServerNotification::raw_response_item_completed(RawResponseItemCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item: serde_json::json!({"id": "raw_1", "type": "reasoning"}),
            })
            .expect("rawResponseItem/completed fixture should serialize"),
            ServerNotification::item_completed(ItemCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item: CodexThreadItem::completed_agent_message("turn_1", "hello"),
            })
            .expect("item/completed fixture should serialize"),
            ServerNotification::thread_status_changed(ThreadStatusChangedEvent {
                thread_id: "thread_1".to_string(),
                status: CodexThreadStatus::Idle,
            })
            .expect("thread/status fixture should serialize"),
            ServerNotification::thread_archived(ThreadArchivedEvent {
                thread_id: "thread_1".to_string(),
            })
            .expect("thread/archived fixture should serialize"),
            ServerNotification::thread_unarchived(ThreadUnarchivedEvent {
                thread_id: "thread_1".to_string(),
            })
            .expect("thread/unarchived fixture should serialize"),
            ServerNotification::thread_name_updated(ThreadNameUpdatedEvent {
                thread_id: "thread_1".to_string(),
                thread_name: Some("thread name".to_string()),
            })
            .expect("thread/name fixture should serialize"),
            ServerNotification::thread_goal_updated(ThreadGoalUpdatedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: Some("turn_1".to_string()),
                goal: ThreadGoal {
                    thread_id: "thread_1".to_string(),
                    objective: "finish task".to_string(),
                    status: ThreadGoalStatus::Active,
                    token_budget: Some(1000),
                    tokens_used: 10,
                    time_used_seconds: 1,
                    created_at: 1,
                    updated_at: 2,
                },
            })
            .expect("thread/goal updated fixture should serialize"),
            ServerNotification::thread_goal_cleared(ThreadGoalClearedEvent {
                thread_id: "thread_1".to_string(),
            })
            .expect("thread/goal cleared fixture should serialize"),
            ServerNotification::thread_token_usage_updated(ThreadTokenUsageUpdatedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                token_usage: ThreadTokenUsage {
                    total: TokenUsageBreakdown {
                        total_tokens: 10,
                        input_tokens: 6,
                        cached_input_tokens: 2,
                        output_tokens: 4,
                        reasoning_output_tokens: 1,
                    },
                    last: TokenUsageBreakdown {
                        total_tokens: 10,
                        input_tokens: 6,
                        cached_input_tokens: 2,
                        output_tokens: 4,
                        reasoning_output_tokens: 1,
                    },
                    model_context_window: Some(128000),
                },
            })
            .expect("thread/token usage fixture should serialize"),
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

        for event_name in event_names {
            assert!(
                CODEX_APP_SERVER_V2_CHAT_SESSION_SUBSET_EVENTS.contains(&event_name),
                "fixture event should be declared in the Codex v2 profile: {event_name}"
            );
        }
        assert_eq!(events[0].params["eventQueue"]["overflow"], "lag_disconnect");
        assert_eq!(events[3].params["thread"]["id"], "thread_1");
        assert_eq!(events[4].params["turn"]["id"], "turn_1");
        assert_eq!(events[5].params["turn"]["status"], "completed");
        assert_eq!(events[6].method, event::TURN_PLAN_UPDATED);
        assert_eq!(events[6].params["plan"][0]["status"], "inProgress");
        assert_eq!(events[7].method, event::TURN_DIFF_UPDATED);
        assert_eq!(events[7].params["diff"], "--- a/file\n+++ b/file\n");
        assert_eq!(events[8].params["item"]["type"], "agentMessage");
        assert_eq!(events[9].params["delta"], "hel");
        assert_eq!(events[10].params["delta"], "scratch");
        assert_eq!(events[11].method, event::ITEM_REASONING_SUMMARY_PART_ADDED);
        assert_eq!(events[11].params["summaryIndex"], 1);
        assert_eq!(events[12].method, event::ITEM_REASONING_TEXT_DELTA);
        assert_eq!(events[12].params["contentIndex"], 0);
        assert_eq!(events[12].params["delta"], "raw scratch");
        assert_eq!(events[13].method, event::ITEM_PLAN_DELTA);
        assert_eq!(events[13].params["delta"], "- inspect\n");
        assert_eq!(events[14].method, event::RAW_RESPONSE_ITEM_COMPLETED);
        assert_eq!(events[14].params["item"]["id"], "raw_1");
    }

    #[test]
    fn r5_turn_steer_and_streaming_events_use_codex_v2_wire_shapes() {
        assert_eq!(method::TURN_STEER, "turn/steer");
        assert_eq!(event::TURN_PLAN_UPDATED, "turn/plan/updated");
        assert_eq!(event::TURN_DIFF_UPDATED, "turn/diff/updated");
        assert_eq!(event::ITEM_PLAN_DELTA, "item/plan/delta");
        assert_eq!(
            event::RAW_RESPONSE_ITEM_COMPLETED,
            "rawResponseItem/completed"
        );

        let steer: TurnSteerParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "expectedTurnId": "turn_1",
            "input": [{ "type": "text", "text": "continue with the safer option" }]
        }))
        .expect("turn/steer should accept Codex v2 UserInput arrays");
        assert_eq!(steer.thread_id, "thread_1");
        assert_eq!(steer.expected_turn_id, "turn_1");
        assert_eq!(steer.input.len(), 1);

        let plan = serde_json::to_value(TurnPlanUpdatedEvent {
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            explanation: Some("Adjusting plan".into()),
            plan: vec![TurnPlanStep {
                step: "inspect current producer".into(),
                status: TurnPlanStepStatus::InProgress,
            }],
        })
        .expect("turn plan update should serialize");
        assert_eq!(plan["threadId"], "thread_1");
        assert_eq!(plan["turnId"], "turn_1");
        assert!(plan.get("thread_id").is_none());
        assert!(plan.get("turn_id").is_none());
        assert_eq!(plan["plan"][0]["status"], "inProgress");

        let diff = serde_json::to_value(TurnDiffUpdatedEvent {
            thread_id: "thread_1".into(),
            turn_id: "turn_1".into(),
            diff: "--- a/file\n+++ b/file\n".into(),
        })
        .expect("turn diff update should serialize");
        assert!(diff.get("unifiedDiff").is_none());
        assert_eq!(diff["diff"], "--- a/file\n+++ b/file\n");
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
                "decision": "approve",
                "permissions": {"network":{"allow":["example.test"]}},
                "scope": "session",
                "strictAutoReview": true
            }))
            .expect("permissions response should deserialize");
        let default_permissions: PermissionsRequestApprovalResponse =
            serde_json::from_value(serde_json::json!({
                "decision": "reject",
                "permissions": {}
            }))
            .expect("permissions response should default missing optional fields");
        let missing_decision_permissions =
            serde_json::from_value::<PermissionsRequestApprovalResponse>(serde_json::json!({
                "permissions": {}
            }));

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
        assert_eq!(permissions.decision, PermissionsApprovalDecision::Approve);
        assert_eq!(permissions.scope, PermissionGrantScope::Session);
        assert_eq!(permissions.strict_auto_review, Some(true));
        assert!(
            missing_decision_permissions.is_err(),
            "permissions response must not default a missing decision"
        );
        assert_eq!(
            default_permissions.decision,
            PermissionsApprovalDecision::Reject
        );
        assert_eq!(default_permissions.scope, PermissionGrantScope::Turn);
        assert_eq!(default_permissions.strict_auto_review, None);
        assert_eq!(
            serde_json::to_value(default_permissions)
                .expect("default permissions response should serialize"),
            serde_json::json!({
                "decision": "reject",
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
                    oauth_login: false,
                    oauth_login_completed_events: false,
                    startup_status_events: true,
                },
                p5: AppServerP5Availability::default(),
                r6: AppServerR6Availability::default(),
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
    fn app_server_capability_helper_advertises_oauth_only_when_ready() {
        let matrix =
            CapabilityMatrix::phase_one().with_app_services(AppServerServiceAvailability {
                mcp: McpServiceAvailability {
                    oauth_login: true,
                    oauth_login_completed_events: true,
                    ..McpServiceAvailability::default()
                },
                ..AppServerServiceAvailability::default()
            });

        assert_eq!(matrix.mcp.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.mcp.methods,
            vec![method::MCP_SERVER_OAUTH_LOGIN.to_string()]
        );
        assert_eq!(
            matrix.mcp.events,
            vec![event::MCP_SERVER_OAUTH_LOGIN_COMPLETED.to_string()]
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
    fn r6_config_warning_is_advertised_only_by_warnings_capability() {
        let matrix =
            CapabilityMatrix::phase_one().with_app_services(AppServerServiceAvailability {
                r6: AppServerR6Availability {
                    config: true,
                    config_warnings: true,
                    ..AppServerR6Availability::default()
                },
                ..AppServerServiceAvailability::default()
            });

        assert_eq!(matrix.config.status, CapabilityStatus::Implemented);
        assert_eq!(
            matrix.config.methods,
            vec![
                method::CONFIG_READ.to_string(),
                method::CONFIG_VALUE_WRITE.to_string(),
                method::CONFIG_BATCH_WRITE.to_string(),
            ]
        );
        assert!(
            !matrix
                .config
                .events
                .contains(&event::CONFIG_WARNING.to_string())
        );
        assert_eq!(matrix.warnings.status, CapabilityStatus::Implemented);
        assert!(
            matrix
                .warnings
                .events
                .contains(&event::CONFIG_WARNING.to_string())
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
            callback_url: "http://127.0.0.1/oauth/callback".into(),
            state: "state".into(),
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
