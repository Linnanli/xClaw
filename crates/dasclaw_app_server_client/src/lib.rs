//! Minimal client helpers for the dasclaw app-server protocol.
//!
//! This crate owns client-side JSON-RPC request construction and response
//! decoding only. It intentionally does not supervise an Electron sidecar
//! process and does not embed runtime/session behavior.

use std::io::{BufRead, Write};

use dasclaw_app_server_protocol::{
    CapabilitiesListResponse, HealthCheckParams, HealthCheckResponse, InitializeParams,
    InitializeResponse, JSON_RPC_VERSION, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    LifecycleStatusResponse, ProtocolSchemaResponse, ServerNotification, ShutdownParams,
    ShutdownResponse, ThreadCreateParams, ThreadCreateResponse, ThreadListResponse,
    ThreadReadParams, ThreadReadResponse, TurnCancelParams, TurnCancelResponse, TurnListParams,
    TurnListResponse, TurnReadParams, TurnReadResponse, TurnStartParams, TurnStartResponse, method,
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

    pub fn health_check(
        &mut self,
        params: HealthCheckParams,
    ) -> Result<HealthCheckResponse, AppServerClientError> {
        self.request(method::HEALTH_CHECK, Some(params))
    }

    pub fn capabilities(&mut self) -> Result<CapabilitiesListResponse, AppServerClientError> {
        self.request::<(), CapabilitiesListResponse>(method::CAPABILITIES_LIST, None)
    }

    pub fn lifecycle_status(&mut self) -> Result<LifecycleStatusResponse, AppServerClientError> {
        self.request::<(), LifecycleStatusResponse>(method::LIFECYCLE_STATUS, None)
    }

    pub fn protocol_schema(&mut self) -> Result<ProtocolSchemaResponse, AppServerClientError> {
        self.request::<(), ProtocolSchemaResponse>(method::PROTOCOL_SCHEMA, None)
    }

    pub fn shutdown(
        &mut self,
        params: ShutdownParams,
    ) -> Result<ShutdownResponse, AppServerClientError> {
        self.request(method::SHUTDOWN, Some(params))
    }

    pub fn thread_create(
        &mut self,
        params: ThreadCreateParams,
    ) -> Result<ThreadCreateResponse, AppServerClientError> {
        self.request(method::THREAD_CREATE, Some(params))
    }

    pub fn thread_list(&mut self) -> Result<ThreadListResponse, AppServerClientError> {
        self.request::<(), ThreadListResponse>(method::THREAD_LIST, None)
    }

    pub fn thread_read(
        &mut self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse, AppServerClientError> {
        self.request(method::THREAD_READ, Some(params))
    }

    pub fn turn_start(
        &mut self,
        params: TurnStartParams,
    ) -> Result<TurnStartResponse, AppServerClientError> {
        self.request(method::TURN_START, Some(params))
    }

    pub fn turn_cancel(
        &mut self,
        params: TurnCancelParams,
    ) -> Result<TurnCancelResponse, AppServerClientError> {
        self.request(method::TURN_CANCEL, Some(params))
    }

    pub fn turn_list(
        &mut self,
        params: TurnListParams,
    ) -> Result<TurnListResponse, AppServerClientError> {
        self.request(method::TURN_LIST, Some(params))
    }

    pub fn turn_read(
        &mut self,
        params: TurnReadParams,
    ) -> Result<TurnReadResponse, AppServerClientError> {
        self.request(method::TURN_READ, Some(params))
    }

    pub fn request_value(
        &mut self,
        method: &'static str,
        params: Option<impl Serialize>,
    ) -> Result<Value, AppServerClientError> {
        self.request(method, params)
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

    fn request<P, R>(
        &mut self,
        method: &'static str,
        params: Option<P>,
    ) -> Result<R, AppServerClientError>
    where
        P: Serialize,
        R: DeserializeOwned,
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
        self.pending_notifications.extend(round_trip.notifications);
        let response_line = round_trip
            .response
            .ok_or(AppServerClientError::NoResponse)?;
        let response = serde_json::from_str::<JsonRpcResponse>(&response_line)
            .map_err(|error| AppServerClientError::Decode(error.to_string()))?;

        if let Some(error) = response.error {
            return Err(AppServerClientError::Response(error));
        }

        let result = response.result.ok_or(AppServerClientError::MissingResult)?;
        serde_json::from_value(result)
            .map_err(|error| AppServerClientError::Decode(error.to_string()))
    }
}

pub trait AppServerTransport {
    fn send_json_rpc(
        &mut self,
        line: &str,
        response_id: Option<&Value>,
    ) -> Result<ClientRoundTrip, AppServerClientError>;
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
            }
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
    #[error("app-server response did not include a result")]
    MissingResult,
    #[error("app-server error: {0:?}")]
    Response(JsonRpcError),
}

fn serialize_optional_params(
    params: Option<impl Serialize>,
) -> Result<Option<Value>, AppServerClientError> {
    params
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| AppServerClientError::Encode(error.to_string()))
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
