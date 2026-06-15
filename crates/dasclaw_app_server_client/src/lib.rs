//! Minimal client helpers for the dasclaw app-server protocol.
//!
//! This crate owns client-side JSON-RPC request construction and response
//! decoding only. It intentionally does not supervise an Electron sidecar
//! process and does not embed runtime/session behavior.

use std::io::{BufRead, Write};

use dasclaw_app_server_protocol::{
    AgentMessageDeltaEvent, CapabilitiesChangedEvent, CapabilitiesListResponse, ErrorCode,
    ErrorEvent, HealthChangedEvent, HealthCheckParams, HealthCheckResponse, InitializeParams,
    InitializeResponse, ItemCompletedEvent, ItemStartedEvent, JSON_RPC_VERSION, JsonRpcError,
    JsonRpcRequest, JsonRpcResponse, LifecycleChangedEvent, LifecycleStatusResponse, LogEntryEvent,
    NotificationsInitializedEvent, ProtocolSchemaResponse, ReasoningSummaryTextDeltaEvent,
    ServerNotification, ShutdownParams, ShutdownResponse, ThreadCreateParams, ThreadCreateResponse,
    ThreadCreatedEvent, ThreadListResponse, ThreadReadParams, ThreadReadResponse,
    ThreadStartResponse, ThreadStartedEvent, TurnCancelParams, TurnCancelResponse,
    TurnCancelledEvent, TurnCompletedEvent, TurnDeltaEvent, TurnFailedEvent, TurnInterruptResponse,
    TurnListParams, TurnListResponse, TurnReadParams, TurnReadResponse, TurnStartParams,
    TurnStartResponse, TurnStartedEvent, WorkspaceInfo, event, method,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Debug)]
pub struct AppServerClient<T> {
    transport: T,
    next_id: u64,
    pending_notifications: Vec<ServerNotification>,
}

impl<T> AppServerClient<T> {
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_id: 1,
            pending_notifications: Vec::new(),
        }
    }

    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn drain_notifications(&mut self) -> Vec<ServerNotification> {
        std::mem::take(&mut self.pending_notifications)
    }

    pub fn drain_typed_notifications(
        &mut self,
    ) -> Result<Vec<AppServerNotification>, AppServerClientError> {
        self.drain_notifications()
            .into_iter()
            .map(AppServerNotification::try_from)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppServerNotification {
    NotificationsInitialized(Box<NotificationsInitializedEvent>),
    LifecycleChanged(LifecycleChangedEvent),
    HealthChanged(HealthChangedEvent),
    CapabilitiesChanged(Box<CapabilitiesChangedEvent>),
    LogEntry(LogEntryEvent),
    ThreadCreated(ThreadCreatedEvent),
    ThreadStarted(ThreadStartedEvent),
    TurnStarted(TurnStartedEvent),
    TurnDelta(TurnDeltaEvent),
    TurnCompleted(TurnCompletedEvent),
    TurnFailed(TurnFailedEvent),
    TurnCancelled(TurnCancelledEvent),
    ItemStarted(ItemStartedEvent),
    AgentMessageDelta(AgentMessageDeltaEvent),
    ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaEvent),
    ItemCompleted(ItemCompletedEvent),
    ProtocolError(ErrorEvent),
    Unknown(ServerNotification),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppServerClientRoundTrip<R> {
    pub result: R,
    pub notifications: Vec<AppServerNotification>,
}

impl TryFrom<ServerNotification> for AppServerNotification {
    type Error = AppServerClientError;

    fn try_from(notification: ServerNotification) -> Result<Self, Self::Error> {
        let method = notification.method.clone();
        Ok(match method.as_str() {
            event::NOTIFICATIONS_INITIALIZED => {
                Self::NotificationsInitialized(Box::new(decode_notification_params(notification)?))
            }
            event::LIFECYCLE_CHANGED => {
                Self::LifecycleChanged(decode_notification_params(notification)?)
            }
            event::HEALTH_CHANGED => Self::HealthChanged(decode_notification_params(notification)?),
            event::CAPABILITIES_CHANGED => {
                Self::CapabilitiesChanged(Box::new(decode_notification_params(notification)?))
            }
            event::LOG_ENTRY => Self::LogEntry(decode_notification_params(notification)?),
            event::THREAD_CREATED => Self::ThreadCreated(decode_notification_params(notification)?),
            event::THREAD_STARTED => Self::ThreadStarted(decode_notification_params(notification)?),
            event::TURN_STARTED => Self::TurnStarted(decode_notification_params(notification)?),
            event::TURN_DELTA => Self::TurnDelta(decode_notification_params(notification)?),
            event::TURN_COMPLETED => Self::TurnCompleted(decode_notification_params(notification)?),
            event::TURN_FAILED => Self::TurnFailed(decode_notification_params(notification)?),
            event::TURN_CANCELLED => Self::TurnCancelled(decode_notification_params(notification)?),
            event::ITEM_STARTED => Self::ItemStarted(decode_notification_params(notification)?),
            event::ITEM_AGENT_MESSAGE_DELTA => {
                Self::AgentMessageDelta(decode_notification_params(notification)?)
            }
            event::ITEM_REASONING_SUMMARY_TEXT_DELTA => {
                Self::ReasoningSummaryTextDelta(decode_notification_params(notification)?)
            }
            event::ITEM_COMPLETED => Self::ItemCompleted(decode_notification_params(notification)?),
            event::ERROR => Self::ProtocolError(decode_notification_params(notification)?),
            _ => Self::Unknown(notification),
        })
    }
}

impl<T> AppServerClient<T>
where
    T: AppServerTransport,
{
    pub fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> Result<InitializeResponse, AppServerClientError> {
        self.request(method::INITIALIZE, Some(params))
    }

    pub fn initialize_with_notifications(
        &mut self,
        params: InitializeParams,
    ) -> Result<AppServerClientRoundTrip<InitializeResponse>, AppServerClientError> {
        self.request_with_notifications(method::INITIALIZE, Some(params))
    }

    pub fn initialize_codex_v2_chat_subset(
        &mut self,
        client: dasclaw_app_server_protocol::ClientInfo,
        workspace: Option<WorkspaceInfo>,
    ) -> Result<InitializeResponse, AppServerClientError> {
        self.initialize(codex_v2_chat_subset_initialize_params(client, workspace))
    }

    pub fn initialize_codex_v2_chat_subset_with_notifications(
        &mut self,
        client: dasclaw_app_server_protocol::ClientInfo,
        workspace: Option<WorkspaceInfo>,
    ) -> Result<AppServerClientRoundTrip<InitializeResponse>, AppServerClientError> {
        self.initialize_with_notifications(codex_v2_chat_subset_initialize_params(
            client, workspace,
        ))
    }

    pub fn health_check(
        &mut self,
        params: HealthCheckParams,
    ) -> Result<HealthCheckResponse, AppServerClientError> {
        self.request(method::HEALTH_CHECK, Some(params))
    }

    pub fn health_check_with_notifications(
        &mut self,
        params: HealthCheckParams,
    ) -> Result<AppServerClientRoundTrip<HealthCheckResponse>, AppServerClientError> {
        self.request_with_notifications(method::HEALTH_CHECK, Some(params))
    }

    pub fn capabilities(&mut self) -> Result<CapabilitiesListResponse, AppServerClientError> {
        self.request::<(), CapabilitiesListResponse>(method::CAPABILITIES_LIST, None)
    }

    pub fn capabilities_with_notifications(
        &mut self,
    ) -> Result<AppServerClientRoundTrip<CapabilitiesListResponse>, AppServerClientError> {
        self.request_with_notifications::<(), CapabilitiesListResponse>(
            method::CAPABILITIES_LIST,
            None,
        )
    }

    pub fn lifecycle_status(&mut self) -> Result<LifecycleStatusResponse, AppServerClientError> {
        self.request::<(), LifecycleStatusResponse>(method::LIFECYCLE_STATUS, None)
    }

    pub fn lifecycle_status_with_notifications(
        &mut self,
    ) -> Result<AppServerClientRoundTrip<LifecycleStatusResponse>, AppServerClientError> {
        self.request_with_notifications::<(), LifecycleStatusResponse>(
            method::LIFECYCLE_STATUS,
            None,
        )
    }

    pub fn protocol_schema(&mut self) -> Result<ProtocolSchemaResponse, AppServerClientError> {
        self.request::<(), ProtocolSchemaResponse>(method::PROTOCOL_SCHEMA, None)
    }

    pub fn protocol_schema_with_notifications(
        &mut self,
    ) -> Result<AppServerClientRoundTrip<ProtocolSchemaResponse>, AppServerClientError> {
        self.request_with_notifications::<(), ProtocolSchemaResponse>(method::PROTOCOL_SCHEMA, None)
    }

    pub fn shutdown(
        &mut self,
        params: ShutdownParams,
    ) -> Result<ShutdownResponse, AppServerClientError> {
        self.request(method::SHUTDOWN, Some(params))
    }

    pub fn shutdown_with_notifications(
        &mut self,
        params: ShutdownParams,
    ) -> Result<AppServerClientRoundTrip<ShutdownResponse>, AppServerClientError> {
        self.request_with_notifications(method::SHUTDOWN, Some(params))
    }

    pub fn thread_create(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<ThreadCreateResponse, AppServerClientError> {
        self.request(method::THREAD_CREATE, Some(params))
    }

    pub fn thread_create_with_notifications(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<AppServerClientRoundTrip<ThreadCreateResponse>, AppServerClientError> {
        self.request_with_notifications(method::THREAD_CREATE, Some(params))
    }

    pub fn thread_start(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<ThreadStartResponse, AppServerClientError> {
        self.request(method::THREAD_START, Some(params))
    }

    pub fn thread_start_with_notifications(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<AppServerClientRoundTrip<ThreadStartResponse>, AppServerClientError> {
        self.request_with_notifications(method::THREAD_START, Some(params))
    }

    pub fn thread_list(&mut self) -> Result<ThreadListResponse, AppServerClientError> {
        self.request::<(), ThreadListResponse>(method::THREAD_LIST, None)
    }

    pub fn thread_list_with_notifications(
        &mut self,
    ) -> Result<AppServerClientRoundTrip<ThreadListResponse>, AppServerClientError> {
        self.request_with_notifications::<(), ThreadListResponse>(method::THREAD_LIST, None)
    }

    pub fn thread_read(
        &mut self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse, AppServerClientError> {
        self.request(method::THREAD_READ, Some(params))
    }

    pub fn thread_read_with_notifications(
        &mut self,
        params: ThreadReadParams,
    ) -> Result<AppServerClientRoundTrip<ThreadReadResponse>, AppServerClientError> {
        self.request_with_notifications(method::THREAD_READ, Some(params))
    }

    pub fn turn_start(
        &mut self,
        params: TurnStartParams,
    ) -> Result<TurnStartResponse, AppServerClientError> {
        self.request(method::TURN_START, Some(params))
    }

    pub fn turn_start_with_notifications(
        &mut self,
        params: TurnStartParams,
    ) -> Result<AppServerClientRoundTrip<TurnStartResponse>, AppServerClientError> {
        self.request_with_notifications(method::TURN_START, Some(params))
    }

    pub fn turn_start_input(
        &mut self,
        thread_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Result<TurnStartResponse, AppServerClientError> {
        self.request(
            method::TURN_START,
            Some(TurnStartInputParams::new(thread_id, input)),
        )
    }

    pub fn turn_start_input_with_notifications(
        &mut self,
        thread_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Result<AppServerClientRoundTrip<TurnStartResponse>, AppServerClientError> {
        self.request_with_notifications(
            method::TURN_START,
            Some(TurnStartInputParams::new(thread_id, input)),
        )
    }

    pub fn turn_cancel(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<TurnCancelResponse, AppServerClientError> {
        self.request(method::TURN_CANCEL, Some(params))
    }

    pub fn turn_cancel_with_notifications(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<AppServerClientRoundTrip<TurnCancelResponse>, AppServerClientError> {
        self.request_with_notifications(method::TURN_CANCEL, Some(params))
    }

    pub fn turn_interrupt(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<TurnInterruptResponse, AppServerClientError> {
        self.request(method::TURN_INTERRUPT, Some(params))
    }

    pub fn turn_interrupt_with_notifications(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<AppServerClientRoundTrip<TurnInterruptResponse>, AppServerClientError> {
        self.request_with_notifications(method::TURN_INTERRUPT, Some(params))
    }

    pub fn turn_list(
        &mut self,
        params: TurnListParams,
    ) -> Result<TurnListResponse, AppServerClientError> {
        self.request(method::TURN_LIST, Some(params))
    }

    pub fn turn_list_with_notifications(
        &mut self,
        params: TurnListParams,
    ) -> Result<AppServerClientRoundTrip<TurnListResponse>, AppServerClientError> {
        self.request_with_notifications(method::TURN_LIST, Some(params))
    }

    pub fn turn_read(
        &mut self,
        params: TurnReadParams,
    ) -> Result<TurnReadResponse, AppServerClientError> {
        self.request(method::TURN_READ, Some(params))
    }

    pub fn turn_read_with_notifications(
        &mut self,
        params: TurnReadParams,
    ) -> Result<AppServerClientRoundTrip<TurnReadResponse>, AppServerClientError> {
        self.request_with_notifications(method::TURN_READ, Some(params))
    }

    pub fn request_value(
        &mut self,
        method: &'static str,
        params: Option<impl Serialize>,
    ) -> Result<Value, AppServerClientError> {
        self.request(method, params)
    }

    pub fn request_value_with_notifications(
        &mut self,
        method: &'static str,
        params: Option<impl Serialize>,
    ) -> Result<AppServerClientRoundTrip<Value>, AppServerClientError> {
        self.request_with_notifications(method, params)
    }

    pub fn notify(
        &mut self,
        method: &'static str,
        params: Option<impl Serialize>,
    ) -> Result<(), AppServerClientError> {
        let params = serialize_optional_params(params)?;
        let request = JsonRpcRequest {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: None,
            method: method.to_string(),
            params,
        };
        let line = serde_json::to_string(&request)
            .map_err(|error| AppServerClientError::Encode(error.to_string()))?;
        let round_trip = self.transport.send_json_rpc(&line, None)?;
        self.pending_notifications.extend(round_trip.notifications);
        Ok(())
    }

    pub fn read_next_notification(
        &mut self,
    ) -> Result<Option<ServerNotification>, AppServerClientError> {
        self.transport.read_json_rpc_notification()
    }

    pub fn read_next_typed_notification(
        &mut self,
    ) -> Result<Option<AppServerNotification>, AppServerClientError> {
        self.read_next_notification()?
            .map(AppServerNotification::try_from)
            .transpose()
    }

    fn request<P, R>(
        &mut self,
        method: &'static str,
        params: Option<P>,
    ) -> Result<R, AppServerClientError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let round_trip = self.execute_request(method, params)?;
        self.pending_notifications
            .extend(round_trip.raw_notifications);
        Ok(round_trip.result)
    }

    pub fn request_with_notifications<P, R>(
        &mut self,
        method: &'static str,
        params: Option<P>,
    ) -> Result<AppServerClientRoundTrip<R>, AppServerClientError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let round_trip = self.execute_request(method, params)?;
        Ok(AppServerClientRoundTrip {
            result: round_trip.result,
            notifications: round_trip.notifications,
        })
    }

    fn execute_request<P, R>(
        &mut self,
        method: &'static str,
        params: Option<P>,
    ) -> Result<DecodedClientRoundTrip<R>, AppServerClientError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let raw = self.execute_raw_request(method, params)?;
        let notifications = raw
            .notifications
            .iter()
            .cloned()
            .map(AppServerNotification::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let result = decode_json_rpc_response::<R>(&raw.response)?;
        Ok(DecodedClientRoundTrip {
            result,
            notifications,
            raw_notifications: raw.notifications,
        })
    }

    fn execute_raw_request<P>(
        &mut self,
        method: &'static str,
        params: Option<P>,
    ) -> Result<RawClientRoundTrip, AppServerClientError>
    where
        P: Serialize,
    {
        let id = Value::from(self.next_id);
        self.next_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(id.clone()),
            method: method.to_string(),
            params: serialize_optional_params(params)?,
        };
        let line = serde_json::to_string(&request)
            .map_err(|error| AppServerClientError::Encode(error.to_string()))?;
        let round_trip = self.transport.send_json_rpc(&line, Some(&id))?;
        let response = round_trip
            .response
            .ok_or(AppServerClientError::NoResponse)?;
        Ok(RawClientRoundTrip {
            response,
            notifications: round_trip.notifications,
        })
    }
}

#[derive(Debug)]
struct RawClientRoundTrip {
    response: String,
    notifications: Vec<ServerNotification>,
}

#[derive(Debug)]
struct DecodedClientRoundTrip<R> {
    result: R,
    notifications: Vec<AppServerNotification>,
    raw_notifications: Vec<ServerNotification>,
}

pub trait AppServerTransport {
    fn send_json_rpc(
        &mut self,
        line: &str,
        response_id: Option<&Value>,
    ) -> Result<ClientRoundTrip, AppServerClientError>;

    fn read_json_rpc_notification(
        &mut self,
    ) -> Result<Option<ServerNotification>, AppServerClientError> {
        Ok(None)
    }
}

impl<F> AppServerTransport for F
where
    F: FnMut(&str, Option<&Value>) -> Result<ClientRoundTrip, AppServerClientError>,
{
    fn send_json_rpc(
        &mut self,
        line: &str,
        response_id: Option<&Value>,
    ) -> Result<ClientRoundTrip, AppServerClientError> {
        self(line, response_id)
    }
}

#[derive(Debug, Default)]
pub struct ClientRoundTrip {
    pub response: Option<String>,
    pub notifications: Vec<ServerNotification>,
}

pub struct InProcessTransport<F> {
    handler: F,
}

impl<F> InProcessTransport<F> {
    #[must_use]
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

impl<F> AppServerTransport for InProcessTransport<F>
where
    F: FnMut(&str) -> Option<String>,
{
    fn send_json_rpc(
        &mut self,
        line: &str,
        _response_id: Option<&Value>,
    ) -> Result<ClientRoundTrip, AppServerClientError> {
        Ok(ClientRoundTrip {
            response: (self.handler)(line),
            notifications: Vec::new(),
        })
    }
}

#[derive(Debug)]
pub struct LineDelimitedTransport<R, W> {
    reader: R,
    writer: W,
    queue_overflow: Option<NotificationQueueOverflowState>,
}

impl<R, W> LineDelimitedTransport<R, W> {
    #[must_use]
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            queue_overflow: None,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (R, W) {
        (self.reader, self.writer)
    }

    fn remember_queue_overflow(
        &mut self,
        notification: &ServerNotification,
    ) -> Option<AppServerClientError> {
        let overflow = notification_queue_overflow_state(notification)?;
        self.queue_overflow = Some(overflow.clone());
        Some(overflow.into_error())
    }

    fn queue_overflow_error(&self) -> Option<AppServerClientError> {
        self.queue_overflow
            .as_ref()
            .map(NotificationQueueOverflowState::to_error)
    }
}

impl<R, W> AppServerTransport for LineDelimitedTransport<R, W>
where
    R: BufRead,
    W: Write,
{
    fn send_json_rpc(
        &mut self,
        line: &str,
        response_id: Option<&Value>,
    ) -> Result<ClientRoundTrip, AppServerClientError> {
        if let Some(error) = self.queue_overflow_error() {
            return Err(error);
        }

        writeln!(self.writer, "{line}")
            .map_err(|error| AppServerClientError::Transport(error.to_string()))?;
        self.writer
            .flush()
            .map_err(|error| AppServerClientError::Transport(error.to_string()))?;

        let Some(response_id) = response_id else {
            return Ok(ClientRoundTrip::default());
        };

        let mut round_trip = ClientRoundTrip::default();
        let mut response_line = String::new();
        loop {
            response_line.clear();
            let bytes = self
                .reader
                .read_line(&mut response_line)
                .map_err(|error| AppServerClientError::Transport(error.to_string()))?;
            if bytes == 0 {
                return Err(AppServerClientError::NoResponse);
            }

            let trimmed = response_line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let value = serde_json::from_str::<Value>(trimmed)
                .map_err(|error| AppServerClientError::Decode(error.to_string()))?;
            if value.get("id") == Some(response_id) {
                round_trip.response = Some(trimmed.to_string());
                return Ok(round_trip);
            }

            if value.get("id").is_none() && value.get("method").is_some() {
                let notification = serde_json::from_value::<ServerNotification>(value)
                    .map_err(|error| AppServerClientError::Decode(error.to_string()))?;
                if let Some(error) = self.remember_queue_overflow(&notification) {
                    return Err(error);
                }
                round_trip.notifications.push(notification);
                continue;
            }

            if let Some(actual) = value.get("id") {
                return Err(AppServerClientError::UnexpectedResponseId {
                    expected: response_id.clone(),
                    actual: actual.clone(),
                });
            }

            return Err(AppServerClientError::Decode(
                "expected app-server response or notification line".to_string(),
            ));
        }
    }

    fn read_json_rpc_notification(
        &mut self,
    ) -> Result<Option<ServerNotification>, AppServerClientError> {
        if let Some(error) = self.queue_overflow_error() {
            return Err(error);
        }

        let mut line = String::new();
        loop {
            line.clear();
            let bytes = self
                .reader
                .read_line(&mut line)
                .map_err(|error| AppServerClientError::Transport(error.to_string()))?;
            if bytes == 0 {
                return Ok(None);
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let value = serde_json::from_str::<Value>(trimmed)
                .map_err(|error| AppServerClientError::Decode(error.to_string()))?;
            if value.get("id").is_none() && value.get("method").is_some() {
                let notification = serde_json::from_value::<ServerNotification>(value)
                    .map_err(|error| AppServerClientError::Decode(error.to_string()))?;
                if let Some(error) = self.remember_queue_overflow(&notification) {
                    return Err(error);
                }
                return Ok(Some(notification));
            }

            if let Some(actual) = value.get("id") {
                return Err(AppServerClientError::UnexpectedResponseWithoutRequest {
                    actual: actual.clone(),
                });
            }

            return Err(AppServerClientError::Decode(
                "expected app-server notification line".to_string(),
            ));
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppServerClientError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("JSON encode error: {0}")]
    Encode(String),
    #[error("JSON decode error: {0}")]
    Decode(String),
    #[error("app-server returned no response")]
    NoResponse,
    #[error("app-server returned response id {actual:?} while waiting for {expected:?}")]
    UnexpectedResponseId { expected: Value, actual: Value },
    #[error("app-server returned response id {actual:?} without an active request")]
    UnexpectedResponseWithoutRequest { actual: Value },
    #[error(
        "app-server notification queue overflowed; reconnect required; retryable={retryable}: {message}"
    )]
    NotificationQueueOverflow { message: String, retryable: bool },
    #[error("app-server response did not include a result")]
    MissingResult,
    #[error("app-server error: {0:?}")]
    Response(Box<JsonRpcError>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NotificationQueueOverflowState {
    message: String,
    retryable: bool,
}

impl NotificationQueueOverflowState {
    fn from_event(event: ErrorEvent) -> Self {
        Self {
            message: event.message,
            retryable: event.retryable,
        }
    }

    fn to_error(&self) -> AppServerClientError {
        AppServerClientError::NotificationQueueOverflow {
            message: self.message.clone(),
            retryable: self.retryable,
        }
    }

    fn into_error(self) -> AppServerClientError {
        AppServerClientError::NotificationQueueOverflow {
            message: self.message,
            retryable: self.retryable,
        }
    }
}

fn notification_queue_overflow_state(
    notification: &ServerNotification,
) -> Option<NotificationQueueOverflowState> {
    if notification.method != event::ERROR {
        return None;
    }

    let event = serde_json::from_value::<ErrorEvent>(notification.params.clone()).ok()?;
    if event.code != ErrorCode::NotificationQueueOverflow {
        return None;
    }

    Some(NotificationQueueOverflowState::from_event(event))
}

fn serialize_optional_params(
    params: Option<impl Serialize>,
) -> Result<Option<Value>, AppServerClientError> {
    params
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| AppServerClientError::Encode(error.to_string()))
}

fn decode_notification_params<T>(
    notification: ServerNotification,
) -> Result<T, AppServerClientError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(notification.params)
        .map_err(|error| AppServerClientError::Decode(error.to_string()))
}

fn decode_json_rpc_response<R>(line: &str) -> Result<R, AppServerClientError>
where
    R: DeserializeOwned,
{
    let response = serde_json::from_str::<JsonRpcResponse>(line)
        .map_err(|error| AppServerClientError::Decode(error.to_string()))?;

    if let Some(error) = response.error {
        return Err(AppServerClientError::Response(Box::new(error)));
    }

    let result = response.result.ok_or(AppServerClientError::MissingResult)?;
    serde_json::from_value(result).map_err(|error| AppServerClientError::Decode(error.to_string()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnStartInputParams {
    thread_id: String,
    input: String,
}

impl TurnStartInputParams {
    fn new(thread_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            input: input.into(),
        }
    }
}

fn codex_v2_chat_subset_initialize_params(
    client: dasclaw_app_server_protocol::ClientInfo,
    workspace: Option<WorkspaceInfo>,
) -> InitializeParams {
    InitializeParams {
        client,
        protocol_version: dasclaw_app_server_protocol::ProtocolVersion::current(),
        workspace,
        requested_capabilities: vec![
            dasclaw_app_server_protocol::CompatibilityProfile::CODEX_APP_SERVER_V2_ID.to_string(),
        ],
        model_provider: None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use dasclaw_app_server::{
        AppServer, RuntimeBridge, RuntimeBridgeError, RuntimeTurnCancelRequest,
        RuntimeTurnStartRequest, run_stdio_server_with_app_server,
    };
    use dasclaw_app_server_protocol::{
        ClientInfo, CompatibilityProfile, LifecycleState, ProtocolVersion, ShutdownReason,
        TransportKind,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn in_process_client_initializes_and_reads_schema() {
        let mut server = AppServer::new();
        let transport = InProcessTransport::new(|line: &str| server.handle_json_rpc(line));
        let mut client = AppServerClient::new(transport);

        let initialize = client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec!["protocol".to_string()],
                model_provider: None,
            })
            .expect("initialize should succeed");
        let schema = client.protocol_schema().expect("schema should decode");

        assert_eq!(initialize.lifecycle.state, LifecycleState::Ready);
        assert!(
            schema
                .methods
                .iter()
                .any(|method| method.method == dasclaw_app_server_protocol::method::INITIALIZE)
        );
    }

    #[test]
    fn client_creates_in_process_threads() {
        let mut server = AppServer::new();
        let transport = InProcessTransport::new(|line: &str| {
            let response = server.handle_json_rpc(line);
            if response.is_some() {
                for _notification in server.drain_json_rpc_notifications() {}
            }
            response
        });
        let mut client = AppServerClient::new(transport);

        client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should succeed before session methods");
        let _ = client.drain_notifications();

        let response = client
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/create should return a thread id");
        let list = client.thread_list().expect("thread/list should decode");
        let read = client
            .thread_read(ThreadReadParams {
                thread_id: response.thread_id.clone(),
            })
            .expect("thread/read should decode");

        assert_eq!(response.thread_id, "thread_1");
        assert_eq!(list.threads.len(), 1);
        assert_eq!(read.thread.thread_id, "thread_1");
        assert_eq!(read.thread.title.as_deref(), Some("Draft"));
    }

    #[test]
    fn client_starts_and_cancels_in_process_turns() {
        let mut server = AppServer::new();
        let transport = InProcessTransport::new(|line: &str| server.handle_json_rpc(line));
        let mut client = AppServerClient::new(transport);

        client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should succeed before session methods");
        let thread = client
            .thread_create(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/create should return a thread id");

        let started = client
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn/start should create a pending in-memory turn");
        let cancelled = client
            .turn_cancel(TurnCancelParams {
                thread_id: thread.thread_id.clone(),
                turn_id: started.turn_id.clone(),
            })
            .expect("turn/cancel should cancel a pending in-memory turn");
        let turns = client
            .turn_list(TurnListParams {
                thread_id: thread.thread_id.clone(),
            })
            .expect("turn/list should decode");
        let read = client
            .turn_read(TurnReadParams {
                thread_id: thread.thread_id,
                turn_id: started.turn_id.clone(),
            })
            .expect("turn/read should decode");

        assert_eq!(started.turn_id, "turn_1");
        assert_eq!(
            started.status,
            dasclaw_app_server_protocol::TurnStatus::Pending
        );
        assert!(cancelled.accepted);
        assert_eq!(
            cancelled.status,
            dasclaw_app_server_protocol::TurnStatus::Cancelled
        );
        assert_eq!(turns.turns.len(), 1);
        assert_eq!(
            read.turn.status,
            dasclaw_app_server_protocol::TurnStatus::Cancelled
        );
    }

    #[test]
    fn client_captures_notifications_from_round_trip_transport() {
        let mut server = AppServer::new();
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        let initialize = client
            .initialize_with_notifications(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should succeed");
        let initialize_notifications = initialize.notifications;
        assert_eq!(initialize_notifications.len(), 3);
        assert!(matches!(
            initialize_notifications[0],
            AppServerNotification::LifecycleChanged(_)
        ));
        assert!(matches!(
            initialize_notifications[2],
            AppServerNotification::CapabilitiesChanged(_)
        ));

        let thread = client
            .thread_create_with_notifications(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/create should return a thread id");
        let started = client
            .turn_start_with_notifications(TurnStartParams {
                thread_id: thread.result.thread_id,
                prompt: "hello".to_string(),
            })
            .expect("turn/start should return a turn id");

        assert_eq!(started.result.turn_id, "turn_1");
        assert_eq!(thread.notifications.len(), 1);
        assert_eq!(started.notifications.len(), 2);
        assert!(matches!(
            thread.notifications[0],
            AppServerNotification::ThreadCreated(_)
        ));
        assert!(matches!(
            started.notifications[0],
            AppServerNotification::LifecycleChanged(_)
        ));
        assert!(matches!(
            started.notifications[1],
            AppServerNotification::TurnStarted(_)
        ));
    }

    #[test]
    fn client_exposes_codex_v2_thread_start_and_turn_interrupt_helpers() {
        let mut server = AppServer::new();
        let transport = InProcessTransport::new(|line: &str| server.handle_json_rpc(line));
        let mut client = AppServerClient::new(transport);

        client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: vec![
                    dasclaw_app_server_protocol::CompatibilityProfile::CODEX_APP_SERVER_V2_ID
                        .to_string(),
                ],
                model_provider: None,
            })
            .expect("initialize should succeed");
        let thread = client
            .thread_start(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/start should return a thread id");
        let started = client
            .turn_start_input(thread.thread_id.clone(), "hello")
            .expect("turn/start input helper should return a turn id");
        let interrupted = client
            .turn_interrupt(TurnCancelParams {
                thread_id: thread.thread_id,
                turn_id: started.turn_id,
            })
            .expect("turn/interrupt should cancel a pending turn");

        assert_eq!(interrupted, TurnInterruptResponse {});
    }

    #[test]
    fn client_helper_initializes_codex_v2_chat_subset_profile() {
        let mut server = AppServer::new();
        let transport = InProcessTransport::new(|line: &str| server.handle_json_rpc(line));
        let mut client = AppServerClient::new(transport);

        let initialize = client
            .initialize_codex_v2_chat_subset(
                ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                None,
            )
            .expect("helper should initialize the Codex v2 chat subset");
        let profile = initialize
            .compatibility_profiles
            .iter()
            .find(|profile| profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_ID)
            .expect("helper should request the Codex v2 chat subset profile");

        assert!(initialize.unavailable_requested_capabilities.is_empty());
        assert_eq!(
            profile.scope,
            dasclaw_app_server_protocol::CompatibilityProfileScope::ChatSessionSubset
        );
        let capabilities = client
            .capabilities()
            .expect("capabilities/list should decode after initialize");

        assert!(
            capabilities
                .compatibility_profiles
                .iter()
                .any(|profile| profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_ID)
        );
    }

    #[test]
    fn codex_v2_client_contract_uses_v2_requests_and_item_streaming_surface() {
        let mut server = AppServer::with_runtime_bridge(Arc::new(CodexV2StreamingBridge));
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        let initialize = client
            .initialize_codex_v2_chat_subset_with_notifications(
                ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                None,
            )
            .expect("initialize should negotiate the Codex v2 chat subset");
        assert!(matches!(
            initialize.notifications.as_slice(),
            [
                AppServerNotification::LifecycleChanged(_),
                AppServerNotification::LifecycleChanged(_),
                AppServerNotification::CapabilitiesChanged(_),
                AppServerNotification::NotificationsInitialized(_)
            ]
        ));
        let thread = client
            .thread_start_with_notifications(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/start should create a thread through the v2 helper");
        assert!(matches!(
            thread.notifications.as_slice(),
            [
                AppServerNotification::ThreadCreated(_),
                AppServerNotification::ThreadStarted(_)
            ]
        ));
        let turn = client
            .turn_start_input_with_notifications(thread.result.thread_id, "hello")
            .expect("turn/start input helper should stream item events through the v2 profile");
        let item_methods = turn
            .notifications
            .iter()
            .filter_map(|notification| match notification {
                AppServerNotification::ItemStarted(_) => Some(event::ITEM_STARTED),
                AppServerNotification::AgentMessageDelta(_) => {
                    Some(event::ITEM_AGENT_MESSAGE_DELTA)
                }
                AppServerNotification::TurnCompleted(_) => Some(event::TURN_COMPLETED),
                AppServerNotification::ItemCompleted(_) => Some(event::ITEM_COMPLETED),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            initialize
                .result
                .compatibility_profiles
                .iter()
                .any(|profile| profile.id == CompatibilityProfile::CODEX_APP_SERVER_V2_ID)
        );
        assert_eq!(turn.result.turn_id, "turn_1");
        assert_eq!(
            item_methods,
            vec![
                event::ITEM_STARTED,
                event::ITEM_AGENT_MESSAGE_DELTA,
                event::ITEM_COMPLETED,
                event::TURN_COMPLETED,
            ]
        );
    }

    #[test]
    fn codex_v2_client_contract_decodes_runtime_failure_as_error_event() {
        let mut server = AppServer::with_runtime_bridge(Arc::new(CodexV2FailingBridge));
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        client
            .initialize_codex_v2_chat_subset_with_notifications(
                ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                None,
            )
            .expect("initialize should negotiate the Codex v2 chat subset");
        let thread = client
            .thread_start_with_notifications(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/start should create a thread");
        let turn = client
            .turn_start_with_notifications(TurnStartParams {
                thread_id: thread.result.thread_id,
                prompt: "hello".to_string(),
            })
            .expect("turn/start should return a pending turn before failure notifications");

        assert_eq!(turn.result.turn_id, "turn_1");
        let terminal_events = turn
            .notifications
            .iter()
            .filter_map(|notification| match notification {
                AppServerNotification::ItemCompleted(event)
                    if event.status == dasclaw_app_server_protocol::TurnStatus::Failed =>
                {
                    Some(event::ITEM_COMPLETED)
                }
                AppServerNotification::TurnFailed(event) if event.error == "runtime failed" => {
                    Some(event::TURN_FAILED)
                }
                AppServerNotification::ProtocolError(event)
                    if event.message == "runtime failed" =>
                {
                    Some(event::ERROR)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            terminal_events,
            vec![event::ITEM_COMPLETED, event::TURN_FAILED, event::ERROR]
        );
    }

    #[test]
    fn codex_v2_client_contract_decodes_interrupt_as_cancelled_item_completion() {
        let mut server = AppServer::new();
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        client
            .initialize_codex_v2_chat_subset_with_notifications(
                ClientInfo {
                    name: "codex".to_string(),
                    version: "2.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                None,
            )
            .expect("initialize should negotiate the Codex v2 chat subset");
        let thread = client
            .thread_start_with_notifications(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/start should create a thread");
        let turn = client
            .turn_start_with_notifications(TurnStartParams {
                thread_id: thread.result.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn/start should create a pending turn");
        let interrupt = client
            .turn_interrupt_with_notifications(TurnCancelParams {
                thread_id: thread.result.thread_id,
                turn_id: turn.result.turn_id,
            })
            .expect("turn/interrupt should cancel the pending turn");

        assert_eq!(interrupt.result, TurnInterruptResponse {});
        let terminal_events = interrupt
            .notifications
            .iter()
            .filter_map(|notification| match notification {
                AppServerNotification::ItemCompleted(event)
                    if event.status == dasclaw_app_server_protocol::TurnStatus::Cancelled =>
                {
                    Some(event::ITEM_COMPLETED)
                }
                AppServerNotification::TurnCancelled(event)
                    if event.status == dasclaw_app_server_protocol::TurnStatus::Cancelled =>
                {
                    Some(event::TURN_CANCELLED)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            terminal_events,
            vec![event::ITEM_COMPLETED, event::TURN_CANCELLED]
        );
    }

    #[test]
    fn line_delimited_transport_preserves_raw_notification_order_before_response() {
        let reader = Cursor::new(
            [
                r#"{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"thread_1","turnId":"turn_1","status":"pending"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/started","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","itemType":"agent_message"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","delta":"hel"}}"#,
                r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turnId":"turn_1","status":"completed","output":"hello"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/completed","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","status":"completed"}}"#,
                r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
            ]
            .join("\n"),
        );
        let writer = Cursor::new(Vec::<u8>::new());
        let transport = LineDelimitedTransport::new(reader, writer);
        let mut client = AppServerClient::new(transport);

        let round_trip = client
            .request_value_with_notifications(method::HEALTH_CHECK, None::<()>)
            .expect("line-delimited transport should decode item notifications");

        assert_eq!(round_trip.result, json!({"ok": true}));
        assert_eq!(round_trip.notifications.len(), 5);
        assert!(matches!(
            round_trip.notifications[0],
            AppServerNotification::TurnStarted(_)
        ));
        assert!(matches!(
            round_trip.notifications[1],
            AppServerNotification::ItemStarted(_)
        ));
        assert!(matches!(
            round_trip.notifications[2],
            AppServerNotification::AgentMessageDelta(_)
        ));
        assert!(matches!(
            round_trip.notifications[3],
            AppServerNotification::TurnCompleted(_)
        ));
        assert!(matches!(
            round_trip.notifications[4],
            AppServerNotification::ItemCompleted(_)
        ));
    }

    #[test]
    fn line_delimited_transport_decodes_reasoning_notification_before_response() {
        let reader = Cursor::new(
            [
                r#"{"jsonrpc":"2.0","method":"item/reasoning/summaryTextDelta","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1:reasoning","summaryIndex":0,"delta":"private scratch"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","delta":"final answer"}}"#,
                r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
            ]
            .join("\n"),
        );
        let writer = Cursor::new(Vec::<u8>::new());
        let transport = LineDelimitedTransport::new(reader, writer);
        let mut client = AppServerClient::new(transport);

        let round_trip = client
            .request_value_with_notifications(method::HEALTH_CHECK, None::<()>)
            .expect("line-delimited transport should decode reasoning notifications");

        assert_eq!(round_trip.result, json!({"ok": true}));
        assert_eq!(round_trip.notifications.len(), 2);
        assert!(matches!(
            round_trip.notifications[0],
            AppServerNotification::ReasoningSummaryTextDelta(
                ReasoningSummaryTextDeltaEvent { ref delta, .. }
            ) if delta == "private scratch"
        ));
        assert!(matches!(
            round_trip.notifications[1],
            AppServerNotification::AgentMessageDelta(AgentMessageDeltaEvent { ref delta, .. })
                if delta == "final answer"
        ));
    }

    #[test]
    fn plain_request_preserves_raw_legacy_and_codex_notification_order_for_later_drain() {
        let reader = Cursor::new(
            [
                r#"{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"thread_1","turnId":"turn_1","status":"pending"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/started","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","itemType":"agent_message"}}"#,
                r#"{"jsonrpc":"2.0","method":"turn/delta","params":{"threadId":"thread_1","turnId":"turn_1","delta":"hel"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","delta":"hel"}}"#,
                r#"{"jsonrpc":"2.0","method":"item/completed","params":{"threadId":"thread_1","turnId":"turn_1","itemId":"turn_1","status":"completed"}}"#,
                r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turnId":"turn_1","status":"completed","output":"hello"}}"#,
                r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
            ]
            .join("\n"),
        );
        let writer = Cursor::new(Vec::<u8>::new());
        let transport = LineDelimitedTransport::new(reader, writer);
        let mut client = AppServerClient::new(transport);

        let result = client
            .request_value(method::HEALTH_CHECK, None::<()>)
            .expect("plain request should decode response");
        let methods = client
            .drain_notifications()
            .into_iter()
            .map(|notification| notification.method)
            .collect::<Vec<_>>();

        assert_eq!(result, json!({"ok": true}));
        assert_eq!(
            methods,
            vec![
                event::TURN_STARTED,
                event::ITEM_STARTED,
                event::TURN_DELTA,
                event::ITEM_AGENT_MESSAGE_DELTA,
                event::ITEM_COMPLETED,
                event::TURN_COMPLETED,
            ]
        );
    }

    #[derive(Debug)]
    struct CodexV2StreamingBridge;

    impl RuntimeBridge for CodexV2StreamingBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            request.updates.delta(
                request.thread_id.clone(),
                request.turn_id.clone(),
                "hel".to_string(),
            );
            request
                .updates
                .complete(request.thread_id, request.turn_id, "hello".to_string());
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
    struct CodexV2FailingBridge;

    impl RuntimeBridge for CodexV2FailingBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            request.updates.fail(
                request.thread_id,
                request.turn_id,
                "runtime failed".to_string(),
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

    #[test]
    fn request_with_notifications_returns_result_and_current_round_trip_notifications() {
        let mut server = AppServer::new();
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        let initialize = client
            .initialize_with_notifications(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should return result and notifications");

        assert_eq!(initialize.result.lifecycle.state, LifecycleState::Ready);
        assert_eq!(initialize.notifications.len(), 3);
        assert!(matches!(
            initialize.notifications[0],
            AppServerNotification::LifecycleChanged(_)
        ));
        assert_eq!(
            client
                .drain_typed_notifications()
                .expect("pending notifications should decode"),
            Vec::<AppServerNotification>::new()
        );
    }

    #[test]
    fn plain_request_still_queues_notifications_for_later_drain() {
        let mut server = AppServer::new();
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("plain initialize should succeed");

        let notifications = client
            .drain_typed_notifications()
            .expect("plain request notifications should decode");
        assert_eq!(notifications.len(), 3);
        assert!(matches!(
            notifications[0],
            AppServerNotification::LifecycleChanged(_)
        ));
    }

    #[test]
    fn typed_with_notifications_methods_cover_thread_and_turn_lifecycle() {
        let mut server = AppServer::new();
        let transport = |line: &str, _response_id: Option<&Value>| {
            let response = server.handle_json_rpc(line);
            let notifications = server.drain_notifications();
            Ok(ClientRoundTrip {
                response,
                notifications,
            })
        };
        let mut client = AppServerClient::new(transport);

        client
            .initialize_with_notifications(InitializeParams {
                client: ClientInfo {
                    name: "legacy-adapter-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should succeed");
        let thread = client
            .thread_create_with_notifications(ThreadCreateParams {
                title: Some("Draft".to_string()),
                workspace_root: None,
            })
            .expect("thread/create should return result and notification");
        let started = client
            .turn_start_with_notifications(TurnStartParams {
                thread_id: thread.result.thread_id.clone(),
                prompt: "hello".to_string(),
            })
            .expect("turn/start should return result and notification");
        let cancelled = client
            .turn_cancel_with_notifications(TurnCancelParams {
                thread_id: thread.result.thread_id,
                turn_id: started.result.turn_id,
            })
            .expect("turn/cancel should return result and notification");

        assert!(matches!(
            thread.notifications.as_slice(),
            [AppServerNotification::ThreadCreated(_)]
        ));
        assert!(matches!(
            started.notifications.as_slice(),
            [
                AppServerNotification::LifecycleChanged(_),
                AppServerNotification::TurnStarted(_)
            ]
        ));
        assert!(cancelled.result.accepted);
        assert!(matches!(
            cancelled.notifications.as_slice(),
            [
                AppServerNotification::TurnCancelled(_),
                AppServerNotification::LifecycleChanged(_)
            ]
        ));
    }

    #[test]
    fn line_delimited_transport_skips_notifications_until_matching_response() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","method":"lifecycle/changed","params":{"lifecycle":{"state":"ready","reason":"runtime_ready","since":"0"},"previousState":"starting"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"result":{"answer":42}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let value = client
            .request_value("test/echo", Some(json!({"ok": true})))
            .expect("matching response should decode");

        assert_eq!(value["answer"], 42);
        assert_eq!(client.drain_notifications().len(), 1);
    }

    #[test]
    fn line_delimited_transport_decodes_turn_notifications_before_matching_response() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","method":"turn/delta","params":{"threadId":"thread_1","turnId":"turn_1","delta":"hel"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turnId":"turn_1","status":"completed","output":"hello"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"turn/cancelled","params":{"threadId":"thread_2","turnId":"turn_2","status":"cancelled"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending"}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let value = client
            .request_value(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect("matching response should decode");
        let notifications = client
            .drain_typed_notifications()
            .expect("turn notifications should decode");

        assert_eq!(value["turnId"], "turn_1");
        assert_eq!(notifications.len(), 3);
        assert!(matches!(
            notifications[0],
            AppServerNotification::TurnDelta(TurnDeltaEvent { ref delta, .. }) if delta == "hel"
        ));
        assert!(matches!(
            notifications[1],
            AppServerNotification::TurnCompleted(TurnCompletedEvent { ref output, .. }) if output == "hello"
        ));
        assert!(matches!(
            notifications[2],
            AppServerNotification::TurnCancelled(TurnCancelledEvent { status, .. })
                if status == dasclaw_app_server_protocol::TurnStatus::Cancelled
        ));
    }

    #[test]
    fn line_delimited_transport_rejects_unexpected_response_id() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":2,"result":{"answer":13}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"result":{"answer":42}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let error = client
            .request_value("test/echo", Some(json!({"ok": true})))
            .expect_err("unexpected response ids should not be swallowed");

        assert!(matches!(
            error,
            AppServerClientError::UnexpectedResponseId { .. }
        ));
    }

    #[test]
    fn line_delimited_transport_reads_notification_after_matching_response() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turnId":"turn_1","status":"completed","output":"hello after response"}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let value = client
            .request_value(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect("matching response should decode");
        let notification = client
            .read_next_typed_notification()
            .expect("post-response notification line should decode")
            .expect("post-response notification should be available");

        assert_eq!(value["turnId"], "turn_1");
        assert!(matches!(
            notification,
            AppServerNotification::TurnCompleted(TurnCompletedEvent { ref output, .. })
                if output == "hello after response"
        ));
    }

    #[test]
    fn line_delimited_transport_reads_failed_notification_after_matching_response() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"turn/failed","params":{"threadId":"thread_1","turnId":"turn_1","status":"failed","error":"runtime failed after response"}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let value = client
            .request_value(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect("matching response should decode");
        let notification = client
            .read_next_typed_notification()
            .expect("post-response failed notification line should decode")
            .expect("post-response failed notification should be available");

        assert_eq!(value["turnId"], "turn_1");
        assert!(matches!(
            notification,
            AppServerNotification::TurnFailed(TurnFailedEvent { ref error, .. })
                if error == "runtime failed after response"
        ));
    }

    #[test]
    fn line_delimited_transport_reads_cancelled_notification_after_matching_response() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"turn/cancelled","params":{"threadId":"thread_1","turnId":"turn_1","status":"cancelled"}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let value = client
            .request_value(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect("matching response should decode");
        let notification = client
            .read_next_typed_notification()
            .expect("post-response cancelled notification line should decode")
            .expect("post-response cancelled notification should be available");

        assert_eq!(value["turnId"], "turn_1");
        assert!(matches!(
            notification,
            AppServerNotification::TurnCancelled(TurnCancelledEvent { status, .. })
                if status == dasclaw_app_server_protocol::TurnStatus::Cancelled
        ));
    }

    #[test]
    fn line_delimited_transport_rejects_malformed_response_line() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","result":{"answer":13}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"result":{"answer":42}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let error = client
            .request_value("test/echo", Some(json!({"ok": true})))
            .expect_err("malformed response lines should not be swallowed");

        assert!(
            matches!(error, AppServerClientError::Decode(message) if message.contains("response or notification"))
        );
    }

    #[test]
    fn line_delimited_transport_reports_queue_overflow_before_response() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","method":"error","params":{"code":"NOTIFICATION_QUEUE_OVERFLOW","message":"client notification queue exceeded bounded capacity; reconnect required","retryable":true}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let error = client
            .request_value(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect_err("overflow notification should force an explicit reconnect error");

        assert!(matches!(
            error,
            AppServerClientError::NotificationQueueOverflow {
                retryable: true,
                ..
            }
        ));
    }

    #[test]
    fn line_delimited_transport_does_not_reuse_connection_after_queue_overflow() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","method":"error","params":{"code":"NOTIFICATION_QUEUE_OVERFLOW","message":"client notification queue exceeded bounded capacity; reconnect required","retryable":true}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending"}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let first_error = client
            .request_value(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect_err("first request should observe the overflow notification");
        let second_error = client
            .request_value("lifecycle/status", Option::<serde_json::Value>::None)
            .expect_err("connection should be marked unusable after overflow");
        let (_reader, writer) = client.into_transport().into_parts();
        let output = String::from_utf8(writer).expect("written requests should be utf8");

        assert!(matches!(
            first_error,
            AppServerClientError::NotificationQueueOverflow { .. }
        ));
        assert!(matches!(
            second_error,
            AppServerClientError::NotificationQueueOverflow { .. }
        ));
        assert_eq!(output.matches(r#""method":"#).count(), 1);
    }

    #[test]
    fn client_detects_queue_overflow_from_real_app_server_transcript() {
        let server = AppServer::with_runtime_bridge(Arc::new(OverflowingRuntimeBridge));
        let requests = [
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "client": {
                        "name": "client-overflow-test",
                        "version": "0.0.0",
                        "transport": "stdio"
                    },
                    "protocolVersion": ProtocolVersion::current(),
                    "requestedCapabilities": []
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "thread/create",
                "params": {"title": "overflow"}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "turn/start",
                "params": {"threadId": "thread_1", "prompt": "overflow"}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "lifecycle/status"
            }),
        ]
        .into_iter()
        .map(|request| serde_json::to_string(&request).expect("request should serialize"))
        .collect::<Vec<_>>()
        .join("\n")
            + "\n";
        let mut transcript = Vec::new();

        run_stdio_server_with_app_server(server, Cursor::new(requests), &mut transcript)
            .expect("real stdio loop should produce the overflow transcript");

        let transport = LineDelimitedTransport::new(Cursor::new(transcript), Vec::new());
        let mut client = AppServerClient::new(transport);

        client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "client-overflow-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should consume the real server response");
        let thread = client
            .thread_create(ThreadCreateParams {
                title: Some("overflow".to_string()),
                workspace_root: None,
            })
            .expect("thread/create should consume the real server response");
        let overflow = client
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                prompt: "overflow".to_string(),
            })
            .expect_err("overflow from the real server transcript should force reconnect");
        let reuse = client
            .lifecycle_status()
            .expect_err("overflowed line transport must not be reused");
        let (_reader, writer) = client.into_transport().into_parts();
        let output = String::from_utf8(writer).expect("client requests should be utf8");

        assert!(matches!(
            overflow,
            AppServerClientError::NotificationQueueOverflow { .. }
        ));
        assert!(matches!(
            reuse,
            AppServerClientError::NotificationQueueOverflow { .. }
        ));
        assert!(!output.contains(r#""id":4"#));
    }

    #[test]
    fn client_health_check_round_trip_applies_terminal_update_from_real_stdio_loop() {
        let server = AppServer::with_runtime_bridge(Arc::new(HealthPollingRuntimeBridge));
        let before_health = [
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "client": {
                        "name": "client-health-poll-test",
                        "version": "0.0.0",
                        "transport": "stdio"
                    },
                    "protocolVersion": ProtocolVersion::current(),
                    "requestedCapabilities": []
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "thread/create",
                "params": {"title": "health poll"}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "turn/start",
                "params": {"threadId": "thread_1", "prompt": "finish before health"}
            }),
        ]
        .into_iter()
        .map(|request| serde_json::to_string(&request).expect("request should serialize"))
        .collect::<Vec<_>>()
        .join("\n")
            + "\n";
        let health = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "health/check",
            "params": {"includeDetails": false}
        }))
        .expect("health request should serialize")
            + "\n";
        let reader = TwoChunkDelayedRead::new(
            before_health.into_bytes(),
            health.into_bytes(),
            Duration::from_millis(40),
        );
        let mut transcript = Vec::new();

        run_stdio_server_with_app_server(server, std::io::BufReader::new(reader), &mut transcript)
            .expect("real stdio loop should produce health polling transcript");

        let transport = LineDelimitedTransport::new(Cursor::new(transcript), Vec::new());
        let mut client = AppServerClient::new(transport);
        client
            .initialize(InitializeParams {
                client: ClientInfo {
                    name: "client-health-poll-test".to_string(),
                    version: "0.0.0".to_string(),
                    transport: TransportKind::Stdio,
                },
                protocol_version: ProtocolVersion::current(),
                workspace: None,
                requested_capabilities: Vec::new(),
                model_provider: None,
            })
            .expect("initialize should decode");
        let thread = client
            .thread_create(ThreadCreateParams {
                title: Some("health poll".to_string()),
                workspace_root: None,
            })
            .expect("thread/create should decode");
        let turn = client
            .turn_start(TurnStartParams {
                thread_id: thread.thread_id,
                prompt: "finish before health".to_string(),
            })
            .expect("turn/start should decode before terminal update");
        let health = client
            .health_check_with_notifications(HealthCheckParams {
                include_details: false,
            })
            .expect("health/check should collect terminal notifications before response");

        assert_eq!(
            turn.status,
            dasclaw_app_server_protocol::TurnStatus::Pending
        );
        assert_eq!(health.result.lifecycle.state, LifecycleState::Ready);
        assert!(health.notifications.iter().any(|notification| {
            matches!(notification, AppServerNotification::TurnCompleted(_))
        }));
        assert!(health.notifications.iter().any(|notification| {
            matches!(
                notification,
                AppServerNotification::LifecycleChanged(event)
                    if event.lifecycle.state == LifecycleState::Ready
            )
        }));
    }

    #[test]
    fn line_delimited_transport_reports_queue_overflow_while_reading_notifications() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","method":"error","params":{"code":"NOTIFICATION_QUEUE_OVERFLOW","message":"client notification queue exceeded bounded capacity; reconnect required","retryable":true}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let error = client
            .read_next_typed_notification()
            .expect_err("overflow notification should not be exposed as a normal event");

        assert!(matches!(
            error,
            AppServerClientError::NotificationQueueOverflow {
                retryable: true,
                ..
            }
        ));
    }

    #[test]
    fn request_value_with_notifications_returns_line_delimited_turn_events() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","method":"turn/delta","params":{"threadId":"thread_1","turnId":"turn_1","delta":"hel"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread_1","turnId":"turn_1","status":"completed","output":"hello"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending"}}"#,
            "\n"
        );
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let round_trip = client
            .request_value_with_notifications(
                "turn/start",
                Some(json!({"threadId": "thread_1", "prompt": "hi"})),
            )
            .expect("round trip should decode");

        assert_eq!(round_trip.result["turnId"], "turn_1");
        assert_eq!(round_trip.notifications.len(), 2);
        assert!(matches!(
            round_trip.notifications[0],
            AppServerNotification::TurnDelta(TurnDeltaEvent { ref delta, .. }) if delta == "hel"
        ));
        assert!(matches!(
            round_trip.notifications[1],
            AppServerNotification::TurnCompleted(TurnCompletedEvent { ref output, .. }) if output == "hello"
        ));
        assert!(
            client
                .drain_typed_notifications()
                .expect("pending notifications should decode")
                .is_empty()
        );
    }

    #[test]
    fn turn_start_input_helper_writes_codex_input_field() {
        let input = r#"{"jsonrpc":"2.0","id":1,"result":{"turnId":"turn_1","status":"pending","lifecycle":{"state":"running","reason":"request_in_progress","since":"1"}}}"#
            .to_string()
            + "\n";
        let transport = LineDelimitedTransport::new(Cursor::new(input), Vec::new());
        let mut client = AppServerClient::new(transport);

        let started = client
            .turn_start_input("thread_1", "hi")
            .expect("turn/start input helper should decode");
        let (_reader, writer) = client.into_transport().into_parts();
        let output = String::from_utf8(writer).expect("written request should be utf8");
        let request = serde_json::from_str::<serde_json::Value>(&output)
            .expect("written request should be JSON-RPC");

        assert_eq!(started.turn_id, "turn_1");
        assert_eq!(request["method"], method::TURN_START);
        assert_eq!(
            request["params"],
            json!({"threadId": "thread_1", "input": "hi"})
        );
        assert!(
            request["params"].get("prompt").is_none(),
            "turn_start_input must not write the legacy prompt field"
        );
    }

    #[test]
    fn notify_writes_without_waiting_for_a_response() {
        let transport = LineDelimitedTransport::new(Cursor::new(""), Vec::new());
        let mut client = AppServerClient::new(transport);

        client
            .notify(
                dasclaw_app_server_protocol::method::SHUTDOWN,
                Some(ShutdownParams {
                    reason: Some(ShutdownReason::Test),
                    timeout_ms: None,
                }),
            )
            .expect("notification should only write");
        let (_reader, writer) = client.into_transport().into_parts();
        let output = String::from_utf8(writer).expect("written request should be utf8");

        assert!(output.contains(r#""method":"shutdown""#));
        assert!(!output.contains(r#""id":"#));
    }

    #[derive(Debug)]
    struct OverflowingRuntimeBridge;

    impl RuntimeBridge for OverflowingRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            for index in 0..=dasclaw_app_server_protocol::DEFAULT_MAX_PENDING_NOTIFICATIONS {
                request.updates.delta(
                    request.thread_id.clone(),
                    request.turn_id.clone(),
                    format!("chunk {index}"),
                );
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
    struct HealthPollingRuntimeBridge;

    impl RuntimeBridge for HealthPollingRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(15));
                request.updates.complete(
                    request.thread_id,
                    request.turn_id,
                    "done before health".to_string(),
                );
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

    struct TwoChunkDelayedRead {
        first: Cursor<Vec<u8>>,
        second: Cursor<Vec<u8>>,
        second_delay: Duration,
        reading_second: bool,
        delayed_second: bool,
    }

    impl TwoChunkDelayedRead {
        fn new(first: Vec<u8>, second: Vec<u8>, second_delay: Duration) -> Self {
            Self {
                first: Cursor::new(first),
                second: Cursor::new(second),
                second_delay,
                reading_second: false,
                delayed_second: false,
            }
        }
    }

    impl Read for TwoChunkDelayedRead {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if !self.reading_second {
                let bytes = self.first.read(buffer)?;
                if bytes > 0 {
                    return Ok(bytes);
                }
                self.reading_second = true;
            }

            if !self.delayed_second {
                thread::sleep(self.second_delay);
                self.delayed_second = true;
            }

            self.second.read(buffer)
        }
    }
}
