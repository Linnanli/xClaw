//! Minimal client helpers for the dasclaw app-server protocol.
//!
//! This crate owns client-side JSON-RPC request construction and response
//! decoding only. It intentionally does not supervise an Electron sidecar
//! process and does not embed runtime/session behavior.

use std::io::{BufRead, Write};

use dasclaw_app_server_protocol::{
    CapabilitiesChangedEvent, CapabilitiesListResponse, HealthChangedEvent, HealthCheckParams,
    HealthCheckResponse, InitializeParams, InitializeResponse, JSON_RPC_VERSION, JsonRpcError,
    JsonRpcRequest, JsonRpcResponse, LifecycleChangedEvent, LifecycleStatusResponse, LogEntryEvent,
    ProtocolSchemaResponse, ServerNotification, ShutdownParams, ShutdownResponse,
    ThreadCreateParams, ThreadCreateResponse, ThreadCreatedEvent, ThreadListResponse,
    ThreadReadParams, ThreadReadResponse, TurnCancelParams, TurnCancelResponse, TurnCancelledEvent,
    TurnCompletedEvent, TurnDeltaEvent, TurnFailedEvent, TurnListParams, TurnListResponse,
    TurnReadParams, TurnReadResponse, TurnStartParams, TurnStartResponse, TurnStartedEvent, event,
    method,
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
    LifecycleChanged(LifecycleChangedEvent),
    HealthChanged(HealthChangedEvent),
    CapabilitiesChanged(Box<CapabilitiesChangedEvent>),
    LogEntry(LogEntryEvent),
    ThreadCreated(ThreadCreatedEvent),
    TurnStarted(TurnStartedEvent),
    TurnDelta(TurnDeltaEvent),
    TurnCompleted(TurnCompletedEvent),
    TurnFailed(TurnFailedEvent),
    TurnCancelled(TurnCancelledEvent),
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
            event::LIFECYCLE_CHANGED => {
                Self::LifecycleChanged(decode_notification_params(notification)?)
            }
            event::HEALTH_CHANGED => Self::HealthChanged(decode_notification_params(notification)?),
            event::CAPABILITIES_CHANGED => {
                Self::CapabilitiesChanged(Box::new(decode_notification_params(notification)?))
            }
            event::LOG_ENTRY => Self::LogEntry(decode_notification_params(notification)?),
            event::THREAD_CREATED => Self::ThreadCreated(decode_notification_params(notification)?),
            event::TURN_STARTED => Self::TurnStarted(decode_notification_params(notification)?),
            event::TURN_DELTA => Self::TurnDelta(decode_notification_params(notification)?),
            event::TURN_COMPLETED => Self::TurnCompleted(decode_notification_params(notification)?),
            event::TURN_FAILED => Self::TurnFailed(decode_notification_params(notification)?),
            event::TURN_CANCELLED => Self::TurnCancelled(decode_notification_params(notification)?),
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
}

impl<R, W> LineDelimitedTransport<R, W> {
    #[must_use]
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    #[must_use]
    pub fn into_parts(self) -> (R, W) {
        (self.reader, self.writer)
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
    #[error("app-server response did not include a result")]
    MissingResult,
    #[error("app-server error: {0:?}")]
    Response(Box<JsonRpcError>),
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use dasclaw_app_server::AppServer;
    use dasclaw_app_server_protocol::{
        ClientInfo, LifecycleState, ProtocolVersion, ShutdownReason, TransportKind,
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
        assert_eq!(started.notifications.len(), 1);
        assert!(matches!(
            thread.notifications[0],
            AppServerNotification::ThreadCreated(_)
        ));
        assert!(matches!(
            started.notifications[0],
            AppServerNotification::TurnStarted(_)
        ));
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
            [AppServerNotification::TurnStarted(_)]
        ));
        assert!(cancelled.result.accepted);
        assert!(matches!(
            cancelled.notifications.as_slice(),
            [AppServerNotification::TurnCancelled(_)]
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
}
