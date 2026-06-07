//! Protocol types for the dasclaw local app-server.
//!
//! This crate intentionally models the GUI control-plane boundary only. It
//! does not define agent-loop internals or tool-execution traits; those stay
//! in `dasclaw_runtime`.

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
    pub const THREAD_LIST: &str = "thread/list";
    pub const THREAD_READ: &str = "thread/read";
    pub const TURN_START: &str = "turn/start";
    pub const TURN_CANCEL: &str = "turn/cancel";
    pub const TURN_LIST: &str = "turn/list";
    pub const TURN_READ: &str = "turn/read";
}

pub mod event {
    pub const LIFECYCLE_CHANGED: &str = "lifecycle/changed";
    pub const HEALTH_CHANGED: &str = "health/changed";
    pub const CAPABILITIES_CHANGED: &str = "capabilities/changed";
    pub const LOG_ENTRY: &str = "log/entry";
    pub const THREAD_CREATED: &str = "thread/created";
    pub const TURN_STARTED: &str = "turn/started";
    pub const TURN_CANCELLED: &str = "turn/cancelled";
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
                &[],
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
            session: Capability::declared_contract(
                "session",
                &[
                    method::THREAD_CREATE,
                    method::THREAD_LIST,
                    method::THREAD_READ,
                    method::TURN_START,
                    method::TURN_CANCEL,
                    method::TURN_LIST,
                    method::TURN_READ,
                ],
                &[
                    event::THREAD_CREATED,
                    event::TURN_STARTED,
                    event::TURN_CANCELLED,
                ],
                "thread and turn protocol skeleton is declared; runtime session host is not wired in Phase 1",
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
}

impl ProtocolSchemaResponse {
    #[must_use]
    pub fn phase_one(capabilities: CapabilityMatrix) -> Self {
        Self {
            protocol_version: ProtocolVersion::current(),
            methods: phase_one_methods(),
            events: phase_one_events(),
            capabilities,
        }
    }
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
        EventSchema::new(event::TURN_STARTED, "session", "TurnStartedEvent"),
        EventSchema::new(event::TURN_CANCELLED, "session", "TurnCancelledEvent"),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Pending,
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
pub struct TurnStartedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCancelledEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

impl ServerNotification {
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

    pub fn turn_started(event: TurnStartedEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_STARTED, event)
    }

    pub fn turn_cancelled(event: TurnCancelledEvent) -> Result<Self, serde_json::Error> {
        Self::new(event::TURN_CANCELLED, event)
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
        assert_eq!(matrix.session.status, CapabilityStatus::Declared);
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
}
