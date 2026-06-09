//! Protocol types for the dasclaw local app-server.
//!
//! This crate intentionally models the GUI control-plane boundary only. It
//! does not define agent-loop internals or tool-execution traits; those stay
//! in `dasclaw_runtime`.

use serde::de::{self, Deserializer};
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
    pub const THREAD_CREATE: &str = "thread/create";
    pub const THREAD_START: &str = "thread/start";
    pub const THREAD_LIST: &str = "thread/list";
    pub const THREAD_READ: &str = "thread/read";
    pub const TURN_START: &str = "turn/start";
    pub const TURN_CANCEL: &str = "turn/cancel";
    pub const TURN_INTERRUPT: &str = "turn/interrupt";
    pub const TURN_LIST: &str = "turn/list";
    pub const TURN_READ: &str = "turn/read";
}

pub mod event {
    pub const NOTIFICATIONS_INITIALIZED: &str = "notifications/initialized";
    pub const LIFECYCLE_CHANGED: &str = "lifecycle/changed";
    pub const HEALTH_CHANGED: &str = "health/changed";
    pub const CAPABILITIES_CHANGED: &str = "capabilities/changed";
    pub const LOG_ENTRY: &str = "log/entry";
    pub const THREAD_CREATED: &str = "thread/created";
    pub const THREAD_STARTED: &str = "thread/started";
    pub const TURN_STARTED: &str = "turn/started";
    pub const TURN_DELTA: &str = "turn/delta";
    pub const TURN_COMPLETED: &str = "turn/completed";
    pub const TURN_FAILED: &str = "turn/failed";
    pub const TURN_CANCELLED: &str = "turn/cancelled";
    pub const ITEM_STARTED: &str = "item/started";
    pub const ITEM_AGENT_MESSAGE_DELTA: &str = "item/agentMessage/delta";
    pub const ITEM_COMPLETED: &str = "item/completed";
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
                    method::THREAD_CREATE,
                    method::THREAD_START,
                    method::THREAD_LIST,
                    method::THREAD_READ,
                    method::TURN_START,
                    method::TURN_CANCEL,
                    method::TURN_INTERRUPT,
                    method::TURN_LIST,
                    method::TURN_READ,
                ],
                &[
                    event::THREAD_CREATED,
                    event::THREAD_STARTED,
                    event::TURN_STARTED,
                    event::TURN_DELTA,
                    event::TURN_COMPLETED,
                    event::TURN_FAILED,
                    event::TURN_CANCELLED,
                    event::ITEM_STARTED,
                    event::ITEM_AGENT_MESSAGE_DELTA,
                    event::ITEM_COMPLETED,
                    event::ERROR,
                ],
            ),
            approval: declared_future_capability("approval"),
            dlp_policy: declared_future_capability("dlp_policy"),
            model_provider: declared_future_capability("model_provider"),
            tools: declared_future_capability("tools"),
            jobs: declared_future_capability("jobs"),
            skills: declared_future_capability("skills"),
            mcp: declared_future_capability("mcp"),
            sandbox: declared_future_capability("sandbox"),
        }
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
            compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
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
    pub const CODEX_APP_SERVER_V2_ID: &'static str = "codex_app_server_v2";

    #[must_use]
    pub fn codex_app_server_v2() -> Self {
        Self {
            id: Self::CODEX_APP_SERVER_V2_ID.to_string(),
            version: "2.0.0-compat".to_string(),
            scope: CompatibilityProfileScope::ChatSessionSubset,
            description: "Codex app-server v2 compatibility profile for initialize/thread/turn chat session streaming only".to_string(),
            methods: vec![
                method::INITIALIZE.to_string(),
                method::THREAD_START.to_string(),
                method::THREAD_READ.to_string(),
                method::TURN_START.to_string(),
                method::TURN_INTERRUPT.to_string(),
            ],
            events: CODEX_APP_SERVER_V2_EVENTS
                .iter()
                .map(|event| (*event).to_string())
                .collect(),
            aliases: vec![
                CompatibilityAlias::new(
                    method::THREAD_CREATE,
                    method::THREAD_START,
                    CompatibilityAliasKind::LegacySmokeSurface,
                ),
                CompatibilityAlias::new(
                    method::TURN_CANCEL,
                    method::TURN_INTERRUPT,
                    CompatibilityAliasKind::Alias,
                ),
                CompatibilityAlias::new(
                    event::TURN_DELTA,
                    event::ITEM_AGENT_MESSAGE_DELTA,
                    CompatibilityAliasKind::LegacySmokeSurface,
                ),
            ],
            capability_opt_outs: vec![
                CapabilityOptOut::phase_one("codex.rich_input"),
                CapabilityOptOut::phase_one("codex.tool_calls"),
                CapabilityOptOut::phase_one("codex.approvals"),
                CapabilityOptOut::phase_one("codex.diff"),
                CapabilityOptOut::phase_one("codex.plan"),
                CapabilityOptOut::phase_one("tools"),
                CapabilityOptOut::phase_one("mcp"),
                CapabilityOptOut::phase_one("skills"),
                CapabilityOptOut::phase_one("dlp_policy"),
                CapabilityOptOut::phase_one("jobs"),
                CapabilityOptOut::phase_one("sandbox"),
            ],
            event_queue: NotificationQueuePolicy::bounded_lag_disconnect(),
        }
    }
}

const CODEX_APP_SERVER_V2_EVENTS: &[&str] = &[
    event::NOTIFICATIONS_INITIALIZED,
    event::LIFECYCLE_CHANGED,
    event::CAPABILITIES_CHANGED,
    event::THREAD_CREATED,
    event::THREAD_STARTED,
    event::TURN_STARTED,
    event::TURN_DELTA,
    event::TURN_COMPLETED,
    event::TURN_FAILED,
    event::TURN_CANCELLED,
    event::ITEM_STARTED,
    event::ITEM_AGENT_MESSAGE_DELTA,
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

impl CompatibilityAlias {
    fn new(legacy: &'static str, compatible: &'static str, kind: CompatibilityAliasKind) -> Self {
        Self {
            legacy: legacy.to_string(),
            compatible: compatible.to_string(),
            kind,
        }
    }
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
            method::THREAD_CREATE,
            "session",
            Some("ThreadCreateParams"),
            "ThreadCreateResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_START,
            "session",
            Some("ThreadCreateParams"),
            "ThreadCreateResponse",
            true,
        ),
        MethodSchema::new(
            method::THREAD_LIST,
            "session",
            None,
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
            method::TURN_CANCEL,
            "session",
            Some("TurnCancelParams"),
            "TurnCancelResponse",
            true,
        ),
        MethodSchema::new(
            method::TURN_INTERRUPT,
            "session",
            Some("TurnCancelParams"),
            "TurnCancelResponse",
            true,
        ),
        MethodSchema::new(
            method::TURN_LIST,
            "session",
            Some("TurnListParams"),
            "TurnListResponse",
            true,
        ),
        MethodSchema::new(
            method::TURN_READ,
            "session",
            Some("TurnReadParams"),
            "TurnReadResponse",
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
        EventSchema::new(event::THREAD_CREATED, "session", "ThreadCreatedEvent"),
        EventSchema::new(event::THREAD_STARTED, "session", "ThreadStartedEvent"),
        EventSchema::new(event::TURN_STARTED, "session", "TurnStartedEvent"),
        EventSchema::new(event::TURN_DELTA, "session", "TurnDeltaEvent"),
        EventSchema::new(event::TURN_COMPLETED, "session", "TurnCompletedEvent"),
        EventSchema::new(event::TURN_FAILED, "session", "TurnFailedEvent"),
        EventSchema::new(event::TURN_CANCELLED, "session", "TurnCancelledEvent"),
        EventSchema::new(event::ITEM_STARTED, "session", "ItemStartedEvent"),
        EventSchema::new(
            event::ITEM_AGENT_MESSAGE_DELTA,
            "session",
            "AgentMessageDeltaEvent",
        ),
        EventSchema::new(event::ITEM_COMPLETED, "session", "ItemCompletedEvent"),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCreateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCreateResponse {
    pub thread_id: String,
    pub lifecycle: LifecycleSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResponse {
    pub threads: Vec<ThreadSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub prompt: String,
}

impl TurnStartParams {
    #[must_use]
    pub fn from_input(thread_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            prompt: input.into(),
        }
    }
}

impl<'de> Deserialize<'de> for TurnStartParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawTurnStartParams {
            thread_id: String,
            #[serde(default)]
            prompt: Option<String>,
            #[serde(default)]
            input: Option<String>,
        }

        let raw = RawTurnStartParams::deserialize(deserializer)?;
        let prompt = match (raw.prompt, raw.input) {
            (Some(prompt), Some(input)) if prompt != input => {
                return Err(de::Error::custom(
                    "turn/start prompt and input must match when both are provided",
                ));
            }
            (Some(prompt), _) | (None, Some(prompt)) => prompt,
            (None, None) => {
                return Err(de::Error::missing_field("prompt or input"));
            }
        };

        Ok(Self {
            thread_id: raw.thread_id,
            prompt,
        })
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
    pub turn_id: String,
    pub status: TurnStatus,
    pub lifecycle: LifecycleSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCancelParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCancelResponse {
    pub accepted: bool,
    pub status: TurnStatus,
    pub lifecycle: LifecycleSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnListParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnListResponse {
    pub turns: Vec<TurnSummary>,
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
    pub turn: TurnSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCreatedEvent {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartedEvent {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnFailedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCancelledEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    AgentMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub item_type: ItemType,
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
pub struct ItemCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub status: TurnStatus,
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

    pub fn thread_created(event: ThreadCreatedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_CREATED, event)
    }

    pub fn thread_started(event: ThreadStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::THREAD_STARTED, event)
    }

    pub fn turn_started(event: TurnStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_STARTED, event)
    }

    pub fn turn_delta(event: TurnDeltaEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_DELTA, event)
    }

    pub fn turn_completed(event: TurnCompletedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_COMPLETED, event)
    }

    pub fn turn_failed(event: TurnFailedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_FAILED, event)
    }

    pub fn turn_cancelled(event: TurnCancelledEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_CANCELLED, event)
    }

    pub fn item_started(event: ItemStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_STARTED, event)
    }

    pub fn agent_message_delta(event: AgentMessageDeltaEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_AGENT_MESSAGE_DELTA, event)
    }

    pub fn item_completed(event: ItemCompletedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::ITEM_COMPLETED, event)
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
                .contains(&method::THREAD_CREATE.to_string())
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
                .contains(&method::TURN_START.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::TURN_CANCEL.to_string())
        );
        assert!(
            matrix
                .session
                .methods
                .contains(&method::TURN_LIST.to_string())
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
                .contains(&event::TURN_DELTA.to_string())
        );
        assert!(
            matrix
                .session
                .events
                .contains(&event::TURN_COMPLETED.to_string())
        );
        assert!(
            matrix
                .session
                .events
                .contains(&event::TURN_FAILED.to_string())
        );
        assert_eq!(matrix.approval.status, CapabilityStatus::Declared);
        assert_eq!(matrix.dlp_policy.status, CapabilityStatus::Declared);
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
        assert!(method_names.contains(&method::THREAD_CREATE));
        assert!(method_names.contains(&method::THREAD_LIST));
        assert!(method_names.contains(&method::THREAD_READ));
        assert!(method_names.contains(&method::TURN_START));
        assert!(method_names.contains(&method::TURN_CANCEL));
        assert!(method_names.contains(&method::TURN_LIST));
        assert!(method_names.contains(&method::TURN_READ));
        assert!(event_names.contains(&event::LIFECYCLE_CHANGED));
        assert!(event_names.contains(&event::CAPABILITIES_CHANGED));
        assert!(event_names.contains(&event::THREAD_CREATED));
        assert!(event_names.contains(&event::TURN_DELTA));
        assert!(event_names.contains(&event::TURN_COMPLETED));
        assert!(event_names.contains(&event::TURN_FAILED));
        assert_eq!(schema.capabilities.logs.status, CapabilityStatus::Declared);
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
    fn codex_v2_profile_declares_compatible_surface_and_legacy_alias_decisions() {
        let profile = CompatibilityProfile::codex_app_server_v2();

        assert_eq!(profile.id, CompatibilityProfile::CODEX_APP_SERVER_V2_ID);
        assert_eq!(profile.scope, CompatibilityProfileScope::ChatSessionSubset);
        assert!(profile.description.contains("chat session streaming"));
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
        assert_eq!(
            profile.events,
            CODEX_APP_SERVER_V2_EVENTS
                .iter()
                .map(|event| (*event).to_string())
                .collect::<Vec<_>>()
        );
        assert!(profile.aliases.iter().any(|alias| {
            alias.legacy == method::THREAD_CREATE
                && alias.compatible == method::THREAD_START
                && alias.kind == CompatibilityAliasKind::LegacySmokeSurface
        }));
        assert!(profile.aliases.iter().any(|alias| {
            alias.legacy == method::TURN_CANCEL
                && alias.compatible == method::TURN_INTERRUPT
                && alias.kind == CompatibilityAliasKind::Alias
        }));
        assert!(profile.aliases.iter().any(|alias| {
            alias.legacy == event::TURN_DELTA
                && alias.compatible == event::ITEM_AGENT_MESSAGE_DELTA
                && alias.kind == CompatibilityAliasKind::LegacySmokeSurface
        }));
        assert_eq!(
            profile.event_queue,
            NotificationQueuePolicy::bounded_lag_disconnect()
        );
        assert!(profile.capability_opt_outs.iter().any(|opt_out| {
            opt_out.capability == "codex.rich_input"
                && opt_out.reason == "phase_1_chat_session_subset"
        }));
        assert!(profile.capability_opt_outs.iter().any(|opt_out| {
            opt_out.capability == "mcp" && opt_out.reason == "phase_1_chat_session_subset"
        }));
        assert!(!profile.methods.iter().any(|method| {
            method.contains("approval") || method.contains("diff") || method.contains("tool")
        }));
        assert!(!profile.events.iter().any(|event| {
            event.contains("approval") || event.contains("diff") || event.contains("tool")
        }));
    }

    #[test]
    fn capabilities_response_can_advertise_compatibility_profiles() {
        let response = CapabilitiesListResponse {
            capabilities: CapabilityMatrix::phase_one(),
            compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
        };
        let value = serde_json::to_value(response).expect("capabilities should serialize");

        assert_eq!(
            value["compatibilityProfiles"][0]["id"],
            CompatibilityProfile::CODEX_APP_SERVER_V2_ID
        );
        assert_eq!(
            value["compatibilityProfiles"][0]["scope"],
            "chat_session_subset"
        );
    }

    #[test]
    fn turn_start_params_accept_codex_v2_input_alias_without_changing_legacy_shape() {
        let from_input: TurnStartParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "input": "hello from input"
        }))
        .expect("turn/start should accept Codex-style input string");
        let from_prompt: TurnStartParams = serde_json::from_value(serde_json::json!({
            "threadId": "thread_1",
            "prompt": "hello from prompt"
        }))
        .expect("turn/start should keep accepting legacy prompt");
        let mismatch = serde_json::from_value::<TurnStartParams>(serde_json::json!({
            "threadId": "thread_1",
            "prompt": "legacy",
            "input": "codex"
        }))
        .expect_err("conflicting prompt/input fields should not be silently normalized");

        assert_eq!(from_input.prompt, "hello from input");
        assert_eq!(from_prompt.prompt, "hello from prompt");
        assert!(mismatch.to_string().contains("prompt and input must match"));
        assert_eq!(
            serde_json::to_value(TurnStartParams::from_input("thread_1", "hello"))
                .expect("turn/start params should serialize"),
            serde_json::json!({"threadId": "thread_1", "prompt": "hello"})
        );
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
                    "requestedCapabilities": [CompatibilityProfile::CODEX_APP_SERVER_V2_ID]
                })),
            },
            JsonRpcRequest {
                jsonrpc: JSON_RPC_VERSION.to_string(),
                id: Some(serde_json::json!("thread")),
                method: method::THREAD_START.to_string(),
                params: Some(serde_json::json!({"title": "Draft"})),
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
                params: Some(serde_json::json!({"threadId": "thread_1", "input": "hello"})),
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
                compatibility_profiles: vec![CompatibilityProfile::codex_app_server_v2()],
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
            ServerNotification::thread_created(ThreadCreatedEvent {
                thread_id: "thread_1".to_string(),
            })
            .expect("thread/created fixture should serialize"),
            ServerNotification::thread_started(ThreadStartedEvent {
                thread_id: "thread_1".to_string(),
            })
            .expect("thread/started fixture should serialize"),
            ServerNotification::turn_started(TurnStartedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                status: TurnStatus::Pending,
            })
            .expect("turn/started fixture should serialize"),
            ServerNotification::turn_delta(TurnDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                delta: "hel".to_string(),
            })
            .expect("turn/delta fixture should serialize"),
            ServerNotification::turn_completed(TurnCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                status: TurnStatus::Completed,
                output: "hello".to_string(),
            })
            .expect("turn/completed fixture should serialize"),
            ServerNotification::turn_failed(TurnFailedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                status: TurnStatus::Failed,
                error: "runtime failed".to_string(),
            })
            .expect("turn/failed fixture should serialize"),
            ServerNotification::turn_cancelled(TurnCancelledEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                status: TurnStatus::Cancelled,
            })
            .expect("turn/cancelled fixture should serialize"),
            ServerNotification::item_started(ItemStartedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1".to_string(),
                item_type: ItemType::AgentMessage,
            })
            .expect("item/started fixture should serialize"),
            ServerNotification::agent_message_delta(AgentMessageDeltaEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1".to_string(),
                delta: "hel".to_string(),
            })
            .expect("item delta fixture should serialize"),
            ServerNotification::item_completed(ItemCompletedEvent {
                thread_id: "thread_1".to_string(),
                turn_id: "turn_1".to_string(),
                item_id: "turn_1".to_string(),
                status: TurnStatus::Completed,
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

        assert_eq!(event_names, CODEX_APP_SERVER_V2_EVENTS);
        assert_eq!(events[0].params["eventQueue"]["overflow"], "lag_disconnect");
        assert_eq!(events[10].params["itemType"], "agent_message");
        assert_eq!(events[11].params["delta"], "hel");
    }
}
