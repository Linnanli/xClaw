//! Line-delimited stdio JSON-RPC entrypoint for the Phase 1 app-server.
//!
//! Each non-empty stdin line is treated as one JSON-RPC request. Each response
//! is written as one stdout line so Electron can supervise this process as a
//! simple sidecar before we commit to a longer-lived socket transport.

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_app_server::{
    AppServer, DasclawAgentRuntimeBridge, app_services::AppServerServices,
    run_stdio_server_with_app_server,
};
use dasclaw_app_server_protocol::{
    CapabilitiesListResponse, ClientInfo, ClientModelConfig, HealthCheckParams,
    HealthCheckResponse, InitializeParams, InitializeResponse, LifecycleStatusResponse,
    ModelProviderInitializeConfig, ProtocolSchemaResponse, ProtocolVersion, ThreadListParams,
    ThreadListResponse, ThreadReadParams, ThreadReadResponse, ThreadStartParams,
    ThreadStartResponse, ThreadTurnsListParams, ThreadTurnsListResponse, TransportKind,
    TurnInterruptParams, TurnInterruptResponse, TurnReadParams, TurnReadResponse, TurnStartParams,
    TurnStartResponse, UserInput,
};
use dasclaw_core::agentic_loop::AgentResponder;
use dasclaw_core::messages::{FinishReason, Role};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use serde::Serialize;

fn main() {
    match parse_run_mode(std::env::args().skip(1)) {
        Ok(RunMode::Stdio) => {
            let server = match build_stdio_app_server() {
                Ok(server) => server,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            };
            if let Err(error) = run_stdio_server_with_app_server(
                server,
                io::BufReader::new(io::stdin()),
                io::stdout().lock(),
            ) {
                eprintln!("dasclaw-app-server stdio loop failed: {error}");
                std::process::exit(1);
            }
        }
        Ok(RunMode::HealthOnce) => print_health_once(),
        Ok(RunMode::VersionJson) => print_version_json(),
        Ok(RunMode::CapabilitiesOnce) => print_capabilities_once(),
        Ok(RunMode::SchemaOnce) => print_schema_once(),
        Ok(RunMode::SelfCheck) => print_self_check(),
        Ok(RunMode::Help) => println!("{}", usage()),
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Stdio,
    HealthOnce,
    VersionJson,
    CapabilitiesOnce,
    SchemaOnce,
    SelfCheck,
    Help,
}

fn parse_run_mode(args: impl IntoIterator<Item = String>) -> Result<RunMode, String> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(RunMode::Stdio),
        [flag] if flag == "--health-once" => Ok(RunMode::HealthOnce),
        [flag] if flag == "--version-json" => Ok(RunMode::VersionJson),
        [flag] if flag == "--capabilities-once" => Ok(RunMode::CapabilitiesOnce),
        [flag] if flag == "--schema-once" => Ok(RunMode::SchemaOnce),
        [flag] if flag == "--self-check" => Ok(RunMode::SelfCheck),
        [flag] if flag == "--help" || flag == "-h" => Ok(RunMode::Help),
        [flag] => Err(format!("unknown argument: {flag}")),
        _ => Err("expected at most one argument".to_string()),
    }
}

fn usage() -> &'static str {
    "usage: dasclaw-app-server [--health-once | --version-json | --capabilities-once | --schema-once | --self-check | --help]"
}

fn build_stdio_app_server() -> Result<AppServer, String> {
    app_server_for_env_runtime_mode()
}

fn app_server_for_env_runtime_mode() -> Result<AppServer, String> {
    let mode = std::env::var("DASCLAW_APP_SERVER_RUNTIME").ok();
    app_server_for_runtime_mode(mode.as_deref())
}

fn app_server_for_runtime_mode(mode: Option<&str>) -> Result<AppServer, String> {
    match mode {
        None => Ok(default_app_server()),
        Some("echo") => Ok(AppServer::with_runtime_responder(Arc::new(EchoResponder))
            .with_app_services(AppServerServices::real())),
        Some("noop") => Ok(AppServer::new()),
        Some(other) => Err(format!(
            "unknown DASCLAW_APP_SERVER_RUNTIME: {other}; expected echo or noop"
        )),
    }
}

fn default_app_server() -> AppServer {
    AppServer::with_runtime_bridge(Arc::new(
        DasclawAgentRuntimeBridge::from_model_provider_snapshot(),
    ))
    .with_app_services(AppServerServices::real())
}

#[derive(Debug)]
struct EchoResponder;

#[async_trait]
impl AgentResponder for EchoResponder {
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        let last_user_text = ctx
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, Role::User))
            .map(|message| message.content.clone())
            .unwrap_or_default();

        Ok(RespondOutput {
            result: RespondResult::Text(format!("echo: {last_user_text}")),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }
}

fn print_version_json() {
    let server = AppServer::new();
    match serde_json::to_string(&server.server_info()) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("dasclaw-app-server failed to serialize version: {error}");
            std::process::exit(1);
        }
    }
}

fn print_health_once() {
    match health_once_response_for_env_runtime_mode().and_then(|health| {
        serde_json::to_string(&health)
            .map_err(|error| format!("dasclaw-app-server failed to serialize health: {error}"))
    }) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn print_capabilities_once() {
    match capabilities_once_response_for_env_runtime_mode().and_then(|capabilities| {
        serde_json::to_string(&capabilities).map_err(|error| {
            format!("dasclaw-app-server failed to serialize capabilities: {error}")
        })
    }) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn print_schema_once() {
    match schema_once_response_for_env_runtime_mode().and_then(|schema| {
        serde_json::to_string(&schema).map_err(|error| {
            format!("dasclaw-app-server failed to serialize protocol schema: {error}")
        })
    }) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn print_self_check() {
    match self_check_report_for_env_runtime_mode() {
        Ok(report) => match serde_json::to_string(&report) {
            Ok(line) => println!("{line}"),
            Err(error) => {
                eprintln!("dasclaw-app-server failed to serialize self-check: {error}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("dasclaw-app-server self-check failed: {error}");
            std::process::exit(1);
        }
    }
}

fn health_once_response_for_env_runtime_mode() -> Result<HealthCheckResponse, String> {
    let mode = std::env::var("DASCLAW_APP_SERVER_RUNTIME").ok();
    health_once_response_for_runtime_mode(mode.as_deref())
}

fn health_once_response_for_runtime_mode(
    mode: Option<&str>,
) -> Result<HealthCheckResponse, String> {
    let mut server = app_server_for_runtime_mode(mode)?;
    Ok(server.health_check(HealthCheckParams {
        include_details: true,
    }))
}

fn capabilities_once_response_for_env_runtime_mode() -> Result<CapabilitiesListResponse, String> {
    let mode = std::env::var("DASCLAW_APP_SERVER_RUNTIME").ok();
    capabilities_once_response_for_runtime_mode(mode.as_deref())
}

fn capabilities_once_response_for_runtime_mode(
    mode: Option<&str>,
) -> Result<CapabilitiesListResponse, String> {
    let mut server = app_server_for_runtime_mode(mode)?;
    Ok(server.capabilities())
}

fn schema_once_response_for_env_runtime_mode() -> Result<ProtocolSchemaResponse, String> {
    let mode = std::env::var("DASCLAW_APP_SERVER_RUNTIME").ok();
    schema_once_response_for_runtime_mode(mode.as_deref())
}

fn schema_once_response_for_runtime_mode(
    mode: Option<&str>,
) -> Result<ProtocolSchemaResponse, String> {
    let server = app_server_for_runtime_mode(mode)?;
    Ok(server.protocol_schema())
}

fn self_check_report_for_env_runtime_mode() -> Result<SelfCheckReport, String> {
    let mode = std::env::var("DASCLAW_APP_SERVER_RUNTIME").ok();
    self_check_report_for_runtime_mode(mode.as_deref())
}

fn self_check_report_for_runtime_mode(mode: Option<&str>) -> Result<SelfCheckReport, String> {
    let server = self_check_app_server_for_runtime_mode(mode)?;
    self_check_report_with_server(server).map_err(|error| error.to_string())
}

fn self_check_app_server_for_runtime_mode(mode: Option<&str>) -> Result<AppServer, String> {
    match mode {
        None => Ok(AppServer::with_runtime_responder(Arc::new(EchoResponder))
            .with_app_services(AppServerServices::real())),
        Some(_) => app_server_for_runtime_mode(mode),
    }
}

fn self_check_report_with_server(
    mut server: AppServer,
) -> Result<SelfCheckReport, dasclaw_app_server::AppServerError> {
    let initialize = server.initialize(InitializeParams {
        client: ClientInfo {
            name: "dasclaw-app-server-self-check".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            transport: TransportKind::Stdio,
        },
        protocol_version: ProtocolVersion::current(),
        workspace: None,
        requested_capabilities: vec![
            "protocol".to_string(),
            "lifecycle".to_string(),
            "health".to_string(),
        ],
        model_provider: Some(self_check_model_provider_config()),
    })?;
    let pending_notifications = server.drain_notifications().len();
    let thread_start = server.thread_start(ThreadStartParams {
        cwd: Some("self-check".to_string()),
        sandbox: None,
        permission_profile: None,
    })?;
    let thread_id = thread_start.thread.id.clone();
    let thread_list = server.thread_list(ThreadListParams {
        cursor: None,
        limit: None,
        sort_direction: None,
    })?;
    let thread_read = server.thread_read(ThreadReadParams {
        thread_id: thread_id.clone(),
    })?;
    let turn_start = server.turn_start(TurnStartParams {
        thread_id: thread_id.clone(),
        input: vec![UserInput::Text {
            text: "self-check prompt exercises runtime bridge session bookkeeping".to_string(),
            text_elements: Vec::new(),
        }],
        cwd: None,
        model: None,
        summary: None,
        sandbox_policy: None,
        permission_profile: None,
    })?;
    let turn_id = turn_start.turn.id.clone();
    let turn_interrupt = server.turn_interrupt(TurnInterruptParams {
        thread_id: thread_id.clone(),
        turn_id: turn_id.clone(),
    })?;
    let turn_list = server.thread_turns_list(ThreadTurnsListParams {
        thread_id: thread_id.clone(),
        cursor: None,
        limit: None,
        sort_direction: None,
    })?;
    let turn_read = server.turn_read(TurnReadParams { thread_id, turn_id })?;
    let session_notifications = server.drain_notifications().len();

    Ok(SelfCheckReport {
        ok: true,
        initialize,
        lifecycle: server.lifecycle_status(),
        health: server.health_check(HealthCheckParams {
            include_details: true,
        }),
        capabilities: server.capabilities(),
        schema: server.protocol_schema(),
        pending_notifications,
        session: SelfCheckSessionReport {
            thread_start,
            thread_list,
            thread_read,
            turn_start,
            turn_interrupt,
            turn_list,
            turn_read,
            pending_notifications: session_notifications,
        },
    })
}

fn self_check_model_provider_config() -> ModelProviderInitializeConfig {
    let selected_model = self_check_model_config("self-check-model");
    ModelProviderInitializeConfig {
        models: vec![selected_model.clone()],
        selected_model,
    }
}

fn self_check_model_config(model_id: &str) -> ClientModelConfig {
    ClientModelConfig {
        model_id: model_id.to_string(),
        display_name: Some(model_id.to_string()),
        provider: Some("self_check".to_string()),
        api_base_url: Some("http://localhost/self-check/v1".to_string()),
        api_key: Some("self-check-api-key".to_string()),
        api_format: Some("openai".to_string()),
        model_call_mode: None,
        source: Some("self_check".to_string()),
        capabilities: Vec::new(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelfCheckReport {
    ok: bool,
    initialize: InitializeResponse,
    lifecycle: LifecycleStatusResponse,
    health: HealthCheckResponse,
    capabilities: CapabilitiesListResponse,
    schema: ProtocolSchemaResponse,
    pending_notifications: usize,
    session: SelfCheckSessionReport,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelfCheckSessionReport {
    thread_start: ThreadStartResponse,
    thread_list: ThreadListResponse,
    thread_read: ThreadReadResponse,
    turn_start: TurnStartResponse,
    turn_interrupt: TurnInterruptResponse,
    turn_list: ThreadTurnsListResponse,
    turn_read: TurnReadResponse,
    pending_notifications: usize,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use dasclaw_app_server::{
        RuntimeApprovalDecision, RuntimeApprovalRequest, RuntimeBridge, RuntimeBridgeError,
        RuntimeBridgeFeatures, RuntimeCommandOutputDeltaUpdate, RuntimeToolResultUpdate,
        RuntimeTurnCancelRequest, RuntimeTurnStartRequest,
    };
    use dasclaw_app_server_protocol::ProtocolVersion;
    use serde_json::Value;

    use dasclaw_app_server::{run_stdio_server, run_stdio_server_with_app_server};

    use super::{RunMode, app_server_for_runtime_mode, parse_run_mode};

    #[test]
    fn parse_run_mode_defaults_to_stdio() {
        assert_eq!(parse_run_mode(Vec::new()).expect("mode"), RunMode::Stdio);
    }

    #[test]
    fn parse_run_mode_recognizes_one_shot_flags() {
        assert_eq!(
            parse_run_mode(["--health-once".to_string()]).expect("mode"),
            RunMode::HealthOnce
        );
        assert_eq!(
            parse_run_mode(["--version-json".to_string()]).expect("mode"),
            RunMode::VersionJson
        );
        assert_eq!(
            parse_run_mode(["--capabilities-once".to_string()]).expect("mode"),
            RunMode::CapabilitiesOnce
        );
        assert_eq!(
            parse_run_mode(["--schema-once".to_string()]).expect("mode"),
            RunMode::SchemaOnce
        );
        assert_eq!(
            parse_run_mode(["--self-check".to_string()]).expect("mode"),
            RunMode::SelfCheck
        );
        assert_eq!(
            parse_run_mode(["--help".to_string()]).expect("mode"),
            RunMode::Help
        );
    }

    #[test]
    fn parse_run_mode_rejects_unknown_or_multiple_args() {
        assert!(parse_run_mode(["--typo".to_string()]).is_err());
        assert!(
            parse_run_mode(["--health-once".to_string(), "--version-json".to_string()]).is_err()
        );
    }

    #[test]
    fn stdio_loop_routes_one_request_per_line() {
        let input = r#"{"jsonrpc":"2.0","id":"req_1","method":"lifecycle/status"}"#;
        let mut output = Vec::new();

        run_stdio_server(Cursor::new(format!("{input}\n")), &mut output)
            .expect("stdio loop should handle a valid line");

        let response = String::from_utf8(output).expect("response should be utf8");
        let value: Value = serde_json::from_str(response.trim()).expect("response should be JSON");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], "req_1");
        assert_eq!(value["result"]["lifecycle"]["state"], "starting");
    }

    #[test]
    fn stdio_loop_writes_notifications_before_response() {
        let input = r#"{"jsonrpc":"2.0","id":"req_1","method":"initialize","params":{"client":{"name":"open-cowork","version":"0.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":[]}}"#;
        let mut output = Vec::new();

        run_stdio_server(Cursor::new(format!("{input}\n")), &mut output)
            .expect("stdio loop should handle initialize");

        let response = String::from_utf8(output).expect("response should be utf8");
        let lines = response.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 5);

        let initializing: Value =
            serde_json::from_str(lines[0]).expect("initializing notification should be JSON");
        let ready: Value =
            serde_json::from_str(lines[1]).expect("ready notification should be JSON");
        let capabilities: Value =
            serde_json::from_str(lines[2]).expect("capability notification should be JSON");
        let initialized: Value =
            serde_json::from_str(lines[3]).expect("initialized notification should be JSON");
        let response: Value = serde_json::from_str(lines[4]).expect("response should be JSON");

        assert_eq!(initializing["method"], "lifecycle/changed");
        assert_eq!(initializing["params"]["lifecycle"]["state"], "initializing");
        assert_eq!(ready["method"], "lifecycle/changed");
        assert_eq!(ready["params"]["lifecycle"]["state"], "ready");
        assert_eq!(capabilities["method"], "capabilities/changed");
        assert_eq!(initialized["method"], "notifications/initialized");
        assert_eq!(
            initialized["params"]["compatibilityProfiles"][0]["id"],
            "codex_app_server_v2_chat_session_subset"
        );
        assert_eq!(response["result"]["lifecycle"]["state"], "ready");
    }

    #[test]
    fn stdio_loop_keeps_server_state_across_lines() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"open-cowork","version":"0.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":[]}}"#;
        let health = r#"{"jsonrpc":"2.0","id":"health","method":"health/check","params":{"includeDetails":true}}"#;
        let mut output = Vec::new();

        run_stdio_server(
            Cursor::new(format!("{initialize}\n{health}\n")),
            &mut output,
        )
        .expect("stdio loop should process multiple lines in one server instance");

        let response = String::from_utf8(output).expect("response should be utf8");
        let lines = response.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 6);

        let init_response: Value = serde_json::from_str(lines[4]).expect("init response JSON");
        let health_response: Value = serde_json::from_str(lines[5]).expect("health response JSON");

        assert_eq!(init_response["result"]["lifecycle"]["state"], "ready");
        assert_eq!(health_response["id"], "health");
        assert_eq!(health_response["result"]["ok"], true);
        assert_eq!(health_response["result"]["lifecycle"]["state"], "ready");
    }

    #[test]
    fn stdio_loop_can_run_with_injected_app_server_runtime_bridge() {
        let initialize = initialize_request("open-cowork", []);
        let create =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#;
        let mut output = Vec::new();
        let bridge = Arc::new(CompletingRuntimeBridge::default());
        let server = dasclaw_app_server::AppServer::with_runtime_bridge(bridge);

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!("{initialize}\n{create}\n{start}\n")),
            &mut output,
        )
        .expect("stdio loop should use the injected server");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();

        assert!(
            values
                .iter()
                .any(|value| value["method"] == "turn/completed")
        );
        assert!(values.iter().any(|value| {
            value["id"] == "turn" && value["result"]["turn"]["status"] == "inProgress"
        }));
    }

    #[test]
    fn stdio_loop_supports_codex_v2_chat_subset_transcript() {
        let initialize = initialize_request("codex", ["codex_app_server_v2_chat_session_subset"]);
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#;
        let mut output = Vec::new();
        let bridge = Arc::new(CompletingRuntimeBridge::default());
        let server = dasclaw_app_server::AppServer::with_runtime_bridge(bridge);

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!("{initialize}\n{thread_start}\n{turn_start}\n")),
            &mut output,
        )
        .expect("stdio loop should support the Codex v2 chat subset transcript");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();
        let initialize_response = values
            .iter()
            .find(|value| value["id"] == "init")
            .expect("initialize response should be present");
        let profile = initialize_response["result"]["compatibilityProfiles"]
            .as_array()
            .expect("compatibility profiles should be an array")
            .iter()
            .find(|profile| profile["id"] == "codex_app_server_v2_chat_session_subset")
            .expect("Codex v2 profile should be advertised");
        let methods = values
            .iter()
            .filter_map(|value| value.get("method").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(profile["scope"], "chat_session_subset");
        assert!(values.iter().any(|value| {
            value["id"] == "thread" && value["result"]["thread"]["id"] == "thread_1"
        }));
        assert!(
            values.iter().any(|value| {
                value["id"] == "turn" && value["result"]["turn"]["id"] == "turn_1"
            })
        );
        assert!(methods.contains(&"notifications/initialized"));
        assert!(methods.contains(&"item/started"));
        assert!(methods.contains(&"item/completed"));
        assert!(methods.contains(&"turn/completed"));
        assert!(!methods.contains(&"turn/delta"));
        let item_completed_position = methods
            .iter()
            .position(|method| *method == "item/completed")
            .expect("item/completed should be present");
        let turn_completed_position = methods
            .iter()
            .position(|method| *method == "turn/completed")
            .expect("turn/completed should be present");
        assert!(item_completed_position < turn_completed_position);
    }

    #[test]
    fn echo_runtime_mode_streams_real_runtime_completion_over_stdio() {
        let initialize =
            initialize_request("open-cowork", ["codex_app_server_v2_chat_session_subset"]);
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello echo","text_elements":[]}]}}"#;
        let mut output = Vec::new();
        let server = app_server_for_runtime_mode(Some("echo")).expect("echo mode should build");

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!("{initialize}\n{thread_start}\n{turn_start}\n")),
            &mut output,
        )
        .expect("echo runtime mode should produce a stdio transcript");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == "item/agentMessage/delta"
                && value["params"]["delta"] == "echo: hello echo"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/completed"
                && value["params"]["item"]["text"] == "echo: hello echo"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "turn/completed" && value["params"]["turn"]["status"] == "completed"
        }));
    }

    #[test]
    fn default_runtime_mode_uses_model_provider_snapshot_bridge() {
        let initialize = initialize_request_with_model_provider(
            "open-cowork",
            [],
            serde_json::json!({
                "models": [test_model_config_with_api_format("gpt-test", "unsupported")],
                "selectedModel": test_model_config_with_api_format("gpt-test", "unsupported")
            }),
        );
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#;
        let mut output = Vec::new();
        let server = app_server_for_runtime_mode(None).expect("default runtime mode should build");

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!("{initialize}\n{thread_start}\n{turn_start}\n")),
            &mut output,
        )
        .expect("stdio loop should process the default runtime mode transcript");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();
        let turn_response = values
            .iter()
            .find(|value| value["id"] == "turn")
            .expect("turn response should be present");
        let thread_response = values
            .iter()
            .find(|value| value["id"] == "thread")
            .expect("thread response should be present");

        assert_eq!(thread_response["result"]["thread"]["id"], "thread_1");
        assert!(turn_response.get("error").is_some());
        assert!(turn_response.get("result").is_none());
    }

    fn initialize_request(
        client_name: &str,
        requested_capabilities: impl IntoIterator<Item = &'static str>,
    ) -> String {
        initialize_request_with_model_provider(
            client_name,
            requested_capabilities,
            serde_json::json!({
                "models": [
                    test_model_config("gpt-test"),
                    test_model_config("gpt-next")
                ],
                "selectedModel": test_model_config("gpt-test")
            }),
        )
    }

    fn initialize_request_with_model_provider(
        client_name: &str,
        requested_capabilities: impl IntoIterator<Item = &'static str>,
        model_provider: Value,
    ) -> String {
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "init",
            "method": "initialize",
            "params": {
                "client": {
                    "name": client_name,
                    "version": "0.0.0",
                    "transport": "stdio"
                },
                "protocolVersion": ProtocolVersion::current(),
                "requestedCapabilities": requested_capabilities.into_iter().collect::<Vec<_>>(),
                "modelProvider": model_provider
            }
        }))
        .expect("initialize request should serialize")
    }

    fn test_model_config(model_id: &str) -> Value {
        test_model_config_with_api_format(model_id, "openai")
    }

    fn test_model_config_with_api_format(model_id: &str, api_format: &str) -> Value {
        serde_json::json!({
            "modelId": model_id,
            "displayName": model_id,
            "provider": "openai",
            "apiBaseUrl": "http://localhost:11434/v1",
            "apiKey": "test-api-key",
            "apiFormat": api_format,
            "source": "test"
        })
    }

    #[test]
    fn runtime_mode_rejects_unknown_values() {
        assert!(app_server_for_runtime_mode(Some("future")).is_err());
    }

    #[test]
    fn stdio_loop_emits_codex_v2_failure_item_terminal_and_error_events() {
        let initialize = initialize_request("codex", ["codex_app_server_v2_chat_session_subset"]);
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#;
        let mut output = Vec::new();
        let bridge = Arc::new(FailingRuntimeBridge);
        let server = dasclaw_app_server::AppServer::with_runtime_bridge(bridge);

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!("{initialize}\n{thread_start}\n{turn_start}\n")),
            &mut output,
        )
        .expect("stdio loop should emit Codex v2 failure events");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == "turn/completed" && value["params"]["turn"]["status"] == "failed"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/completed" && value["params"]["item"]["type"] == "agentMessage"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "error" && value["params"]["message"] == "runtime failed"
        }));
        let item_completed_position = values
            .iter()
            .position(|value| value["method"] == "item/completed")
            .expect("item/completed should be present");
        let turn_completed_position = values
            .iter()
            .position(|value| value["method"] == "turn/completed")
            .expect("turn/completed should be present");
        let error_position = values
            .iter()
            .position(|value| value["method"] == "error")
            .expect("error should be present");
        assert!(item_completed_position < turn_completed_position);
        assert!(turn_completed_position < error_position);
    }

    #[test]
    fn stdio_loop_emits_codex_v2_interrupt_item_terminal_events() {
        let initialize = initialize_request("codex", ["codex_app_server_v2_chat_session_subset"]);
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#;
        let interrupt = r#"{"jsonrpc":"2.0","id":"interrupt","method":"turn/interrupt","params":{"threadId":"thread_1","turnId":"turn_1"}}"#;
        let mut output = Vec::new();

        run_stdio_server_with_app_server(
            dasclaw_app_server::AppServer::new(),
            Cursor::new(format!(
                "{initialize}\n{thread_start}\n{turn_start}\n{interrupt}\n"
            )),
            &mut output,
        )
        .expect("stdio loop should emit Codex v2 interrupt events");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == "turn/completed"
                && value["params"]["turn"]["status"] == "interrupted"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/completed" && value["params"]["item"]["type"] == "agentMessage"
        }));
        assert!(values.iter().any(|value| {
            value["id"] == "interrupt" && value["result"] == serde_json::json!({})
        }));
        let item_completed_position = values
            .iter()
            .position(|value| value["method"] == "item/completed")
            .expect("item/completed should be present");
        let turn_completed_position = values
            .iter()
            .position(|value| value["method"] == "turn/completed")
            .expect("turn/completed should be present");
        assert!(item_completed_position < turn_completed_position);
    }

    #[test]
    fn stdio_loop_streams_runtime_notifications_after_response_without_another_request() {
        let initialize = initialize_request("open-cowork", []);
        let create =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Draft"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"hello","text_elements":[]}]}}"#;
        let mut output = Vec::new();
        let bridge = Arc::new(DelayedCompletingRuntimeBridge);
        let server = dasclaw_app_server::AppServer::with_runtime_bridge(bridge);

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!("{initialize}\n{create}\n{start}\n")),
            &mut output,
        )
        .expect("stdio loop should stream delayed runtime updates");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();

        let turn_response_position = values
            .iter()
            .position(|value| {
                value["id"] == "turn" && value["result"]["turn"]["status"] == "inProgress"
            })
            .expect("turn/start response should be written");
        let completed_position = values
            .iter()
            .position(|value| value["method"] == "turn/completed")
            .expect("delayed turn/completed notification should be written");

        assert!(completed_position > turn_response_position);
    }

    #[test]
    fn stdio_loop_round_trips_approval_server_request_and_approve_response() {
        let initialize = initialize_request("open-cowork", []);
        let create = r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Approval"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"run shell","text_elements":[]}]}}"#;
        let approval_response = r#"{"jsonrpc":"2.0","id":"approval_00000000-0000-0000-0000-000000000001","result":{"decision":{"kind":"approve"}}}"#;
        let mut output = Vec::new();
        let server = dasclaw_app_server::AppServer::with_runtime_bridge(Arc::new(
            StdioApprovalRuntimeBridge::default(),
        ));

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!(
                "{initialize}\n{create}\n{start}\n{approval_response}\n"
            )),
            &mut output,
        )
        .expect("stdio loop should round-trip an approved server request");

        let values = json_lines(output);
        assert!(values.iter().any(|value| {
            value["method"] == "item/commandExecution/requestApproval"
                && value["id"] == "approval_00000000-0000-0000-0000-000000000001"
                && value["params"]["itemId"] == "turn_1:tool:shell"
                && value["params"]["command"] == "echo ok"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/commandExecution/outputDelta"
                && value["params"]["itemId"] == "turn_1:tool:shell"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "serverRequest/resolved"
                && value["params"]["requestId"] == "approval_00000000-0000-0000-0000-000000000001"
                && value["params"]["outcome"] == "approved"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/commandExecution/terminalInteraction"
                && value["params"]["itemId"] == "turn_1:tool:shell"
                && value["params"]["isError"] == false
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "item/completed"
                && value["params"]["item"]["text"] == "approved shell"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "turn/completed" && value["params"]["turn"]["status"] == "completed"
        }));
    }

    #[test]
    fn stdio_loop_round_trips_approval_server_request_and_reject_response() {
        let initialize = initialize_request("open-cowork", []);
        let create = r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"Approval reject"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"run shell","text_elements":[]}]}}"#;
        let approval_response = r#"{"jsonrpc":"2.0","id":"approval_00000000-0000-0000-0000-000000000001","result":{"decision":{"kind":"reject","data":{"reason":"not now"}}}}"#;
        let mut output = Vec::new();
        let server = dasclaw_app_server::AppServer::with_runtime_bridge(Arc::new(
            StdioApprovalRuntimeBridge::default(),
        ));

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!(
                "{initialize}\n{create}\n{start}\n{approval_response}\n"
            )),
            &mut output,
        )
        .expect("stdio loop should round-trip a rejected server request");

        let values = json_lines(output);
        assert!(values.iter().any(|value| {
            value["method"] == "item/commandExecution/requestApproval"
                && value["id"] == "approval_00000000-0000-0000-0000-000000000001"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "serverRequest/resolved"
                && value["params"]["requestId"] == "approval_00000000-0000-0000-0000-000000000001"
                && value["params"]["outcome"] == "rejected"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "turn/completed"
                && value["params"]["turn"]["status"] == "failed"
                && value["params"]["turn"]["error"]["message"] == "approval rejected: not now"
        }));
        assert!(
            !values
                .iter()
                .any(|value| value["method"] == "turn/completed"
                    && value["params"]["turn"]["status"] == "completed")
        );
        assert!(
            !values.iter().any(|value| {
                value["method"] == "item/commandExecution/terminalInteraction"
                    && value["params"]["isError"] == false
            }),
            "rejected approval must not execute the tool: {values:?}"
        );
    }

    #[test]
    fn stdio_loop_disconnects_after_notification_queue_overflow() {
        let initialize = initialize_request("open-cowork", []);
        let create = r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"cwd":"overflow"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":[{"type":"text","text":"overflow","text_elements":[]}]}}"#;
        let after_overflow =
            r#"{"jsonrpc":"2.0","id":"after_overflow","method":"lifecycle/status"}"#;
        let mut output = Vec::new();
        let server =
            dasclaw_app_server::AppServer::with_runtime_bridge(Arc::new(OverflowingRuntimeBridge));

        run_stdio_server_with_app_server(
            server,
            Cursor::new(format!(
                "{initialize}\n{create}\n{start}\n{after_overflow}\n"
            )),
            &mut output,
        )
        .expect("stdio loop should write overflow and disconnect cleanly");

        let response = String::from_utf8(output).expect("response should be utf8");
        let values = response
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect::<Vec<_>>();

        assert!(values.iter().any(|value| {
            value["method"] == "error" && value["params"]["code"] == "NOTIFICATION_QUEUE_OVERFLOW"
        }));
        assert!(!values.iter().any(|value| value["id"] == "turn"));
        assert!(!values.iter().any(|value| value["id"] == "after_overflow"));
    }

    #[test]
    fn stdio_loop_exits_after_shutdown() {
        let shutdown =
            r#"{"jsonrpc":"2.0","id":"stop","method":"shutdown","params":{"reason":"test"}}"#;
        let health_after_shutdown =
            r#"{"jsonrpc":"2.0","id":"ignored","method":"health/check","params":{}}"#;
        let mut output = Vec::new();

        run_stdio_server(
            Cursor::new(format!("{shutdown}\n{health_after_shutdown}\n")),
            &mut output,
        )
        .expect("stdio loop should stop cleanly after shutdown");

        let response = String::from_utf8(output).expect("response should be utf8");
        let lines = response.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);

        let lifecycle_notification: Value =
            serde_json::from_str(lines[0]).expect("shutdown notification should be JSON");
        let shutdown_response: Value =
            serde_json::from_str(lines[1]).expect("shutdown response should be JSON");

        assert_eq!(lifecycle_notification["method"], "lifecycle/changed");
        assert_eq!(shutdown_response["id"], "stop");
        assert_eq!(shutdown_response["result"]["lifecycle"]["state"], "stopped");
    }

    #[test]
    fn stdio_loop_skips_blank_lines() {
        let mut output = Vec::new();

        run_stdio_server(Cursor::new("\n\n"), &mut output)
            .expect("blank lines should not fail the loop");

        assert!(output.is_empty());
    }

    #[test]
    fn health_once_shape_is_serializable() {
        let health = super::health_once_response_for_runtime_mode(None)
            .expect("default health-once should build");
        let value = serde_json::to_value(health).expect("health response should serialize");

        assert_eq!(value["ok"], false);
        assert_eq!(value["lifecycle"]["state"], "starting");
        assert!(value["services"].is_array());
        assert_service_status(&value, "logs", "ready");
        assert_service_status(&value, "jobs", "ready");
        assert_service_status(&value, "skills", "ready");
        assert_service_status(&value, "mcp", "ready");
    }

    #[test]
    fn version_json_shape_is_serializable() {
        let server = dasclaw_app_server::AppServer::new();
        let value =
            serde_json::to_value(server.server_info()).expect("server info should serialize");

        assert_eq!(value["name"], "dasclaw_app_server");
        assert_eq!(value["protocolVersion"]["major"], 0);
    }

    #[test]
    fn capabilities_once_shape_is_serializable() {
        let capabilities = super::capabilities_once_response_for_runtime_mode(None)
            .expect("default capabilities-once should build");
        let value = serde_json::to_value(capabilities).expect("capabilities should serialize");

        assert_eq!(value["capabilities"]["protocol"]["status"], "implemented");
        assert_eq!(value["capabilities"]["logs"]["status"], "implemented");
        assert_eq!(value["capabilities"]["jobs"]["status"], "implemented");
        assert_eq!(value["capabilities"]["skills"]["status"], "implemented");
        assert_eq!(value["capabilities"]["mcp"]["status"], "implemented");
        assert_eq!(value["capabilities"]["dlpPolicy"]["status"], "declared");
        assert!(
            !value["capabilities"]["mcp"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method == "mcpServer/oauth/login")
        );
        assert!(
            !value["capabilities"]["mcp"]["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event == "mcpServer/oauthLogin/completed")
        );
    }

    #[test]
    fn schema_once_shape_is_serializable() {
        let schema = super::schema_once_response_for_runtime_mode(None)
            .expect("default schema should build");
        let value = serde_json::to_value(schema).expect("schema should serialize");

        assert_eq!(value["protocolVersion"]["major"], 0);
        assert!(value["methods"].is_array());
        assert!(value["events"].is_array());
        assert_eq!(value["capabilities"]["protocol"]["status"], "implemented");
        assert_eq!(value["capabilities"]["logs"]["status"], "implemented");
        assert_eq!(value["capabilities"]["jobs"]["status"], "implemented");
        assert_eq!(value["capabilities"]["skills"]["status"], "implemented");
        assert_eq!(value["capabilities"]["mcp"]["status"], "implemented");
    }

    #[test]
    fn noop_runtime_mode_keeps_interactive_services_declared_and_disabled() {
        let mut server = app_server_for_runtime_mode(Some("noop")).expect("noop mode should build");

        let capabilities =
            serde_json::to_value(server.capabilities()).expect("capabilities should serialize");
        assert_eq!(capabilities["capabilities"]["logs"]["status"], "declared");
        assert_eq!(capabilities["capabilities"]["jobs"]["status"], "declared");
        assert_eq!(capabilities["capabilities"]["skills"]["status"], "declared");
        assert_eq!(capabilities["capabilities"]["mcp"]["status"], "declared");

        let health = serde_json::to_value(server.health_check(
            dasclaw_app_server_protocol::HealthCheckParams {
                include_details: true,
            },
        ))
        .expect("health should serialize");
        assert_service_status(&health, "logs", "disabled");
        assert_service_status(&health, "jobs", "disabled");
        assert_service_status(&health, "skills", "disabled");
        assert_service_status(&health, "mcp", "disabled");
    }

    #[test]
    fn noop_one_shot_capabilities_keep_services_declared() {
        let capabilities = super::capabilities_once_response_for_runtime_mode(Some("noop"))
            .expect("noop capabilities-once should build");
        let value = serde_json::to_value(capabilities).expect("capabilities should serialize");

        assert_eq!(value["capabilities"]["logs"]["status"], "declared");
        assert_eq!(value["capabilities"]["jobs"]["status"], "declared");
        assert_eq!(value["capabilities"]["skills"]["status"], "declared");
        assert_eq!(value["capabilities"]["mcp"]["status"], "declared");
    }

    #[test]
    fn noop_one_shot_health_keeps_services_disabled() {
        let health = super::health_once_response_for_runtime_mode(Some("noop"))
            .expect("noop health-once should build");
        let value = serde_json::to_value(health).expect("health should serialize");

        assert_service_status(&value, "logs", "disabled");
        assert_service_status(&value, "jobs", "disabled");
        assert_service_status(&value, "skills", "disabled");
        assert_service_status(&value, "mcp", "disabled");
    }

    fn assert_service_status(value: &Value, service_name: &str, status: &str) {
        assert!(
            value["services"]
                .as_array()
                .expect("services should be an array")
                .iter()
                .any(|service| {
                    service["service"] == service_name && service["status"] == status
                }),
            "{service_name} should be {status}"
        );
    }

    #[test]
    fn self_check_shape_is_serializable_and_initializes_server() {
        let report =
            super::self_check_report_for_runtime_mode(None).expect("self-check should pass");
        let serialized = serde_json::to_string(&report).expect("self-check should serialize");
        assert!(!serialized.contains("self-check-api-key"));
        let value = serde_json::to_value(report).expect("self-check should serialize");

        assert_eq!(value["ok"], true);
        assert_eq!(value["initialize"]["lifecycle"]["state"], "ready");
        assert_eq!(value["lifecycle"]["lifecycle"]["state"], "ready");
        assert_eq!(value["health"]["ok"], true);
        assert_eq!(value["pendingNotifications"], 4);
        assert!(value["schema"]["methods"].is_array());
        assert_eq!(value["session"]["threadStart"]["thread"]["id"], "thread_1");
        assert_eq!(
            value["session"]["threadList"]["data"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(value["session"]["threadRead"]["thread"]["id"], "thread_1");
        assert_eq!(value["session"]["turnStart"]["turn"]["id"], "turn_1");
        assert_eq!(value["session"]["turnInterrupt"], serde_json::json!({}));
        assert_eq!(
            value["session"]["turnList"]["data"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            value["session"]["turnRead"]["turn"]["status"],
            "interrupted"
        );
        assert_eq!(value["session"]["pendingNotifications"], 7);
    }

    fn json_lines(output: Vec<u8>) -> Vec<Value> {
        String::from_utf8(output)
            .expect("response should be utf8")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("line should be JSON"))
            .collect()
    }

    #[derive(Debug, Default)]
    struct CompletingRuntimeBridge {
        prompts: Mutex<Vec<String>>,
    }

    impl RuntimeBridge for CompletingRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            self.prompts
                .lock()
                .expect("prompt lock")
                .push(request.prompt.clone());
            request.updates.complete(
                request.thread_id,
                request.turn_id,
                format!("runtime saw: {}", request.prompt),
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
    struct FailingRuntimeBridge;

    impl RuntimeBridge for FailingRuntimeBridge {
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

    #[derive(Debug)]
    struct DelayedCompletingRuntimeBridge;

    impl RuntimeBridge for DelayedCompletingRuntimeBridge {
        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(25));
                request.updates.complete(
                    request.thread_id,
                    request.turn_id,
                    format!("runtime saw: {}", request.prompt),
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

    const STDIO_APPROVAL_REQUEST_ID: &str = "00000000-0000-0000-0000-000000000001";

    #[derive(Debug, Default)]
    struct StdioApprovalRuntimeBridge {
        pending: Mutex<HashMap<String, RuntimeTurnStartRequest>>,
    }

    impl RuntimeBridge for StdioApprovalRuntimeBridge {
        fn features(&self) -> RuntimeBridgeFeatures {
            RuntimeBridgeFeatures {
                approval: true,
                tools: true,
                sandbox: true,
                ..RuntimeBridgeFeatures::default()
            }
        }

        fn start_turn(&self, request: RuntimeTurnStartRequest) -> Result<(), RuntimeBridgeError> {
            request.updates.command_output_delta(
                request.thread_id.clone(),
                request.turn_id.clone(),
                RuntimeCommandOutputDeltaUpdate {
                    item_id: format!("{}:tool:shell", request.turn_id),
                    delta: r#"{"cmd":"echo ok"}"#.to_string(),
                },
            );
            request.updates.approval_requested(
                request.thread_id.clone(),
                request.turn_id.clone(),
                RuntimeApprovalRequest {
                    request_id: STDIO_APPROVAL_REQUEST_ID.to_string(),
                    tool_call_id: "shell".to_string(),
                    tool_name: "shell".to_string(),
                    command: Some("echo ok".to_string()),
                    description: "Run shell command".to_string(),
                    display_parameters: serde_json::json!({"cmd":"echo ok"}),
                    allow_always: true,
                },
            );
            self.pending
                .lock()
                .expect("pending approval lock")
                .insert(STDIO_APPROVAL_REQUEST_ID.to_string(), request);
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
            let request = self
                .pending
                .lock()
                .map_err(|_| RuntimeBridgeError::retryable("pending approval lock poisoned"))?
                .remove(&decision.request_id)
                .ok_or_else(|| RuntimeBridgeError::retryable("approval request is not pending"))?;

            match decision.decision {
                dasclaw_runtime::ApprovalDecision::Approve
                | dasclaw_runtime::ApprovalDecision::ApproveAlways => {
                    request.updates.tool_result(
                        request.thread_id.clone(),
                        request.turn_id.clone(),
                        RuntimeToolResultUpdate {
                            item_id: format!("{}:tool:shell", request.turn_id),
                            content: "shell output".to_string(),
                            is_error: false,
                        },
                    );
                    request.updates.complete(
                        request.thread_id,
                        request.turn_id,
                        "approved shell".to_string(),
                    );
                }
                dasclaw_runtime::ApprovalDecision::Reject { reason } => {
                    request.updates.fail(
                        request.thread_id,
                        request.turn_id,
                        format!(
                            "approval rejected: {}",
                            reason.unwrap_or_else(|| "no reason".to_string())
                        ),
                    );
                }
            }
            Ok(())
        }

        fn shutdown(&self) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.clear();
            }
        }
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
}
